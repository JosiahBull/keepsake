#[path = "../../core/tests/support/mod.rs"]
mod test_db;
use test_db as db;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use keepsake_server::router;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn call(app: &Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
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
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(response.headers().contains_key("x-request-id"));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)))
    };
    (status, value)
}
async fn expect(
    app: &Router,
    method: &str,
    path: &str,
    body: Value,
    expected: StatusCode,
) -> Value {
    let (status, body) = call(app, method, path, body).await;
    assert_eq!(status, expected, "{method} {path}: {body}");
    body
}
struct Household {
    _directory: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    app: Router,
}
impl Household {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let pool = db::connect(&format!(
            "sqlite:{}",
            directory.path().join("profiles.db").display()
        ))
        .await
        .unwrap();
        let app = router(pool.clone(), directory.path());
        Self {
            _directory: directory,
            pool,
            app,
        }
    }
    async fn partner(&self) -> Value {
        expect(
            &self.app,
            "POST",
            "/api/profiles",
            json!({"name":"Partner", "handle":"partner"}),
            StatusCode::CREATED,
        )
        .await
    }
    async fn fact(&self, owner: i64, suffix: &str) -> Value {
        expect(&self.app, "POST", &format!("/api/profiles/{owner}/facts"), json!({
            "prompt": format!("Synthetic question {suffix}?"), "answer": format!("Synthetic answer {suffix}"),
            "context":"Source detail retained in copies", "source":"Synthetic test source", "tags":["household"],
            "practices":[{"kind":"cloze", "prompt":format!("Complete synthetic {suffix}: ___"), "answer":format!("Synthetic answer {suffix}")}]
        }), StatusCode::CREATED).await
    }
    async fn get(&self, path: &str) -> Value {
        expect(&self.app, "GET", path, Value::Null, StatusCode::OK).await
    }
    async fn offer(&self, source: &Value, uuid: &str) -> Value {
        expect(
            &self.app,
            "POST",
            "/api/profiles/1/shares",
            json!({"submission_id":uuid, "recipient_id":2, "fact_ids":[source["id"]]}),
            StatusCode::CREATED,
        )
        .await
    }
}
fn attempt(question: &Value) -> Value {
    json!({"submission_id":uuid::Uuid::new_v4().to_string(), "target_id":question["target_id"],
        "practice_id":question["practice"]["id"], "content_revision":question["content_revision"],
        "expected_version":question["schedule_version"], "kind":"attempt", "context":"scheduled",
        "method":"flashcard", "outcome":"good", "response_ms":1500, "attempted":true,
        "assisted":false, "prior_exposure":false})
}

#[tokio::test]
async fn profile_management_preserves_identity_and_reports_invalid_input() {
    let h = Household::new().await;
    let initial = h.get("/api/profiles").await;
    assert_eq!(initial[0]["id"], 1);
    assert_eq!(initial[0]["setup_complete"], false);
    let renamed = expect(
        &h.app,
        "PATCH",
        "/api/profiles/1",
        json!({"name":"Josiah"}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(renamed["id"], initial[0]["id"]);
    assert_eq!(renamed["handle"], initial[0]["handle"]);
    assert_eq!(renamed["setup_complete"], true);
    for input in [
        json!({"name":" ","handle":"blank"}),
        json!({"name":"Partner","handle":"BAD HANDLE"}),
        json!({"name":"Partner","handle":"123"}),
        json!({"name":"Partner","handle":""}),
    ] {
        expect(
            &h.app,
            "POST",
            "/api/profiles",
            input,
            StatusCode::BAD_REQUEST,
        )
        .await;
    }
    let partner = h.partner().await;
    assert_eq!(partner["id"], 2);
    expect(
        &h.app,
        "POST",
        "/api/profiles",
        json!({"name":"Another", "handle":"partner"}),
        StatusCode::CONFLICT,
    )
    .await;
    expect(
        &h.app,
        "PATCH",
        "/api/profiles/2",
        json!({"name":" "}),
        StatusCode::BAD_REQUEST,
    )
    .await;
    expect(
        &h.app,
        "GET",
        "/api/profiles/999",
        Value::Null,
        StatusCode::NOT_FOUND,
    )
    .await;
    expect(
        &h.app,
        "GET",
        "/api/profiles/0/facts",
        Value::Null,
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(h.get("/api/profiles/2/facts").await["total"], 0);
    h.pool.close().await;
}

#[tokio::test]
async fn legacy_content_routes_stop_resolving_after_a_second_profile_exists() {
    let h = Household::new().await;
    let fact = h.fact(1, "owner").await;
    assert_eq!(h.get("/api/facts").await["total"], 1);
    h.partner().await;
    for path in [
        "/api/facts",
        "/api/practice",
        "/api/facts/sample",
        "/api/study/queue",
        "/api/study/settings",
        "/api/study/events",
    ] {
        let body = expect(&h.app, "GET", path, Value::Null, StatusCode::BAD_REQUEST).await;
        assert!(body["error"].as_str().unwrap().contains("profile_required"));
    }
    expect(
        &h.app,
        "GET",
        &format!("/api/facts/{}", fact["id"]),
        Value::Null,
        StatusCode::BAD_REQUEST,
    )
    .await;
    expect(
        &h.app,
        "POST",
        "/api/facts",
        json!({"prompt":"Unscoped?", "answer":"Rejected"}),
        StatusCode::BAD_REQUEST,
    )
    .await;
    expect(&h.app, "POST", "/api/study/exposures", json!({"submission_id":uuid::Uuid::new_v4().to_string(), "fact_ids":[fact["id"]], "method":"matching"}), StatusCode::BAD_REQUEST).await;
    assert_eq!(h.get("/api/profiles/1/facts").await["total"], 1);
    assert_eq!(h.get("/api/profiles/2/facts").await["total"], 0);
    h.pool.close().await;
}

#[tokio::test]
async fn every_content_and_study_id_is_checked_against_its_actor() {
    let h = Household::new().await;
    h.partner().await;
    let own = h.fact(1, "first").await;
    let other = h.fact(2, "second").await;
    for path in [
        format!("/api/profiles/2/facts/{}", own["id"]),
        format!("/api/profiles/2/facts/{}/reviews", own["id"]),
        format!("/api/profiles/2/study/facts/{}", own["id"]),
    ] {
        expect(&h.app, "GET", &path, Value::Null, StatusCode::NOT_FOUND).await;
    }
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/2/facts/{}/practices", own["id"]),
        json!([{"kind":"flashcard", "prompt":"Injected?", "answer":"No"}]),
        StatusCode::NOT_FOUND,
    )
    .await;
    expect(
        &h.app,
        "POST",
        &format!(
            "/api/profiles/2/study/practices/{}/approve",
            own["practices"][0]["id"]
        ),
        Value::Null,
        StatusCode::NOT_FOUND,
    )
    .await;
    expect(&h.app, "POST", "/api/profiles/2/reviews", json!({"fact_id":own["id"], "practice_id":own["practices"][0]["id"], "method":"flashcard", "outcome":"good", "response_ms":1}), StatusCode::NOT_FOUND).await;
    let first_queue = h.get("/api/profiles/1/study/queue").await;
    let other_queue = h.get("/api/profiles/2/study/queue").await;
    let request = attempt(&first_queue["items"][0]);
    expect(
        &h.app,
        "POST",
        "/api/profiles/2/study/events",
        request.clone(),
        StatusCode::NOT_FOUND,
    )
    .await;
    let saved = expect(
        &h.app,
        "POST",
        "/api/profiles/1/study/events",
        request.clone(),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(saved["qualifying"], true);
    expect(
        &h.app,
        "POST",
        "/api/profiles/2/study/events",
        request.clone(),
        StatusCode::NOT_FOUND,
    )
    .await;
    let mut stolen_uuid = attempt(&other_queue["items"][0]);
    stolen_uuid["submission_id"] = request["submission_id"].clone();
    expect(
        &h.app,
        "POST",
        "/api/profiles/2/study/events",
        stolen_uuid,
        StatusCode::NOT_FOUND,
    )
    .await;
    let expose_id = uuid::Uuid::new_v4().to_string();
    let expose = json!({"submission_id":expose_id, "fact_ids":[own["id"]], "method":"matching"});
    expect(
        &h.app,
        "POST",
        "/api/profiles/2/study/exposures",
        expose.clone(),
        StatusCode::NOT_FOUND,
    )
    .await;
    expect(
        &h.app,
        "POST",
        "/api/profiles/1/study/exposures",
        expose,
        StatusCode::OK,
    )
    .await;
    expect(
        &h.app,
        "POST",
        "/api/profiles/2/study/exposures",
        json!({"submission_id":expose_id, "fact_ids":[other["id"]], "method":"matching"}),
        StatusCode::NOT_FOUND,
    )
    .await;
    assert!(
        h.get("/api/profiles/2/study/events").await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        h.get(&format!("/api/profiles/2/facts/{}/reviews", other["id"]))
            .await["total"],
        0
    );
    let own_list = h.get("/api/profiles/1/facts").await;
    assert_eq!(own_list["total"], 1);
    assert_eq!(own_list["items"][0]["id"], own["id"]);
    assert_eq!(
        h.get("/api/profiles/2/facts?search=first").await["total"],
        0
    );
    assert_eq!(
        h.get("/api/profiles/2/practice").await["items"][0]["fact"]["id"],
        other["id"]
    );
    h.pool.close().await;
}

#[tokio::test]
async fn personal_settings_and_due_budgets_do_not_change_another_profile() {
    let h = Household::new().await;
    h.partner().await;
    h.fact(1, "first").await;
    h.fact(2, "second").await;
    let settings = h.get("/api/profiles/2/study/settings").await;
    let changed = expect(&h.app, "PUT", "/api/profiles/2/study/settings", json!({"timezone":"Pacific/Auckland", "new_limit":0, "daily_budget_seconds":300, "expected_version":settings["version"]}), StatusCode::OK).await;
    assert_eq!(changed["new_limit"], 0);
    assert_eq!(
        h.get("/api/profiles/1/study/settings").await["new_limit"],
        5
    );
    assert_eq!(
        h.get("/api/profiles/1/study/queue").await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        h.get("/api/profiles/2/study/queue").await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    h.pool.close().await;
}

#[tokio::test]
async fn sharing_offers_hide_answers_and_accept_frozen_copies_with_fresh_memory() {
    let h = Household::new().await;
    h.partner().await;
    let source = h.fact(1, "shared").await;
    let queue = h.get("/api/profiles/1/study/queue").await;
    expect(
        &h.app,
        "POST",
        "/api/profiles/1/study/events",
        attempt(&queue["items"][0]),
        StatusCode::CREATED,
    )
    .await;
    let cloze = source["practices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "cloze")
        .unwrap();
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/1/study/practices/{}/approve", cloze["id"]),
        Value::Null,
        StatusCode::NO_CONTENT,
    )
    .await;
    let uuid = uuid::Uuid::new_v4().to_string();
    let offers = h.offer(&source, &uuid).await;
    assert_eq!(h.offer(&source, &uuid).await, offers);
    // Later source content must not change an already-issued offer.
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/1/facts/{}/practices", source["id"]),
        json!([{"kind":"flashcard","prompt":"Added after offer?","answer":source["answer"]}]),
        StatusCode::OK,
    )
    .await;
    let id = offers[0]["id"].as_i64().unwrap();
    let inbox = h
        .get("/api/profiles/2/shares?direction=inbox&status=pending&limit=1")
        .await;
    assert_eq!(inbox["total"], 1);
    for field in [
        "answer",
        "context",
        "source",
        "snapshot",
        "snapshot_json",
        "practices",
    ] {
        assert!(
            inbox["items"][0].get(field).is_none(),
            "inbox exposed {field}"
        );
    }
    assert_eq!(h.get("/api/profiles/2/facts").await["total"], 0);
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/1/shares/{id}/accept"),
        Value::Null,
        StatusCode::NOT_FOUND,
    )
    .await;
    let accepted = expect(
        &h.app,
        "POST",
        &format!("/api/profiles/2/shares/{id}/accept"),
        Value::Null,
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        expect(
            &h.app,
            "POST",
            &format!("/api/profiles/2/shares/{id}/accept"),
            Value::Null,
            StatusCode::OK
        )
        .await,
        accepted
    );
    let copy_id = accepted["accepted_fact_id"].as_i64().unwrap();
    assert_ne!(copy_id, source["id"].as_i64().unwrap());
    let copy = h.get(&format!("/api/profiles/2/facts/{copy_id}")).await;
    for field in ["prompt", "answer", "context", "source", "tags"] {
        assert_eq!(copy[field], source[field]);
    }
    assert_eq!(copy["shared_by"], offers[0]["sender_name"]);
    assert_eq!(
        copy["practices"].as_array().unwrap().len(),
        source["practices"].as_array().unwrap().len()
    );
    let fresh = h
        .get(&format!("/api/profiles/2/study/facts/{copy_id}"))
        .await;
    assert_eq!(fresh["phase"], "new");
    assert!(fresh["stability"].is_null());
    assert!(fresh["last_review_at"].is_null());
    assert!(fresh["blocked_until"].is_null());
    assert_eq!(
        h.get(&format!("/api/profiles/2/facts/{copy_id}/reviews"))
            .await["total"],
        0
    );
    assert!(
        h.get("/api/profiles/2/study/events").await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let copied_cloze = copy["practices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["kind"] == "cloze")
        .unwrap();
    let approval: i64 =
        sqlx::query_scalar("SELECT qualifying FROM target_practices WHERE practice_id=?")
            .bind(copied_cloze["id"].as_i64().unwrap())
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert_eq!(approval, 0);
    // A later generated variant stays on the source rather than mutating the copy.
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/1/facts/{}/practices", source["id"]),
        json!([{"kind":"flashcard","prompt":"Later variant?","answer":source["answer"]}]),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        h.get(&format!("/api/profiles/2/facts/{copy_id}")).await["practices"],
        copy["practices"]
    );
    h.pool.close().await;
}

#[tokio::test]
async fn share_batch_operations_are_atomic_and_sender_and_recipient_actions_are_scoped() {
    let h = Household::new().await;
    h.partner().await;
    let source = h.fact(1, "first").await;
    let second = h.fact(1, "second").await;
    expect(&h.app, "POST", "/api/profiles/1/shares", json!({"submission_id":uuid::Uuid::new_v4().to_string(), "recipient_id":2, "fact_ids":[source["id"],999999]}), StatusCode::NOT_FOUND).await;
    assert_eq!(h.get("/api/profiles/2/shares").await["total"], 0);
    let first = h.offer(&source, &uuid::Uuid::new_v4().to_string()).await;
    let other = h.offer(&second, &uuid::Uuid::new_v4().to_string()).await;
    let first_id = first[0]["id"].as_i64().unwrap();
    let other_id = other[0]["id"].as_i64().unwrap();
    expect(
        &h.app,
        "POST",
        "/api/profiles/2/shares/accept-batch",
        json!({"share_ids":[first_id,999999]}),
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(h.get("/api/profiles/2/facts").await["total"], 0);
    assert_eq!(
        h.get("/api/profiles/2/shares?status=pending").await["total"],
        2
    );
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/2/shares/{other_id}/withdraw"),
        Value::Null,
        StatusCode::NOT_FOUND,
    )
    .await;
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/1/shares/{other_id}/decline"),
        Value::Null,
        StatusCode::NOT_FOUND,
    )
    .await;
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/1/shares/{other_id}/withdraw"),
        Value::Null,
        StatusCode::OK,
    )
    .await;
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/2/shares/{other_id}/accept"),
        Value::Null,
        StatusCode::CONFLICT,
    )
    .await;
    expect(
        &h.app,
        "POST",
        "/api/profiles/2/shares/accept-batch",
        json!({"share_ids":[first_id,other_id]}),
        StatusCode::CONFLICT,
    )
    .await;
    assert_eq!(h.get("/api/profiles/2/facts").await["total"], 0);
    let accepted = expect(
        &h.app,
        "POST",
        "/api/profiles/2/shares/accept-batch",
        json!({"share_ids":[first_id]}),
        StatusCode::OK,
    )
    .await;
    assert_eq!(accepted[0]["status"], "accepted");
    assert_eq!(h.get("/api/profiles/2/facts").await["total"], 1);
    let third = h.offer(&second, &uuid::Uuid::new_v4().to_string()).await;
    let third_id = third[0]["id"].as_i64().unwrap();
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/2/shares/{third_id}/decline"),
        Value::Null,
        StatusCode::OK,
    )
    .await;
    expect(
        &h.app,
        "POST",
        &format!("/api/profiles/2/shares/{third_id}/accept"),
        Value::Null,
        StatusCode::CONFLICT,
    )
    .await;
    let page = h.get("/api/profiles/1/shares?direction=sent&limit=1").await;
    assert_eq!(page["total"], 3);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert!(page["next_cursor"].as_i64().is_some());
    assert_eq!(
        h.get(&format!(
            "/api/profiles/1/shares?direction=sent&before_id={}",
            page["next_cursor"]
        ))
        .await["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    h.pool.close().await;
}

#[tokio::test]
async fn openapi_scoped_paths_declare_every_path_parameter_and_unique_operations() {
    let h = Household::new().await;
    let schema = h.get("/api/openapi.json").await;
    let paths = schema["paths"].as_object().unwrap();
    let mut operations = std::collections::HashSet::new();
    for (path, item) in paths {
        for verb in ["get", "post", "patch", "put", "delete"] {
            let Some(operation) = item.get(verb) else {
                continue;
            };
            if let Some(id) = operation["operationId"].as_str() {
                assert!(operations.insert(id), "duplicate operationId {id}");
            }
            for parameter in path.split('/').filter_map(|part| {
                part.strip_prefix('{')
                    .and_then(|part| part.strip_suffix('}'))
            }) {
                let definition = item["parameters"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .chain(operation["parameters"].as_array().into_iter().flatten())
                    .find(|p| p["name"] == parameter && p["in"] == "path")
                    .unwrap_or_else(|| panic!("missing {parameter} in {verb} {path}"));
                assert_eq!(
                    definition["required"], true,
                    "optional path parameter {parameter} in {verb} {path}"
                );
            }
        }
    }
    for path in [
        "facts",
        "facts/{id}",
        "facts/{id}/reviews",
        "practice",
        "facts/sample",
        "study/queue",
        "study/settings",
        "study/events",
        "study/exposures",
        "study/practices/{id}/approve",
        "shares",
        "shares/accept-batch",
        "shares/{id}/accept",
    ] {
        assert!(
            paths.contains_key(&format!("/api/profiles/{{profile_id}}/{path}")),
            "missing scopedpath {path}"
        );
    }
    h.pool.close().await;
}

#[tokio::test]
async fn foreign_practice_and_feedback_ids_cannot_be_combined_with_an_owned_target() {
    let h = Household::new().await;
    h.partner().await;
    let own = h.fact(1, "first").await;
    let other = h.fact(2, "second").await;
    let own_queue = h.get("/api/profiles/1/study/queue").await;
    let other_queue = h.get("/api/profiles/2/study/queue").await;
    let mut foreign_practice = attempt(&own_queue["items"][0]);
    foreign_practice["practice_id"] = other_queue["items"][0]["practice"]["id"].clone();
    expect(
        &h.app,
        "POST",
        "/api/profiles/1/study/events",
        foreign_practice,
        StatusCode::NOT_FOUND,
    )
    .await;
    expect(
        &h.app,
        "POST",
        "/api/profiles/1/reviews",
        json!({
            "fact_id":own["id"], "practice_id":other_queue["items"][0]["practice"]["id"],
            "method":"flashcard", "outcome":"good", "response_ms":1500,
        }),
        StatusCode::NOT_FOUND,
    )
    .await;
    let mut feedback_request = attempt(&other_queue["items"][0]);
    feedback_request["kind"] = json!("exposure");
    feedback_request["outcome"] = Value::Null;
    let feedback = expect(
        &h.app,
        "POST",
        "/api/profiles/2/study/events",
        feedback_request,
        StatusCode::CREATED,
    )
    .await;
    let mut foreign_feedback = attempt(&own_queue["items"][0]);
    foreign_feedback["feedback_event_id"] = feedback["event_id"].clone();
    expect(
        &h.app,
        "POST",
        "/api/profiles/1/study/events",
        foreign_feedback,
        StatusCode::NOT_FOUND,
    )
    .await;
    expect(&h.app, "POST", "/api/profiles/1/study/exposures", json!({
        "submission_id":uuid::Uuid::new_v4().to_string(), "fact_ids":[own["id"],other["id"]], "method":"matching",
    }), StatusCode::NOT_FOUND).await;
    assert!(
        h.get("/api/profiles/1/study/events").await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        h.get("/api/profiles/1/study/queue").await["items"][0]["schedule_version"],
        own_queue["items"][0]["schedule_version"]
    );
    let saved = expect(
        &h.app,
        "POST",
        "/api/profiles/1/study/events",
        attempt(&own_queue["items"][0]),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(saved["qualifying"], true);
    h.pool.close().await;
}
