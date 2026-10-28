mod support;

use std::time::Duration;

#[test]
fn an_unrecognised_ceiling_stops_the_server() {
    let mut server = support::Server::spawn(&[("KEEPSAKE_MCP", "wrtie")]);
    assert!(!server.wait(Duration::from_secs(10)).success());
    assert!(
        server
            .output()
            .contains("unknown agent access mode 'wrtie'"),
        "{}",
        server.output()
    );
}
