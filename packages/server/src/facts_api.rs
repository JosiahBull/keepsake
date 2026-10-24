use axum::{
    Json,
    extract::{Path, Query, State},
    http::{StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use sqlx::SqlitePool;
use utoipa::ToSchema;

use crate::facts::{self, Fact, NewFact, NewPractice, NewReview, Review};
use keepsake_core::paging::{
    self, FactPage, FactQuery, FactSample, PracticeQuery, PracticeSession, ReviewPage, ReviewQuery,
    SampleQuery,
};

#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: String,
}

pub struct ApiError(StatusCode, String);

impl ApiError {
    fn bad(error: anyhow::Error) -> Self {
        Self(StatusCode::BAD_REQUEST, error.to_string())
    }

    fn missing() -> Self {
        Self(StatusCode::NOT_FOUND, "Fact not found".into())
    }

    fn internal(error: anyhow::Error) -> Self {
        if crate::db::is_overloaded(&error) {
            return Self(
                StatusCode::SERVICE_UNAVAILABLE,
                "Library is busy. Try again shortly.".into(),
            );
        }
        tracing::error!(%error, "fact storage operation failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not access the library".into(),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.0;
        let mut response = (status, Json(ErrorBody { error: self.1 })).into_response();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            response
                .headers_mut()
                .insert(RETRY_AFTER, "1".parse().expect("valid retry delay"));
        }
        response
    }
}

#[utoipa::path(get, path = "/api/facts", params(FactQuery), responses(
    (status = 200, body = FactPage), (status = 400, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn list(
    State(pool): State<SqlitePool>,
    Query(query): Query<FactQuery>,
) -> Result<Json<FactPage>, ApiError> {
    query.validate().map_err(ApiError::bad)?;
    Ok(Json(
        paging::facts(&pool, &query)
            .await
            .map_err(ApiError::internal)?,
    ))
}

#[utoipa::path(get, path = "/api/practice", params(PracticeQuery), responses(
    (status = 200, body = PracticeSession), (status = 400, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn practice(
    State(pool): State<SqlitePool>,
    Query(query): Query<PracticeQuery>,
) -> Result<Json<PracticeSession>, ApiError> {
    query.validate().map_err(ApiError::bad)?;
    Ok(Json(
        paging::practice(&pool, &query)
            .await
            .map_err(ApiError::internal)?,
    ))
}

#[utoipa::path(get, path = "/api/facts/sample", params(SampleQuery), responses(
    (status = 200, body = FactSample), (status = 400, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn sample(
    State(pool): State<SqlitePool>,
    Query(query): Query<SampleQuery>,
) -> Result<Json<FactSample>, ApiError> {
    query.validate().map_err(ApiError::bad)?;
    Ok(Json(
        paging::sample(&pool, &query)
            .await
            .map_err(ApiError::internal)?,
    ))
}

#[utoipa::path(post, path = "/api/facts", request_body = NewFact, responses(
    (status = 201, body = Fact), (status = 400, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn create(
    State(pool): State<SqlitePool>,
    Json(fact): Json<NewFact>,
) -> Result<(StatusCode, Json<Fact>), ApiError> {
    facts::validate_new_fact(&fact).map_err(ApiError::bad)?;
    let saved = facts::create(&pool, fact)
        .await
        .map_err(ApiError::internal)?;

    Ok((StatusCode::CREATED, Json(saved)))
}

#[utoipa::path(get, path = "/api/facts/{id}", params(("id" = i64, Path)), responses(
    (status = 200, body = Fact), (status = 404, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn get(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<Fact>, ApiError> {
    Ok(Json(
        facts::get(&pool, id)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(ApiError::missing)?,
    ))
}

#[utoipa::path(post, path = "/api/facts/{id}/practices", params(("id" = i64, Path)), request_body = [NewPractice], responses(
    (status = 200, body = Fact), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn add_practices(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Json(practices): Json<Vec<NewPractice>>,
) -> Result<Json<Fact>, ApiError> {
    if practices.is_empty() || practices.len() > 32 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Provide between 1 and 32 practices".into(),
        ));
    }
    for practice in &practices {
        facts::validate_practice(practice).map_err(ApiError::bad)?;
    }
    facts::get(&pool, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::missing)?;
    Ok(Json(
        facts::add_practices(&pool, id, practices)
            .await
            .map_err(ApiError::internal)?,
    ))
}

#[utoipa::path(post, path = "/api/reviews", request_body = NewReview, responses(
    (status = 201, body = Review), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn review(
    State(pool): State<SqlitePool>,
    Json(review): Json<NewReview>,
) -> Result<(StatusCode, Json<Review>), ApiError> {
    let fact = facts::get(&pool, review.fact_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::missing)?;
    facts::validate_review(&review, &fact).map_err(ApiError::bad)?;
    Ok((
        StatusCode::CREATED,
        Json(
            facts::record_review(&pool, review)
                .await
                .map_err(ApiError::internal)?,
        ),
    ))
}

#[utoipa::path(get, path = "/api/facts/{id}/reviews", params(("id" = i64, Path), ReviewQuery), responses(
    (status = 200, body = ReviewPage), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn reviews(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Query(query): Query<ReviewQuery>,
) -> Result<Json<ReviewPage>, ApiError> {
    query.validate().map_err(ApiError::bad)?;
    if !paging::fact_exists(&pool, id)
        .await
        .map_err(ApiError::internal)?
    {
        return Err(ApiError::missing());
    }
    Ok(Json(
        paging::reviews(&pool, id, &query)
            .await
            .map_err(ApiError::internal)?,
    ))
}
