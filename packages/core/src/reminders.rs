//! Daily practice reminders for Web Push subscriptions. Delivery belongs to the server.
use anyhow::Context;
use chrono::{TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::ToSchema;

use crate::scheduling::{self, StudyError, StudySummary};

/// A product default to evaluate, not a researched optimum.
pub const DEFAULT_MINUTE_OF_DAY: i64 = 8 * 60;
/// Bounds fan-out from one profile; registering past this replaces the oldest device.
pub const MAX_DEVICES: i64 = 20;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceKeys {
    pub p256dh: String,
    pub auth: String,
}

/// The endpoint and keys of a browser's PushSubscription.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewDevice {
    pub endpoint: String,
    pub keys: DeviceKeys,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceEndpoint {
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct ReminderSettings {
    /// The earliest local reminder time, in minutes after midnight.
    pub minute_of_day: i64,
    /// The study timezone that defines the local day.
    pub timezone: String,
    /// Subscribed browsers that receive this profile's reminders.
    pub devices: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReminderUpdate {
    pub minute_of_day: i64,
}

#[derive(Clone, Debug)]
pub struct Device {
    pub endpoint: String,
    pub origin: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Clone, Debug)]
pub struct Reminder {
    pub profile_id: i64,
    pub name: String,
    pub summary: StudySummary,
}

fn validate_device(origin: &str, device: &NewDevice) -> anyhow::Result<()> {
    for (value, limit, message) in [
        (device.endpoint.as_str(), 2048, "Push endpoint is invalid"),
        (origin, 255, "Origin is invalid"),
    ] {
        if !(1..=limit).contains(&value.len())
            || value.chars().any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(StudyError::Invalid(message.into()).into());
        }
    }
    for key in [&device.keys.p256dh, &device.keys.auth] {
        if !(1..=256).contains(&key.len())
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'='))
        {
            return Err(StudyError::Invalid("Push keys must be base64url".into()).into());
        }
    }
    Ok(())
}

/// Store `candidate` unless a key already exists, then return the stored key.
pub async fn vapid_key(pool: &SqlitePool, candidate: [u8; 32]) -> anyhow::Result<Vec<u8>> {
    crate::db::write_transaction(pool, move |connection| {
        Box::pin(async move {
            let candidate = candidate.as_slice();
            sqlx::query!(
                "INSERT INTO push_keys(id, private_key) VALUES (1, ?) ON CONFLICT(id) DO NOTHING",
                candidate
            )
            .execute(&mut *connection)
            .await?;
            Ok(
                sqlx::query_scalar!("SELECT private_key FROM push_keys WHERE id = 1")
                    .fetch_one(connection)
                    .await?,
            )
        })
    })
    .await
}

pub async fn settings_for(pool: &SqlitePool, profile_id: i64) -> anyhow::Result<ReminderSettings> {
    let timezone = scheduling::settings_for(pool, profile_id).await?.timezone;
    let minute_of_day = sqlx::query_scalar!(
        "SELECT minute_of_day FROM reminder_settings WHERE profile_id = ?",
        profile_id
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(DEFAULT_MINUTE_OF_DAY);
    let devices = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!: i64\" FROM push_devices WHERE profile_id = ?",
        profile_id
    )
    .fetch_one(pool)
    .await?;
    Ok(ReminderSettings {
        minute_of_day,
        timezone,
        devices,
    })
}

pub async fn update_settings_for(
    pool: &SqlitePool,
    profile_id: i64,
    update: ReminderUpdate,
) -> anyhow::Result<ReminderSettings> {
    let minute = update.minute_of_day;
    if !(0..1440).contains(&minute) {
        return Err(StudyError::Invalid("minute_of_day must be between 0 and 1439".into()).into());
    }
    crate::db::write_transaction(pool, move |connection| {
        Box::pin(async move {
            crate::profiles::require(connection, profile_id).await?;
            sqlx::query!(
                "INSERT INTO reminder_settings(profile_id, minute_of_day) VALUES (?, ?)
                 ON CONFLICT(profile_id) DO UPDATE SET minute_of_day = excluded.minute_of_day",
                profile_id,
                minute
            )
            .execute(connection)
            .await?;
            Ok(())
        })
    })
    .await?;
    settings_for(pool, profile_id).await
}

/// Add or refresh this browser for a profile. Retrying the same subscription is harmless.
/// `origin` is the browser's view of this server, used for notification links.
pub async fn register_device_for(
    pool: &SqlitePool,
    profile_id: i64,
    origin: String,
    device: NewDevice,
) -> anyhow::Result<ReminderSettings> {
    validate_device(&origin, &device)?;
    crate::db::write_transaction(pool, move |connection| {
        let (origin, device) = (origin.clone(), device.clone());
        Box::pin(async move {
            crate::profiles::require(connection, profile_id).await?;
            sqlx::query!(
                "INSERT INTO push_devices(profile_id, endpoint, origin, p256dh, auth)
                 VALUES (?, ?, ?, ?, ?) ON CONFLICT(profile_id, endpoint) DO NOTHING",
                profile_id,
                device.endpoint,
                origin,
                device.keys.p256dh,
                device.keys.auth
            )
            .execute(&mut *connection)
            .await?;
            // Every profile on a shared browser must encrypt with the subscription's current keys.
            sqlx::query!(
                "UPDATE push_devices SET origin = ?, p256dh = ?, auth = ? WHERE endpoint = ?",
                origin,
                device.keys.p256dh,
                device.keys.auth,
                device.endpoint
            )
            .execute(&mut *connection)
            .await?;
            sqlx::query!(
                "DELETE FROM push_devices WHERE profile_id = ? AND id NOT IN (
                   SELECT id FROM push_devices WHERE profile_id = ? ORDER BY id DESC LIMIT ?
                 )",
                profile_id,
                profile_id,
                MAX_DEVICES
            )
            .execute(&mut *connection)
            .await?;
            sqlx::query!(
                "INSERT INTO reminder_settings(profile_id, minute_of_day) VALUES (?, ?)
                 ON CONFLICT(profile_id) DO NOTHING",
                profile_id,
                DEFAULT_MINUTE_OF_DAY
            )
            .execute(connection)
            .await?;
            Ok(())
        })
    })
    .await?;
    settings_for(pool, profile_id).await
}

pub async fn remove_device_for(
    pool: &SqlitePool,
    profile_id: i64,
    endpoint: &str,
) -> anyhow::Result<ReminderSettings> {
    crate::profiles::require(&mut *pool.acquire().await?, profile_id).await?;
    sqlx::query!(
        "DELETE FROM push_devices WHERE profile_id = ? AND endpoint = ?",
        profile_id,
        endpoint
    )
    .execute(pool)
    .await?;
    settings_for(pool, profile_id).await
}

pub async fn devices_for(pool: &SqlitePool, profile_id: i64) -> anyhow::Result<Vec<Device>> {
    Ok(sqlx::query_as!(
        Device,
        "SELECT endpoint, origin, p256dh, auth FROM push_devices WHERE profile_id = ? ORDER BY id",
        profile_id
    )
    .fetch_all(pool)
    .await?)
}

/// Remove a subscription that its push service reports as expired, for every profile.
pub async fn forget_endpoint(pool: &SqlitePool, endpoint: &str) -> anyhow::Result<u64> {
    Ok(
        sqlx::query!("DELETE FROM push_devices WHERE endpoint = ?", endpoint)
            .execute(pool)
            .await?
            .rows_affected(),
    )
}

/// Profiles with at least one subscribed device, in a stable order.
pub async fn subscribed_profiles(pool: &SqlitePool) -> anyhow::Result<Vec<i64>> {
    Ok(
        sqlx::query_scalar!("SELECT DISTINCT profile_id FROM push_devices ORDER BY profile_id")
            .fetch_all(pool)
            .await?,
    )
}

/// Claim today's reminder once its local time has passed and practice is ready.
///
/// A claim is recorded before delivery, so a crash can skip a day but never repeat one.
/// Nothing is claimed before the chosen time, when nothing is ready, or after the daily
/// budget is spent; a later check the same day can still claim it.
pub async fn claim_for(
    pool: &SqlitePool,
    profile_id: i64,
    now: i64,
) -> anyhow::Result<Option<Reminder>> {
    let Some(saved) = sqlx::query!(
        "SELECT minute_of_day, last_sent_on FROM reminder_settings WHERE profile_id = ?",
        profile_id
    )
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let timezone: Tz = scheduling::settings_for(pool, profile_id)
        .await?
        .timezone
        .parse()
        .context("invalid stored timezone")?;
    let local = Utc
        .timestamp_opt(now, 0)
        .single()
        .context("timestamp out of range")?
        .with_timezone(&timezone);
    let today = local.date_naive().to_string();
    if i64::from(local.hour() * 60 + local.minute()) < saved.minute_of_day
        || saved.last_sent_on.as_deref() == Some(today.as_str())
    {
        return Ok(None);
    }
    let summary = scheduling::summary_at_for(pool, profile_id, now).await?;
    if summary.budget_reached || summary.due + summary.new_available == 0 {
        return Ok(None);
    }
    let claimed = sqlx::query!(
        "UPDATE reminder_settings SET last_sent_on = ?
         WHERE profile_id = ? AND last_sent_on IS NOT ?",
        today,
        profile_id,
        today
    )
    .execute(pool)
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return Ok(None);
    }
    let name = crate::profiles::get(pool, profile_id)
        .await?
        .context("reminder profile missing")?
        .name;
    Ok(Some(Reminder {
        profile_id,
        name,
        summary,
    }))
}
