use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, StatusCode, header::ORIGIN},
};
use keepsake_core::reminders::{self, DeviceEndpoint, NewDevice, ReminderSettings, ReminderUpdate};
use serde::Serialize;
use sqlx::SqlitePool;
use utoipa::ToSchema;

use crate::{
    facts_api::{ApiError, ErrorBody},
    notifications::{Delivery, Notifier},
    profile_context::Actor,
};

#[derive(Serialize, ToSchema)]
pub struct PushKey {
    /// The VAPID application server key, base64url-encoded.
    pub public_key: String,
}

#[utoipa::path(get, path = "/api/push/key", operation_id = "push_key", responses(
    (status = 200, body = PushKey), (status = 500, body = ErrorBody), (status = 503, body = ErrorBody)
))]
pub async fn key(Extension(notifier): Extension<Arc<Notifier>>) -> Result<Json<PushKey>, ApiError> {
    notifier
        .public_key()
        .await
        .map(|public_key| Json(PushKey { public_key }))
        .map_err(ApiError::internal)
}

#[utoipa::path(get, path = "/api/profiles/{profile_id}/reminders", operation_id = "get_reminders", params(("profile_id" = i64, Path)), responses(
    (status = 200, body = ReminderSettings), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)
))]
pub async fn settings(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
) -> Result<Json<ReminderSettings>, ApiError> {
    reminders::settings_for(&pool, profile_id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(put, path = "/api/profiles/{profile_id}/reminders", operation_id = "update_reminders", params(("profile_id" = i64, Path)), request_body = ReminderUpdate, responses(
    (status = 200, body = ReminderSettings), (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)
))]
pub async fn update(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Json(update): Json<ReminderUpdate>,
) -> Result<Json<ReminderSettings>, ApiError> {
    reminders::update_settings_for(&pool, profile_id, update)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(put, path = "/api/profiles/{profile_id}/reminders/devices", operation_id = "register_reminder_device", params(("profile_id" = i64, Path)), request_body = NewDevice, responses(
    (status = 200, description = "This browser receives the profile's reminders", body = ReminderSettings),
    (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)
))]
pub async fn register(
    State(pool): State<SqlitePool>,
    Extension(notifier): Extension<Arc<Notifier>>,
    Actor(profile_id): Actor,
    headers: HeaderMap,
    Json(device): Json<NewDevice>,
) -> Result<Json<ReminderSettings>, ApiError> {
    // Browsers send Origin with every PUT; notification links must open the same origin.
    let origin = headers
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Origin header is required"))?
        .to_owned();
    notifier
        .validate(&origin, &device)
        .map_err(|message| ApiError::new(StatusCode::BAD_REQUEST, message))?;
    reminders::register_device_for(&pool, profile_id, origin, device)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(delete, path = "/api/profiles/{profile_id}/reminders/devices", operation_id = "remove_reminder_device", params(("profile_id" = i64, Path)), request_body = DeviceEndpoint, responses(
    (status = 200, description = "This browser no longer receives the profile's reminders", body = ReminderSettings),
    (status = 400, body = ErrorBody), (status = 404, body = ErrorBody)
))]
pub async fn remove(
    State(pool): State<SqlitePool>,
    Actor(profile_id): Actor,
    Json(device): Json<DeviceEndpoint>,
) -> Result<Json<ReminderSettings>, ApiError> {
    reminders::remove_device_for(&pool, profile_id, &device.endpoint)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

#[utoipa::path(post, path = "/api/profiles/{profile_id}/reminders/test", operation_id = "test_reminder", params(("profile_id" = i64, Path)), request_body = DeviceEndpoint, responses(
    (status = 204, description = "The push service accepted a confirmation for this browser"),
    (status = 404, body = ErrorBody), (status = 410, body = ErrorBody), (status = 502, body = ErrorBody)
))]
pub async fn test(
    Extension(notifier): Extension<Arc<Notifier>>,
    Actor(profile_id): Actor,
    Json(device): Json<DeviceEndpoint>,
) -> Result<StatusCode, ApiError> {
    match notifier.test(profile_id, &device.endpoint).await {
        Ok(Some(Delivery::Delivered)) => Ok(StatusCode::NO_CONTENT),
        Ok(Some(Delivery::Expired)) => Err(ApiError::new(
            StatusCode::GONE,
            "This browser's subscription has expired. Turn reminders on again.",
        )),
        Ok(Some(Delivery::Rejected(status))) => Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("The push service rejected the reminder (HTTP {status})."),
        )),
        Ok(None) => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Reminders are not on for this browser",
        )),
        Err(error) if error.chain().any(|cause| cause.is::<reqwest::Error>()) => {
            tracing::warn!(%error, profile_id, "could not send test reminder");
            Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                "Could not reach the push service. Try again shortly.",
            ))
        }
        Err(error) => Err(ApiError::internal(error)),
    }
}
