pub use keepsake_core::{db, facts};
pub mod config;
mod context;
mod facts_api;
mod headers;
pub mod health;
pub mod http;
pub mod lifecycle;
pub mod limits;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    middleware::{from_fn, from_fn_with_state},
    routing::{get, post},
};
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::Path;
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
};
use utoipa::{OpenApi, ToSchema};

#[derive(Serialize, ToSchema)]
pub struct Health {
    pub status: String,
}

#[utoipa::path(get, path = "/api/health", responses(
    (status = 200, description = "Server and database are available", body = Health),
    (status = 503, description = "Database unavailable")
))]
async fn health(State(pool): State<SqlitePool>) -> Result<Json<Health>, StatusCode> {
    sqlx::query_scalar!("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|error| {
            tracing::error!(%error, "database health check failed");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok(Json(Health {
        status: "ok".into(),
    }))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        facts_api::list,
        facts_api::practice,
        facts_api::sample,
        facts_api::create,
        facts_api::get,
        facts_api::add_practices,
        facts_api::review,
        facts_api::reviews
    ),
    components(schemas(
        Health,
        facts::Fact,
        facts::NewFact,
        facts::Practice,
        facts::NewPractice,
        facts::PracticeKind,
        facts::NewReview,
        facts::Review,
        facts::ReviewMethod,
        facts::ReviewOutcome,
        keepsake_core::paging::FactSummary,
        keepsake_core::paging::FactPage,
        keepsake_core::paging::ReviewPage,
        keepsake_core::paging::PracticeQuestion,
        keepsake_core::paging::PracticeSession,
        keepsake_core::paging::FactSample,
        facts_api::ErrorBody
    ))
)]
pub struct ApiDoc;

pub fn router(pool: SqlitePool, web_dir: &Path) -> Router {
    router_with_limits(pool, web_dir, limits::Limits::default())
}

pub fn router_with_limits(pool: SqlitePool, web_dir: &Path, limits: limits::Limits) -> Router {
    let api = Router::new()
        .route("/health", get(health))
        .route("/facts", get(facts_api::list).post(facts_api::create))
        .route("/practice", get(facts_api::practice))
        .route("/facts/sample", get(facts_api::sample))
        .route("/facts/{id}", get(facts_api::get))
        .route("/facts/{id}/practices", post(facts_api::add_practices))
        .route("/facts/{id}/reviews", get(facts_api::reviews))
        .route("/reviews", post(facts_api::review))
        .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
        .fallback(|| async { StatusCode::NOT_FOUND });
    let app = Router::new()
        .nest("/api", api)
        .route("/api", get(|| async { StatusCode::NOT_FOUND }))
        .fallback_service(
            ServeDir::new(web_dir).not_found_service(ServeFile::new(web_dir.join("index.html"))),
        )
        .with_state(pool)
        .layer(DefaultBodyLimit::max(limits.max_body_bytes))
        .layer(CompressionLayer::new())
        .layer(from_fn_with_state(
            limits::Admission::new(limits),
            limits::admit,
        ));
    context::wrap(app).layer(from_fn(headers::response_headers))
}
