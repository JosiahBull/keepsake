//! Images and audio clips asked with a fact's question.
//!
//! Uploads are stored before the fact that uses them, then attached when it is saved.
//! Nothing is stored on a client's word: [`inspect`] decodes every file as the format it
//! declares, and the schema refuses any content type outside the same allowlist.
mod inspect;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{SqliteConnection, SqlitePool};
use tokio::sync::Semaphore;
use utoipa::ToSchema;

pub use inspect::{
    Inspection, MAX_AUDIO_MS, MAX_IMAGE_SIDE, SUPPORTED_FORMATS, canonical_type, inspect, sniff,
    type_for_extension,
};

use crate::scheduling::StudyError;

/// The largest file accepted. Mirrors the `media.byte_size` check constraint.
pub const MAX_MEDIA_BYTES: usize = 5 * 1024 * 1024;
/// Images and clips one question may carry. Mirrors the `fact_media.position` constraint.
pub const MAX_QUESTION_MEDIA: usize = 4;
/// Stored media per profile, so uploads cannot fill the disk the library lives on.
pub const MAX_PROFILE_MEDIA_BYTES: i64 = 1024 * 1024 * 1024;

/// Decoding holds up to an image's working memory; two at once bounds the process.
static INSPECTIONS: Semaphore = Semaphore::const_new(2);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Image,
    Audio,
}

impl MediaKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
        }
    }

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "image" => Ok(Self::Image),
            "audio" => Ok(Self::Audio),
            other => anyhow::bail!("unknown media kind {other}"),
        }
    }
}

/// A stored file, before or after it is attached to a question.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct MediaUpload {
    pub media_id: i64,
    pub kind: MediaKind,
    pub content_type: String,
    pub byte_size: i64,
    /// Measured by decoding; images only.
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Measured from the stream; clips only, absent when the stream does not say.
    pub duration_ms: Option<i64>,
}

/// An upload to attach to a new fact's question, in display order.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewQuestionMedia {
    pub media_id: i64,
    /// Describes an image for screen readers without giving away the answer.
    #[serde(default)]
    pub alt: String,
}

/// An image or clip shown with a question. Fetch its bytes from `/media/{media_id}`.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct QuestionMedia {
    pub media_id: i64,
    pub kind: MediaKind,
    pub content_type: String,
    pub byte_size: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    pub alt: String,
}

/// Bytes to serve, with the content type detected when they were stored.
pub struct MediaFile {
    pub content_type: String,
    pub data: Vec<u8>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn invalid(message: impl Into<String>) -> anyhow::Error {
    StudyError::Invalid(message.into()).into()
}

/// Store a file for this profile after [`inspect`] accepts it as `declared_type`,
/// reusing an identical earlier upload.
pub async fn upload(
    pool: &SqlitePool,
    profile_id: i64,
    declared_type: &str,
    bytes: Vec<u8>,
) -> anyhow::Result<MediaUpload> {
    let bytes: Arc<[u8]> = bytes.into();
    let inspection = {
        let _permit = INSPECTIONS.acquire().await?;
        let (declared_type, bytes) = (declared_type.to_owned(), bytes.clone());
        tokio::task::spawn_blocking(move || inspect(&declared_type, &bytes)).await??
    };
    crate::db::write_transaction(pool, move |connection| {
        let bytes = bytes.clone();
        Box::pin(async move {
            crate::profiles::require(connection, profile_id).await?;
            store(connection, profile_id, inspection, &bytes).await
        })
    })
    .await
}

async fn store(
    connection: &mut SqliteConnection,
    profile_id: i64,
    inspection: Inspection,
    bytes: &[u8],
) -> anyhow::Result<MediaUpload> {
    remove_abandoned(connection, profile_id).await?;
    let sha256 = sha256_hex(bytes);
    if !holds(connection, profile_id, &sha256).await? {
        ensure_room(connection, profile_id, bytes.len() as i64).await?;
        let kind = inspection.kind.as_str();
        let byte_size = bytes.len() as i64;
        sqlx::query!(
            "INSERT INTO media(profile_id, kind, content_type, sha256, byte_size,
               width, height, duration_ms, data)
             VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)",
            profile_id,
            kind,
            inspection.content_type,
            sha256,
            byte_size,
            inspection.width,
            inspection.height,
            inspection.duration_ms,
            bytes
        )
        .execute(&mut *connection)
        .await?;
    }
    stored(connection, profile_id, &sha256).await
}

/// An upload abandoned with its form is never attached. A day leaves time to finish
/// the fact it was chosen for; anything attached is kept by its question.
async fn remove_abandoned(
    connection: &mut SqliteConnection,
    profile_id: i64,
) -> anyhow::Result<()> {
    sqlx::query!(
        "DELETE FROM media
         WHERE profile_id=? AND created_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day')
           AND NOT EXISTS (SELECT 1 FROM fact_media WHERE media_id=media.id)",
        profile_id
    )
    .execute(connection)
    .await?;
    Ok(())
}

async fn holds(
    connection: &mut SqliteConnection,
    profile_id: i64,
    sha256: &str,
) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM media WHERE profile_id=? AND sha256=?) AS \"held!: bool\"",
        profile_id,
        sha256
    )
    .fetch_one(connection)
    .await?)
}

async fn ensure_room(
    connection: &mut SqliteConnection,
    profile_id: i64,
    adding: i64,
) -> anyhow::Result<()> {
    let used = sqlx::query_scalar!(
        "SELECT coalesce(sum(byte_size), 0) AS \"used!: i64\" FROM media WHERE profile_id=?",
        profile_id
    )
    .fetch_one(connection)
    .await?;
    if used.saturating_add(adding) > MAX_PROFILE_MEDIA_BYTES {
        return Err(StudyError::Conflict(
            "This library's images and clips have reached 1 GiB.".into(),
        )
        .into());
    }
    Ok(())
}

async fn stored(
    connection: &mut SqliteConnection,
    profile_id: i64,
    sha256: &str,
) -> anyhow::Result<MediaUpload> {
    let row = sqlx::query!(
        "SELECT id AS \"id!\", kind, content_type, byte_size, width, height, duration_ms
         FROM media WHERE profile_id=? AND sha256=?",
        profile_id,
        sha256
    )
    .fetch_one(connection)
    .await?;
    Ok(MediaUpload {
        media_id: row.id,
        kind: MediaKind::parse(&row.kind)?,
        content_type: row.content_type,
        byte_size: row.byte_size,
        width: row.width,
        height: row.height,
        duration_ms: row.duration_ms,
    })
}

/// The bytes of a file this profile owns, attached or not.
pub async fn read(
    pool: &SqlitePool,
    profile_id: i64,
    media_id: i64,
) -> anyhow::Result<Option<MediaFile>> {
    let mut connection = pool.acquire().await?;
    crate::profiles::require(&mut connection, profile_id).await?;
    Ok(sqlx::query_as!(
        MediaFile,
        "SELECT content_type, data FROM media WHERE id=? AND profile_id=?",
        media_id,
        profile_id
    )
    .fetch_optional(&mut *connection)
    .await?)
}

/// A question's media, in display order.
pub(crate) async fn for_fact(
    connection: &mut SqliteConnection,
    fact_id: i64,
) -> anyhow::Result<Vec<QuestionMedia>> {
    sqlx::query!(
        "SELECT m.id AS \"id!\", m.kind, m.content_type, m.byte_size,
           m.width, m.height, m.duration_ms, fm.alt
         FROM fact_media fm JOIN media m ON m.id=fm.media_id
         WHERE fm.fact_id=? ORDER BY fm.position",
        fact_id
    )
    .fetch_all(connection)
    .await?
    .into_iter()
    .map(|row| {
        Ok(QuestionMedia {
            media_id: row.id,
            kind: MediaKind::parse(&row.kind)?,
            content_type: row.content_type,
            byte_size: row.byte_size,
            width: row.width,
            height: row.height,
            duration_ms: row.duration_ms,
            alt: row.alt,
        })
    })
    .collect()
}

pub(crate) fn validate(media: &[NewQuestionMedia]) -> anyhow::Result<()> {
    anyhow::ensure!(
        media.len() <= MAX_QUESTION_MEDIA,
        "a question can have at most {MAX_QUESTION_MEDIA} images or clips"
    );
    let mut seen = std::collections::HashSet::new();
    for item in media {
        anyhow::ensure!(item.media_id > 0, "media_id must be positive");
        anyhow::ensure!(seen.insert(item.media_id), "attach each media item once");
        anyhow::ensure!(
            item.alt.len() <= 1_000,
            "media description exceeds 1000 bytes"
        );
        anyhow::ensure!(
            !item.alt.contains('\0'),
            "media description must not contain null bytes"
        );
    }
    Ok(())
}

/// Attach uploads to a newly inserted fact, after checking the profile owns them.
pub(crate) async fn attach(
    connection: &mut SqliteConnection,
    profile_id: i64,
    fact_id: i64,
    media: &[NewQuestionMedia],
) -> anyhow::Result<()> {
    for (position, item) in media.iter().enumerate() {
        let owned = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM media WHERE id=? AND profile_id=?) AS \"owned!: bool\"",
            item.media_id,
            profile_id
        )
        .fetch_one(&mut *connection)
        .await?;
        if !owned {
            return Err(invalid(format!(
                "Media {} is not in this library. Upload it again.",
                item.media_id
            )));
        }
        let position = position as i64;
        sqlx::query!(
            "INSERT INTO fact_media(fact_id, position, media_id, alt) VALUES(?, ?, ?, ?)",
            fact_id,
            position,
            item.media_id,
            item.alt
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// Give a recipient their own copy of a sender's file, reusing an identical one they hold.
/// The sender's copy was inspected when it was stored, so its measurements carry over.
pub(crate) async fn copy(
    connection: &mut SqliteConnection,
    from_profile: i64,
    media_id: i64,
    to_profile: i64,
) -> anyhow::Result<i64> {
    let source = sqlx::query!(
        "SELECT sha256, byte_size FROM media WHERE id=? AND profile_id=?",
        media_id,
        from_profile
    )
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| StudyError::Conflict("Shared media is no longer available".into()))?;
    remove_abandoned(connection, to_profile).await?;
    if !holds(connection, to_profile, &source.sha256).await? {
        ensure_room(connection, to_profile, source.byte_size).await?;
        sqlx::query!(
            "INSERT INTO media(profile_id, kind, content_type, sha256, byte_size,
               width, height, duration_ms, data)
             SELECT ?, kind, content_type, sha256, byte_size, width, height, duration_ms, data
             FROM media WHERE id=?",
            to_profile,
            media_id
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(stored(connection, to_profile, &source.sha256)
        .await?
        .media_id)
}
