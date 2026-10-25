#[path = "../../core/tests/support/mod.rs"]
mod test_db;
use test_db as db;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use keepsake_server::router;
use tower::ServiceExt;

#[tokio::test]
async fn cache_and_security_headers_cover_api_assets_shell_and_rejections() {
    let directory = tempfile::tempdir().unwrap();
    let web = directory.path().join("web");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("index.html"), "<h1>Keepsake</h1>").unwrap();
    std::fs::write(web.join("assets/app-abcd1234.js"), "console.log('ready')").unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("library.db").display()
    ))
    .await
    .unwrap();
    let app = router(pool.clone(), &web);
    for (method, path, body, status, cache) in [
        ("GET", "/api/facts", "", StatusCode::OK, "no-store"),
        ("GET", "/api/health", "", StatusCode::OK, "no-store"),
        (
            "POST",
            "/api/facts",
            "{",
            StatusCode::BAD_REQUEST,
            "no-store",
        ),
        (
            "DELETE",
            "/api/facts",
            "",
            StatusCode::METHOD_NOT_ALLOWED,
            "no-store",
        ),
        ("GET", "/api/missing", "", StatusCode::NOT_FOUND, "no-store"),
        (
            "GET",
            "/assets/missing.js",
            "",
            StatusCode::NOT_FOUND,
            "no-store",
        ),
        (
            "GET",
            "/assets/app-abcd1234.js",
            "",
            StatusCode::OK,
            "public, max-age=31536000, immutable",
        ),
        (
            "GET",
            "/",
            "",
            StatusCode::OK,
            "public, max-age=0, must-revalidate",
        ),
        (
            "GET",
            "/index.html",
            "",
            StatusCode::OK,
            "public, max-age=0, must-revalidate",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::ORIGIN, "https://untrusted.example")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{method} {path}");
        let headers = response.headers();
        assert_eq!(headers[header::CACHE_CONTROL], cache, "{path}");
        assert_eq!(headers["cdn-cache-control"], cache);
        assert_eq!(headers["cloudflare-cdn-cache-control"], cache);
        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert_eq!(headers["x-frame-options"], "DENY");
        assert_eq!(headers["referrer-policy"], "no-referrer");
        assert_eq!(headers["cross-origin-opener-policy"], "same-origin");
        assert!(!headers.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    }
    // Payload-limit rejections are produced before a handler executes.
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/facts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("x".repeat(512 * 1024 + 1)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    pool.close().await;
}
