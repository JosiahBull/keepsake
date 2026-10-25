use axum::{Json, extract::State, http::StatusCode};
use keepsake_core::profiles::{self, NewProfile, Profile, UpdateProfile};
use sqlx::SqlitePool;

use crate::{
    facts_api::{ApiError, ErrorBody},
    profile_context::Actor,
};

#[utoipa::path(get, path = "/api/profiles", operation_id = "list_profiles", responses(
    (status = 200, body = [Profile]), (status = 500, body = ErrorBody)
))]
pub async fn list(State(pool): State<SqlitePool>) -> Result<Json<Vec<Profile>>, ApiError> {
    profiles::list(&pool)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(post, path = "/api/profiles", operation_id = "create_profile", request_body = NewProfile, responses(
    (status = 201, body = Profile), (status = 400, body = ErrorBody),
    (status = 409, body = ErrorBody), (status = 500, body = ErrorBody)
))]
pub async fn create(
    State(pool): State<SqlitePool>,
    Json(input): Json<NewProfile>,
) -> Result<(StatusCode, Json<Profile>), ApiError> {
    let saved = profiles::create(&pool, input)
        .await
        .map_err(ApiError::internal)?;
    Ok((StatusCode::CREATED, Json(saved)))
}

#[utoipa::path(get, path = "/api/profiles/{profile_id}", operation_id = "get_profile", params(("profile_id" = i64, Path)), responses(
    (status = 200, body = Profile), (status = 404, body = ErrorBody)
))]
pub async fn get(
    State(pool): State<SqlitePool>,
    Actor(id): Actor,
) -> Result<Json<Profile>, ApiError> {
    profiles::get(&pool, id)
        .await
        .map_err(ApiError::internal)?
        .map(Json)
        .ok_or_else(|| ApiError::internal(profiles::ProfileError::Missing.into()))
}

#[utoipa::path(patch, path = "/api/profiles/{profile_id}", operation_id = "rename_profile", params(("profile_id" = i64, Path)), request_body = UpdateProfile, responses(
    (status = 200, body = Profile), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)
))]
pub async fn rename(
    State(pool): State<SqlitePool>,
    Actor(id): Actor,
    Json(input): Json<UpdateProfile>,
) -> Result<Json<Profile>, ApiError> {
    profiles::rename(&pool, id, input)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}
