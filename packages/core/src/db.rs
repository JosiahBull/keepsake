use anyhow::Context;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::{str::FromStr, time::Duration};

pub async fn connect(url: &str) -> anyhow::Result<sqlx::SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_millis(250))
        // Matching keys ignore surrounding whitespace and Unicode letter case.
        // SQLite's built-in NOCASE only handles ASCII.
        .collation("NOCASE", |left, right| {
            left.trim().to_lowercase().cmp(&right.trim().to_lowercase())
        });
    if let Some(parent) = options
        .get_filename()
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "create SQLite directory {}; it must be writable by the running user",
                parent.display()
            )
        })?;
    }
    let filename = options.get_filename().to_path_buf();
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .with_context(|| {
            format!(
                "open or create SQLite database {}; its directory must be writable by the running user, including for WAL files",
                filename.display()
            )
        })?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// A failed write is retryable only when no commit was attempted and rollback succeeded.
#[derive(Debug)]
pub enum WriteError {
    Overloaded(anyhow::Error),
    RollbackFailed {
        operation: anyhow::Error,
        rollback: sqlx::Error,
    },
    CommitFailed(sqlx::Error),
}

impl std::fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Overloaded(_) => "library is busy; try again shortly",
            Self::RollbackFailed { .. } => "could not confirm transaction rollback",
            Self::CommitFailed(_) => "could not confirm transaction commit; write was not retried",
        })
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Overloaded(error) => Some(error.as_ref()),
            Self::RollbackFailed { rollback, .. } => Some(rollback),
            Self::CommitFailed(error) => Some(error),
        }
    }
}

fn sqlite_busy(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(error) => error
            .code()
            .and_then(|code| code.parse::<i32>().ok())
            .is_some_and(|code| matches!(code & 0xff, 5 | 6)),
        _ => false,
    }
}

/// Classify storage pressure without treating an uncertain commit as safe to repeat.
pub fn is_overloaded(error: &anyhow::Error) -> bool {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<WriteError>() {
            return match error {
                WriteError::Overloaded(_) => true,
                WriteError::RollbackFailed { .. } | WriteError::CommitFailed(_) => false,
            };
        }
        if let Some(error) = cause.downcast_ref::<sqlx::Error>() {
            // `sqlx::Error` is `#[non_exhaustive]`.
            return matches!(error, sqlx::Error::PoolTimedOut) || sqlite_busy(error);
        }
    }
    false
}

type WriteFuture<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send + 'a>>;

/// Retry a complete database-only transaction, never individual statements or a commit.
/// Each SQLite lock wait is capped at 250ms; three attempts use 25ms/50ms backoff.
/// Callers must not perform network requests or other external effects in `operation`.
pub async fn write_transaction<T, F>(pool: &sqlx::SqlitePool, mut operation: F) -> anyhow::Result<T>
where
    T: Send,
    F: for<'a> FnMut(&'a mut sqlx::SqliteConnection) -> WriteFuture<'a, T>,
{
    for attempt in 0..3 {
        let result: anyhow::Result<()> = match pool.begin_with("BEGIN IMMEDIATE").await {
            Ok(mut transaction) => match operation(&mut transaction).await {
                Ok(value) => {
                    transaction
                        .commit()
                        .await
                        .map_err(WriteError::CommitFailed)?;
                    return Ok(value);
                }
                Err(error) => {
                    if let Err(rollback) = transaction.rollback().await {
                        return Err(WriteError::RollbackFailed {
                            operation: error,
                            rollback,
                        }
                        .into());
                    }
                    Err(error)
                }
            },
            Err(error) => Err(error.into()),
        };
        if let Err(error) = result {
            if !is_overloaded(&error) {
                return Err(error);
            }
            // Pool exhaustion has already waited for its acquisition deadline.
            let timed_out = error.chain().any(|cause| {
                matches!(
                    cause.downcast_ref::<sqlx::Error>(),
                    Some(sqlx::Error::PoolTimedOut)
                )
            });
            if attempt == 2 || timed_out {
                return Err(WriteError::Overloaded(error).into());
            }
        }
        tokio::time::sleep(Duration::from_millis(25 << attempt)).await;
    }
    unreachable!("write attempts are bounded")
}
