//! Web Push delivery. Subscriptions and reminder policy are stored by `keepsake_core::reminders`.
use std::{net::IpAddr, sync::Arc, time::Duration};

use anyhow::{Context, anyhow};
use axum::http::{HeaderValue, Uri};
use base64ct::{Base64UrlUnpadded, Encoding};
use keepsake_core::{
    profiles,
    reminders::{self, Device, NewDevice, Reminder},
    scheduling,
};
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;
use web_push_native::{
    Auth, WebPushBuilder,
    jwt_simple::algorithms::{ECDSAP256PublicKeyLike, ES256KeyPair},
    p256::PublicKey,
};

use crate::config::PushConfig;

/// Push services keep an undelivered reminder this long; VAPID tokens share the lifetime.
const TTL: Duration = Duration::from_secs(12 * 60 * 60);
const CHECK_INTERVAL: Duration = Duration::from_secs(60);

pub struct Notifier {
    pool: SqlitePool,
    config: PushConfig,
    client: reqwest::Client,
    identity: OnceCell<Identity>,
}

struct Identity {
    key: ES256KeyPair,
    public_key: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Delivery {
    Delivered,
    /// The push service no longer knows the subscription, so every profile forgets it.
    Expired,
    Rejected(u16),
}

/// A declarative push message. Safari 18.4 and later display it without running the
/// service worker; other browsers pass it to the service worker, which displays it.
#[derive(Serialize)]
pub struct Message {
    web_push: u16,
    notification: Notification,
}

#[derive(Serialize)]
struct Notification {
    title: String,
    body: String,
    navigate: String,
    lang: &'static str,
    dir: &'static str,
    tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_badge: Option<String>,
}

impl Message {
    fn new(title: String, body: String, navigate: String, tag: String, badge: Option<i64>) -> Self {
        Self {
            web_push: 8030,
            notification: Notification {
                title,
                body,
                navigate,
                lang: "en",
                dir: "ltr",
                tag,
                app_badge: badge.map(|count| count.to_string()),
            },
        }
    }

    pub fn reminder(reminder: &Reminder, origin: &str) -> Self {
        let summary = &reminder.summary;
        let reviews = plural(summary.due, "review", "reviews");
        let new = plural(summary.new_available, "new fact", "new facts");
        let body = match (summary.due, summary.new_available) {
            (0, count) => format!("{new} {} ready to learn.", verb(count)),
            (count, 0) => format!("{reviews} {} ready.", verb(count)),
            _ => format!("{reviews} and {new} are ready."),
        };
        Self::new(
            format!("Time to practise, {}", reminder.name),
            body,
            format!("{origin}/p/{}/practice", reminder.profile_id),
            format!("reminder-{}", reminder.profile_id),
            Some(summary.due + summary.new_available),
        )
    }

    fn test(profile_id: i64, name: &str, origin: &str) -> Self {
        Self::new(
            "Reminders are on".into(),
            format!("{name} will get one reminder a day when practice is ready."),
            format!("{origin}/p/{profile_id}/settings"),
            format!("test-{profile_id}"),
            None,
        )
    }
}

fn plural(count: i64, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn verb(count: i64) -> &'static str {
    if count == 1 { "is" } else { "are" }
}

fn decode(value: &str) -> Option<Vec<u8>> {
    Base64UrlUnpadded::decode_vec(value.trim_end_matches('=')).ok()
}

fn auth_secret(value: &str) -> Option<Auth> {
    let bytes: [u8; 16] = decode(value)?.try_into().ok()?;
    Some(Auth::from(bytes))
}

fn host(endpoint: &str) -> String {
    // Endpoints are bearer capabilities; logs keep only the push service's host.
    endpoint
        .parse::<Uri>()
        .ok()
        .and_then(|uri| uri.host().map(str::to_owned))
        .unwrap_or_default()
}

impl Notifier {
    pub fn new(pool: SqlitePool, config: PushConfig) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Keepsake/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("build push HTTP client")?;
        Ok(Self {
            pool,
            config,
            client,
            identity: OnceCell::new(),
        })
    }

    async fn identity(&self) -> anyhow::Result<&Identity> {
        self.identity
            .get_or_try_init(|| async {
                let candidate: [u8; 32] = ES256KeyPair::generate()
                    .to_bytes()
                    .try_into()
                    .map_err(|_| anyhow!("unexpected VAPID key length"))?;
                let stored = reminders::vapid_key(&self.pool, candidate).await?;
                let key = ES256KeyPair::from_bytes(&stored)
                    .map_err(|error| anyhow!("stored VAPID key is invalid: {error}"))?;
                let public_key = Base64UrlUnpadded::encode_string(
                    &key.public_key().public_key().to_bytes_uncompressed(),
                );
                Ok(Identity { key, public_key })
            })
            .await
    }

    /// The application server key browsers subscribe with.
    pub async fn public_key(&self) -> anyhow::Result<String> {
        Ok(self.identity().await?.public_key.clone())
    }

    /// Reject subscriptions this server could not encrypt for or should not contact.
    pub fn validate(&self, origin: &str, device: &NewDevice) -> Result<(), &'static str> {
        let insecure = self.config.allow_insecure_endpoints;
        let endpoint: Uri = device
            .endpoint
            .parse()
            .map_err(|_| "Push endpoint must be a URL")?;
        match endpoint.scheme_str() {
            Some("https") => {}
            Some("http") if insecure => {}
            _ => return Err("Push endpoint must use HTTPS"),
        }
        let authority = endpoint
            .authority()
            .ok_or("Push endpoint must include a host")?;
        if authority.as_str().contains('@') {
            return Err("Push endpoint must not contain credentials");
        }
        let host = authority.host();
        let literal = host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok();
        if !insecure && (literal || host.eq_ignore_ascii_case("localhost")) {
            return Err("Push endpoint must name a push service");
        }
        decode(&device.keys.p256dh)
            .and_then(|bytes| PublicKey::from_sec1_bytes(&bytes).ok())
            .ok_or("Push key p256dh is invalid")?;
        auth_secret(&device.keys.auth).ok_or("Push key auth is invalid")?;
        let parsed: Uri = origin.parse().map_err(|_| "Origin is invalid")?;
        match (parsed.scheme_str(), parsed.authority()) {
            (Some(scheme @ ("http" | "https")), Some(authority))
                if origin == format!("{scheme}://{authority}") => {}
            _ => return Err("Origin is invalid"),
        }
        Ok(())
    }

    pub async fn send(&self, device: &Device, message: &Message) -> anyhow::Result<Delivery> {
        let identity = self.identity().await?;
        let public = decode(&device.p256dh)
            .and_then(|bytes| PublicKey::from_sec1_bytes(&bytes).ok())
            .context("stored push key p256dh is invalid")?;
        let auth = auth_secret(&device.auth).context("stored push key auth is invalid")?;
        let mut request = WebPushBuilder::new(
            device.endpoint.parse().context("stored push endpoint")?,
            public,
            auth,
        )
        .with_valid_duration(TTL)
        .with_vapid(&identity.key, &self.config.contact)
        .build(serde_json::to_vec(message)?)
        .map_err(|error| anyhow!("encrypt push message: {error}"))?;
        request
            .headers_mut()
            .insert("urgency", HeaderValue::from_static("normal"));
        let response = self
            .client
            .execute(reqwest::Request::try_from(request)?)
            .await
            .context("send push message")?;
        Ok(match response.status().as_u16() {
            200..=299 => Delivery::Delivered,
            404 | 410 => {
                reminders::forget_endpoint(&self.pool, &device.endpoint).await?;
                Delivery::Expired
            }
            status => Delivery::Rejected(status),
        })
    }

    /// Send one confirmation to a profile's registered browser. `None` if it is not registered.
    pub async fn test(&self, profile_id: i64, endpoint: &str) -> anyhow::Result<Option<Delivery>> {
        let Some(device) = reminders::devices_for(&self.pool, profile_id)
            .await?
            .into_iter()
            .find(|device| device.endpoint == endpoint)
        else {
            return Ok(None);
        };
        let name = profiles::get(&self.pool, profile_id)
            .await?
            .context("profile missing")?
            .name;
        let message = Message::test(profile_id, &name, &device.origin);
        self.send(&device, &message).await.map(Some)
    }

    /// Claim and send due reminders, returning the profiles that were reminded.
    pub async fn remind(&self, now: i64) -> Vec<i64> {
        let profiles = match reminders::subscribed_profiles(&self.pool).await {
            Ok(profiles) => profiles,
            Err(error) => {
                tracing::warn!(%error, "could not list reminder subscriptions");
                return Vec::new();
            }
        };
        let mut reminded = Vec::new();
        for profile_id in profiles {
            let reminder = match reminders::claim_for(&self.pool, profile_id, now).await {
                Ok(Some(reminder)) => reminder,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(%error, profile_id, "could not check reminder");
                    continue;
                }
            };
            let devices = match reminders::devices_for(&self.pool, profile_id).await {
                Ok(devices) => devices,
                Err(error) => {
                    tracing::warn!(%error, profile_id, "could not load reminder devices");
                    continue;
                }
            };
            let mut delivered = 0;
            for device in &devices {
                let host = host(&device.endpoint);
                match self
                    .send(device, &Message::reminder(&reminder, &device.origin))
                    .await
                {
                    Ok(Delivery::Delivered) => delivered += 1,
                    Ok(Delivery::Expired) => {
                        tracing::info!(profile_id, %host, "forgot expired push subscription")
                    }
                    Ok(Delivery::Rejected(status)) => {
                        tracing::warn!(profile_id, status, %host, "push service rejected reminder")
                    }
                    Err(error) => {
                        tracing::warn!(%error, profile_id, %host, "could not send reminder")
                    }
                }
            }
            tracing::info!(
                profile_id,
                delivered,
                devices = devices.len(),
                "daily reminder sent"
            );
            reminded.push(profile_id);
        }
        reminded
    }
}

/// Check for due reminders each minute until shutdown.
pub async fn run(notifier: Arc<Notifier>, shutdown: CancellationToken) {
    let mut interval = tokio::time::interval(CHECK_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => return,
            _ = interval.tick() => {}
        }
        tokio::select! {
            _ = shutdown.cancelled() => return,
            _ = notifier.remind(scheduling::now()) => {}
        }
    }
}
