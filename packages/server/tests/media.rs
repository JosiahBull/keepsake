#[path = "../../core/tests/support/mod.rs"]
mod test_db;
use test_db as db;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use keepsake_core::{
    facts::Fact,
    media::{MAX_MEDIA_BYTES, MediaUpload},
    profiles::{self, NewProfile},
};
use keepsake_server::router;
use tower::ServiceExt;

/// A quarter second of 16-bit mono PCM at 16 kHz.
fn wav() -> Vec<u8> {
    let data: Vec<u8> = (0..4000u32)
        .flat_map(|index| ((index % 200) as i16 * 50 - 5000).to_le_bytes())
        .collect();
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    out.extend_from_slice(&16_000u32.to_le_bytes());
    out.extend_from_slice(&32_000u32.to_le_bytes());
    out.extend_from_slice(b"\x02\0\x10\0data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

async fn app() -> (tempfile::TempDir, sqlx::SqlitePool, Router) {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("media.db").display()
    ))
    .await
    .unwrap();
    let app = router(pool.clone(), directory.path());
    (directory, pool, app)
}

async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

async fn body(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

fn upload(path: &str, declared: Option<&str>, bytes: impl Into<Body>) -> Request<Body> {
    let mut request = Request::builder().method("POST").uri(path);
    if let Some(declared) = declared {
        request = request.header(header::CONTENT_TYPE, declared);
    }
    request.body(bytes.into()).unwrap()
}

fn get(path: &str, range: Option<&str>) -> Request<Body> {
    let mut request = Request::builder()
        .uri(path)
        .header(header::ACCEPT_ENCODING, "gzip");
    if let Some(range) = range {
        request = request.header(header::RANGE, range);
    }
    request.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn an_uploaded_clip_is_attached_and_served_with_ranges_to_its_owner_only() {
    let (_directory, pool, app) = app().await;
    let guest = profiles::create(
        &pool,
        NewProfile {
            name: "Guest".into(),
            handle: "guest".into(),
        },
    )
    .await
    .unwrap();

    let wav = wav();
    let response = send(
        &app,
        upload("/api/profiles/1/media", Some("audio/x-wav"), wav.clone()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let saved: MediaUpload = serde_json::from_slice(&body(response).await).unwrap();
    assert_eq!(saved.content_type, "audio/wav");
    assert_eq!(saved.byte_size, wav.len() as i64);
    assert_eq!(saved.duration_ms, Some(250));

    let fact = serde_json::json!({
        "prompt": "What does this word mean?",
        "answer": "Water",
        "media": [{"media_id": saved.media_id}],
    });
    let response = send(
        &app,
        Request::builder()
            .method("POST")
            .uri("/api/profiles/1/facts")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(fact.to_string()))
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: Fact = serde_json::from_slice(&body(response).await).unwrap();
    assert_eq!(created.media[0].media_id, saved.media_id);

    let path = format!("/api/profiles/1/media/{}", saved.media_id);
    let whole = send(&app, get(&path, None)).await;
    assert_eq!(whole.status(), StatusCode::OK);
    let headers = whole.headers().clone();
    assert_eq!(headers[header::CONTENT_TYPE], "audio/wav");
    assert_eq!(headers[header::ACCEPT_RANGES], "bytes");
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "private, max-age=31536000, immutable"
    );
    assert_eq!(headers["cdn-cache-control"], "no-store");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert!(!headers.contains_key(header::CONTENT_ENCODING));
    assert_eq!(body(whole).await, wav);

    let partial = send(&app, get(&path, Some("bytes=0-3"))).await;
    assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        partial.headers()[header::CONTENT_RANGE],
        format!("bytes 0-3/{}", wav.len())
    );
    assert_eq!(body(partial).await, b"RIFF");

    let beyond = send(&app, get(&path, Some(&format!("bytes={}-", wav.len())))).await;
    assert_eq!(beyond.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        beyond.headers()[header::CONTENT_RANGE],
        format!("bytes */{}", wav.len())
    );
    assert_eq!(beyond.headers()[header::CACHE_CONTROL], "no-store");

    let other = send(
        &app,
        get(
            &format!("/api/profiles/{}/media/{}", guest.id, saved.media_id),
            None,
        ),
    )
    .await;
    assert_eq!(other.status(), StatusCode::NOT_FOUND);
    assert_eq!(other.headers()[header::CACHE_CONTROL], "no-store");
}

#[tokio::test]
async fn uploads_are_refused_unless_they_decode_as_their_declared_type() {
    let (_directory, _pool, app) = app().await;
    for (declared, bytes, message) in [
        (None, wav(), "Content-Type"),
        (
            Some("image/svg+xml"),
            b"<svg/>".to_vec(),
            "Unsupported file type",
        ),
        (Some("audio/mpeg"), wav(), "do not match"),
        (Some("audio/wav"), wav()[..44].to_vec(), "damaged"),
    ] {
        let response = send(&app, upload("/api/profiles/1/media", declared, bytes)).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{declared:?}");
        let error: serde_json::Value = serde_json::from_slice(&body(response).await).unwrap();
        assert!(
            error["error"].as_str().unwrap().contains(message),
            "{declared:?}: {error}"
        );
    }

    // Larger than the ordinary JSON body limit, within the media limit, is decoded.
    let mut large = wav();
    large.resize(MAX_MEDIA_BYTES, 0);
    let response = send(
        &app,
        upload("/api/profiles/1/media", Some("audio/wav"), large.clone()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    large.push(0);
    let response = send(
        &app,
        upload("/api/profiles/1/media", Some("audio/wav"), large),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
