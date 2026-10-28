mod support;
use support as db;

use keepsake_core::facts::{self, NewFact};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

fn fact() -> NewFact {
    NewFact {
        prompt: "A question".into(),
        answer: "An answer".into(),
        context: String::new(),
        source: String::new(),
        tags: vec![],
        practices: vec![],
        media: vec![],
    }
}

async fn pool() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite:{}",
        directory.path().join("contention.db").display()
    );
    let pool = db::connect(&url).await.unwrap();
    (directory, pool)
}

#[tokio::test]
async fn retries_after_a_temporary_writer_and_saves_each_fact_once() {
    let (_directory, pool) = pool().await;
    let writer = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let write_pool = pool.clone();
    let write =
        tokio::spawn(async move { facts::create_many(&write_pool, vec![fact(), fact()]).await });
    tokio::time::sleep(Duration::from_millis(350)).await;
    writer.rollback().await.unwrap();
    let saved = write.await.unwrap().unwrap();
    assert_eq!(saved.len(), 2);
    assert_eq!(facts::list(&pool).await.unwrap().items.len(), 2);
    let practices: i64 = sqlx::query_scalar("SELECT count(*) FROM practices")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(practices, 2);
    pool.close().await;
}

#[tokio::test]
async fn sustained_contention_stops_and_is_typed_as_overload() {
    let (_directory, pool) = pool().await;
    let writer = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let started = Instant::now();
    let error = facts::create(&pool, fact()).await.unwrap_err();
    assert!(db::is_overloaded(&error));
    assert!(matches!(
        error.downcast_ref::<db::WriteError>(),
        Some(db::WriteError::Overloaded(_))
    ));
    assert!(started.elapsed() < Duration::from_secs(3));
    writer.rollback().await.unwrap();
    assert!(facts::list(&pool).await.unwrap().items.is_empty());
    pool.close().await;
}

#[tokio::test]
async fn retries_the_whole_body_only_after_confirmed_rollback() {
    let (_directory, pool) = pool().await;
    // Obtain a real SQLite BUSY error, then inject it after a statement succeeded.
    let writer = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    let mut blocked = pool.acquire().await.unwrap();
    sqlx::query("PRAGMA busy_timeout = 0")
        .execute(&mut *blocked)
        .await
        .unwrap();
    let busy = sqlx::query(
        "INSERT INTO facts (profile_id, prompt, answer) VALUES (1, 'blocked', 'blocked')",
    )
    .execute(&mut *blocked)
    .await
    .unwrap_err();
    writer.rollback().await.unwrap();
    drop(blocked);
    let busy = Arc::new(Mutex::new(Some(busy)));
    let attempts = Arc::new(AtomicUsize::new(0));
    db::write_transaction(&pool, |connection| {
        let busy = busy.clone();
        let attempts = attempts.clone();
        Box::pin(async move {
            attempts.fetch_add(1, Ordering::SeqCst);
            sqlx::query(
                "INSERT INTO facts (profile_id, prompt, answer) VALUES (1, 'once', 'once')",
            )
            .execute(&mut *connection)
            .await?;
            let error = busy.lock().unwrap().take();
            if let Some(error) = error {
                return Err(error.into());
            }
            Ok(())
        })
    })
    .await
    .unwrap();
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
    assert_eq!(facts::list(&pool).await.unwrap().items.len(), 1);
    pool.close().await;
}

#[tokio::test]
async fn permanent_errors_do_not_repeat_the_operation() {
    let (_directory, pool) = pool().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let error = db::write_transaction(&pool, |connection| {
        let attempts = attempts.clone();
        Box::pin(async move {
            attempts.fetch_add(1, Ordering::SeqCst);
            sqlx::query("INSERT INTO facts (prompt, answer) VALUES ('', '')")
                .execute(connection)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap_err();
    assert!(!db::is_overloaded(&error));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    pool.close().await;
}
