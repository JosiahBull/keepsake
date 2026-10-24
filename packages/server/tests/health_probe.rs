#![cfg(unix)]

mod support;

use std::{process::Command, time::Duration};
use support::Server;

#[test]
fn executable_probe_reaches_running_server_without_loading_other_configuration() {
    let mut server = Server::start(&[]);
    let unused = server.directory.path().join("probe-must-not-create.db");
    let output = Command::new(env!("CARGO_BIN_EXE_keepsake-server"))
        .arg("--health-check")
        .current_dir(server.directory.path())
        .env("BIND_ADDR", server.address.to_string())
        .env("DATABASE_URL", format!("sqlite:{}", unused.display()))
        .env("WEB_DIR", "this-directory-does-not-exist")
        .env("MAX_CONNECTIONS", "invalid")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(!unused.exists());
    server.signal("-TERM");
    assert!(server.wait(Duration::from_secs(4)).success());
}

#[test]
fn missing_or_empty_production_frontend_fails_before_creating_database() {
    for web in ["this-directory-does-not-exist", ""] {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("must-not-create.db");
        let url = format!("sqlite:{}", database.display());
        let mut server = Server::spawn(&[("WEB_DIR", web), ("DATABASE_URL", &url)]);
        assert!(!server.wait(Duration::from_secs(3)).success());
        assert!(server.output().contains("WEB_DIR"));
        assert!(!database.exists());
    }
}

#[test]
fn unavailable_server_makes_the_executable_probe_fail_without_creating_database() {
    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_keepsake-server"))
        .arg("--health-check")
        .current_dir(directory.path())
        .env("BIND_ADDR", "127.0.0.1:0")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!directory.path().join("data").exists());
}
