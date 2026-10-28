//! Images and audio clips asked with a fact's question.
//!
//! An upload is checked by decoding it ([`inspect`]), turned into the one format it is
//! served in, and written under the data directory beside the database ([`MediaFiles`]).
//! SQLite holds what describes each file, never its bytes, and the upload itself is not
//! kept. Uploads are stored before the fact that uses them, then attached when it is saved.
mod files;
mod inspect;
mod mp4;
mod render;

use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{SqliteConnection, SqlitePool};
use tokio::sync::Semaphore;
use utoipa::ToSchema;

pub use files::MediaFiles;
pub use inspect::{
    Inspection, MAX_AUDIO_MS, MAX_IMAGE_SIDE, SUPPORTED_FORMATS, canonical_type, inspect, sniff,
    type_for_extension,
};
pub use render::MAX_SERVED_SIDE;

use crate::scheduling::StudyError;

/// The largest file accepted, and the largest served. Mirrors the `media.byte_size` check.
pub const MAX_MEDIA_BYTES: usize = 5 * 1024 * 1024;
/// Images and clips one question may carry. Mirrors the `fact_media.position` constraint.
pub const MAX_QUESTION_MEDIA: usize = 4;
/// Served media per profile, so uploads cannot fill the disk the library lives on.
pub const MAX_PROFILE_MEDIA_BYTES: i64 = 1024 * 1024 * 1024;
/// How old an unreferenced file must be before the server removes it.
const ORPHAN_AGE: Duration = Duration::from_secs(60 * 60);

/// Decoding and encoding hold up to an image's working memory; two at once bounds the
/// process, and the AVIF encoder already uses every core.
static PROCESSING: Semaphore = Semaphore::const_new(2);

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

    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "image" => Ok(Self::Image),
            "audio" => Ok(Self::Audio),
            other => anyhow::bail!("unknown media kind {other}"),
        }
    }
}

/// A stored file, before or after it is attached to a question. Describes what is
/// served: AVIF for images and AAC in MP4 for clips.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct MediaUpload {
    pub media_id: i64,
    pub kind: MediaKind,
    pub content_type: String,
    pub byte_size: i64,
    /// Images only.
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// Clips only.
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

/// Where a served file is, and what it is.
pub struct StoredFile {
    pub path: PathBuf,
    pub content_type: String,
    pub byte_size: i64,
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn invalid(message: impl Into<String>) -> anyhow::Error {
    StudyError::Invalid(message.into()).into()
}

/// [`inspect`] off the async runtime, sharing the limit on concurrent decodes.
pub async fn check(declared_type: &str, bytes: Arc<[u8]>) -> anyhow::Result<Inspection> {
    let _permit = PROCESSING.acquire().await?;
    let declared_type = declared_type.to_owned();
    tokio::task::spawn_blocking(move || inspect(&declared_type, &bytes)).await?
}

/// What was written to disk, before a row describes it.
struct Placed {
    kind: MediaKind,
    content_type: &'static str,
    file_sha256: String,
    byte_size: i64,
    width: Option<i64>,
    height: Option<i64>,
    duration_ms: Option<i64>,
}

/// Check an upload as `declared_type`, write it in the format it is served in, and
/// record it for this profile. An identical earlier upload is reused, not processed again.
pub async fn upload(
    pool: &SqlitePool,
    profile_id: i64,
    declared_type: &str,
    bytes: Vec<u8>,
) -> anyhow::Result<MediaUpload> {
    inspect::precheck(declared_type, &bytes)?;
    let files = MediaFiles::for_pool(pool);
    let upload_sha256 = sha256_hex(&bytes);
    if let Some((existing, path)) = held(pool, &files, profile_id, &upload_sha256).await?
        && path.is_file()
    {
        return Ok(existing);
    }
    let placed = {
        let _permit = PROCESSING.acquire().await?;
        let (declared_type, files) = (declared_type.to_owned(), files.clone());
        tokio::task::spawn_blocking(move || -> anyhow::Result<Placed> {
            let (_, content) = inspect::decode(&declared_type, &bytes)?;
            let rendered = render::render(content)?;
            let file_sha256 = files.place(rendered.kind, rendered.content_type, &rendered.data)?;
            Ok(Placed {
                kind: rendered.kind,
                content_type: rendered.content_type,
                file_sha256,
                byte_size: rendered.data.len() as i64,
                width: rendered.width,
                height: rendered.height,
                duration_ms: rendered.duration_ms,
            })
        })
        .await??
    };
    let placed = Arc::new(placed);
    crate::db::write_transaction(pool, move |connection| {
        let (placed, upload_sha256) = (placed.clone(), upload_sha256.clone());
        Box::pin(async move {
            crate::profiles::require(connection, profile_id).await?;
            remove_abandoned(connection, profile_id).await?;
            if let Some(file_sha256) = file_of(connection, profile_id, &upload_sha256).await? {
                // The file was missing and has just been written again.
                anyhow::ensure!(
                    file_sha256 == placed.file_sha256,
                    "stored media for upload {upload_sha256} no longer matches its file"
                );
            } else {
                ensure_room(connection, profile_id, placed.byte_size).await?;
                let kind = placed.kind.as_str();
                sqlx::query!(
                    "INSERT INTO media(profile_id, kind, content_type, upload_sha256, file_sha256,
                       byte_size, width, height, duration_ms)
                     VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    profile_id,
                    kind,
                    placed.content_type,
                    upload_sha256,
                    placed.file_sha256,
                    placed.byte_size,
                    placed.width,
                    placed.height,
                    placed.duration_ms
                )
                .execute(&mut *connection)
                .await?;
            }
            stored(connection, profile_id, &upload_sha256).await
        })
    })
    .await
}

/// An upload abandoned with its form is never attached. A day leaves time to finish the
/// fact it was chosen for; anything attached is kept by its question. Its file is
/// removed by the server's next [`housekeeping`].
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

async fn file_of(
    connection: &mut SqliteConnection,
    profile_id: i64,
    upload_sha256: &str,
) -> anyhow::Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT file_sha256 FROM media WHERE profile_id=? AND upload_sha256=?",
        profile_id,
        upload_sha256
    )
    .fetch_optional(connection)
    .await?)
}

async fn held(
    pool: &SqlitePool,
    files: &MediaFiles,
    profile_id: i64,
    upload_sha256: &str,
) -> anyhow::Result<Option<(MediaUpload, PathBuf)>> {
    let mut connection = pool.acquire().await?;
    let Some(file_sha256) = file_of(&mut connection, profile_id, upload_sha256).await? else {
        return Ok(None);
    };
    let upload = stored(&mut connection, profile_id, upload_sha256).await?;
    let path = files.path(upload.kind, &file_sha256, &upload.content_type);
    Ok(Some((upload, path)))
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
    upload_sha256: &str,
) -> anyhow::Result<MediaUpload> {
    let row = sqlx::query!(
        "SELECT id AS \"id!\", kind, content_type, byte_size, width, height, duration_ms
         FROM media WHERE profile_id=? AND upload_sha256=?",
        profile_id,
        upload_sha256
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

/// The served file for media this profile owns, attached or not.
pub async fn locate(
    pool: &SqlitePool,
    profile_id: i64,
    media_id: i64,
) -> anyhow::Result<Option<StoredFile>> {
    let mut connection = pool.acquire().await?;
    crate::profiles::require(&mut connection, profile_id).await?;
    let Some(row) = sqlx::query!(
        "SELECT kind, content_type, file_sha256, byte_size FROM media WHERE id=? AND profile_id=?",
        media_id,
        profile_id
    )
    .fetch_optional(&mut *connection)
    .await?
    else {
        return Ok(None);
    };
    let path = MediaFiles::for_pool(pool).path(
        MediaKind::parse(&row.kind)?,
        &row.file_sha256,
        &row.content_type,
    );
    Ok(Some(StoredFile {
        path,
        content_type: row.content_type,
        byte_size: row.byte_size,
    }))
}

/// What the server tidied as it started.
#[derive(Debug, Default)]
pub struct Housekeeping {
    pub tmp_removed: usize,
    pub orphans_removed: usize,
}

/// Empty `tmp/`, and remove served files no row references. For the server at startup
/// only: a CLI running alongside it may have work in `tmp/`.
pub async fn housekeeping(pool: &SqlitePool) -> anyhow::Result<Housekeeping> {
    let referenced: HashSet<String> = sqlx::query_scalar!("SELECT DISTINCT file_sha256 FROM media")
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();
    let files = MediaFiles::for_pool(pool);
    Ok(
        tokio::task::spawn_blocking(move || -> std::io::Result<Housekeeping> {
            Ok(Housekeeping {
                tmp_removed: files.clean_tmp()?,
                orphans_removed: files.sweep(&referenced, ORPHAN_AGE)?,
            })
        })
        .await??,
    )
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

/// Give a recipient their own record of a sender's file. Files are named by their
/// contents, so both records share one file; nothing is copied.
pub(crate) async fn copy(
    connection: &mut SqliteConnection,
    from_profile: i64,
    media_id: i64,
    to_profile: i64,
) -> anyhow::Result<i64> {
    let source = sqlx::query!(
        "SELECT upload_sha256, byte_size FROM media WHERE id=? AND profile_id=?",
        media_id,
        from_profile
    )
    .fetch_optional(&mut *connection)
    .await?
    .ok_or_else(|| StudyError::Conflict("Shared media is no longer available".into()))?;
    remove_abandoned(connection, to_profile).await?;
    if file_of(connection, to_profile, &source.upload_sha256)
        .await?
        .is_none()
    {
        ensure_room(connection, to_profile, source.byte_size).await?;
        sqlx::query!(
            "INSERT INTO media(profile_id, kind, content_type, upload_sha256, file_sha256,
               byte_size, width, height, duration_ms)
             SELECT ?, kind, content_type, upload_sha256, file_sha256, byte_size, width, height,
               duration_ms
             FROM media WHERE id=?",
            to_profile,
            media_id
        )
        .execute(&mut *connection)
        .await?;
    }
    Ok(stored(connection, to_profile, &source.upload_sha256)
        .await?
        .media_id)
}
