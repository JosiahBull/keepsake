//! Consistent SQLite snapshots without opening or migrating the source for writes.
//!
//! A backup's media files sit beside it in `PATH-media/`, as SQLite's own sidecars do,
//! laid out as the data directory's `media/` is. Every file is checked against its hash
//! on the way in and on the way out.
use anyhow::{Context, bail, ensure};
use serde::Serialize;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use crate::media::{MediaFiles, MediaKind, sha256_hex};

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub path: std::path::PathBuf,
    pub facts: i64,
    pub practices: i64,
    pub reviews: i64,
    /// Image and audio files copied with the database.
    pub media: i64,
}

/// Where a snapshot's media files come from, and so where they go.
enum Media {
    /// From a live data directory into `DESTINATION-media/`.
    Backup(MediaFiles),
    /// From `SOURCE-media/` into the destination's data directory.
    Restore(PathBuf),
}

/// The directory beside a backup that holds its media files.
pub fn media_dir(backup: &Path) -> PathBuf {
    let mut name = backup.file_name().unwrap_or_default().to_os_string();
    name.push("-media");
    backup.with_file_name(name)
}

/// Capture committed data, including WAL pages, and publish a private snapshot atomically.
/// Existing destinations are never replaced. No migrations run on either database.
pub async fn create(source_url: &str, destination: &Path) -> anyhow::Result<Snapshot> {
    let options = SqliteConnectOptions::from_str(source_url)?;
    let media = Media::Backup(MediaFiles::for_database(options.get_filename()));
    snapshot(options, destination, media).await
}

/// Restore into a data directory of its own: one with no `media/` yet. A database's
/// server removes files its rows do not reference, so two databases cannot share one.
pub async fn restore(source: &Path, destination: &Path) -> anyhow::Result<Snapshot> {
    let target = MediaFiles::for_database(&std::path::absolute(destination)?);
    ensure!(
        !tokio::fs::try_exists(target.media()).await?,
        "restore into a directory without a media folder; {} already exists",
        target.media().display()
    );
    snapshot(
        SqliteConnectOptions::new().filename(source),
        destination,
        Media::Restore(media_dir(source)),
    )
    .await
}

async fn snapshot(
    options: SqliteConnectOptions,
    destination: &Path,
    media: Media,
) -> anyhow::Result<Snapshot> {
    let destination = std::path::absolute(destination)?;
    for suffix in ["", "-wal", "-shm", "-media"] {
        let mut name = destination
            .file_name()
            .context("missing filename")?
            .to_os_string();
        name.push(suffix);
        let path = destination.with_file_name(name);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => bail!(
                "destination or SQLite sidecar already exists: {}",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let options = options
        .read_only(true)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(5));
    if options.get_filename() == Path::new(":memory:") {
        bail!("snapshots require a file-backed source database");
    }
    let mut source = SqliteConnection::connect_with(&options)
        .await
        .context("open source database read-only")?;
    let temporary =
        tempfile::NamedTempFile::new_in(destination.parent().context("missing parent")?)?;
    let temporary_path = temporary
        .path()
        .to_str()
        .context("snapshot path must be UTF-8")?;
    sqlx::query("VACUUM INTO ?")
        .bind(temporary_path)
        .execute(&mut source)
        .await
        .context("create consistent SQLite snapshot")?;
    source.close().await?;

    let mut snapshot = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(temporary.path())
            .read_only(true),
    )
    .await?;
    let integrity: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(&mut snapshot)
        .await?;
    if integrity != ["ok"] {
        bail!("snapshot failed SQLite integrity check");
    }
    if !sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut snapshot)
        .await?
        .is_empty()
    {
        bail!("snapshot contains invalid foreign keys");
    }
    let dirty: i64 = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!: i64\" FROM _sqlx_migrations WHERE NOT success"
    )
    .fetch_one(&mut snapshot)
    .await
    .context("source is not a migrated Keepsake database")?;
    if dirty != 0 {
        bail!("snapshot contains an unfinished migration");
    }
    let report = Snapshot {
        path: destination.clone(),
        facts: sqlx::query_scalar!("SELECT count(*) AS \"count!: i64\" FROM facts")
            .fetch_one(&mut snapshot)
            .await?,
        practices: sqlx::query_scalar!("SELECT count(*) AS \"count!: i64\" FROM practices")
            .fetch_one(&mut snapshot)
            .await?,
        reviews: sqlx::query_scalar!("SELECT count(*) AS \"count!: i64\" FROM reviews")
            .fetch_one(&mut snapshot)
            .await?,
        media: 0,
    };
    // A database from before question media has no media table, and so no files.
    let has_media: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='media')",
    )
    .fetch_one(&mut snapshot)
    .await?;
    let files: Vec<(String, String, String)> = if has_media {
        sqlx::query_as("SELECT DISTINCT kind, content_type, file_sha256 FROM media")
            .fetch_all(&mut snapshot)
            .await?
    } else {
        Vec::new()
    };
    snapshot.close().await?;
    let report = Snapshot {
        media: files.len() as i64,
        ..report
    };

    // Stage the files beside where they will live, then publish them with one rename.
    let (staging, published) = match &media {
        Media::Backup(_) => (
            destination
                .parent()
                .context("missing parent")?
                .to_path_buf(),
            media_dir(&destination),
        ),
        Media::Restore(_) => {
            let target = MediaFiles::for_database(&destination);
            (target.root().to_path_buf(), target.media())
        }
    };
    // Copying and verifying can cover a gigabyte of media: keep it off the runtime.
    let media_count = report.media;
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let staged = if files.is_empty() {
            None
        } else {
            let staged = tempfile::TempDir::new_in(&staging)?;
            for (kind, content_type, sha256) in &files {
                let relative = MediaFiles::relative(MediaKind::parse(kind)?, sha256, content_type);
                let source = match &media {
                    Media::Backup(live) => live.media().join(&relative),
                    Media::Restore(backup) => backup.join(&relative),
                };
                copy_verified(&source, &staged.path().join(&relative), sha256)?;
            }
            Some(staged)
        };

        temporary.as_file().sync_all()?;
        if let Some(staged) = staged {
            std::fs::rename(staged.keep(), &published)
                .with_context(|| format!("publish media files at {}", published.display()))?;
        }
        if let Err(error) = temporary.persist_noclobber(&destination) {
            if media_count > 0 {
                let _ = std::fs::remove_dir_all(&published);
            }
            return Err(error).context("publish snapshot without overwriting destination");
        }
        #[cfg(unix)]
        std::fs::File::open(destination.parent().context("missing parent")?)?.sync_all()?;
        Ok(())
    })
    .await??;
    Ok(report)
}

/// Copy a media file, refusing one whose contents no longer match its name.
fn copy_verified(source: &Path, destination: &Path, sha256: &str) -> anyhow::Result<()> {
    let data =
        std::fs::read(source).with_context(|| format!("read media file {}", source.display()))?;
    ensure!(
        sha256_hex(&data) == sha256,
        "media file {} is damaged: its contents do not match its name",
        source.display()
    );
    std::fs::create_dir_all(destination.parent().context("missing parent")?)?;
    std::fs::write(destination, &data)?;
    std::fs::File::open(destination)?.sync_all()?;
    Ok(())
}
