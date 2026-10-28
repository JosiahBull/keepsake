//! How much of a profile's library an MCP client may reach.
//!
//! Stored per profile, because each household member decides whether their own
//! library goes to a model provider. What is served is the stored mode clamped to the
//! `KEEPSAKE_MCP` ceiling, which only the host's operator can raise.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::ToSchema;

use crate::profiles::ProfileError;

/// Ordered from least to most capable: `min` with a ceiling can only lower it.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum McpMode {
    /// Nothing is served.
    #[default]
    Off,
    /// Library and study tools that cannot change anything.
    Read,
    /// Everything in `Read`, plus adding facts and practice variants.
    Write,
}

impl McpMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

impl std::str::FromStr for McpMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "off" => Ok(Self::Off),
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            other => Err(format!(
                "unknown agent access mode '{other}' (expected off, read or write)"
            )),
        }
    }
}

/// The mode this profile has chosen, before any ceiling applies.
pub async fn mode(pool: &SqlitePool, profile_id: i64) -> anyhow::Result<McpMode> {
    let stored = sqlx::query_scalar!("SELECT mcp_mode FROM profiles WHERE id=?", profile_id)
        .fetch_optional(pool)
        .await?
        .ok_or(ProfileError::Missing)?;
    stored.parse().map_err(anyhow::Error::msg)
}

pub async fn set_mode(
    pool: &SqlitePool,
    profile_id: i64,
    mode: McpMode,
) -> anyhow::Result<McpMode> {
    let value = mode.as_str();
    let updated = sqlx::query!(
        "UPDATE profiles SET mcp_mode=? WHERE id=?",
        value,
        profile_id
    )
    .execute(pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(ProfileError::Missing.into());
    }
    Ok(mode)
}

#[cfg(test)]
mod tests {
    use super::McpMode;

    /// A ceiling relies on this order: `min` read the wrong way round would raise access.
    #[test]
    fn modes_order_from_least_to_most_capable() {
        assert!(McpMode::Off < McpMode::Read && McpMode::Read < McpMode::Write);
        for (ceiling, stored, served) in [
            (McpMode::Off, McpMode::Write, McpMode::Off),
            (McpMode::Read, McpMode::Write, McpMode::Read),
            (McpMode::Write, McpMode::Read, McpMode::Read),
            (McpMode::Write, McpMode::Off, McpMode::Off),
        ] {
            assert_eq!(ceiling.min(stored), served);
        }
    }

    #[test]
    fn parsing_is_exact() {
        for mode in [McpMode::Off, McpMode::Read, McpMode::Write] {
            assert_eq!(mode.as_str().parse::<McpMode>(), Ok(mode));
        }
        assert!("wrtie".parse::<McpMode>().is_err());
        assert!("".parse::<McpMode>().is_err());
    }
}
