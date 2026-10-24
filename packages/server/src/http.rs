//! Bounded connection ownership; the lifecycle deadline also bounds this drain.
use axum::Router;
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::{conn::auto::Builder, graceful::GracefulShutdown},
    service::TowerToHyperService,
};
use std::time::Duration;
use tokio::{net::TcpListener, task::JoinSet};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug)]
pub struct HttpConfig {
    pub max_connections: usize,
    pub header_timeout: Duration,
    /// A hard lifetime cap, including idle keep-alive and blocked response writes.
    pub connection_timeout: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            max_connections: 128,
            header_timeout: Duration::from_secs(15),
            connection_timeout: Duration::from_secs(120),
        }
    }
}

pub async fn serve(
    listener: TcpListener,
    app: Router,
    config: HttpConfig,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    let mut builder = Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(config.header_timeout)
        .max_buf_size(64 * 1024)
        .max_headers(64);
    builder
        .http2()
        .timer(TokioTimer::new())
        .max_concurrent_streams(32)
        .keep_alive_interval(Duration::from_secs(30))
        .keep_alive_timeout(Duration::from_secs(10));
    let graceful = GracefulShutdown::new();
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                if let Err(error) = result { tracing::warn!(%error, "connection task failed"); }
            }
            accepted = listener.accept(), if connections.len() < config.max_connections => {
                let (stream, _) = match accepted {
                    Ok(connection) => connection,
                    Err(error) => {
                        tracing::warn!(%error, "accept failed; backing off");
                        tokio::select! {
                            _ = shutdown.cancelled() => break,
                            _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
                        }
                    }
                };
                let _ = stream.set_nodelay(true);
                let service = TowerToHyperService::new(app.clone());
                let connection = builder.serve_connection(TokioIo::new(stream), service).into_owned();
                let watcher = graceful.watcher();
                connections.spawn(async move {
                    match tokio::time::timeout(config.connection_timeout, watcher.watch(connection)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => tracing::debug!(%error, "connection closed"),
                        Err(_) => tracing::debug!("connection lifetime exceeded"),
                    }
                });
            }
        }
    }

    drop(listener);
    tracing::info!(connections = connections.len(), "draining HTTP connections");
    graceful.shutdown().await;
    while connections.join_next().await.is_some() {}
    // Dropping this future on deadline or repeated signal also drops JoinSet, aborting connections.
    Ok(())
}
