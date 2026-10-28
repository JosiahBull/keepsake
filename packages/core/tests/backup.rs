mod support;
use support as db;
#[path = "support/media_files.rs"]
mod files;

use keepsake_core::{
    backup,
    facts::{self, NewFact, NewReview, ReviewMethod, ReviewOutcome},
    media::{self, NewQuestionMedia},
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

#[tokio::test]
async fn backups_carry_media_files_and_restore_them_into_a_directory_of_their_own() {
    let directory = tempfile::tempdir().unwrap();
    let live = directory.path().join("live");
    std::fs::create_dir(&live).unwrap();
    let source = live.join("keepsake.db");
    let url = format!("sqlite:{}", source.display());
    let pool = db::connect(&url).await.unwrap();
    let image = media::upload(&pool, 1, "image/png", files::png(1))
        .await
        .unwrap();
    let clip = media::upload(&pool, 1, "audio/wav", files::wav(200, 1))
        .await
        .unwrap();
    let attach = |media_id| NewQuestionMedia {
        media_id,
        alt: String::new(),
    };
    let saved = facts::create(
        &pool,
        NewFact {
            prompt: "Which bird?".into(),
            answer: "Tūī".into(),
            context: String::new(),
            source: String::new(),
            tags: vec![],
            practices: vec![],
            media: vec![attach(image.media_id), attach(clip.media_id)],
        },
    )
    .await
    .unwrap();
    let served = |pool: sqlx::SqlitePool, id| async move {
        std::fs::read(media::locate(&pool, 1, id).await.unwrap().unwrap().path).unwrap()
    };

    let backups = directory.path().join("backups");
    std::fs::create_dir(&backups).unwrap();
    let snapshot = backups.join("keepsake.db");
    let report = backup::create(&url, &snapshot).await.unwrap();
    assert_eq!(report.media, 2);
    let media_dir = backup::media_dir(&snapshot);
    assert_eq!(media_dir, backups.join("keepsake.db-media"));
    assert!(media_dir.join("images").is_dir() && media_dir.join("audio").is_dir());

    // The live directory already has media/: restoring beside it would share it.
    let error = backup::restore(&snapshot, &live.join("restored.db"))
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("without a media folder"),
        "{error:#}"
    );

    let restored_dir = directory.path().join("restored");
    std::fs::create_dir(&restored_dir).unwrap();
    let restored = restored_dir.join("keepsake.db");
    assert_eq!(
        backup::restore(&snapshot, &restored).await.unwrap().media,
        2
    );
    let reopened = db::connect(&format!("sqlite:{}", restored.display()))
        .await
        .unwrap();
    assert_eq!(
        facts::get(&reopened, saved.id)
            .await
            .unwrap()
            .unwrap()
            .media
            .len(),
        2
    );
    for id in [image.media_id, clip.media_id] {
        assert_eq!(
            served(reopened.clone(), id).await,
            served(pool.clone(), id).await
        );
    }
    reopened.close().await;

    // A file changed in the backup is refused, and nothing is left behind.
    let damaged = std::fs::read_dir(media_dir.join("images"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(&damaged, b"not the image").unwrap();
    let elsewhere = directory.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    let error = backup::restore(&snapshot, &elsewhere.join("keepsake.db"))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("is damaged"), "{error:#}");
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 0);

    // So is a backup of a library whose file has gone missing.
    std::fs::remove_file(
        media::locate(&pool, 1, clip.media_id)
            .await
            .unwrap()
            .unwrap()
            .path,
    )
    .unwrap();
    let incomplete = backups.join("incomplete.db");
    assert!(backup::create(&url, &incomplete).await.is_err());
    assert!(!incomplete.exists() && !backup::media_dir(&incomplete).exists());
    pool.close().await;
}
