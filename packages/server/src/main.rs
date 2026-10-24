use keepsake_server::{config::Config, db, router};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let config = Config::from_env()?;
    let pool = db::connect(&config.database_url).await?;
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(address = %listener.local_addr()?, "keepsake is listening");
    let result = axum::serve(listener, router(pool.clone(), &config.web_dir))
        .with_graceful_shutdown(shutdown())
        .await;
    pool.close().await;
    result?;
    Ok(())
}

async fn shutdown() {
    let interrupt = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl-C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = interrupt => {}, _ = terminate => {} }
}
