mod support;

use std::{path::Path, time::Duration};

/// A directory of default lists with one list, `birds`, of one call; `credit` is its credit.
fn lists(root: &Path, credit: &str) -> String {
    let list = root.join("lists/birds");
    std::fs::create_dir_all(list.join("media")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/tests/fixtures/media/tone.m4a"),
        list.join("media/call.m4a"),
    )
    .unwrap();
    let json = serde_json::json!({
        "id": "birds",
        "title": "Birds",
        "items": [{
            "key": "tone-call",
            "prompt": "Which bird is calling?",
            "answer": "A test tone",
            "tags": ["birds"],
            "media": [{"file": "media/call.m4a", "alt": "A steady tone", "credit": credit}],
        }],
    });
    std::fs::write(list.join("list.json"), json.to_string()).unwrap();
    root.join("lists").display().to_string()
}

#[test]
fn starting_the_server_adds_default_lists_to_both_household_libraries() {
    let directory = tempfile::tempdir().unwrap();
    let lists = lists(directory.path(), "Generated test tone");
    // A fresh database, so the household's profiles exist.
    let database = format!("sqlite:{}", directory.path().join("keepsake.db").display());
    let mut server = support::Server::spawn(&[
        ("DATABASE_URL", database.as_str()),
        ("DEFAULT_LISTS_DIR", lists.as_str()),
    ]);
    server.wait_for_log("default lists checked");
    assert!(server.output().contains("added=2"), "{}", server.output());
    for profile in [1, 2] {
        let page = server.request(
            "GET",
            &format!("/api/profiles/{profile}/facts?search=calling"),
            "",
        );
        assert!(page.contains("\"answer\":\"A test tone\""), "{page}");
        assert!(page.contains("\"ai_generated\":false"), "{page}");
    }
}

#[test]
fn a_malformed_default_list_stops_startup_before_the_database_opens() {
    let directory = tempfile::tempdir().unwrap();
    let lists = lists(directory.path(), " ");
    let database = directory.path().join("keepsake.db");
    let url = format!("sqlite:{}", database.display());
    let mut server = support::Server::spawn(&[
        ("DATABASE_URL", url.as_str()),
        ("DEFAULT_LISTS_DIR", lists.as_str()),
    ]);
    assert!(!server.wait(Duration::from_secs(10)).success());
    assert!(
        server.output().contains("media/call.m4a needs a credit"),
        "{}",
        server.output()
    );
    assert!(!database.exists());
}
