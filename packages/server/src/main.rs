use keepsake_core::default_lists::{self, DefaultList};
use keepsake_server::{
    config::Config, db, health, http, lifecycle, notifications, router_with_mcp,
};
use sqlx::SqlitePool;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some(health::FLAG) {
        return health::probe();
    }
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let config = Config::from_env()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(lifecycle::run(config.shutdown_grace, |shutdown| {
        serve(config, shutdown)
    }));
    // A filesystem operation on Tokio's blocking pool must not make a forced stop unbounded.
    runtime.shutdown_timeout(Duration::from_secs(1));
    result
}

async fn serve(config: Config, shutdown: CancellationToken) -> anyhow::Result<()> {
    tracing::info!("starting keepsake");
    // Read before opening SQLite, so a broken list stops startup without touching it.
    let lists = match &config.default_lists {
        Some(directory) => default_lists::load(directory)?,
        None => Vec::new(),
    };
    let pool = db::connect(&config.database_url).await?;
    // Work left in tmp/ by a previous run is incomplete by definition.
    let tidied = keepsake_core::media::housekeeping(&pool).await?;
    tracing::info!(
        tmp_removed = tidied.tmp_removed,
        orphans_removed = tidied.orphans_removed,
        "media directory tidied"
    );
    let result = async {
        if shutdown.is_cancelled() {
            return Ok(());
        }
        let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
        tracing::info!(address = %listener.local_addr()?, "keepsake is listening");
        let notifier = Arc::new(notifications::Notifier::new(pool.clone(), config.push)?);
        // A child token also stops reminders if serving fails before a shutdown signal.
        let reminders_stop = shutdown.child_token();
        let reminders = tokio::spawn(notifications::run(notifier.clone(), reminders_stop.clone()));
        let lists_stop = shutdown.child_token();
        let lists = tokio::spawn(install_default_lists(
            pool.clone(),
            lists,
            lists_stop.clone(),
        ));
        let app = router_with_mcp(
            pool.clone(),
            &config.web_dir,
            config.limits,
            notifier,
            config.mcp,
        );
        let served = http::serve(listener, app, config.http, shutdown).await;
        reminders_stop.cancel();
        lists_stop.cancel();
        if let Err(error) = reminders.await {
            tracing::warn!(%error, "reminder task failed");
        }
        if let Err(error) = lists.await {
            tracing::warn!(%error, "default list task failed");
        }
        served
    }
    .await;
    pool.close().await;
    tracing::info!("database pool closed");
    result
}

/// Add what the default lists have that the household's libraries lack. The first time,
/// converting their images and clips can take minutes, so it runs while serving.
async fn install_default_lists(pool: SqlitePool, lists: Vec<DefaultList>, stop: CancellationToken) {
    if lists.is_empty() {
        return;
    }
    let items: usize = lists.iter().map(DefaultList::len).sum();
    tracing::info!(lists = lists.len(), items, "checking default lists");
    match default_lists::install(&pool, &lists, || stop.is_cancelled()).await {
        Ok(installed) => {
            for member in installed.missing {
                tracing::warn!(
                    member,
                    "no profile for this member; default lists skipped them"
                );
            }
            for (item, error) in &installed.failed {
                tracing::warn!(
                    item,
                    error = format!("{error:#}"),
                    "default list item not added"
                );
            }
            tracing::info!(added = installed.added, "default lists checked");
        }
        Err(error) => tracing::error!(error = format!("{error:#}"), "default lists not added"),
    }
}
