pub use keepsake_core::{db, facts};
mod agent_access_api;
pub mod config;
mod context;
mod facts_api;
mod headers;
pub mod health;
pub mod http;
pub mod lifecycle;
pub mod limits;
mod media_api;
pub mod notifications;
mod notifications_api;
mod profile_context;
mod profile_schema;
mod profiles_api;
mod shares_api;
mod study_api;

use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    middleware::{from_fn, from_fn_with_state},
    routing::{any, get, post, put},
};
use keepsake_mcp::{McpConfig, McpMode};
use notifications::Notifier;
use profile_schema::ProfilePaths;
use serde::Serialize;
use sqlx::SqlitePool;
use std::{path::Path, sync::Arc};
use tower_http::{
    compression::{
        CompressionLayer,
        predicate::{DefaultPredicate, NotForContentType, Predicate},
    },
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
        facts_api::reviews,
        media_api::upload,
        media_api::download,
        study_api::queue,
        study_api::summary,
        study_api::submit,
        study_api::expose,
        study_api::settings,
        study_api::update_settings,
        study_api::approve,
        study_api::target,
        study_api::events,
        profiles_api::list,
        profiles_api::create,
        profiles_api::get,
        profiles_api::rename,
        shares_api::list,
        shares_api::send,
        shares_api::accept_batch,
        shares_api::accept,
        shares_api::decline,
        shares_api::withdraw,
        notifications_api::key,
        notifications_api::settings,
        notifications_api::update,
        notifications_api::register,
        notifications_api::remove,
        notifications_api::test,
        agent_access_api::get,
        agent_access_api::update
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
        keepsake_core::media::MediaKind,
        keepsake_core::media::MediaUpload,
        keepsake_core::media::NewQuestionMedia,
        keepsake_core::media::QuestionMedia,
        keepsake_core::paging::FactSummary,
        keepsake_core::paging::FactPage,
        keepsake_core::paging::ReviewPage,
        keepsake_core::paging::PracticeQuestion,
        keepsake_core::paging::PracticeSession,
        keepsake_core::paging::FactSample,
        facts_api::ErrorBody,
        keepsake_core::scheduling::StudyQueue,
        keepsake_core::scheduling::StudySummary,
        keepsake_core::scheduling::StudyQuestion,
        keepsake_core::scheduling::RatingIntervals,
        keepsake_core::scheduling::Schedule,
        keepsake_core::scheduling::StudySubmission,
        keepsake_core::scheduling::StudyResult,
        keepsake_core::scheduling::EventKind,
        keepsake_core::scheduling::StudyContext,
        keepsake_core::scheduling::ExposureSubmission,
        keepsake_core::scheduling::ExposureResult,
        keepsake_core::scheduling::SchedulerSettings,
        keepsake_core::scheduling::SettingsUpdate,
        keepsake_core::scheduling::StudyEvent,
        keepsake_core::scheduling::StudyEventPage,
        keepsake_core::profiles::Profile,
        keepsake_core::profiles::NewProfile,
        keepsake_core::profiles::UpdateProfile,
        keepsake_core::sharing::Share,
        keepsake_core::sharing::SharePage,
        keepsake_core::sharing::ShareRequest,
        keepsake_core::sharing::ShareBatch,
        notifications_api::PushKey,
        keepsake_core::reminders::ReminderSettings,
        keepsake_core::reminders::ReminderUpdate,
        keepsake_core::reminders::NewDevice,
        keepsake_core::reminders::DeviceKeys,
        keepsake_core::reminders::DeviceEndpoint,
        keepsake_core::agent_access::McpMode,
        agent_access_api::AgentAccess,
        agent_access_api::AgentAccessUpdate
    )),
    modifiers(&ProfilePaths)
)]
pub struct ApiDoc;

pub fn router(pool: SqlitePool, web_dir: &Path) -> Router {
    router_with_limits(pool, web_dir, limits::Limits::default())
}

pub fn router_with_limits(pool: SqlitePool, web_dir: &Path, limits: limits::Limits) -> Router {
    let notifier = Notifier::new(pool.clone(), config::PushConfig::default())
        .expect("default push client configuration is valid");
    router_with(pool, web_dir, limits, Arc::new(notifier))
}

pub fn router_with(
    pool: SqlitePool,
    web_dir: &Path,
    limits: limits::Limits,
    notifier: Arc<Notifier>,
) -> Router {
    router_with_mcp(pool, web_dir, limits, notifier, McpConfig::default())
}

/// The full application, with the MCP endpoint mounted when `mcp.ceiling` allows it.
pub fn router_with_mcp(
    pool: SqlitePool,
    web_dir: &Path,
    limits: limits::Limits,
    notifier: Arc<Notifier>,
    mcp: McpConfig,
) -> Router {
    let scoped = content_routes()
        .route("/shares", get(shares_api::list).post(shares_api::send))
        .route("/shares/accept-batch", post(shares_api::accept_batch))
        .route("/shares/{id}/accept", post(shares_api::accept))
        .route("/shares/{id}/decline", post(shares_api::decline))
        .route("/shares/{id}/withdraw", post(shares_api::withdraw))
        .route(
            "/reminders",
            get(notifications_api::settings).put(notifications_api::update),
        )
        .route(
            "/reminders/devices",
            put(notifications_api::register).delete(notifications_api::remove),
        )
        .route("/reminders/test", post(notifications_api::test))
        .route(
            "/agent-access",
            get(agent_access_api::get).put(agent_access_api::update),
        );
    let api = content_routes()
        .route("/health", get(health))
        .route("/push/key", get(notifications_api::key))
        .route(
            "/profiles",
            get(profiles_api::list).post(profiles_api::create),
        )
        .route(
            "/profiles/{profile_id}",
            get(profiles_api::get).patch(profiles_api::rename),
        )
        .nest("/profiles/{profile_id}", scoped)
        .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }))
        .fallback(|| async { StatusCode::NOT_FOUND });
    let mut app = Router::new()
        .nest("/api", api)
        .route("/api", get(|| async { StatusCode::NOT_FOUND }));
    // A ceiling of off, the default, is the only thing that makes /mcp not a route at all.
    // Otherwise it stays mounted and each profile's own setting decides what it serves.
    match mcp.ceiling {
        McpMode::Off => {}
        McpMode::Read | McpMode::Write => {
            tracing::info!(
                ceiling = mcp.ceiling.as_str(),
                "mcp endpoints mounted at /mcp/PROFILE; each serves at most its profile's setting"
            );
            app = app.route("/mcp/{profile}", any(agent_access_api::endpoint));
        }
    }
    let app = app
        .fallback_service(
            ServeDir::new(web_dir).not_found_service(ServeFile::new(web_dir.join("index.html"))),
        )
        .with_state(pool)
        .layer(Extension(notifier))
        .layer(Extension(Arc::new(mcp)))
        .layer(DefaultBodyLimit::max(limits.max_body_bytes))
        // Audio is already compressed, and a gzipped body could not answer byte ranges.
        .layer(
            CompressionLayer::new()
                .compress_when(DefaultPredicate::new().and(NotForContentType::const_new("audio/"))),
        )
        .layer(from_fn_with_state(
            limits::Admission::new(limits),
            limits::admit,
        ));
    context::wrap(app).layer(from_fn(headers::response_headers))
}

fn content_routes() -> Router<SqlitePool> {
    Router::new()
        .route("/facts", get(facts_api::list).post(facts_api::create))
        .route("/practice", get(facts_api::practice))
        .route("/facts/sample", get(facts_api::sample))
        .route("/facts/{id}", get(facts_api::get))
        .route("/facts/{id}/practices", post(facts_api::add_practices))
        .route("/facts/{id}/reviews", get(facts_api::reviews))
        .route("/reviews", post(facts_api::review))
        .route(
            "/media",
            // The upload streams into tmp/ and enforces the media limit itself.
            post(media_api::upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/media/{id}", get(media_api::download))
        .route("/study/queue", get(study_api::queue))
        .route("/study/summary", get(study_api::summary))
        .route(
            "/study/events",
            get(study_api::events).post(study_api::submit),
        )
        .route("/study/exposures", post(study_api::expose))
        .route(
            "/study/settings",
            get(study_api::settings).put(study_api::update_settings),
        )
        .route("/study/practices/{id}/approve", post(study_api::approve))
        .route("/study/facts/{id}", get(study_api::target))
}
