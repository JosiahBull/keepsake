//! Consistent SQLite snapshots without opening or migrating the source for writes.
use anyhow::{Context, bail};
use serde::Serialize;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{path::Path, str::FromStr, time::Duration};

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub path: std::path::PathBuf,
    pub facts: i64,
    pub practices: i64,
    pub reviews: i64,
}

/// Capture committed data, including WAL pages, and publish a private snapshot atomically.
/// Existing destinations are never replaced. No migrations run on either database.
pub async fn create(source_url: &str, destination: &Path) -> anyhow::Result<Snapshot> {
    snapshot(SqliteConnectOptions::from_str(source_url)?, destination).await
}

pub async fn restore(source: &Path, destination: &Path) -> anyhow::Result<Snapshot> {
    snapshot(SqliteConnectOptions::new().filename(source), destination).await
}

async fn snapshot(options: SqliteConnectOptions, destination: &Path) -> anyhow::Result<Snapshot> {
    let destination = std::path::absolute(destination)?;
    for suffix in ["", "-wal", "-shm"] {
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
    let dirty: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE NOT success")
        .fetch_one(&mut snapshot)
        .await
        .context("source is not a migrated Keepsake database")?;
    if dirty != 0 {
        bail!("snapshot contains an unfinished migration");
    }
    let report = Snapshot {
        path: destination.clone(),
        facts: sqlx::query_scalar("SELECT count(*) FROM facts")
            .fetch_one(&mut snapshot)
            .await?,
        practices: sqlx::query_scalar("SELECT count(*) FROM practices")
            .fetch_one(&mut snapshot)
            .await?,
        reviews: sqlx::query_scalar("SELECT count(*) FROM reviews")
            .fetch_one(&mut snapshot)
            .await?,
    };
    snapshot.close().await?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(&destination)
        .context("publish snapshot without overwriting destination")?;
    #[cfg(unix)]
    std::fs::File::open(destination.parent().context("missing parent")?)?.sync_all()?;
    Ok(report)
}
