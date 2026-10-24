use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use keepsake_server::{
    db,
    facts::{self, NewFact},
    router,
};
use serde_json::Value;
use tower::ServiceExt;

async fn get(app: axum::Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn exposes_page_cursors_global_samples_and_validated_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("api-pages.db").display()
    ))
    .await
    .unwrap();
    for index in 0..4 {
        facts::create(
            &pool,
            NewFact {
                prompt: format!("Question {index}"),
                answer: format!("Answer {index}"),
                context: "Source context".into(),
                source: format!("https://example.org/{index}"),
                tags: vec!["topic".into()],
                practices: vec![],
            },
        )
        .await
        .unwrap();
    }
    let app = router(pool.clone(), directory.path());
    let (status, page) = get(app.clone(), "/api/facts?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert_eq!(page["total"], 4);
    assert_eq!(page["next_cursor"], 3);
    assert!(page["items"][0].get("practices").is_none());
    let (_, next) = get(app.clone(), "/api/facts?limit=2&before_id=3").await;
    assert_eq!(next["items"][0]["id"], 2);
    assert!(next["next_cursor"].is_null());
    let (_, filtered) = get(app.clone(), "/api/facts?search=Question%202").await;
    assert_eq!(filtered["total"], 1);
    let (_, session) = get(app.clone(), "/api/practice?limit=2").await;
    assert_eq!(session["items"].as_array().unwrap().len(), 2);
    assert_eq!(session["total"], 4);
    assert_eq!(session["items"][0]["fact"]["context"], "Source context");
    let (_, sample) = get(app.clone(), "/api/facts/sample?limit=3").await;
    assert_eq!(sample["items"].as_array().unwrap().len(), 3);
    let (_, reviews) = get(app.clone(), "/api/facts/1/reviews?limit=2").await;
    assert!(reviews["items"].as_array().unwrap().is_empty());
    for path in [
        "/api/facts?limit=101",
        "/api/facts?before_id=0",
        "/api/practice?limit=51",
        "/api/facts/sample?limit=5",
        "/api/facts/1/reviews?limit=0",
    ] {
        let (status, error) = get(app.clone(), path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert!(error["error"].is_string());
    }
    assert_eq!(
        get(app, "/api/facts/999/reviews").await.0,
        StatusCode::NOT_FOUND
    );
    pool.close().await;
}
