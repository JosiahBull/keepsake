mod support;

use support::Server;

#[test]
fn starts_with_missing_database_and_nested_parent_directory() {
    // Server runs in a fresh temporary working directory, with no data directory.
    let server = Server::start(&[("DATABASE_URL", "sqlite:data/nested/keepsake.db")]);
    assert!(
        server
            .directory
            .path()
            .join("data/nested/keepsake.db")
            .is_file()
    );
    assert!(
        server
            .request("GET", "/api/health", "")
            .starts_with("HTTP/1.1 200")
    );
    assert!(
        server
            .request(
                "POST",
                "/api/facts",
                r#"{"prompt":"Wai?","answer":"Water"}"#
            )
            .starts_with("HTTP/1.1 201")
    );
}
