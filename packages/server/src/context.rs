use axum::{
    Json, Router,
    extract::{MatchedPath, Request},
    http::{StatusCode, header},
    middleware::{Next, from_fn},
    response::{IntoResponse, Response},
};
use std::time::Instant;
use tower_http::catch_panic::CatchPanicLayer;
use tracing::Instrument;
use uuid::Uuid;

pub(crate) fn wrap(app: Router) -> Router {
    // Context stays outside panic recovery so recovered failures receive IDs and a summary too.
    app.layer(CatchPanicLayer::new())
        .layer(from_fn(request_context))
}

async fn request_context(request: Request, next: Next) -> Response {
    let request_id = Uuid::new_v4().to_string();
    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unmatched")
        .to_owned();
    let is_api = request.uri().path() == "/api" || request.uri().path().starts_with("/api/");
    let span = tracing::info_span!("http.request", %request_id, %method, %route);

    async move {
        let started = Instant::now();
        let mut response = next.run(request).await;
        if is_api {
            normalize_error(&mut response);
        }
        response.headers_mut().insert(
            "x-request-id",
            request_id.parse().expect("UUID is a header value"),
        );
        tracing::info!(
            status = response.status().as_u16(),
            duration_ms = started.elapsed().as_millis(),
            "request complete"
        );
        response
    }
    .instrument(span)
    .await
}

fn normalize_error(response: &mut Response) {
    let status = response.status();
    let json = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|kind| kind.split(';').next() == Some("application/json"));
    let message = if status.is_server_error() && status != StatusCode::SERVICE_UNAVAILABLE {
        // Internal diagnostics belong in server logs, never in a response body.
        Some("Could not access the library")
    } else if (status.is_client_error() || status.is_server_error()) && !json {
        Some(status.canonical_reason().unwrap_or("Request failed"))
    } else {
        None
    };
    let Some(message) = message else {
        return;
    };
    let replacement = Json(serde_json::json!({"error": message})).into_response();
    *response.body_mut() = replacement.into_body();
    // The old body may have been compressed. Preserve control headers such as Retry-After.
    for name in [
        header::CONTENT_LENGTH,
        header::CONTENT_ENCODING,
        header::ETAG,
        header::TRANSFER_ENCODING,
    ] {
        response.headers_mut().remove(name);
    }
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "application/json".parse().expect("static header"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::DefaultBodyLimit,
        response::Html,
        routing::{get, post},
    };
    use http_body_util::BodyExt;
    use keepsake_core::facts::NewFact;
    use tower::ServiceExt;

    fn app() -> Router {
        wrap(
            Router::new()
                .route(
                    "/api/facts",
                    post(|Json(_fact): Json<NewFact>| async { StatusCode::CREATED }),
                )
                .route(
                    "/api/panic",
                    get(|| async {
                        panic!("private panic detail");
                        #[allow(unreachable_code)]
                        StatusCode::OK
                    }),
                )
                .route(
                    "/api/overload",
                    get(|| async {
                        (
                            StatusCode::SERVICE_UNAVAILABLE,
                            [(header::RETRY_AFTER, "1")],
                            Json(
                                serde_json::json!({"error":"Library is busy. Try again shortly."}),
                            ),
                        )
                    }),
                )
                .route(
                    "/page",
                    get(|| async { (StatusCode::NOT_FOUND, Html("<h1>SPA</h1>")) }),
                )
                .layer(DefaultBodyLimit::max(64))
                .layer(tower_http::compression::CompressionLayer::new()),
        )
    }

    #[tokio::test]
    async fn framework_rejections_are_json_and_have_unique_request_ids() {
        let app = app();
        let mut ids = std::collections::HashSet::new();
        for (method, path, body, status) in [
            ("POST", "/api/facts", "{", StatusCode::BAD_REQUEST),
            ("POST", "/api/facts", "{}", StatusCode::UNPROCESSABLE_ENTITY),
            ("DELETE", "/api/facts", "", StatusCode::METHOD_NOT_ALLOWED),
            ("GET", "/api/missing", "", StatusCode::NOT_FOUND),
            (
                "POST",
                "/api/facts",
                &"x".repeat(100),
                StatusCode::PAYLOAD_TOO_LARGE,
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header(header::CONTENT_TYPE, "application/json")
                        .header(header::ACCEPT_ENCODING, "gzip")
                        .body(Body::from(body.to_owned()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            let id = response.headers()["x-request-id"].to_str().unwrap();
            assert!(Uuid::parse_str(id).is_ok());
            assert!(ids.insert(id.to_owned()));
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let error: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert!(error["error"].is_string());
        }
    }

    #[tokio::test]
    async fn panic_is_scrubbed_overload_headers_survive_and_static_bodies_stay_html() {
        let app = app();
        let panic = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/panic")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(panic.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(panic.headers().contains_key("x-request-id"));
        let body = panic.into_body().collect().await.unwrap().to_bytes();
        assert!(!String::from_utf8_lossy(&body).contains("private panic detail"));
        assert!(serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error"].is_string());
        let overloaded = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/overload")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(overloaded.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(overloaded.headers()[header::RETRY_AFTER], "1");
        let bytes = overloaded.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&bytes).contains("Library is busy"));
        let static_response = app
            .oneshot(Request::builder().uri("/page").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert!(
            static_response.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
        assert_eq!(
            static_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
            "<h1>SPA</h1>"
        );
    }
}
