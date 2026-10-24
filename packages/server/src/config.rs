use std::{net::SocketAddr, path::PathBuf};

pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub web_dir: PathBuf,
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
        })
    }
}
