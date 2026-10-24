use keepsake_server::{config::Config, db, health, http, lifecycle, router_with_limits};
use std::time::Duration;
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
    let pool = db::connect(&config.database_url).await?;
    let result = async {
        if shutdown.is_cancelled() {
            return Ok(());
        }
        let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
        tracing::info!(address = %listener.local_addr()?, "keepsake is listening");
        let app = router_with_limits(pool.clone(), &config.web_dir, config.limits);
        http::serve(listener, app, config.http, shutdown).await?;
        Ok(())
    }
    .await;
    pool.close().await;
    tracing::info!("database pool closed");
    result
}
