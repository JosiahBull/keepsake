//! Explicit request-local ownership, with sole-profile compatibility for old clients.
use axum::{
    extract::{FromRequestParts, MatchedPath, RawPathParams},
    http::request::Parts,
};
use keepsake_core::profiles;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::facts_api::ApiError;

pub struct Actor(pub i64);

impl FromRequestParts<SqlitePool> for Actor {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        pool: &SqlitePool,
    ) -> Result<Self, Self::Rejection> {
        let scoped = parts
            .extensions
            .get::<MatchedPath>()
            .is_some_and(|path| path.as_str().contains("{profile_id}"));
        if !scoped {
            return profiles::sole(pool)
                .await
                .map(|profile| Self(profile.id))
                .map_err(ApiError::internal);
        }
        let params = RawPathParams::from_request_parts(parts, pool)
            .await
            .map_err(|error| ApiError::bad(anyhow::anyhow!(error.to_string())))?;
        let id = params
            .iter()
            .find(|(name, _)| *name == "profile_id")
            .and_then(|(_, value)| value.parse::<i64>().ok())
            .filter(|id| *id > 0)
            .ok_or_else(|| ApiError::bad(anyhow::anyhow!("profile_id must be positive")))?;
        profiles::get(pool, id)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::internal(profiles::ProfileError::Missing.into()))?;
        Ok(Self(id))
    }
}

// Named fields let the same handler extract an item ID in scoped and legacy routes.
#[derive(Deserialize)]
pub struct ItemPath {
    pub id: i64,
}
