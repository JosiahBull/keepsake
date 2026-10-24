use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use keepsake_core::scheduling::{
    self, ExposureResult, ExposureSubmission, QueueQuery, Schedule, SchedulerSettings,
    SettingsUpdate, StudyEventPage, StudyQueue, StudyResult, StudySubmission,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::facts_api::{ApiError, ErrorBody};

#[utoipa::path(get, path = "/api/study/queue", params(
    ("limit" = Option<i64>, Query, description = "Maximum questions, from 1 to 20 (default 20)")
), responses(
    (status = 200, body = StudyQueue), (status = 400, body = ErrorBody),
    (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn queue(
    State(pool): State<SqlitePool>,
    Query(query): Query<QueueQuery>,
) -> Result<Json<StudyQueue>, ApiError> {
    query.validate().map_err(ApiError::bad)?;
    scheduling::queue(&pool, &query)
        .await
        .map(Json)
        .map_err(ApiError::study)
}

#[utoipa::path(post, path = "/api/study/events", request_body = StudySubmission, responses(
    (status = 201, body = StudyResult), (status = 400, body = ErrorBody),
    (status = 404, body = ErrorBody), (status = 409, body = ErrorBody),
    (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn submit(
    State(pool): State<SqlitePool>,
    Json(submission): Json<StudySubmission>,
) -> Result<(StatusCode, Json<StudyResult>), ApiError> {
    let saved = scheduling::submit(&pool, submission)
        .await
        .map_err(ApiError::study)?;
    Ok((StatusCode::CREATED, Json(saved)))
}

#[utoipa::path(post, path = "/api/study/exposures", request_body = ExposureSubmission, responses(
    (status = 200, body = ExposureResult), (status = 400, body = ErrorBody),
    (status = 404, body = ErrorBody), (status = 409, body = ErrorBody),
    (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn expose(
    State(pool): State<SqlitePool>,
    Json(submission): Json<ExposureSubmission>,
) -> Result<Json<ExposureResult>, ApiError> {
    scheduling::expose(&pool, submission)
        .await
        .map(Json)
        .map_err(ApiError::study)
}

#[utoipa::path(get, path = "/api/study/settings", responses(
    (status = 200, body = SchedulerSettings), (status = 500, body = ErrorBody),
    (status = 503, body = ErrorBody)
))]
pub async fn settings(State(pool): State<SqlitePool>) -> Result<Json<SchedulerSettings>, ApiError> {
    scheduling::settings(&pool)
        .await
        .map(Json)
        .map_err(ApiError::study)
}

#[utoipa::path(put, path = "/api/study/settings", request_body = SettingsUpdate, responses(
    (status = 200, body = SchedulerSettings), (status = 400, body = ErrorBody),
    (status = 409, body = ErrorBody), (status = 500, body = ErrorBody),
    (status = 503, body = ErrorBody)
))]
pub async fn update_settings(
    State(pool): State<SqlitePool>,
    Json(update): Json<SettingsUpdate>,
) -> Result<Json<SchedulerSettings>, ApiError> {
    scheduling::update_settings(&pool, update)
        .await
        .map(Json)
        .map_err(ApiError::study)
}

#[utoipa::path(post, path = "/api/study/practices/{id}/approve", params(
    ("id" = i64, Path, description = "A source-checked flashcard or cloze practice ID")
), responses(
    (status = 204, description = "Practice approved without resetting its recall schedule"),
    (status = 400, body = ErrorBody), (status = 404, body = ErrorBody),
    (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn approve(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    scheduling::approve_practice(&pool, id)
        .await
        .map_err(ApiError::study)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/api/study/facts/{id}", params(("id" = i64, Path)), responses(
    (status = 200, body = Schedule), (status = 404, body = ErrorBody),
    (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn target(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<Schedule>, ApiError> {
    scheduling::target(&pool, id)
        .await
        .map_err(ApiError::study)?
        .map(Json)
        .ok_or_else(|| {
            ApiError::study(
                scheduling::StudyError::Missing("Recall target not found".into()).into(),
            )
        })
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventQuery {
    after: i64,
    limit: i64,
}

impl Default for EventQuery {
    fn default() -> Self {
        Self {
            after: 0,
            limit: 100,
        }
    }
}

#[utoipa::path(get, path = "/api/study/events", params(
    ("after" = Option<i64>, Query, description = "Continue after this event ID (default 0)"),
    ("limit" = Option<i64>, Query, description = "Page size, from 1 to 1000 (default 100)")
), responses(
    (status = 200, body = StudyEventPage), (status = 400, body = ErrorBody),
    (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn events(
    State(pool): State<SqlitePool>,
    Query(query): Query<EventQuery>,
) -> Result<Json<StudyEventPage>, ApiError> {
    scheduling::export_events(&pool, query.after, query.limit)
        .await
        .map(Json)
        .map_err(ApiError::study)
}
