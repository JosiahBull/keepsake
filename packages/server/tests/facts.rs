use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use keepsake_server::{
    db,
    facts::{self, NewFact, NewReview, ReviewMethod, ReviewOutcome},
    router,
};
use tower::ServiceExt;

fn fact() -> NewFact {
    NewFact {
        prompt: "What does WAL stand for?".into(),
        answer: "Write-ahead logging".into(),
        context: "SQLite appends changes to a separate log before checkpointing.".into(),
        source: "https://sqlite.org/wal.html".into(),
        tags: vec!["sqlite".into()],
        practices: vec![],
    }
}

#[tokio::test]
async fn api_round_trip_and_validation_statuses() {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("facts.db").display()
    ))
    .await
    .unwrap();
    let app = router(pool.clone(), directory.path());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/facts")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&fact()).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let saved: facts::Fact = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved.practices.len(), 1);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/facts/{}", saved.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut invalid = fact();
    invalid.prompt = "  ".into();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/facts")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&invalid).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/facts/987654321")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/reviews")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&NewReview {
                        fact_id: saved.id,
                        practice_id: Some(saved.practices[0].id),
                        method: ReviewMethod::Flashcard,
                        outcome: ReviewOutcome::Again,
                        response_ms: 1234,
                    })
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().len(), 1);
    pool.close().await;
}
