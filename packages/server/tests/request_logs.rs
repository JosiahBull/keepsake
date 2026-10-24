#![cfg(unix)]

mod support;

use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};
use support::Server;

#[test]
fn info_logs_correlate_responses_without_query_headers_or_fact_content() {
    let mut server = Server::start(&[]);
    let body = r#"{"prompt":"never_log_fact","answer":"never_log_answer"}"#;
    let mut stream = TcpStream::connect(server.address).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write!(stream,
        "POST /api/facts?private_query=never_log_query HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer never_log_authorization\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 201"));
    let request_id = response
        .lines()
        .find_map(|line| line.strip_prefix("x-request-id: "))
        .unwrap()
        .trim();
    assert!(uuid::Uuid::parse_str(request_id).is_ok());
    server.signal("-TERM");
    assert!(server.wait(Duration::from_secs(4)).success());
    let logs = server.output();
    assert!(logs.contains(request_id));
    assert!(logs.contains("method=POST"));
    assert!(logs.contains("route=/api/facts"));
    assert!(logs.contains("status=201"));
    assert!(logs.contains("duration_ms="));
    assert_eq!(logs.matches("request complete").count(), 1);
    for secret in [
        "never_log_fact",
        "never_log_answer",
        "never_log_query",
        "never_log_authorization",
    ] {
        assert!(!logs.contains(secret), "request content leaked into logs");
    }
}
