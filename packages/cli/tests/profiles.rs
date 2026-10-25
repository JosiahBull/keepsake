#[path = "../../core/tests/support/mod.rs"]
mod support;

use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Command, Output},
};

fn command(database: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_keepsake"));
    command
        .env_remove("KEEPSAKE_PROFILE")
        .env_remove("DATABASE_URL")
        .args([
            "--database",
            &format!("sqlite:{}", database.display()),
            "--json",
        ]);
    command
}
fn cli(database: &Path, args: &[&str]) -> Output {
    command(database).args(args).output().unwrap()
}
fn value(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn household(database: &Path) {
    if !database.exists() {
        support::prepare(database);
    }
    value(cli(database, &["profiles", "rename", "default", "Josiah"]));
    value(cli(
        database,
        &["profiles", "create", "Partner", "--handle", "partner"],
    ));
}
fn add(database: &Path, actor: &str, question: &str) -> Value {
    value(cli(
        database,
        &[
            "--profile",
            actor,
            "add",
            "--prompt",
            question,
            "--answer",
            "Answer",
        ],
    ))
}

#[test]
fn profiles_scope_capture_search_settings_and_environment_selection() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("profiles.db");
    support::prepare(&db);
    let original = value(cli(
        &db,
        &["add", "--prompt", "Original?", "--answer", "Preserved"],
    ));
    household(&db);
    let profiles = value(cli(&db, &["profiles", "list"]));
    assert_eq!(profiles[0]["name"], "Josiah");
    assert_eq!(profiles[0]["handle"], "default");
    assert_eq!(profiles[1]["handle"], "partner");
    assert!(!cli(&db, &["list"]).status.success());
    let own = value(cli(&db, &["--profile", "1", "list"]));
    assert_eq!(own["total"], 1);
    assert_eq!(own["items"][0]["id"], original["id"]);
    assert_eq!(
        value(cli(&db, &["--profile", "partner", "list"]))["total"],
        0
    );
    let other = value(
        command(&db)
            .env("KEEPSAKE_PROFILE", "partner")
            .args(["add", "--prompt", "Partner only?", "--answer", "Yes"])
            .output()
            .unwrap(),
    );
    let id = other["id"].as_i64().unwrap().to_string();
    assert!(
        !cli(&db, &["--profile", "default", "show", &id])
            .status
            .success()
    );
    assert_eq!(
        value(cli(
            &db,
            &["--profile", "partner", "list", "--search", "Original"]
        ))["total"],
        0
    );
    assert_eq!(
        value(
            command(&db)
                .env("KEEPSAKE_PROFILE", "partner")
                .args(["--profile", "default", "show", &original["id"].to_string(),])
                .output()
                .unwrap()
        )["answer"],
        "Preserved"
    );
    value(cli(
        &db,
        &[
            "--profile",
            "partner",
            "study",
            "settings",
            "--new-limit",
            "0",
            "--timezone",
            "Pacific/Auckland",
        ],
    ));
    assert_eq!(
        value(cli(&db, &["--profile", "default", "study", "settings"]))["new_limit"],
        5
    );
    assert!(
        value(cli(&db, &["--profile", "partner", "due"]))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        value(cli(&db, &["--profile", "default", "due"]))["items"][0]["fact"]["id"],
        original["id"]
    );
}

#[test]
fn sharing_copies_once_keeps_profiles_independent_and_survives_household_backup() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("profiles.db");
    household(&db);
    let source = add(&db, "default", "Source fact?");
    let id = source["id"].as_i64().unwrap().to_string();
    let args = [
        "--profile",
        "default",
        "share",
        &id,
        "--to",
        "partner",
        "--submission-id",
        "f929721e-bde5-43d0-a3cc-19a538540aa1",
    ];
    let offers = value(cli(&db, &args));
    assert_eq!(value(cli(&db, &args)), offers);
    let share_id = offers[0]["id"].as_i64().unwrap().to_string();
    assert_eq!(
        value(cli(&db, &["--profile", "partner", "list"]))["total"],
        0
    );
    let inbox = value(cli(
        &db,
        &["--profile", "partner", "shares", "list", "--limit", "1"],
    ));
    assert_eq!(inbox["total"], 1);
    assert!(inbox["items"][0].get("answer").is_none());
    assert!(
        !cli(
            &db,
            &["--profile", "default", "shares", "accept", &share_id]
        )
        .status
        .success()
    );
    let accepted = value(cli(
        &db,
        &["--profile", "partner", "shares", "accept", &share_id],
    ));
    assert_eq!(
        value(cli(
            &db,
            &["--profile", "partner", "shares", "accept", &share_id]
        )),
        accepted
    );
    let copy_id = accepted[0]["accepted_fact_id"]
        .as_i64()
        .unwrap()
        .to_string();
    assert_ne!(copy_id, id);
    let copy = value(cli(&db, &["--profile", "partner", "show", &copy_id]));
    assert_eq!(copy["prompt"], source["prompt"]);
    assert_eq!(copy["answer"], source["answer"]);
    assert_eq!(
        value(cli(&db, &["--profile", "partner", "list"]))["total"],
        1
    );
    assert!(
        value(cli(&db, &["--profile", "partner", "study", "events"]))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let queue = value(cli(&db, &["--profile", "partner", "due"]));
    assert_eq!(
        queue["items"][0]["fact"]["id"],
        json!(copy_id.parse::<i64>().unwrap())
    );
    let backup = temp.path().join("backup.db");
    value(cli(
        &db,
        &["--profile", "partner", "backup", backup.to_str().unwrap()],
    ));
    assert_eq!(
        value(cli(&backup, &["profiles", "list"]))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        value(cli(
            &backup,
            &["--profile", "default", "shares", "list", "--sent"]
        ))["items"][0]["status"],
        "accepted"
    );
    assert_eq!(
        value(cli(&backup, &["--profile", "partner", "list"]))["total"],
        1
    );
}

#[cfg(unix)]
#[test]
fn ambiguous_capture_and_foreign_generation_stop_before_fetch_or_agent() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("profiles.db");
    household(&db);
    let source = add(&db, "default", "Source fact?");
    let marker = temp.path().join("called");
    let executable = temp.path().join("agent");
    std::fs::write(
        &executable,
        format!("#!/bin/sh\ntouch '{}'\nexit 77\n", marker.display()),
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let output = cli(
        &db,
        &[
            "add",
            "--prompt",
            "Question?",
            "--answer",
            "Answer",
            "--agent",
            "claude",
            "--agent-command",
            executable.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--profile"));
    assert!(!marker.exists());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}/notes", listener.local_addr().unwrap());
    let output = cli(
        &db,
        &[
            "add",
            "--url",
            &url,
            "--prompt",
            "Remember",
            "--agent-command",
            executable.to_str().unwrap(),
            "--agent",
            "claude",
            "--dry-run",
        ],
    );
    assert!(!output.status.success());
    assert!(matches!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    ));
    assert!(!marker.exists());
    let id = source["id"].as_i64().unwrap().to_string();
    assert!(
        !cli(
            &db,
            &[
                "--profile",
                "partner",
                "generate",
                &id,
                "--agent",
                "claude",
                "--agent-command",
                executable.to_str().unwrap()
            ]
        )
        .status
        .success()
    );
    assert!(!marker.exists());
    let file = temp.path().join("reviewed.json");
    std::fs::write(
        &file,
        br#"{"prompt":"Imported?","answer":"Yes","profile_id":1}"#,
    )
    .unwrap();
    let imported = cli(
        &db,
        &[
            "--profile",
            "partner",
            "add",
            "--file",
            file.to_str().unwrap(),
        ],
    );
    // A payload cannot override the selected owner, whether unknown fields are
    // rejected or ignored by the canonical NewFact format.
    if imported.status.success() {
        assert_eq!(
            value(cli(&db, &["--profile", "partner", "list"]))["total"],
            1
        );
    }
    assert_eq!(
        value(cli(&db, &["--profile", "default", "list"]))["total"],
        1
    );
}

#[cfg(unix)]
#[test]
fn capture_keeps_the_resolved_owner_when_a_profile_is_created_during_generation() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("profiles.db");
    support::prepare(&db);
    value(cli(&db, &["profiles", "list"]));
    let executable = temp.path().join("agent");
    std::fs::write(&executable, format!(
        "#!/bin/sh\ncat >/dev/null\n'{}' --database 'sqlite:{}' profiles create Late --handle late >/dev/null || exit 77\nprintf '%s' '{{\"practices\":[{{\"kind\":\"flashcard\",\"prompt\":\"Generated question?\",\"answer\":\"Canonical answer\"}}]}}'\n",
        env!("CARGO_BIN_EXE_keepsake"), db.display(),
    )).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let saved = value(cli(
        &db,
        &[
            "add",
            "--prompt",
            "Canonical question?",
            "--answer",
            "Canonical answer",
            "--agent",
            "claude",
            "--agent-command",
            executable.to_str().unwrap(),
        ],
    ));
    assert_eq!(
        value(cli(
            &db,
            &["--profile", "default", "show", &saved["id"].to_string()]
        ))["answer"],
        "Canonical answer"
    );
    assert_eq!(value(cli(&db, &["--profile", "late", "list"]))["total"], 0);
    assert!(!cli(&db, &["list"]).status.success());
}

#[test]
fn fresh_household_requires_an_owner_and_seeds_separate_libraries_with_pending_offers() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("fresh.db");
    let rejected = cli(
        &db,
        &[
            "add",
            "--prompt",
            "Preview?",
            "--answer",
            "Yes",
            "--dry-run",
        ],
    );
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("--profile"));
    assert!(!db.exists());
    let profiles = value(cli(&db, &["profiles", "list"]));
    assert_eq!(profiles.as_array().unwrap().len(), 2);
    assert_eq!(profiles[0]["name"], "Josiah");
    assert_eq!(profiles[0]["handle"], "default");
    assert_eq!(profiles[1]["name"], "Ansam");
    assert_eq!(profiles[1]["handle"], "ansam");
    assert_eq!(
        value(cli(&db, &["--profile", "default", "list"]))["total"],
        40
    );
    assert_eq!(
        value(cli(&db, &["--profile", "ansam", "list"]))["total"],
        60
    );
    let inbox = value(cli(
        &db,
        &[
            "--profile",
            "default",
            "shares",
            "list",
            "--status",
            "pending",
        ],
    ));
    assert_eq!(inbox["total"], 60);
    assert!(
        inbox["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|offer| offer["sender_id"] == 2
                && offer["recipient_id"] == 1
                && offer.get("answer").is_none())
    );
}

#[test]
fn first_capture_can_explicitly_choose_ansam_before_database_creation() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("fresh.db");
    let saved = add(&db, "ansam", "Fresh Ansam capture?");
    assert_eq!(saved["prompt"], "Fresh Ansam capture?");
    assert_eq!(
        value(cli(&db, &["--profile", "default", "list"]))["total"],
        40
    );
    assert_eq!(
        value(cli(&db, &["--profile", "ansam", "list"]))["total"],
        61
    );
    assert!(
        !cli(
            &db,
            &["--profile", "default", "show", &saved["id"].to_string()]
        )
        .status
        .success()
    );
}
