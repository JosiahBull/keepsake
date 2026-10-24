//! One signal listener and one deadline covering HTTP draining and database closure.
#[cfg(unix)]
use anyhow::Context;
use std::{future::Future, time::Duration};
use tokio_util::sync::CancellationToken;

pub async fn run<F, Fut>(grace: Duration, application: F) -> anyhow::Result<()>
where
    F: FnOnce(CancellationToken) -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    // Install before calling the application: startup may wait on SQLite migrations.
    let mut signals = Signals::install()?;
    let shutdown = CancellationToken::new();
    let application = application(shutdown.clone());
    tokio::pin!(application);

    let signal = tokio::select! {
        result = &mut application => return result,
        signal = signals.recv() => signal,
    };
    tracing::info!(signal, grace_secs = grace.as_secs(), "shutdown requested");
    shutdown.cancel();

    tokio::select! {
        result = &mut application => {
            tracing::info!(clean = result.is_ok(), "shutdown complete");
            result
        }
        _ = tokio::time::sleep(grace) => {
            tracing::warn!("shutdown deadline exceeded; stopping remaining work");
            anyhow::bail!("shutdown deadline exceeded")
        }
        signal = signals.recv() => {
            tracing::warn!(signal, "repeated shutdown signal; stopping remaining work");
            anyhow::bail!("forced shutdown after repeated signal")
        }
    }
}

#[cfg(unix)]
struct Signals {
    interrupt: tokio::signal::unix::Signal,
    terminate: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl Signals {
    fn install() -> anyhow::Result<Self> {
        use tokio::signal::unix::{SignalKind, signal};
        Ok(Self {
            interrupt: signal(SignalKind::interrupt()).context("install SIGINT handler")?,
            terminate: signal(SignalKind::terminate()).context("install SIGTERM handler")?,
        })
    }

    async fn recv(&mut self) -> &'static str {
        tokio::select! {
            _ = self.interrupt.recv() => "SIGINT",
            _ = self.terminate.recv() => "SIGTERM",
        }
    }
}

#[cfg(not(unix))]
struct Signals;

#[cfg(not(unix))]
impl Signals {
    fn install() -> anyhow::Result<Self> {
        Ok(Self)
    }

    async fn recv(&mut self) -> &'static str {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "could not receive Ctrl-C");
            std::future::pending::<()>().await;
        }
        "Ctrl-C"
    }
}
