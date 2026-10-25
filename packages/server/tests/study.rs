use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use http_body_util::BodyExt;
use keepsake_core::facts::{self, NewFact, NewPractice, PracticeKind};
use keepsake_server::{db, router};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, HeaderMap, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
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
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, headers, body)
}

async fn library() -> (tempfile::TempDir, sqlx::SqlitePool, Router, facts::Fact) {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("study.db").display()
    ))
    .await
    .unwrap();
    let fact = facts::create(
        &pool,
        NewFact {
            prompt: "Synthetic recall question?".into(),
            answer: "Synthetic canonical answer".into(),
            context: "Source-checked synthetic fixture.".into(),
            source: "Synthetic test source".into(),
            tags: vec![],
            practices: vec![
                NewPractice {
                    kind: PracticeKind::Cloze,
                    prompt: "Complete: Synthetic canonical ___.".into(),
                    answer: "Synthetic canonical answer".into(),
                    choices: vec![],
                    explanation: String::new(),
                },
                NewPractice {
                    kind: PracticeKind::MultipleChoice,
                    prompt: "Which is the synthetic answer?".into(),
                    answer: "Synthetic canonical answer".into(),
                    choices: vec!["Synthetic canonical answer".into(), "Other answer".into()],
                    explanation: String::new(),
                },
            ],
        },
    )
    .await
    .unwrap();
    let app = router(pool.clone(), directory.path());
    (directory, pool, app, fact)
}

fn attempt(question: &Value) -> Value {
    json!({
        "submission_id": uuid::Uuid::new_v4().to_string(),
        "target_id": question["target_id"],
        "practice_id": question["practice"]["id"],
        "content_revision": question["content_revision"],
        "expected_version": question["schedule_version"],
        "kind": "attempt",
        "context": "scheduled",
        "method": "flashcard",
        "outcome": "good",
        "response_ms": 1500,
        "attempted": true,
        "assisted": false,
        "prior_exposure": false,
    })
}

#[tokio::test]
async fn scheduled_attempt_is_idempotent_and_rejects_stale_or_changed_retries() {
    let (_directory, pool, app, fact) = library().await;
    let (status, _, queue) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(queue["items"].as_array().unwrap().len(), 1);
    assert_eq!(queue["items"][0]["practice"]["kind"], "flashcard");
    assert_eq!(queue["items"][0]["intervals"]["again"], 600);
    let request = attempt(&queue["items"][0]);

    let (status, headers, saved) = call(&app, "POST", "/api/study/events", request.clone()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(headers["cache-control"], "no-store");
    assert!(headers.contains_key("x-request-id"));
    assert_eq!(saved["qualifying"], true);
    assert_eq!(saved["review"]["fact_id"], fact.id);
    assert_eq!(saved["review"]["outcome"], "good");
    assert!(saved["schedule"]["stability"].as_f64().unwrap() > 0.0);
    assert!(saved["next_due_at"].as_i64().is_some());

    let (status, _, duplicate) = call(&app, "POST", "/api/study/events", request.clone()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(duplicate, saved);

    let mut stale = request.clone();
    stale["submission_id"] = json!(uuid::Uuid::new_v4().to_string());
    assert_eq!(
        call(&app, "POST", "/api/study/events", stale).await.0,
        StatusCode::CONFLICT
    );
    let mut altered = request;
    altered["outcome"] = json!("easy");
    assert_eq!(
        call(&app, "POST", "/api/study/events", altered).await.0,
        StatusCode::CONFLICT
    );
    let (_, _, history) = call(
        &app,
        "GET",
        &format!("/api/facts/{}/reviews", fact.id),
        Value::Null,
    )
    .await;
    assert_eq!(history["total"], 1);
    let (_, _, events) = call(&app, "GET", "/api/study/events?limit=1", Value::Null).await;
    assert_eq!(events["items"].as_array().unwrap().len(), 1);
    let event = &events["items"][0];
    assert_eq!(event["qualifying"], true);
    assert_eq!(event["algorithm"], "fsrs-6.6.2");
    let replay: Value = serde_json::from_str(event["result_json"].as_str().unwrap()).unwrap();
    assert_eq!(replay, saved);
    let (_, _, next) = call(
        &app,
        "GET",
        &format!("/api/study/events?after={}", event["id"]),
        Value::Null,
    )
    .await;
    assert!(next["items"].as_array().unwrap().is_empty());
    pool.close().await;
}

#[tokio::test]
async fn only_the_same_current_feedback_can_complete_an_unaided_attempt() {
    let (_directory, pool, app, fact) = library().await;
    let (_, _, queue) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    let mut feedback_request = attempt(&queue["items"][0]);
    feedback_request["kind"] = json!("exposure");
    feedback_request["outcome"] = Value::Null;
    let (status, _, feedback) =
        call(&app, "POST", "/api/study/events", feedback_request.clone()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(feedback["qualifying"], false);
    assert!(feedback["schedule"]["stability"].is_null());
    assert!(feedback["review"].is_null());
    assert!(feedback["schedule"]["blocked_until"].as_i64().is_some());
    assert_eq!(
        call(&app, "POST", "/api/study/events", feedback_request)
            .await
            .2,
        feedback
    );

    let mut rating = attempt(&queue["items"][0]);
    rating["expected_version"] = feedback["schedule"]["version"].clone();
    // Without the link, the server's exposure cooldown still applies.
    assert_eq!(
        call(&app, "POST", "/api/study/events", rating.clone())
            .await
            .0,
        StatusCode::CONFLICT
    );
    rating["feedback_event_id"] = feedback["event_id"].clone();
    let mut changed_attempt = rating.clone();
    changed_attempt["response_ms"] = json!(9999);
    assert_eq!(
        call(&app, "POST", "/api/study/events", changed_attempt)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (status, _, saved) = call(&app, "POST", "/api/study/events", rating.clone()).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(saved["qualifying"], true);
    assert_eq!(saved["review"]["response_ms"], 1500);
    assert!(saved["schedule"]["stability"].as_f64().unwrap() > 0.0);
    assert_eq!(
        call(&app, "POST", "/api/study/events", rating).await.2,
        saved
    );
    let (_, _, history) = call(
        &app,
        "GET",
        &format!("/api/facts/{}/reviews", fact.id),
        Value::Null,
    )
    .await;
    assert_eq!(history["total"], 1);
    let (_, _, events) = call(&app, "GET", "/api/study/events", Value::Null).await;
    assert_eq!(events["items"].as_array().unwrap().len(), 2);
    pool.close().await;
}

#[tokio::test]
async fn unsupported_feedback_links_cannot_hide_study_time_or_change_history() {
    let (_directory, pool, app, fact) = library().await;
    let (_, _, queue) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    let mut forged = attempt(&queue["items"][0]);
    forged["context"] = json!("supplementary");
    forged["feedback_event_id"] = json!(999999);
    assert_eq!(
        call(&app, "POST", "/api/study/events", forged.clone())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, _, untouched) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    assert_eq!(untouched["spent_seconds"], 0);
    assert_eq!(
        untouched["items"][0]["schedule_version"],
        queue["items"][0]["schedule_version"]
    );

    let mut feedback_request = attempt(&queue["items"][0]);
    feedback_request["kind"] = json!("exposure");
    feedback_request["outcome"] = Value::Null;
    let (status, _, feedback) = call(&app, "POST", "/api/study/events", feedback_request).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, _, before) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    assert_eq!(before["spent_seconds"], 1);

    forged["expected_version"] = feedback["schedule"]["version"].clone();
    forged["feedback_event_id"] = feedback["event_id"].clone();
    assert_eq!(
        call(&app, "POST", "/api/study/events", forged.clone())
            .await
            .0,
        StatusCode::CONFLICT
    );
    forged["context"] = json!("scheduled");
    forged["assisted"] = json!(true);
    assert_eq!(
        call(&app, "POST", "/api/study/events", forged).await.0,
        StatusCode::CONFLICT
    );
    let (_, _, after) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    assert_eq!(after["spent_seconds"], before["spent_seconds"]);
    let (_, _, schedule) = call(
        &app,
        "GET",
        &format!("/api/study/facts/{}", fact.id),
        Value::Null,
    )
    .await;
    assert_eq!(schedule, feedback["schedule"]);
    let (_, _, history) = call(
        &app,
        "GET",
        &format!("/api/facts/{}/reviews", fact.id),
        Value::Null,
    )
    .await;
    assert_eq!(history["total"], 0);
    let (_, _, events) = call(&app, "GET", "/api/study/events", Value::Null).await;
    assert_eq!(events["items"].as_array().unwrap().len(), 1);
    pool.close().await;
}

#[tokio::test]
async fn linking_new_feedback_cannot_turn_an_existing_exposure_into_unaided_evidence() {
    let (_directory, pool, app, fact) = library().await;
    let (_, _, queue) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    let (status, _, _) = call(
        &app,
        "POST",
        "/api/study/exposures",
        json!({
            "submission_id": uuid::Uuid::new_v4().to_string(),
            "fact_ids": [fact.id],
            "method": "matching",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, _, exposed) = call(
        &app,
        "GET",
        &format!("/api/study/facts/{}", fact.id),
        Value::Null,
    )
    .await;
    let mut feedback_request = attempt(&queue["items"][0]);
    feedback_request["expected_version"] = exposed["version"].clone();
    feedback_request["kind"] = json!("exposure");
    feedback_request["outcome"] = Value::Null;
    let (status, _, feedback) = call(&app, "POST", "/api/study/events", feedback_request).await;
    if status == StatusCode::CREATED {
        // Saving an exposure is valid; it must never authorize a later recall update.
        let mut rating = attempt(&queue["items"][0]);
        rating["expected_version"] = feedback["schedule"]["version"].clone();
        rating["feedback_event_id"] = feedback["event_id"].clone();
        assert_eq!(
            call(&app, "POST", "/api/study/events", rating).await.0,
            StatusCode::CONFLICT
        );
    } else {
        assert_eq!(status, StatusCode::CONFLICT);
    }
    let (_, _, schedule) = call(
        &app,
        "GET",
        &format!("/api/study/facts/{}", fact.id),
        Value::Null,
    )
    .await;
    assert!(schedule["stability"].is_null());
    assert!(schedule["last_review_at"].is_null());
    let (_, _, history) = call(
        &app,
        "GET",
        &format!("/api/facts/{}/reviews", fact.id),
        Value::Null,
    )
    .await;
    assert_eq!(history["total"], 0);
    pool.close().await;
}

#[tokio::test]
async fn exposure_and_legacy_recognition_preserve_memory_and_stale_the_old_question() {
    let (_directory, pool, app, fact) = library().await;
    let (_, _, queue) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    let path = format!("/api/study/facts/{}", fact.id);
    let (_, _, before) = call(&app, "GET", &path, Value::Null).await;
    let exposure = json!({
        "submission_id": uuid::Uuid::new_v4().to_string(),
        "fact_ids": [fact.id],
        "method": "matching",
    });
    let (status, _, result) = call(&app, "POST", "/api/study/exposures", exposure.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["recorded"], 1);
    assert_eq!(
        call(&app, "POST", "/api/study/exposures", exposure).await.2,
        result
    );
    let (_, _, after) = call(&app, "GET", &path, Value::Null).await;
    for field in [
        "stability",
        "difficulty",
        "due_at",
        "phase",
        "last_review_at",
    ] {
        assert_eq!(before[field], after[field], "exposure changed {field}");
    }
    assert!(after["version"].as_i64().unwrap() > before["version"].as_i64().unwrap());
    assert!(after["blocked_until"].as_i64().is_some());
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/study/events",
            attempt(&queue["items"][0])
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, _, blocked) = call(&app, "GET", "/api/study/queue", Value::Null).await;
    assert!(blocked["items"].as_array().unwrap().is_empty());

    let (status, _, _) = call(
        &app,
        "POST",
        "/api/reviews",
        json!({
            "fact_id": fact.id,
            "practice_id": null,
            "method": "matching",
            "outcome": "easy",
            "response_ms": 300,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, _, recognition) = call(&app, "GET", &path, Value::Null).await;
    for field in [
        "stability",
        "difficulty",
        "due_at",
        "phase",
        "last_review_at",
    ] {
        assert_eq!(
            before[field], recognition[field],
            "recognition changed {field}"
        );
    }
    let (_, _, events) = call(&app, "GET", "/api/study/events", Value::Null).await;
    assert_eq!(events["items"].as_array().unwrap().len(), 2);
    assert!(
        events["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|event| event["qualifying"] == false)
    );
    pool.close().await;
}

#[tokio::test]
async fn settings_and_approvals_validate_requests_without_resetting_schedules() {
    let (_directory, pool, app, fact) = library().await;
    let (_, _, settings) = call(&app, "GET", "/api/study/settings", Value::Null).await;
    assert_eq!(settings["timezone"], "Europe/London");
    assert_eq!(settings["new_limit"], 5);
    assert_eq!(settings["daily_budget_seconds"], 900);
    let update = json!({
        "timezone": "Pacific/Auckland",
        "new_limit": 3,
        "daily_budget_seconds": 600,
        "expected_version": settings["version"],
    });
    let (status, _, changed) = call(&app, "PUT", "/api/study/settings", update.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(changed["timezone"], "Pacific/Auckland");
    assert_eq!(changed["new_limit"], 3);
    assert_eq!(changed["desired_retention"], settings["desired_retention"]);
    assert_eq!(changed["parameters"], settings["parameters"]);
    assert_eq!(
        call(&app, "PUT", "/api/study/settings", update).await.0,
        StatusCode::CONFLICT
    );
    let invalid = json!({
        "timezone": "Not/A_Timezone",
        "new_limit": 3,
        "daily_budget_seconds": 600,
        "expected_version": changed["version"],
    });
    assert_eq!(
        call(&app, "PUT", "/api/study/settings", invalid).await.0,
        StatusCode::BAD_REQUEST
    );

    let path = format!("/api/study/facts/{}", fact.id);
    let (_, _, before) = call(&app, "GET", &path, Value::Null).await;
    for practice in &fact.practices {
        let response = call(
            &app,
            "POST",
            &format!("/api/study/practices/{}/approve", practice.id),
            Value::Null,
        )
        .await;
        assert_eq!(
            response.0,
            if practice.kind == PracticeKind::MultipleChoice {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::NO_CONTENT
            }
        );
    }
    assert_eq!(call(&app, "GET", &path, Value::Null).await.2, before);
    for path in [
        "/api/study/facts/99999",
        "/api/study/practices/99999/approve",
    ] {
        let method = if path.ends_with("approve") {
            "POST"
        } else {
            "GET"
        };
        assert_eq!(
            call(&app, method, path, Value::Null).await.0,
            StatusCode::NOT_FOUND
        );
    }
    for path in [
        "/api/study/queue?limit=0",
        "/api/study/queue?limit=21",
        "/api/study/events?after=-1",
        "/api/study/events?limit=1001",
    ] {
        let (status, _, error) = call(&app, "GET", path, Value::Null).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}");
        assert!(error["error"].is_string());
    }
    pool.close().await;
}

#[tokio::test]
async fn study_writes_keep_retryable_contention_responses() {
    let (_directory, pool, app, _fact) = library().await;
    let (_, _, settings) = call(&app, "GET", "/api/study/settings", Value::Null).await;
    let writer = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let (status, headers, body) = call(
        &app,
        "PUT",
        "/api/study/settings",
        json!({
            "timezone": "Europe/London",
            "new_limit": 0,
            "daily_budget_seconds": 600,
            "expected_version": settings["version"],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers["retry-after"], "1");
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(body["error"], "Library is busy. Try again shortly.");
    writer.rollback().await.unwrap();
    pool.close().await;
}
