#![cfg(unix)]

mod support;

use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};
use support::Server;

fn stopped(mut server: Server) {
    server.signal("-TERM");
    assert!(
        server.wait(Duration::from_secs(4)).success(),
        "{}",
        server.output()
    );
}

#[test]
fn partial_headers_and_idle_connections_have_independent_deadlines() {
    for settings in [
        [
            ("HEADER_READ_TIMEOUT_SECS", "1"),
            ("CONNECTION_TIMEOUT_SECS", "10"),
        ],
        [
            ("HEADER_READ_TIMEOUT_SECS", "10"),
            ("CONNECTION_TIMEOUT_SECS", "1"),
        ],
    ] {
        let server = Server::start(&settings);
        let mut stalled = TcpStream::connect(server.address).unwrap();
        stalled
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stalled.write_all(b"G").unwrap();
        let mut received = Vec::new();
        let result = stalled.read_to_end(&mut received);
        assert!(
            result.is_ok()
                || result.is_err_and(|error| error.kind() == std::io::ErrorKind::ConnectionReset)
        );
        assert!(received.is_empty() || received.starts_with(b"HTTP/1.1 408"));
        assert!(
            server
                .request("GET", "/api/health", "")
                .starts_with("HTTP/1.1 200")
        );
        stopped(server);
    }
}

#[test]
fn slow_body_deadline_releases_the_request_slot() {
    let server = Server::start(&[("REQUEST_TIMEOUT_SECS", "1"), ("MAX_IN_FLIGHT", "1")]);
    let mut stalled = TcpStream::connect(server.address).unwrap();
    stalled
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stalled.write_all(b"POST /api/facts HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{").unwrap();
    let mut response = [0; 4096];
    let bytes = stalled.read(&mut response).unwrap();
    assert!(
        std::str::from_utf8(&response[..bytes])
            .unwrap()
            .starts_with("HTTP/1.1 408")
    );
    drop(stalled);
    assert!(
        server
            .request("GET", "/api/health", "")
            .starts_with("HTTP/1.1 200")
    );
    stopped(server);
}

#[test]
fn connection_ceiling_queues_acceptance_and_recovers_when_a_peer_leaves() {
    let server = Server::start(&[("MAX_CONNECTIONS", "1")]);
    let mut first = TcpStream::connect(server.address).unwrap();
    first.write_all(b"G").unwrap();
    let mut second = TcpStream::connect(server.address).unwrap();
    second
        .set_read_timeout(Some(Duration::from_millis(150)))
        .unwrap();
    second
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut byte = [0];
    let error = second.read(&mut byte).unwrap_err();
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    drop(first);
    second
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut response = String::new();
    second.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    stopped(server);
}

#[test]
fn configured_body_size_is_enforced_and_invalid_limits_fail_startup() {
    let server = Server::start(&[("MAX_BODY_BYTES", "16")]);
    assert!(
        server
            .request(
                "POST",
                "/api/facts",
                r#"{"prompt":"Long prompt","answer":"Answer"}"#
            )
            .starts_with("HTTP/1.1 413")
    );
    stopped(server);
    let mut invalid = Server::spawn(&[("MAX_CONNECTIONS", "0")]);
    assert!(!invalid.wait(Duration::from_secs(3)).success());
    assert!(invalid.output().contains("MAX_CONNECTIONS"));
}
