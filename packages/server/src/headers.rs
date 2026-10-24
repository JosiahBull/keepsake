use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, header},
    middleware::Next,
    response::Response,
};

const SECURITY_HEADERS: &[(&str, &str)] = &[
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "no-referrer"),
    ("x-frame-options", "DENY"),
    ("cross-origin-opener-policy", "same-origin"),
];

pub(crate) async fn response_headers(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let api = path == "/api" || path.starts_with("/api/");
    let fingerprinted_asset = path.starts_with("/assets/");
    let mut response = next.run(request).await;

    let cache = if api || response.status().is_client_error() || response.status().is_server_error()
    {
        "no-store"
    } else if fingerprinted_asset {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=0, must-revalidate"
    };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    // Keep personal library responses out of shared caches, including proxy-specific policy.
    headers.insert("cdn-cache-control", HeaderValue::from_static(cache));
    headers.insert(
        "cloudflare-cdn-cache-control",
        HeaderValue::from_static(cache),
    );
    for (name, value) in SECURITY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}
