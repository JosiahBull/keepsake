mod support;
use support as db;

use keepsake_core::facts;

#[tokio::test]
async fn creates_missing_database_and_nested_directories_then_reopens_existing_data() {
    let directory = tempfile::tempdir().unwrap();
    let filename = directory.path().join("new/nested/library.db");
    assert!(!filename.exists());
    let url = format!("sqlite:{}?mode=rw", filename.display());

    let pool = db::connect(&url).await.unwrap();
    assert!(filename.is_file());
    let saved = facts::create(
        &pool,
        facts::NewFact {
            prompt: "What does wai mean?".into(),
            answer: "Water".into(),
            context: String::new(),
            source: String::new(),
            tags: vec![],
            practices: vec![],
            media: vec![],
        },
    )
    .await
    .unwrap();
    pool.close().await;

    let pool = db::connect(&url).await.unwrap();
    assert_eq!(
        facts::get(&pool, saved.id).await.unwrap().unwrap().answer,
        "Water"
    );
    pool.close().await;
}

#[tokio::test]
async fn reports_database_path_when_sqlite_cannot_open_it() {
    let directory = tempfile::tempdir().unwrap();
    let filename = directory.path().join("directory-not-a-file.db");
    std::fs::create_dir(&filename).unwrap();
    let url = format!("sqlite:{}", filename.display());

    let error = db::connect(&url).await.unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains(&filename.display().to_string()));
    assert!(message.contains("must be writable"));
    assert!(message.contains("unable to open database file"));
}

#[tokio::test]
async fn reports_parent_path_when_directory_creation_fails() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("file-not-a-directory");
    std::fs::write(&parent, b"preserve me").unwrap();
    let url = format!("sqlite:{}", parent.join("library.db").display());

    let error = db::connect(&url).await.unwrap_err();
    assert!(error.to_string().contains(&parent.display().to_string()));
    assert!(error.to_string().contains("create SQLite directory"));
    assert_eq!(std::fs::read(&parent).unwrap(), b"preserve me");
}
