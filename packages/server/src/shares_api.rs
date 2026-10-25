use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use keepsake_core::sharing::{self, Share, ShareBatch, SharePage, ShareQuery, ShareRequest};
use sqlx::SqlitePool;

use crate::{
    facts_api::{ApiError, ErrorBody},
    profile_context::{Actor, ItemPath},
};

#[utoipa::path(get, path = "/api/profiles/{profile_id}/shares", operation_id = "list_shares", params(("profile_id" = i64, Path), ShareQuery), responses(
    (status = 200, body = SharePage), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)
))]
pub async fn list(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Query(query): Query<ShareQuery>,
) -> Result<Json<SharePage>, ApiError> {
    sharing::page(&pool, profile_id, &query)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(post, path = "/api/profiles/{profile_id}/shares", operation_id = "send_shares", params(("profile_id" = i64, Path)), request_body = ShareRequest, responses(
    (status = 201, body = [Share]), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)
))]
pub async fn send(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Json(input): Json<ShareRequest>,
) -> Result<(StatusCode, Json<Vec<Share>>), ApiError> {
    sharing::send(&pool, profile_id, input)
        .await
        .map(|saved| (StatusCode::CREATED, Json(saved)))
        .map_err(ApiError::internal)
}

#[utoipa::path(post, path = "/api/profiles/{profile_id}/shares/accept-batch", operation_id = "accept_share_batch", params(("profile_id" = i64, Path)), request_body = ShareBatch, responses(
    (status = 200, body = [Share]), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)
))]
pub async fn accept_batch(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Json(input): Json<ShareBatch>,
) -> Result<Json<Vec<Share>>, ApiError> {
    sharing::accept_batch(&pool, profile_id, input)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(post, path = "/api/profiles/{profile_id}/shares/{id}/accept", operation_id = "accept_share", params(("profile_id" = i64, Path), ("id" = i64, Path)), responses(
    (status = 200, body = Share), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)
))]
pub async fn accept(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Path(ItemPath { id }): Path<ItemPath>,
) -> Result<Json<Share>, ApiError> {
    sharing::accept(&pool, profile_id, id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(post, path = "/api/profiles/{profile_id}/shares/{id}/decline", operation_id = "decline_share", params(("profile_id" = i64, Path), ("id" = i64, Path)), responses(
    (status = 200, body = Share), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)
))]
pub async fn decline(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Path(ItemPath { id }): Path<ItemPath>,
) -> Result<Json<Share>, ApiError> {
    sharing::decline(&pool, profile_id, id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(post, path = "/api/profiles/{profile_id}/shares/{id}/withdraw", operation_id = "withdraw_share", params(("profile_id" = i64, Path), ("id" = i64, Path)), responses(
    (status = 200, body = Share), (status = 404, body = ErrorBody), (status = 409, body = ErrorBody)
))]
pub async fn withdraw(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Path(ItemPath { id }): Path<ItemPath>,
) -> Result<Json<Share>, ApiError> {
    sharing::withdraw(&pool, profile_id, id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}
