//! Household defaults preserve copies already accepted through explicit sharing.
use keepsake_core::{
    db,
    facts::{self, NewFact, ReviewMethod, ReviewOutcome},
    profiles::{self, NewProfile, UpdateProfile},
    scheduling::{self, EventKind, QueueQuery, StudyContext, StudySubmission},
    sharing::{self, ShareRequest},
};
use sqlx::sqlite::SqliteConnectOptions;

#[tokio::test]
async fn accepted_copies_keep_their_owner_provenance_and_learning_state_after_household_seeding() {
    let directory = tempfile::tempdir().unwrap();
    let migrations = directory.path().join("profile-migrations");
    std::fs::create_dir(&migrations).unwrap();
    for (name, contents) in [
        (
            "0002_facts_and_reviews.sql",
            include_str!("../migrations/0002_facts_and_reviews.sql"),
        ),
        (
            "0003_query_indexes.sql",
            include_str!("../migrations/0003_query_indexes.sql"),
        ),
        (
            "0004_scheduling.sql",
            include_str!("../migrations/0004_scheduling.sql"),
        ),
        (
            "0005_profiles.sql",
            include_str!("../migrations/0005_profiles.sql"),
        ),
        // Repository code used to build the fixture reads question media. Adding
        // that schema early still leaves household seeding to run afterwards.
        (
            "0009_question_media.sql",
            include_str!("../migrations/0009_question_media.sql"),
        ),
    ] {
        std::fs::write(migrations.join(name), contents).unwrap();
    }

    let path = directory.path().join("accepted-copy.sqlite3");
    let pool = sqlx::SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    sqlx::migrate::Migrator::new(migrations)
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();

    let josiah = profiles::rename(
        &pool,
        1,
        UpdateProfile {
            name: "Josiah".into(),
        },
    )
    .await
    .unwrap();
    let ansam = profiles::create(
        &pool,
        NewProfile {
            name: "Ansam".into(),
            handle: "ansam".into(),
        },
    )
    .await
    .unwrap();
    // Match an actual seed by prompt/source. Existing practice content is kept;
    // the accepted copy must not be reclassified by its shared te reo tags.
    let source = facts::create_for(
        &pool,
        ansam.id,
        NewFact {
            prompt: "What does “kai” mean in te reo Māori (noun)?".into(),
            answer: "Food; a meal".into(),
            context: "A household member's existing note".into(),
            source: "https://maoridictionary.co.nz/search?idiom=0&keywords=kai&loan=0&phrase=0&proverb=0".into(),
            tags: vec!["te-reo-māori".into(), "household".into()],
            practices: vec![],
            media: vec![],
        },
    )
    .await
    .unwrap();
    let sent = sharing::send(
        &pool,
        ansam.id,
        ShareRequest {
            submission_id: uuid::Uuid::new_v4().to_string(),
            recipient_id: josiah.id,
            fact_ids: vec![source.id],
        },
    )
    .await
    .unwrap()
    .remove(0);
    let accepted = sharing::accept(&pool, josiah.id, sent.id).await.unwrap();
    let copy_id = accepted.accepted_fact_id.unwrap();
    let question = scheduling::queue_for(&pool, josiah.id, &QueueQuery::default())
        .await
        .unwrap()
        .items
        .remove(0);
    assert_eq!(question.fact.id, copy_id);
    scheduling::submit_for(
        &pool,
        josiah.id,
        StudySubmission {
            submission_id: uuid::Uuid::new_v4().to_string(),
            target_id: question.target_id,
            practice_id: Some(question.practice.id),
            content_revision: question.content_revision,
            expected_version: question.schedule_version,
            feedback_event_id: None,
            kind: EventKind::Attempt,
            context: StudyContext::Scheduled,
            method: ReviewMethod::Flashcard,
            outcome: Some(ReviewOutcome::Good),
            response_ms: 1700,
            attempted: true,
            assisted: false,
            prior_exposure: false,
        },
    )
    .await
    .unwrap();
    let before_copy =
        serde_json::to_string(&facts::get_for(&pool, josiah.id, copy_id).await.unwrap()).unwrap();
    let before_schedule = serde_json::to_string(
        &scheduling::target_for(&pool, josiah.id, copy_id)
            .await
            .unwrap(),
    )
    .unwrap();
    let before_events = serde_json::to_string(
        &scheduling::export_events_for(&pool, josiah.id, 0, 100)
            .await
            .unwrap(),
    )
    .unwrap();
    let before_reviews =
        serde_json::to_string(&facts::reviews_for(&pool, josiah.id, copy_id).await.unwrap())
            .unwrap();
    pool.close().await;

    // Exercise production migrations directly, with no behavior-fixture reset.
    let pool = db::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    assert_eq!(profiles::list(&pool).await.unwrap().len(), 2);
    assert_eq!(
        serde_json::to_string(&facts::get_for(&pool, josiah.id, copy_id).await.unwrap()).unwrap(),
        before_copy
    );
    assert_eq!(
        serde_json::to_string(
            &scheduling::target_for(&pool, josiah.id, copy_id)
                .await
                .unwrap()
        )
        .unwrap(),
        before_schedule
    );
    assert_eq!(
        serde_json::to_string(
            &scheduling::export_events_for(&pool, josiah.id, 0, 100)
                .await
                .unwrap()
        )
        .unwrap(),
        before_events
    );
    assert_eq!(
        serde_json::to_string(&facts::reviews_for(&pool, josiah.id, copy_id).await.unwrap())
            .unwrap(),
        before_reviews
    );
    assert!(
        facts::get_for(&pool, ansam.id, copy_id)
            .await
            .unwrap()
            .is_none()
    );
    let offer: (String, i64, i64) =
        sqlx::query_as("SELECT status, recipient_id, accepted_fact_id FROM fact_shares WHERE id=?")
            .bind(sent.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(offer, ("accepted".into(), josiah.id, copy_id));
    let redundant: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact_shares
         WHERE sender_id=? AND recipient_id=? AND source_fact_id=? AND status='pending'",
    )
    .bind(ansam.id)
    .bind(josiah.id)
    .bind(source.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(redundant, 0);
    let invalid_recipients: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact_shares s JOIN facts f ON f.id=s.accepted_fact_id
         WHERE s.status='accepted' AND f.profile_id<>s.recipient_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(invalid_recipients, 0);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
}
