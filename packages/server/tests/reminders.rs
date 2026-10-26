#[path = "../../core/tests/support/mod.rs"]
mod test_db;
use test_db as db;

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode},
    routing::post,
};
use base64ct::{Base64UrlUnpadded, Encoding};
use http_body_util::BodyExt;
use keepsake_server::{config::PushConfig, limits::Limits, notifications::Notifier, router_with};
use serde_json::{Value, json};
use tower::ServiceExt;
use web_push_native::{
    Auth,
    jwt_simple::{
        algorithms::{ECDSAP256PublicKeyLike, ES256PublicKey},
        claims::NoCustomClaims,
    },
    p256::{SecretKey, elliptic_curve::sec1::ToEncodedPoint},
};

const ORIGIN: &str = "https://keepsake.example";

type Received = Arc<Mutex<Vec<(String, HeaderMap, Bytes)>>>;

/// A local push service: `gone` subscriptions answer 410, everything else is accepted.
async fn push_service() -> (String, Received) {
    let received = Received::default();
    let app = Router::new()
        .route(
            "/push/{id}",
            post(
                |State(received): State<Received>,
                 Path(id): Path<String>,
                 headers: HeaderMap,
                 body: Bytes| async move {
                    let gone = id == "gone";
                    received.lock().unwrap().push((id, headers, body));
                    if gone {
                        StatusCode::GONE
                    } else {
                        StatusCode::CREATED
                    }
                },
            ),
        )
        .with_state(received.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}/push"), received)
}

/// A browser's subscription secret and the JSON its PushSubscription would expose.
struct Browser {
    secret: SecretKey,
    auth: [u8; 16],
}

impl Browser {
    fn new(seed: u8) -> Self {
        Self {
            secret: SecretKey::from_slice(&[seed; 32]).unwrap(),
            auth: [seed; 16],
        }
    }

    fn subscription(&self, endpoint: &str) -> Value {
        let public = self.secret.public_key().to_encoded_point(false);
        json!({"endpoint": endpoint, "keys": {
            "p256dh": Base64UrlUnpadded::encode_string(public.as_bytes()),
            "auth": Base64UrlUnpadded::encode_string(&self.auth),
        }})
    }

    fn read(&self, body: &Bytes) -> Value {
        let plain =
            web_push_native::decrypt(body.to_vec(), &self.secret, &Auth::from(self.auth)).unwrap();
        serde_json::from_slice(&plain).unwrap()
    }
}

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    origin: bool,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if origin {
        request = request.header("origin", ORIGIN);
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(serde_json::to_vec(&body).unwrap())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

struct Fixture {
    _directory: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    notifier: Arc<Notifier>,
    app: Router,
}

async fn fixture(allow_insecure_endpoints: bool) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("reminders.db").display()
    ))
    .await
    .unwrap();
    let notifier = Arc::new(
        Notifier::new(
            pool.clone(),
            PushConfig {
                contact: "mailto:operator@keepsake.example".into(),
                allow_insecure_endpoints,
            },
        )
        .unwrap(),
    );
    let app = router_with(
        pool.clone(),
        directory.path(),
        Limits::default(),
        notifier.clone(),
    );
    Fixture {
        _directory: directory,
        pool,
        notifier,
        app,
    }
}

#[tokio::test]
async fn reminders_are_encrypted_signed_and_sent_once_per_local_day() {
    let (service, received) = push_service().await;
    let f = fixture(true).await;
    let (status, key) = call(&f.app, "GET", "/api/push/key", false, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let public_key = key["public_key"].as_str().unwrap().to_owned();
    assert_eq!(
        Base64UrlUnpadded::decode_vec(&public_key).unwrap().len(),
        65
    );

    let phone = Browser::new(7);
    let endpoint = format!("{service}/phone");
    let subscription = phone.subscription(&endpoint);
    let (status, error) = call(
        &f.app,
        "PUT",
        "/api/profiles/1/reminders/devices",
        false,
        subscription.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    let (status, saved) = call(
        &f.app,
        "PUT",
        "/api/profiles/1/reminders/devices",
        true,
        subscription,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["devices"], 1);
    assert_eq!(saved["minute_of_day"], 480);

    let (status, _) = call(
        &f.app,
        "POST",
        "/api/profiles/1/reminders/test",
        false,
        json!({"endpoint": endpoint}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    {
        let received = received.lock().unwrap();
        let (_, headers, body) = &received[0];
        assert_eq!(headers["content-encoding"], "aes128gcm");
        assert_eq!(headers["ttl"], "43200");
        assert_eq!(headers["urgency"], "normal");
        let authorization = headers["authorization"].to_str().unwrap();
        assert!(authorization.starts_with("vapid t="));
        assert!(authorization.ends_with(&format!(", k={public_key}")));
        let token = authorization
            .strip_prefix("vapid t=")
            .and_then(|rest| rest.split(", ").next())
            .unwrap();
        let verifier =
            ES256PublicKey::from_bytes(&Base64UrlUnpadded::decode_vec(&public_key).unwrap())
                .unwrap();
        let claims = verifier
            .verify_token::<NoCustomClaims>(token, None)
            .unwrap();
        assert_eq!(
            claims.subject.as_deref(),
            Some("mailto:operator@keepsake.example")
        );
        let audience = claims.audiences.unwrap().into_string().unwrap();
        assert!(service.starts_with(&audience), "{audience}");
        let lifetime = claims.expires_at.unwrap() - claims.issued_at.unwrap();
        assert!(
            lifetime.as_secs() <= 24 * 3600,
            "Apple rejects longer tokens"
        );
        let message = phone.read(body);
        assert_eq!(message["web_push"], 8030);
        assert_eq!(message["notification"]["title"], "Reminders are on");
        assert_eq!(
            message["notification"]["navigate"],
            format!("{ORIGIN}/p/1/settings")
        );
    }

    // 2026-10-21T08:00:00Z is 09:00 in London, after the default 08:00 reminder time.
    let morning = 1_792_569_600;
    assert!(
        f.notifier.remind(morning).await.is_empty(),
        "nothing is ready"
    );
    let (status, _) = call(
        &f.app,
        "POST",
        "/api/profiles/1/facts",
        false,
        json!({"prompt":"Synthetic reminder question?","answer":"Synthetic answer",
            "context":"","source":"","tags":[],"practices":[]}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, summary) = call(
        &f.app,
        "GET",
        "/api/profiles/1/study/summary",
        false,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(summary["new_available"], 1);

    assert_eq!(f.notifier.remind(morning).await, vec![1]);
    assert!(f.notifier.remind(morning + 3600).await.is_empty());
    let reminder = {
        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        phone.read(&received[1].2)
    };
    assert_eq!(
        reminder["notification"],
        json!({
            "title": "Time to practise, Existing profile",
            "body": "1 new fact is ready to learn.",
            "navigate": format!("{ORIGIN}/p/1/practice"),
            "lang": "en",
            "dir": "ltr",
            "tag": "reminder-1",
            "app_badge": "1"
        })
    );
    f.pool.close().await;
}

#[tokio::test]
async fn expired_subscriptions_are_forgotten_and_devices_can_leave() {
    let (service, received) = push_service().await;
    let f = fixture(true).await;
    let old = Browser::new(3);
    let gone = format!("{service}/gone");
    let kept = format!("{service}/kept");
    for endpoint in [&gone, &kept] {
        let (status, _) = call(
            &f.app,
            "PUT",
            "/api/profiles/1/reminders/devices",
            true,
            old.subscription(endpoint),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, error) = call(
        &f.app,
        "POST",
        "/api/profiles/1/reminders/test",
        false,
        json!({"endpoint": gone}),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    assert!(error["error"].as_str().unwrap().contains("expired"));
    assert_eq!(received.lock().unwrap().len(), 1);
    let (_, settings) = call(
        &f.app,
        "GET",
        "/api/profiles/1/reminders",
        false,
        Value::Null,
    )
    .await;
    assert_eq!(settings["devices"], 1);
    let (status, _) = call(
        &f.app,
        "POST",
        "/api/profiles/1/reminders/test",
        false,
        json!({"endpoint": gone}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, settings) = call(
        &f.app,
        "DELETE",
        "/api/profiles/1/reminders/devices",
        false,
        json!({"endpoint": kept}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(settings["devices"], 0);
    let (status, settings) = call(
        &f.app,
        "PUT",
        "/api/profiles/1/reminders",
        false,
        json!({"minute_of_day": 1110}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(settings["minute_of_day"], 1110);
    f.pool.close().await;
}

#[tokio::test]
async fn production_validation_only_accepts_https_push_services() {
    let f = fixture(false).await;
    let browser = Browser::new(5);
    let mut tampered = browser.subscription("https://push.example/abc");
    tampered["keys"]["auth"] = json!("c2hvcnQ");
    for (subscription, message) in [
        (
            browser.subscription("http://push.example/abc"),
            "Push endpoint must use HTTPS",
        ),
        (
            browser.subscription("https://127.0.0.1/abc"),
            "Push endpoint must name a push service",
        ),
        (
            browser.subscription("https://user@push.example/abc"),
            "Push endpoint must not contain credentials",
        ),
        (tampered, "Push key auth is invalid"),
    ] {
        let (status, error) = call(
            &f.app,
            "PUT",
            "/api/profiles/1/reminders/devices",
            true,
            subscription,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["error"], message);
    }
    let (status, saved) = call(
        &f.app,
        "PUT",
        "/api/profiles/1/reminders/devices",
        true,
        browser.subscription("https://push.example/abc"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    // A restarted server reuses the stored VAPID identity, so subscriptions stay valid.
    let first = f.notifier.public_key().await.unwrap();
    let restarted = Notifier::new(f.pool.clone(), PushConfig::default()).unwrap();
    assert_eq!(restarted.public_key().await.unwrap(), first);
    f.pool.close().await;
}
