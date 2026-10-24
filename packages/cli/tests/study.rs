use serde_json::{Value, json};
use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

fn command(directory: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_keepsake"));
    command
        .current_dir(directory)
        .env_remove("DATABASE_URL")
        .env_remove("KEEPSAKE_PROFILE")
        .args([
            "--database",
            &format!("sqlite:{}", directory.join("library.db").display()),
            "--json",
        ]);
    command
}

fn cli(directory: &Path, arguments: &[&str]) -> Output {
    command(directory).args(arguments).output().unwrap()
}

fn output_json(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn add_fact(directory: &Path) -> Value {
    output_json(cli(
        directory,
        &[
            "add",
            "--prompt",
            "Synthetic study question?",
            "--answer",
            "Synthetic study answer",
        ],
    ))
}

#[test]
fn standalone_study_submission_retries_once_and_exports_replayable_events() {
    let directory = tempfile::tempdir().unwrap();
    let fact = add_fact(directory.path());
    let queue = output_json(cli(directory.path(), &["due", "--limit", "1"]));
    assert_eq!(queue["items"].as_array().unwrap().len(), 1);
    let question = &queue["items"][0];
    assert_eq!(question["fact"]["id"], fact["id"]);
    assert_eq!(question["practice"]["kind"], "flashcard");
    let request = json!({
        "submission_id": "67e1b699-7628-4c21-a490-877d636cf693",
        "target_id": question["target_id"],
        "practice_id": question["practice"]["id"],
        "content_revision": question["content_revision"],
        "expected_version": question["schedule_version"],
        "kind": "attempt",
        "context": "scheduled",
        "method": "flashcard",
        "outcome": "good",
        "response_ms": 1200,
        "attempted": true,
        "assisted": false,
        "prior_exposure": false,
    });
    let file = directory.path().join("attempt.json");
    std::fs::write(&file, serde_json::to_vec(&request).unwrap()).unwrap();
    let arguments = ["study", "submit", "--file", file.to_str().unwrap()];
    let saved = output_json(cli(directory.path(), &arguments));
    assert_eq!(saved["qualifying"], true);
    assert_eq!(saved["review"]["outcome"], "good");
    assert_eq!(saved["review"]["response_ms"], 1200);
    assert!(saved["schedule"]["stability"].as_f64().unwrap() > 0.0);
    assert_eq!(output_json(cli(directory.path(), &arguments)), saved);

    let events = output_json(cli(directory.path(), &["study", "events", "--limit", "1"]));
    assert_eq!(events["items"].as_array().unwrap().len(), 1);
    let event = &events["items"][0];
    assert_eq!(event["algorithm"], "fsrs-6.6.2");
    assert_eq!(event["qualifying"], true);
    let replay: Value = serde_json::from_str(event["result_json"].as_str().unwrap()).unwrap();
    assert_eq!(replay, saved);
    let cursor = event["id"].as_i64().unwrap().to_string();
    assert!(
        output_json(cli(
            directory.path(),
            &["study", "events", "--after", &cursor]
        ))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        output_json(cli(directory.path(), &["due"]))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let mut stale = request.clone();
    stale["submission_id"] = json!("ee587201-5568-43ce-a2c7-9435097b9583");
    std::fs::write(&file, serde_json::to_vec(&stale).unwrap()).unwrap();
    assert!(!cli(directory.path(), &arguments).status.success());
    let mut altered = request;
    altered["outcome"] = json!("easy");
    std::fs::write(&file, serde_json::to_vec(&altered).unwrap()).unwrap();
    assert!(!cli(directory.path(), &arguments).status.success());
    assert_eq!(
        output_json(cli(directory.path(), &["study", "events"]))["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn settings_updates_preserve_model_parameters_and_pause_new_intake() {
    let directory = tempfile::tempdir().unwrap();
    add_fact(directory.path());
    let before = output_json(cli(directory.path(), &["study", "settings"]));
    let changed = output_json(cli(
        directory.path(),
        &[
            "study",
            "settings",
            "--timezone",
            "Pacific/Auckland",
            "--new-limit",
            "0",
            "--daily-budget-seconds",
            "300",
        ],
    ));
    assert_eq!(changed["timezone"], "Pacific/Auckland");
    assert_eq!(changed["new_limit"], 0);
    assert_eq!(changed["daily_budget_seconds"], 300);
    assert_eq!(changed["desired_retention"], before["desired_retention"]);
    assert_eq!(changed["parameters"], before["parameters"]);
    assert_eq!(changed["parameter_version"], before["parameter_version"]);
    assert_eq!(
        output_json(cli(directory.path(), &["study", "settings"])),
        changed
    );
    assert!(
        output_json(cli(directory.path(), &["due"]))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !cli(
            directory.path(),
            &["study", "settings", "--timezone", "Not/A_Zone"]
        )
        .status
        .success()
    );
    assert!(
        !cli(
            directory.path(),
            &["study", "settings", "--new-limit", "101"]
        )
        .status
        .success()
    );
    assert!(
        !cli(
            directory.path(),
            &["study", "settings", "--daily-budget-seconds", "59"]
        )
        .status
        .success()
    );
    assert_eq!(
        output_json(cli(directory.path(), &["study", "settings"])),
        changed
    );
}

#[test]
fn approval_accepts_source_checked_recall_but_not_recognition() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("fact.json");
    let request = json!({
        "prompt": "Synthetic approval question?",
        "answer": "Canonical answer",
        "practices": [
            {"kind": "cloze", "prompt": "Complete the canonical ___", "answer": "Canonical answer"},
            {"kind": "multiple_choice", "prompt": "Choose the answer", "answer": "Canonical answer", "choices": ["Canonical answer", "Other answer"]},
        ],
    });
    std::fs::write(&file, serde_json::to_vec(&request).unwrap()).unwrap();
    let saved = output_json(cli(
        directory.path(),
        &["add", "--file", file.to_str().unwrap()],
    ));
    let cloze = saved["practices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|practice| practice["kind"] == "cloze")
        .unwrap();
    let choice = saved["practices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|practice| practice["kind"] == "multiple_choice")
        .unwrap();
    let cloze_id = cloze["id"].as_i64().unwrap().to_string();
    let choice_id = choice["id"].as_i64().unwrap().to_string();
    assert_eq!(
        output_json(cli(directory.path(), &["study", "approve", &cloze_id])),
        json!({
            "practice_id": cloze["id"], "approved": true,
        })
    );
    assert!(
        !cli(directory.path(), &["study", "approve", &choice_id])
            .status
            .success()
    );
    assert!(
        !cli(directory.path(), &["study", "approve", "99999"])
            .status
            .success()
    );
    let queue = output_json(cli(directory.path(), &["due"]));
    assert_eq!(queue["items"].as_array().unwrap().len(), 1);
    assert_eq!(queue["items"][0]["fact"]["id"], saved["id"]);
    assert!(
        output_json(cli(directory.path(), &["study", "events"]))["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn malformed_or_oversized_submission_is_rejected_before_database_creation() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("attempt.json");
    std::fs::write(&file, b"not JSON").unwrap();
    assert!(
        !cli(
            directory.path(),
            &["study", "submit", "--file", file.to_str().unwrap()]
        )
        .status
        .success()
    );
    assert!(!directory.path().join("library.db").exists());
    std::fs::write(&file, vec![b' '; 1024 * 1024 + 1]).unwrap();
    let output = cli(
        directory.path(),
        &["study", "submit", "--file", file.to_str().unwrap()],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exceeds 1 MiB"));
    assert!(!directory.path().join("library.db").exists());

    let mut child = command(directory.path())
        .args(["study", "submit", "--file", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    assert!(!child.wait_with_output().unwrap().status.success());
    assert!(!directory.path().join("library.db").exists());
}
