use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use keepsake_server::{db, router};
use tower::ServiceExt;

#[tokio::test]
async fn serves_spa_and_keeps_unknown_api_routes_out_of_it() {
    let directory = tempfile::tempdir().unwrap();
    let web = directory.path().join("web");
    std::fs::create_dir(&web).unwrap();
    std::fs::write(web.join("index.html"), "<h1>Keepsake</h1>").unwrap();
    std::fs::write(web.join("app.js"), "console.log('keepsake');").unwrap();
    let url = format!(
        "sqlite:{}",
        directory.path().join("nested/test.db").display()
    );
    let pool = db::connect(&url).await.unwrap();
    let app = router(pool.clone(), &web);
    for (path, status, expected) in [
        ("/api/health", StatusCode::OK, Some("\"status\":\"ok\"")),
        ("/api/openapi.json", StatusCode::OK, Some("/api/health")),
        ("/api/facts", StatusCode::OK, Some("\"items\":[]")),
        ("/api/missing", StatusCode::NOT_FOUND, None),
        ("/api", StatusCode::NOT_FOUND, None),
        ("/", StatusCode::OK, Some("<h1>Keepsake</h1>")),
        ("/app.js", StatusCode::OK, Some("console.log")),
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        if let Some(expected) = expected {
            assert!(
                std::str::from_utf8(&bytes).unwrap().contains(expected),
                "{path}"
            );
        }
    }
    pool.close().await;
    let reopened = db::connect(&url).await.unwrap();
    reopened.close().await;
}

#[tokio::test]
async fn health_reports_unavailable_database() {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("test.db").display()
    ))
    .await
    .unwrap();
    let app = router(pool.clone(), directory.path());
    pool.close().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
