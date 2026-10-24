use crate::{http::HttpConfig, limits::Limits};
use anyhow::{Context, ensure};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub web_dir: PathBuf,
    pub shutdown_grace: Duration,
    pub http: HttpConfig,
    pub limits: Limits,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let web_dir = match std::env::var_os("WEB_DIR") {
            Some(directory) => {
                let directory = PathBuf::from(directory);
                ensure!(
                    !directory.as_os_str().is_empty(),
                    "WEB_DIR must not be empty"
                );
                let index = directory.join("index.html");
                let file = std::fs::File::open(&index).with_context(|| {
                    format!(
                        "WEB_DIR must contain a readable index.html: {}",
                        index.display()
                    )
                })?;
                ensure!(
                    file.metadata()?.is_file(),
                    "WEB_DIR/index.html must be a file"
                );
                directory
            }
            // The development API can run before Vite has built any static files.
            None => PathBuf::from("packages/web/dist"),
        };
        Ok(Self {
            bind_addr: std::env::var("BIND_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:8080".into())
                .parse()?,
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data/keepsake.db".into()),
            web_dir,
            shutdown_grace: seconds_from_env("SHUTDOWN_GRACE_SECS", 10)?,
            http: HttpConfig {
                max_connections: count_from_env("MAX_CONNECTIONS", 128, 16_384)?,
                header_timeout: seconds_from_env("HEADER_READ_TIMEOUT_SECS", 15)?,
                connection_timeout: seconds_from_env("CONNECTION_TIMEOUT_SECS", 120)?,
            },
            limits: Limits {
                max_body_bytes: count_from_env("MAX_BODY_BYTES", 512 * 1024, 16 * 1024 * 1024)?,
                max_in_flight: count_from_env("MAX_IN_FLIGHT", 32, 16_384)?,
                request_timeout: seconds_from_env("REQUEST_TIMEOUT_SECS", 30)?,
            },
        })
    }
}

fn count_from_env(name: &str, default: usize, maximum: usize) -> anyhow::Result<usize> {
    let count = match std::env::var(name) {
        Ok(value) => value
            .parse::<usize>()
            .with_context(|| format!("{name} must be a positive integer"))?,
        Err(std::env::VarError::NotPresent) => default,
        Err(error) => return Err(error).with_context(|| format!("read {name}")),
    };
    ensure!(
        (1..=maximum).contains(&count),
        "{name} must be between 1 and {maximum}"
    );
    Ok(count)
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
