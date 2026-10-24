#![cfg(unix)]

mod support;

use sqlx::{Connection, Executor};
use std::{io::Write, net::TcpStream, time::Duration};
use support::Server;

#[test]
fn sigterm_drains_http_closes_database_and_preserves_committed_facts() {
    let mut server = Server::start(&[]);
    assert!(
        server
            .request(
                "POST",
                "/api/facts",
                r#"{"prompt":"Retained?","answer":"Yes"}"#
            )
            .starts_with("HTTP/1.1 201")
    );
    server.signal("-TERM");
    assert!(server.wait(Duration::from_secs(4)).success());
    assert!(server.output().contains("database pool closed"));
    assert!(server.output().contains("shutdown complete"));

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let pool = keepsake_server::db::connect(&format!(
            "sqlite:{}",
            server.directory.path().join("library.db").display()
        ))
        .await
        .unwrap();
        assert_eq!(
            keepsake_server::facts::list(&pool).await.unwrap()[0].answer,
            "Yes"
        );
        pool.close().await;
    });
}

#[test]
fn stalled_request_cannot_hold_shutdown_past_its_deadline() {
    let mut server = Server::start(&[("SHUTDOWN_GRACE_SECS", "1")]);
    let mut stalled = TcpStream::connect(server.address).unwrap();
    stalled.write_all(b"POST /api/facts HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{").unwrap();
    // The completed health response proves the accept loop is running while the other request stalls.
    assert!(
        server
            .request("GET", "/api/health", "")
            .starts_with("HTTP/1.1 200")
    );
    server.signal("-TERM");
    let status = server.wait(Duration::from_secs(4));
    assert!(!status.success());
    assert!(server.output().contains("shutdown deadline exceeded"));
}

#[test]
fn second_signal_escalates_without_waiting_for_the_grace_period() {
    let mut server = Server::start(&[("SHUTDOWN_GRACE_SECS", "30")]);
    let mut stalled = TcpStream::connect(server.address).unwrap();
    stalled.write_all(b"POST /api/facts HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{").unwrap();
    assert!(
        server
            .request("GET", "/api/health", "")
            .starts_with("HTTP/1.1 200")
    );
    server.signal("-INT");
    server.wait_for_log("shutdown requested");
    server.signal("-TERM");
    assert!(!server.wait(Duration::from_secs(3)).success());
    assert!(server.output().contains("repeated shutdown signal"));
}

#[test]
fn startup_signal_is_caught_while_sqlite_is_locked() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite:{}?mode=rwc",
        directory.path().join("locked.db").display()
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut connection = runtime
        .block_on(sqlx::SqliteConnection::connect(&url))
        .unwrap();
    runtime
        .block_on(connection.execute("BEGIN EXCLUSIVE"))
        .unwrap();
    let mut server = Server::spawn(&[("DATABASE_URL", &url), ("SHUTDOWN_GRACE_SECS", "10")]);
    server.wait_for_log("starting keepsake");
    server.signal("-TERM");
    server.wait_for_log("shutdown requested");
    assert!(!server.output().contains("keepsake is listening"));
    runtime.block_on(connection.execute("ROLLBACK")).unwrap();
    assert!(
        server.wait(Duration::from_secs(5)).success(),
        "{}",
        server.output()
    );
    assert!(!server.output().contains("keepsake is listening"));
}
