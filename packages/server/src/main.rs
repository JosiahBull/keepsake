use keepsake_server::{config::Config, db, lifecycle, router};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
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
        axum::serve(listener, router(pool.clone(), &config.web_dir))
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await?;
        Ok(())
    }
    .await;
    pool.close().await;
    tracing::info!("database pool closed");
    result
}
