//! Empty, fully migrated temporary fixtures for behavior tests. Reopening a
//! fixture retains its content; production household defaults have separate tests.
#![allow(dead_code, unused_imports)]

use std::{path::Path, str::FromStr};

pub use keepsake_core::db::{WriteError, is_overloaded, write_transaction};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

pub async fn connect(url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(url)?;
    let fresh = !options.get_filename().exists();
    let pool = keepsake_core::db::connect(url).await?;
    if fresh {
        empty(&pool).await?;
    }
    Ok(pool)
}

/// Clear only fixture content in one FK-checked transaction.
pub async fn empty(pool: &SqlitePool) -> anyhow::Result<()> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::raw_sql(include_str!("reset.sql"))
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

/// Prepare a fresh empty fixture before starting a CLI or server subprocess.
/// Use from synchronous tests; asynchronous tests should call `connect` directly.
pub fn prepare(path: &Path) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("create fixture runtime")
        .block_on(async {
            let url = format!("sqlite:{}", path.display());
            let pool = keepsake_core::db::connect(&url)
                .await
                .expect("migrate temporary fixture");
            empty(&pool).await.expect("reset temporary fixture");
            pool.close().await;
        });
}
