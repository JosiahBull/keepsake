//! The MCP endpoint driven as raw JSON-RPC over HTTP, through the full router.
#[path = "../../core/tests/support/mod.rs"]
mod test_db;
use test_db as db;

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use keepsake_core::{
    facts::{self, NewFact},
    profiles::{self, NewProfile},
};
use keepsake_mcp::{McpConfig, McpMode};
use keepsake_server::{limits::Limits, notifications::Notifier, router_with_mcp};
use serde_json::{Value, json};
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:8080";

/// A 1×1 PNG; uploads must decode, so this one is real.
const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0,
    0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

struct App {
    _directory: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    router: Router,
}

async fn app(ceiling: McpMode, allowed_hosts: &[&str]) -> App {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("mcp.db").display()
    ))
    .await
    .unwrap();
    let notifier = Notifier::new(pool.clone(), Default::default()).unwrap();
    let router = router_with_mcp(
        pool.clone(),
        directory.path(),
        Limits::default(),
        Arc::new(notifier),
        McpConfig {
            ceiling,
            allowed_hosts: allowed_hosts.iter().map(|host| host.to_string()).collect(),
        },
    );
    App {
        _directory: directory,
        pool,
        router,
    }
}

fn rpc_request(profile: &str, method: &str, params: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/mcp/{profile}"))
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18")
        .body(Body::from(
            json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string(),
        ))
        .unwrap()
}

async fn send(app: &App, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = app.router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, body.to_vec())
}

async fn rpc(app: &App, profile: &str, method: &str, params: Value) -> Value {
    let (status, body) = send(app, rpc_request(profile, method, params)).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    serde_json::from_slice(&body).unwrap()
}

async fn tool_names(app: &App, profile: &str) -> Vec<String> {
    let listed = rpc(app, profile, "tools/list", json!({})).await;
    let mut names: Vec<String> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

async fn call(app: &App, profile: &str, tool: &str, arguments: Value) -> Value {
    rpc(
        app,
        profile,
        "tools/call",
        json!({"name": tool, "arguments": arguments}),
    )
    .await
}

fn text(result: &Value) -> &str {
    result["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
}

async fn set_access(app: &App, profile_id: i64, mode: &str) -> (StatusCode, Value) {
    let (status, body) = send(
        app,
        Request::builder()
            .method("PUT")
            .uri(format!("/api/profiles/{profile_id}/agent-access"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"mode": mode}).to_string()))
            .unwrap(),
    )
    .await;
    (status, serde_json::from_slice(&body).unwrap())
}

fn fact(prompt: &str) -> NewFact {
    NewFact {
        prompt: prompt.into(),
        answer: "Water".into(),
        context: String::new(),
        source: String::new(),
        tags: vec!["te-reo".into()],
        practices: vec![],
        media: vec![],
    }
}

const READ_TOOLS: [&str; 5] = [
    "get_fact",
    "list_tags",
    "search_facts",
    "study_overview",
    "view_media",
];

#[tokio::test]
async fn without_a_ceiling_there_is_no_endpoint_and_no_choice() {
    let app = app(McpMode::Off, &[]).await;
    let (status, body) = send(&app, rpc_request("default", "tools/list", json!({}))).await;
    // The SPA fallback answers instead; nothing speaks JSON-RPC.
    assert_ne!(status, StatusCode::OK);
    assert!(!String::from_utf8_lossy(&body).contains("jsonrpc"));
    let (status, body) = set_access(&app, 1, "read").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("KEEPSAKE_MCP allows at most 'off'")
    );
}

#[tokio::test]
async fn each_profile_chooses_its_access_and_changes_apply_on_the_next_call() {
    let app = app(McpMode::Write, &[]).await;
    let guest = profiles::create(
        &app.pool,
        NewProfile {
            name: "Guest".into(),
            handle: "guest".into(),
        },
    )
    .await
    .unwrap();
    facts::create_for(&app.pool, 1, fact("What does wai mean?"))
        .await
        .unwrap();

    let info = rpc(
        &app,
        "default",
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1"},
        }),
    )
    .await;
    assert_eq!(info["result"]["serverInfo"]["name"], "keepsake");
    assert!(
        info["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("Existing profile's library only")
    );

    // Off by default: the endpoint exists but serves nothing.
    assert!(tool_names(&app, "default").await.is_empty());
    let refused = call(&app, "default", "search_facts", json!({})).await;
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("tool not found")
    );

    let (status, access) = set_access(&app, 1, "read").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        access,
        json!({"mode": "read", "ceiling": "write", "effective": "read"})
    );
    assert_eq!(tool_names(&app, "default").await, READ_TOOLS);
    let found = call(&app, "default", "search_facts", json!({"search": "WAI"})).await;
    assert!(
        text(&found).contains("| What does wai mean? | Water | te-reo |"),
        "{found}"
    );
    let refused = call(&app, "default", "add_facts", json!({"facts": []})).await;
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("tool not found")
    );
    // Another profile's choice is its own.
    assert!(tool_names(&app, "guest").await.is_empty());
    assert!(tool_names(&app, &guest.id.to_string()).await.is_empty());

    set_access(&app, 1, "write").await;
    let mut all = READ_TOOLS.map(String::from).to_vec();
    all.extend(["add_facts".into(), "add_practices".into()]);
    all.sort();
    assert_eq!(tool_names(&app, "default").await, all);

    set_access(&app, 1, "off").await;
    assert!(tool_names(&app, "default").await.is_empty());
}

#[tokio::test]
async fn the_ceiling_caps_every_profile_and_refuses_more() {
    let app = app(McpMode::Read, &[]).await;
    let (status, body) = set_access(&app, 1, "write").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"].as_str().unwrap().contains("at most 'read'"));
    let (status, access) = set_access(&app, 1, "read").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(access["effective"], "read");
    assert_eq!(tool_names(&app, "default").await, READ_TOOLS);
}

#[tokio::test]
async fn writing_saves_checked_facts_with_media_and_refuses_bad_drafts_whole() {
    let app = app(McpMode::Write, &[]).await;
    set_access(&app, 1, "write").await;
    let png = {
        use base64ct::{Base64, Encoding};
        Base64::encode_string(PNG)
    };

    let refused = call(
        &app,
        "default",
        "add_facts",
        json!({"facts": [
            {"prompt": "What does wai mean?", "answer": "Water"},
            {"prompt": "Which bird is this?", "answer": "Tūī",
             "media": [{"data_base64": png, "content_type": "image/jpeg"}]},
        ]}),
    )
    .await;
    assert_eq!(refused["result"]["isError"], true);
    assert_eq!(
        text(&refused),
        "Fact 2, media 1: The file is not a JPEG image. Its contents do not match its type."
    );
    assert_eq!(facts::list_for(&app.pool, 1).await.unwrap().total, 0);

    let saved = call(
        &app,
        "default",
        "add_facts",
        json!({"facts": [{
            "prompt": "Which bird is this?",
            "answer": "Tūī",
            "source": "Field guide",
            "tags": ["birds"],
            "media": [{"data_base64": png, "content_type": "image/png", "description": "A dark bird"}],
            "practices": [{"kind": "multiple_choice", "prompt": "Which bird?", "answer": "Tūī",
                           "choices": ["Tūī", "Kākā"]}],
        }]}),
    )
    .await;
    assert!(text(&saved).starts_with("Saved 1 fact(s)."), "{saved}");
    let stored = facts::list_for(&app.pool, 1).await.unwrap().items.remove(0);
    assert_eq!(stored.media[0].alt, "A dark bird");

    let shown = call(&app, "default", "get_fact", json!({"fact_id": stored.id})).await;
    assert!(
        text(&shown).contains("| image | image/png | 1×1 px | A dark bird |"),
        "{shown}"
    );
    let image = call(
        &app,
        "default",
        "view_media",
        json!({"media_id": stored.media[0].media_id}),
    )
    .await;
    assert_eq!(image["result"]["content"][0]["type"], "image");
    assert_eq!(image["result"]["content"][0]["mimeType"], "image/png");
    assert_eq!(image["result"]["content"][0]["data"], png);

    let added = call(
        &app,
        "default",
        "add_practices",
        json!({"fact_id": stored.id, "practices": [
            {"kind": "cloze", "prompt": "The ___ is a dark bird.", "answer": "Tūī"}
        ]}),
    )
    .await;
    assert!(
        text(&added).starts_with("Added 1 practice variant(s)"),
        "{added}"
    );
    let missing = call(&app, "default", "get_fact", json!({"fact_id": 999})).await;
    assert_eq!(text(&missing), "Fact 999 is not in this library.");
}

#[tokio::test]
async fn browsers_and_unlisted_hosts_are_refused() {
    let app = app(McpMode::Read, &["keepsake.example.com"]).await;
    set_access(&app, 1, "read").await;

    let mut from_page = rpc_request("default", "tools/list", json!({}));
    from_page
        .headers_mut()
        .insert(header::ORIGIN, "https://attacker.example".parse().unwrap());
    let (status, _) = send(&app, from_page).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    for (host, allowed) in [
        ("rebound.attacker.example", false),
        ("keepsake.example.com", true),
        ("localhost:8080", true),
    ] {
        let mut request = rpc_request("default", "tools/list", json!({}));
        request
            .headers_mut()
            .insert(header::HOST, host.parse().unwrap());
        let (status, _) = send(&app, request).await;
        assert_eq!(status == StatusCode::OK, allowed, "{host}: {status}");
    }

    let (status, body) = send(&app, rpc_request("nobody", "tools/list", json!({}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["error"],
        "Profile not found"
    );
}
