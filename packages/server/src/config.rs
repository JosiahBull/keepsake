use crate::{http::HttpConfig, limits::Limits};
use anyhow::{Context, ensure};
use keepsake_mcp::{McpConfig, McpMode};
use std::{net::SocketAddr, path::PathBuf, time::Duration};

pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub web_dir: PathBuf,
    /// The household's default lists, added to Josiah's and Ansam's libraries. None adds none.
    pub default_lists: Option<PathBuf>,
    pub shutdown_grace: Duration,
    pub http: HttpConfig,
    pub limits: Limits,
    pub push: PushConfig,
    pub mcp: McpConfig,
}

#[derive(Clone, Debug)]
pub struct PushConfig {
    /// The VAPID subject push services may use to contact the operator.
    pub contact: String,
    /// Allows plain-HTTP and loopback push endpoints, for tests with a local push service.
    pub allow_insecure_endpoints: bool,
}

impl Default for PushConfig {
    fn default() -> Self {
        Self {
            contact: "https://github.com/JosiahBull/keepsake".into(),
            allow_insecure_endpoints: false,
        }
    }
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
            default_lists: match std::env::var_os("DEFAULT_LISTS_DIR") {
                Some(directory) => {
                    ensure!(!directory.is_empty(), "DEFAULT_LISTS_DIR must not be empty");
                    Some(PathBuf::from(directory))
                }
                None => None,
            },
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
            push: PushConfig {
                contact: match std::env::var("PUSH_CONTACT") {
                    Ok(contact) => {
                        // Apple's push service rejects VAPID subjects that are not mailto: or https: URLs.
                        ensure!(
                            (contact.starts_with("mailto:") || contact.starts_with("https://"))
                                && contact.len() <= 255
                                && !contact.chars().any(char::is_whitespace),
                            "PUSH_CONTACT must be a mailto: or https: URL"
                        );
                        contact
                    }
                    Err(std::env::VarError::NotPresent) => PushConfig::default().contact,
                    Err(error) => return Err(error).context("read PUSH_CONTACT"),
                },
                allow_insecure_endpoints: false,
            },
            mcp: McpConfig {
                ceiling: mcp_ceiling()?,
                allowed_hosts: mcp_allowed_hosts()?,
            },
        })
    }
}

/// `KEEPSAKE_MCP`: the most agent access any profile may choose. Unset means off.
///
/// An unrecognised value stops startup instead of falling back. Both guesses are wrong:
/// a typo for `write` that silently serves nothing is a confusing afternoon, and one that
/// fell back the other way would give an agent access nobody chose.
fn mcp_ceiling() -> anyhow::Result<McpMode> {
    match std::env::var("KEEPSAKE_MCP") {
        Ok(value) => value
            .parse()
            .map_err(|error: String| anyhow::anyhow!("KEEPSAKE_MCP: {error}")),
        Err(std::env::VarError::NotPresent) => Ok(McpMode::Off),
        Err(error) => Err(error).context("read KEEPSAKE_MCP"),
    }
}

/// `KEEPSAKE_MCP_ALLOWED_HOSTS`: comma-separated `Host` values accepted besides loopback,
/// such as `keepsake.example.com`. Anything else is refused, to defeat DNS rebinding.
fn mcp_allowed_hosts() -> anyhow::Result<Vec<String>> {
    let value = match std::env::var("KEEPSAKE_MCP_ALLOWED_HOSTS") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(Vec::new()),
        Err(error) => return Err(error).context("read KEEPSAKE_MCP_ALLOWED_HOSTS"),
    };
    value
        .split(',')
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(|host| {
            ensure!(
                host.len() <= 255
                    && host
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']')),
                "KEEPSAKE_MCP_ALLOWED_HOSTS entries must be host names or host:port, such as keepsake.example.com"
            );
            Ok(host.to_ascii_lowercase())
        })
        .collect()
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
