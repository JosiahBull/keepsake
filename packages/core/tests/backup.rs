mod support;
use support as db;

use keepsake_core::{
    backup,
    facts::{self, NewFact, NewReview, ReviewMethod, ReviewOutcome},
};

#[tokio::test]
async fn snapshots_committed_wal_and_restores_ids_history_and_schema() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.db");
    let pool = db::connect(&format!("sqlite:{}", source.display()))
        .await
        .unwrap();
    sqlx::query("PRAGMA wal_autocheckpoint = 0")
        .execute(&pool)
        .await
        .unwrap();
    let input = NewFact {
        prompt: "WAL?".into(),
        answer: "Write-ahead logging".into(),
        context: "Committed WAL data".into(),
        source: "https://sqlite.org/wal.html".into(),
        tags: vec!["sqlite".into()],
        practices: vec![],
        media: vec![],
    };
    let saved = facts::create(&pool, input.clone()).await.unwrap();
    facts::record_review(
        &pool,
        NewReview {
            fact_id: saved.id,
            practice_id: Some(saved.practices[0].id),
            method: ReviewMethod::Flashcard,
            outcome: ReviewOutcome::Good,
            response_ms: 123,
        },
    )
    .await
    .unwrap();
    let snapshot = directory.path().join("backup.db");
    let report = backup::create(&format!("sqlite:{}", source.display()), &snapshot)
        .await
        .unwrap();
    assert_eq!((report.facts, report.practices, report.reviews), (1, 1, 1));
    facts::create(&pool, input).await.unwrap();
    let restored = directory.path().join("restored.db");
    backup::restore(&snapshot, &restored).await.unwrap();
    let reopened = db::connect(&format!("sqlite:{}", restored.display()))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(facts::get(&reopened, saved.id).await.unwrap().unwrap()).unwrap(),
        serde_json::to_value(saved).unwrap()
    );
    assert_eq!(facts::list(&reopened).await.unwrap().items.len(), 1);
    assert_eq!(
        facts::reviews(&reopened, 1).await.unwrap().items[0].response_ms,
        123
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&snapshot).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    reopened.close().await;
    pool.close().await;
}

#[tokio::test]
async fn failures_never_replace_targets_or_create_source_databases() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("absent.db");
    let target = directory.path().join("target.db");
    assert!(backup::restore(&source, &target).await.is_err());
    assert!(!source.exists());
    assert!(!target.exists());
    std::fs::write(&source, "corrupt").unwrap();
    assert!(backup::restore(&source, &target).await.is_err());
    assert!(!target.exists());
    std::fs::write(&target, "keep me").unwrap();
    assert!(backup::restore(&source, &target).await.is_err());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
    std::fs::remove_file(&target).unwrap();
    std::fs::write(directory.path().join("target.db-wal"), "keep sidecar").unwrap();
    assert!(backup::restore(&source, &target).await.is_err());
    assert!(!target.exists());
    let empty = directory.path().join("empty.db");
    std::fs::write(&empty, []).unwrap();
    assert!(
        backup::restore(&empty, &directory.path().join("empty-copy.db"))
            .await
            .is_err()
    );
    assert!(!directory.path().join("empty-copy.db").exists());
}
