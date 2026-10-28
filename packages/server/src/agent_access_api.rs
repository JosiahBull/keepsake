use std::sync::Arc;

use axum::{
    Extension, Json,
    body::Body,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use keepsake_core::{
    agent_access::{self, McpMode},
    profiles,
};
use keepsake_mcp::McpConfig;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tower::ServiceExt;
use utoipa::ToSchema;

use crate::{
    facts_api::{ApiError, ErrorBody},
    profile_context::Actor,
};

/// Agent access as the settings page needs it. Two of the three fields are facts about
/// how the server was started, which no stored row can hold: without them the page
/// could not explain why a choice does nothing.
#[derive(Serialize, ToSchema)]
pub struct AgentAccess {
    /// What this profile has chosen.
    pub mode: McpMode,
    /// The most this server allows, from `KEEPSAKE_MCP`. `off` means agent access is
    /// unavailable here, whatever is chosen.
    pub ceiling: McpMode,
    /// What is served: `mode` limited to `ceiling`.
    pub effective: McpMode,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AgentAccessUpdate {
    pub mode: McpMode,
}

fn view(mode: McpMode, ceiling: McpMode) -> AgentAccess {
    AgentAccess {
        mode,
        ceiling,
        effective: ceiling.min(mode),
    }
}

#[utoipa::path(get, path = "/api/profiles/{profile_id}/agent-access", operation_id = "get_agent_access", params(("profile_id" = i64, Path)), responses(
    (status = 200, body = AgentAccess), (status = 404, body = ErrorBody), (status = 500, body = ErrorBody)
))]
pub async fn get(
    State(pool): State<SqlitePool>,
    Extension(config): Extension<Arc<McpConfig>>,
    Actor(profile_id): Actor,
) -> Result<Json<AgentAccess>, ApiError> {
    let mode = agent_access::mode(&pool, profile_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(view(mode, config.ceiling)))
}

#[utoipa::path(put, path = "/api/profiles/{profile_id}/agent-access", operation_id = "update_agent_access", params(("profile_id" = i64, Path)), request_body = AgentAccessUpdate, responses(
    (status = 200, body = AgentAccess), (status = 404, body = ErrorBody),
    (status = 422, body = ErrorBody), (status = 500, body = ErrorBody)
))]
pub async fn update(
    State(pool): State<SqlitePool>,
    Extension(config): Extension<Arc<McpConfig>>,
    Actor(profile_id): Actor,
    Json(input): Json<AgentAccessUpdate>,
) -> Result<Json<AgentAccess>, ApiError> {
    // Refused rather than clamped: storing more than is served would make the settings
    // page describe a server that does not exist.
    if input.mode > config.ceiling {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "KEEPSAKE_MCP allows at most '{}' on this server, so agent access cannot be \
                 set to '{}'. Change KEEPSAKE_MCP and restart the server to raise it.",
                config.ceiling.as_str(),
                input.mode.as_str()
            ),
        ));
    }
    let mode = agent_access::set_mode(&pool, profile_id, input.mode)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(view(mode, config.ceiling)))
}

/// `/mcp/{profile}`: one profile's library, by handle or ID. Mounted only when
/// `KEEPSAKE_MCP` allows anything; the profile's own setting decides what it serves.
pub async fn endpoint(
    State(pool): State<SqlitePool>,
    Extension(config): Extension<Arc<McpConfig>>,
    Path(selector): Path<String>,
    request: Request,
) -> Response {
    let profile = match profiles::resolve(&pool, &selector).await {
        Ok(profile) => profile,
        Err(error) => return ApiError::internal(error).into_response(),
    };
    match keepsake_mcp::http_service(pool, &config, profile)
        .oneshot(request)
        .await
    {
        Ok(response) => response.map(Body::new),
        Err(never) => match never {},
    }
}
