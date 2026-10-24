use anyhow::{Context, ensure};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub web_dir: PathBuf,
    pub shutdown_grace: Duration,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:8080".into())
                .parse()?,
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data/keepsake.db".into()),
            web_dir: std::env::var_os("WEB_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("packages/web/dist")),
            shutdown_grace: seconds_from_env("SHUTDOWN_GRACE_SECS", 10)?,
        })
    }
}

pub(crate) fn seconds_from_env(name: &str, default: u64) -> anyhow::Result<Duration> {
    let seconds = match std::env::var(name) {
        Ok(value) => value
            .parse::<u64>()
            .with_context(|| format!("{name} must be a positive number of seconds"))?,
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => return Err(error).with_context(|| format!("read {name}")),
    };
    ensure!(
        (1..=3600).contains(&seconds),
        "{name} must be between 1 and 3600 seconds"
    );
    Ok(Duration::from_secs(seconds))
}
