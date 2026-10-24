use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_body_bytes: usize,
    pub max_in_flight: usize,
    /// Starts after headers and includes receiving and parsing the request body.
    pub request_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_body_bytes: 512 * 1024,
            max_in_flight: 32,
            request_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Admission {
    slots: Arc<Semaphore>,
    timeout: Duration,
}

impl Admission {
    pub fn new(limits: Limits) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(limits.max_in_flight)),
            timeout: limits.request_timeout,
        }
    }
}

pub(crate) async fn admit(
    State(admission): State<Admission>,
    request: Request,
    next: Next,
) -> Response {
    let Ok(_permit) = admission.slots.try_acquire() else {
        let mut response = (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "The server is busy. Try again shortly."})),
        )
            .into_response();
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, "1".parse().expect("static header"));
        return response;
    };
    match tokio::time::timeout(admission.timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => (
            StatusCode::REQUEST_TIMEOUT,
            Json(serde_json::json!({"error": "Request deadline exceeded"})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, middleware::from_fn_with_state, routing::get};
    use tower::ServiceExt;

    #[tokio::test]
    async fn shedding_and_deadlines_release_slots_for_the_next_request() {
        let entered = Arc::new(tokio::sync::Notify::new());
        let handler_entered = entered.clone();
        let app = Router::new()
            .route(
                "/slow",
                get(move || {
                    let entered = handler_entered.clone();
                    async move {
                        entered.notify_one();
                        std::future::pending::<StatusCode>().await
                    }
                }),
            )
            .route("/fast", get(|| async { StatusCode::OK }))
            .layer(from_fn_with_state(
                Admission::new(Limits {
                    max_in_flight: 1,
                    request_timeout: Duration::from_millis(100),
                    ..Limits::default()
                }),
                admit,
            ));
        let slow = tokio::spawn(
            app.clone()
                .oneshot(Request::builder().uri("/slow").body(Body::empty()).unwrap()),
        );
        entered.notified().await;
        let overloaded = app
            .clone()
            .oneshot(Request::builder().uri("/fast").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(overloaded.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(overloaded.headers()[header::RETRY_AFTER], "1");
        assert_eq!(
            slow.await.unwrap().unwrap().status(),
            StatusCode::REQUEST_TIMEOUT
        );
        let recovered = app
            .oneshot(Request::builder().uri("/fast").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(recovered.status(), StatusCode::OK);
    }
}
