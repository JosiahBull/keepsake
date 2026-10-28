mod support;
use support as db;

use chrono::DateTime;
use keepsake_core::{
    facts::{self, NewFact, NewPractice, NewReview, PracticeKind, ReviewMethod, ReviewOutcome},
    scheduling::{
        self, EventKind, ExposureSubmission, QueueQuery, SettingsUpdate, StudyContext, StudyError,
        StudyQuestion, StudySubmission,
    },
};
use serde_json::{Value, json};
use sqlx::sqlite::SqliteConnectOptions;

type LegacyReview = (i64, i64, Option<i64>, String, String, i64, String);

fn timestamp(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value).unwrap().timestamp()
}

fn now() -> i64 {
    timestamp("2026-10-20T12:00:00Z")
}

async fn temporary_pool() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("scheduling.sqlite3").display()
    ))
    .await
    .unwrap();
    (directory, pool)
}

fn fact(index: usize) -> NewFact {
    NewFact {
        prompt: format!("Synthetic recall question {index}?"),
        answer: format!("Canonical answer {index}"),
        context: "Synthetic scheduling test data".into(),
        source: "https://example.com/scheduling-fixture".into(),
        tags: vec!["scheduling-fixture".into()],
        practices: vec![],
        media: vec![],
    }
}

fn submission(question: &StudyQuestion, outcome: ReviewOutcome) -> StudySubmission {
    StudySubmission {
        submission_id: uuid::Uuid::new_v4().to_string(),
        target_id: question.target_id,
        practice_id: Some(question.practice.id),
        feedback_event_id: None,
        content_revision: question.content_revision,
        expected_version: question.schedule_version,
        kind: EventKind::Attempt,
        context: StudyContext::Scheduled,
        method: match question.practice.kind {
            PracticeKind::Flashcard => ReviewMethod::Flashcard,
            PracticeKind::Cloze => ReviewMethod::Cloze,
            other => panic!("scheduled recall cannot use {other}"),
        },
        outcome: Some(outcome),
        response_ms: 2_000,
        attempted: true,
        assisted: false,
        prior_exposure: false,
    }
}

async fn counts(pool: &sqlx::SqlitePool) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM facts), (SELECT count(*) FROM practices),
         (SELECT count(*) FROM learning_targets), (SELECT count(*) FROM schedule_states)",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn first_question(pool: &sqlx::SqlitePool, at: i64) -> StudyQuestion {
    scheduling::queue_at(pool, &QueueQuery::default(), at)
        .await
        .unwrap()
        .items
        .into_iter()
        .next()
        .expect("fixture must have a question available")
}

#[tokio::test]
async fn upgrading_preserves_legacy_reviews_without_inventing_recall_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let migrations = directory.path().join("legacy-migrations");
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
    ] {
        std::fs::write(migrations.join(name), contents).unwrap();
    }
    let database = directory.path().join("legacy.sqlite3");
    let pool = sqlx::SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::migrate::Migrator::new(migrations)
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        "INSERT INTO facts(id,prompt,answer,context,source,tags,created_at)
         VALUES(1,'What does wai mean?','Water','A legacy note','A phrasebook','[\"te reo\"]','2025-01-01T00:00:00Z'),
               (2,'What does kai mean?','Food','','','[]','2025-01-01T00:00:00Z');
         INSERT INTO practices(id,fact_id,kind,prompt,answer)
         VALUES(1,1,'flashcard','What does wai mean?','Water'),
               (2,2,'flashcard','An unchecked generated question?','An incorrect generated answer');
         INSERT INTO reviews(id,fact_id,practice_id,method,outcome,response_ms,reviewed_at)
         VALUES(1,1,1,'flashcard','easy',1200,'2025-01-02T00:00:00Z'),
               (2,1,NULL,'matching','good',2300,'2025-01-03T00:00:00Z');",
    )
    .execute(&pool)
    .await
    .unwrap();
    let original_reviews: Vec<LegacyReview> =
        sqlx::query_as("SELECT id,fact_id,practice_id,method,outcome,response_ms,reviewed_at FROM reviews ORDER BY id")
            .fetch_all(&pool).await.unwrap();
    pool.close().await;

    let pool = db::connect(&format!("sqlite:{}", database.display()))
        .await
        .unwrap();
    let restored_reviews: Vec<LegacyReview> =
        sqlx::query_as("SELECT id,fact_id,practice_id,method,outcome,response_ms,reviewed_at FROM reviews ORDER BY id")
            .fetch_all(&pool).await.unwrap();
    assert_eq!(restored_reviews, original_reviews);
    let historical: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
           (SELECT count(*) FROM facts WHERE id IN (1, 2)),
           (SELECT count(*) FROM practices WHERE fact_id IN (1, 2)),
           (SELECT count(*) FROM reviews WHERE fact_id IN (1, 2)),
           (SELECT count(*) FROM learning_targets WHERE fact_id IN (1, 2))",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(historical, (2, 3, 2, 2));
    // Household migration assigns tagged te reo to Ansam without rewriting
    // the fact, its practices, or the historical learning evidence.
    let first_owner = keepsake_core::profiles::resolve(&pool, "ansam")
        .await
        .unwrap()
        .id;
    let saved = facts::get_for(&pool, first_owner, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.context, "A legacy note");
    assert_eq!(saved.source, "A phrasebook");
    assert_eq!(saved.tags, ["te reo"]);
    let generated = facts::get_for(&pool, 1, 2).await.unwrap().unwrap();
    let untouched_variant = generated
        .practices
        .iter()
        .find(|practice| practice.id == 2)
        .unwrap();
    assert_eq!(untouched_variant.prompt, "An unchecked generated question?");
    assert_eq!(untouched_variant.answer, "An incorrect generated answer");
    let queue = scheduling::queue_at_for(&pool, 1, &QueueQuery::default(), now())
        .await
        .unwrap();
    let migrated_question = queue
        .items
        .iter()
        .find(|question| question.fact.id == 2)
        .unwrap();
    assert_eq!(migrated_question.practice.prompt, "What does kai mean?");
    assert_eq!(migrated_question.practice.answer, "Food");
    let schedule = scheduling::target_for(&pool, first_owner, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(schedule.phase, "new");
    assert_eq!(schedule.stability, None);
    assert_eq!(schedule.difficulty, None);
    assert_eq!(schedule.last_review_at, None);
    assert_eq!(schedule.introduced_at, None);
    assert!(
        scheduling::export_events_for(&pool, first_owner, 0, 100)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    pool.close().await;
}

#[tokio::test]
async fn creating_a_fact_and_its_recall_target_is_atomic() {
    let (_directory, pool) = temporary_pool().await;
    sqlx::raw_sql(
        "CREATE TRIGGER reject_schedule BEFORE INSERT ON schedule_states
         BEGIN SELECT RAISE(ABORT, 'synthetic schedule failure'); END;",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(facts::create(&pool, fact(1)).await.is_err());
    assert_eq!(counts(&pool).await, (0, 0, 0, 0));
    sqlx::query("DROP TRIGGER reject_schedule")
        .execute(&pool)
        .await
        .unwrap();
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    assert_eq!(counts(&pool).await, (1, 1, 1, 1));
    assert_eq!(
        scheduling::target(&pool, saved.id)
            .await
            .unwrap()
            .unwrap()
            .version,
        1
    );
    pool.close().await;
}

#[tokio::test]
async fn five_generated_variants_share_one_obligation_and_require_recall_approval() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let before = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    let variants = [
        PracticeKind::Flashcard,
        PracticeKind::Cloze,
        PracticeKind::MultipleChoice,
        PracticeKind::Explain,
        PracticeKind::Apply,
    ]
    .into_iter()
    .map(|kind| NewPractice {
        kind,
        prompt: if kind == PracticeKind::Cloze {
            "Recall this: ___".into()
        } else {
            format!("Alternate {kind} question")
        },
        answer: saved.answer.clone(),
        choices: if kind == PracticeKind::MultipleChoice {
            vec![
                saved.answer.clone(),
                "A plausible but incorrect alternative".into(),
            ]
        } else {
            vec![]
        },
        explanation: "Generated and initially unverified".into(),
    })
    .collect();
    let updated = facts::add_practices(&pool, saved.id, variants)
        .await
        .unwrap();
    assert_eq!(updated.practices.len(), 6);
    assert_eq!(counts(&pool).await, (1, 6, 1, 1));
    assert_eq!(
        serde_json::to_value(scheduling::target(&pool, saved.id).await.unwrap()).unwrap(),
        serde_json::to_value(Some(&before)).unwrap()
    );

    let initial = first_question(&pool, now()).await;
    assert_eq!(initial.practice.id, saved.practices[0].id);
    let cloze = updated
        .practices
        .iter()
        .find(|practice| practice.kind == PracticeKind::Cloze)
        .unwrap();
    scheduling::approve_practice(&pool, cloze.id).await.unwrap();
    let choice = updated
        .practices
        .iter()
        .find(|practice| practice.kind == PracticeKind::MultipleChoice)
        .unwrap();
    assert!(
        scheduling::approve_practice(&pool, choice.id)
            .await
            .is_err()
    );
    let queue = scheduling::queue_at(&pool, &QueueQuery::default(), now())
        .await
        .unwrap();
    assert_eq!(queue.items.len(), 1);
    assert_eq!(queue.items[0].target_id, before.target_id);
    assert_eq!(queue.items[0].schedule_version, before.version);
    assert!(matches!(
        queue.items[0].practice.kind,
        PracticeKind::Flashcard | PracticeKind::Cloze
    ));
    pool.close().await;
}

#[tokio::test]
async fn retrying_a_lost_response_returns_the_original_result_without_duplicate_evidence() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let request = submission(&question, ReviewOutcome::Good);
    let accepted = scheduling::submit_at(&pool, request.clone(), now())
        .await
        .unwrap();
    let replayed = scheduling::submit_at(&pool, request.clone(), now() + 3600)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&accepted).unwrap(),
        serde_json::to_value(&replayed).unwrap()
    );
    assert!(accepted.qualifying);
    assert_eq!(accepted.schedule.version, question.schedule_version + 1);
    assert_eq!(accepted.schedule.last_review_at, Some(now()));
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 1);
    let events = scheduling::export_events(&pool, 0, 100).await.unwrap();
    assert_eq!(events.items.len(), 1);
    assert_eq!(events.items[0].algorithm, "fsrs-6.6.2");
    assert_eq!(events.items[0].parameter_version, 1);
    assert_eq!(
        serde_json::from_str::<Value>(&events.items[0].request_json).unwrap(),
        serde_json::to_value(&request).unwrap()
    );
    assert_eq!(
        events.items[0].after_json,
        serde_json::to_string(&accepted.schedule).unwrap()
    );
    assert_eq!(
        events.items[0].result_json,
        serde_json::to_string(&accepted).unwrap()
    );

    let mut different = request;
    different.outcome = Some(ReviewOutcome::Again);
    let error = scheduling::submit_at(&pool, different, now() + 7200)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StudyError>(),
        Some(StudyError::Conflict(_))
    ));
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 1);
    pool.close().await;
}

#[tokio::test]
async fn concurrent_tabs_cannot_both_advance_the_same_schedule_version() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let first = submission(&question, ReviewOutcome::Good);
    let second = submission(&question, ReviewOutcome::Easy);
    let (left, right) = tokio::join!(
        scheduling::submit_at(&pool, first, now()),
        scheduling::submit_at(&pool, second, now()),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let rejected = left.err().or_else(|| right.err()).unwrap();
    assert!(matches!(
        rejected.downcast_ref::<StudyError>(),
        Some(StudyError::Conflict(_))
    ));
    assert_eq!(
        scheduling::target(&pool, saved.id)
            .await
            .unwrap()
            .unwrap()
            .version,
        question.schedule_version + 1
    );
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 1);
    assert_eq!(
        scheduling::export_events(&pool, 0, 100)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    pool.close().await;
}

#[tokio::test]
async fn an_event_write_failure_rolls_back_memory_and_review_history() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let before = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    let question = first_question(&pool, now()).await;
    sqlx::raw_sql(
        "CREATE TRIGGER reject_event BEFORE INSERT ON study_events
         BEGIN SELECT RAISE(ABORT, 'synthetic event failure'); END;",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        scheduling::submit_at(&pool, submission(&question, ReviewOutcome::Good), now())
            .await
            .is_err()
    );
    let after = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(after).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 0);
    assert!(
        scheduling::export_events(&pool, 0, 100)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    pool.close().await;
}

#[tokio::test]
async fn answer_exposure_blocks_immediate_assessment_without_advancing_memory() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let before = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    let exposure = ExposureSubmission {
        submission_id: uuid::Uuid::new_v4().to_string(),
        fact_ids: vec![saved.id],
        method: ReviewMethod::Flashcard,
    };
    assert_eq!(
        scheduling::expose_at(&pool, exposure.clone(), now())
            .await
            .unwrap()
            .recorded,
        1
    );
    assert_eq!(
        scheduling::expose_at(&pool, exposure, now() + 100)
            .await
            .unwrap()
            .recorded,
        1
    );
    let after = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(after.stability, before.stability);
    assert_eq!(after.difficulty, before.difficulty);
    assert_eq!(after.due_at, before.due_at);
    assert_eq!(after.last_review_at, before.last_review_at);
    assert_eq!(after.introduced_at, before.introduced_at);
    assert_eq!(after.version, before.version + 1);
    assert_eq!(after.blocked_until, Some(now() + 600));
    assert!(
        scheduling::queue_at(&pool, &QueueQuery::default(), now() + 599)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        scheduling::queue_at(&pool, &QueueQuery::default(), now() + 600)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let events = scheduling::export_events(&pool, 0, 100).await.unwrap();
    assert_eq!(events.items.len(), 1);
    assert!(!events.items[0].qualifying);
    assert_eq!(events.items[0].kind, "exposure");
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 0);
    pool.close().await;
}

#[tokio::test]
async fn legacy_recognition_reviews_remain_supplementary_evidence() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let accepted = scheduling::submit_at(&pool, submission(&question, ReviewOutcome::Good), now())
        .await
        .unwrap();
    facts::record_review(
        &pool,
        NewReview {
            fact_id: saved.id,
            practice_id: None,
            method: ReviewMethod::Matching,
            outcome: ReviewOutcome::Easy,
            response_ms: 1234,
        },
    )
    .await
    .unwrap();
    let after = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(after.stability, accepted.schedule.stability);
    assert_eq!(after.difficulty, accepted.schedule.difficulty);
    assert_eq!(after.due_at, accepted.schedule.due_at);
    assert_eq!(after.last_review_at, accepted.schedule.last_review_at);
    assert_eq!(after.version, accepted.schedule.version + 1);
    let events = scheduling::export_events(&pool, 0, 100).await.unwrap();
    assert_eq!(events.items.len(), 2);
    assert!(!events.items[1].qualifying);
    assert_eq!(events.items[1].context, "supplementary");
    assert_eq!(events.items[1].method, "matching");
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 2);
    pool.close().await;
}

#[tokio::test]
async fn recognition_and_assisted_answers_cannot_initialize_recall_memory() {
    let (_directory, pool) = temporary_pool().await;
    let mut input = fact(1);
    input.practices.push(NewPractice {
        kind: PracticeKind::MultipleChoice,
        prompt: "Choose the canonical answer".into(),
        answer: input.answer.clone(),
        choices: vec![input.answer.clone(), "A different answer".into()],
        explanation: String::new(),
    });
    let saved = facts::create(&pool, input).await.unwrap();
    let question = first_question(&pool, now()).await;
    let choice = saved
        .practices
        .iter()
        .find(|practice| practice.kind == PracticeKind::MultipleChoice)
        .unwrap();
    let mut recognition = submission(&question, ReviewOutcome::Easy);
    recognition.practice_id = Some(choice.id);
    recognition.method = ReviewMethod::MultipleChoice;
    recognition.context = StudyContext::Supplementary;
    let recognized = scheduling::submit_at(&pool, recognition, now())
        .await
        .unwrap();
    assert!(!recognized.qualifying);
    assert_eq!(recognized.schedule.introduced_at, None);

    let mut assisted = submission(&question, ReviewOutcome::Good);
    assisted.expected_version = recognized.schedule.version;
    assisted.assisted = true;
    let assisted = scheduling::submit_at(&pool, assisted, now() + 1)
        .await
        .unwrap();
    assert!(!assisted.qualifying);
    assert_eq!(assisted.schedule.stability, None);
    assert_eq!(assisted.schedule.difficulty, None);
    assert_eq!(assisted.schedule.last_review_at, None);
    assert_eq!(assisted.schedule.due_at, None);
    assert_eq!(assisted.schedule.blocked_until, Some(now() + 601));
    assert!(
        scheduling::export_events(&pool, 0, 100)
            .await
            .unwrap()
            .items
            .iter()
            .all(|event| !event.qualifying)
    );
    pool.close().await;
}

#[tokio::test]
async fn exposure_batches_roll_back_if_any_fact_is_missing() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let before = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    let error = scheduling::expose_at(
        &pool,
        ExposureSubmission {
            submission_id: uuid::Uuid::new_v4().to_string(),
            fact_ids: vec![saved.id, saved.id + 100],
            method: ReviewMethod::Flashcard,
        },
        now(),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StudyError>(),
        Some(StudyError::Missing(_))
    ));
    let after = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert!(
        scheduling::export_events(&pool, 0, 100)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    pool.close().await;
}

#[tokio::test]
async fn again_is_due_after_ten_minutes_and_due_backlog_pauses_new_intake() {
    let (_directory, pool) = temporary_pool().await;
    facts::create_many(&pool, vec![fact(1), fact(2)])
        .await
        .unwrap();
    let question = first_question(&pool, now()).await;
    assert_eq!(question.intervals.again, 600);
    let accepted = scheduling::submit_at(&pool, submission(&question, ReviewOutcome::Again), now())
        .await
        .unwrap();
    assert!(accepted.qualifying);
    assert_eq!(accepted.schedule.phase, "learning");
    assert_eq!(accepted.schedule.due_at, Some(now() + 600));
    let before_due = scheduling::queue_at(&pool, &QueueQuery::default(), now() + 599)
        .await
        .unwrap();
    assert!(
        before_due
            .items
            .iter()
            .all(|item| item.target_id != question.target_id)
    );
    let due = scheduling::queue_at(&pool, &QueueQuery::default(), now() + 600)
        .await
        .unwrap();
    assert_eq!(due.total_due, 1);
    assert_eq!(due.items.len(), 1);
    assert_eq!(due.items[0].target_id, question.target_id);
    assert_eq!(due.new_remaining, 0);
    let events = scheduling::export_events(&pool, 0, 100).await.unwrap();
    assert_eq!(events.items[0].policy_override_seconds, Some(600));
    assert!(events.items[0].model_interval_days.unwrap() > 0.0);
    pool.close().await;
}

#[tokio::test]
async fn a_stale_new_item_cannot_bypass_a_due_backlog_from_another_tab() {
    let (_directory, pool) = temporary_pool().await;
    facts::create_many(&pool, vec![fact(1), fact(2)])
        .await
        .unwrap();
    let initial = scheduling::queue_at(&pool, &QueueQuery::default(), now())
        .await
        .unwrap();
    assert_eq!(initial.items.len(), 2);
    scheduling::submit_at(
        &pool,
        submission(&initial.items[0], ReviewOutcome::Again),
        now(),
    )
    .await
    .unwrap();

    let error = scheduling::submit_at(
        &pool,
        submission(&initial.items[1], ReviewOutcome::Good),
        now() + 600,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StudyError>(),
        Some(StudyError::Conflict(_))
    ));
    let untouched = scheduling::target(&pool, initial.items[1].fact.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched.version, initial.items[1].schedule_version);
    assert_eq!(untouched.introduced_at, None);
    assert_eq!(untouched.stability, None);
    assert_eq!(untouched.due_at, None);
    pool.close().await;
}

#[tokio::test]
async fn attempted_feedback_persists_across_restart_and_allows_its_own_rating() {
    let (directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let mut feedback = submission(&question, ReviewOutcome::Good);
    feedback.kind = EventKind::Exposure;
    feedback.outcome = None;
    let exposed = scheduling::submit_at(&pool, feedback, now()).await.unwrap();
    assert!(!exposed.qualifying);
    assert_eq!(exposed.schedule.stability, None);
    assert_eq!(exposed.schedule.blocked_until, Some(now() + 600));
    let after_feedback = scheduling::queue_at(&pool, &QueueQuery::default(), now() + 1)
        .await
        .unwrap();
    assert!(after_feedback.items.is_empty());
    assert_eq!(after_feedback.spent_seconds, 2);
    pool.close().await;

    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("scheduling.sqlite3").display()
    ))
    .await
    .unwrap();
    let persisted = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(persisted.version, exposed.schedule.version);
    assert_eq!(persisted.blocked_until, Some(now() + 600));
    let mut rating = submission(&question, ReviewOutcome::Good);
    rating.expected_version = persisted.version;
    rating.feedback_event_id = Some(exposed.event_id);
    let accepted = scheduling::submit_at(&pool, rating, now() + 2)
        .await
        .unwrap();
    assert!(accepted.qualifying);
    assert!(accepted.schedule.stability.is_some());
    assert_eq!(accepted.schedule.blocked_until, None);
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 1);
    let after_rating = scheduling::queue_at(&pool, &QueueQuery::default(), now() + 2)
        .await
        .unwrap();
    assert_eq!(after_rating.spent_seconds, 2);
    let events = scheduling::export_events(&pool, 0, 100).await.unwrap();
    assert_eq!(events.items.len(), 2);
    assert_eq!(events.items[0].kind, "exposure");
    assert!(!events.items[0].qualifying);
    assert!(events.items[1].qualifying);
    pool.close().await;
}

#[tokio::test]
async fn feedback_cooldown_bypass_requires_the_exact_latest_unaided_attempt() {
    for invalid_proof in [
        "changed response time",
        "assisted feedback",
        "later exposure",
    ] {
        let (_directory, pool) = temporary_pool().await;
        let saved = facts::create(&pool, fact(1)).await.unwrap();
        let question = first_question(&pool, now()).await;
        let mut feedback = submission(&question, ReviewOutcome::Good);
        feedback.kind = EventKind::Exposure;
        feedback.outcome = None;
        feedback.assisted = invalid_proof == "assisted feedback";
        let feedback = scheduling::submit_at(&pool, feedback, now()).await.unwrap();
        if invalid_proof == "later exposure" {
            scheduling::expose_at(
                &pool,
                ExposureSubmission {
                    submission_id: uuid::Uuid::new_v4().to_string(),
                    fact_ids: vec![saved.id],
                    method: ReviewMethod::Flashcard,
                },
                now() + 1,
            )
            .await
            .unwrap();
        }
        let current = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
        let mut rating = submission(&question, ReviewOutcome::Good);
        rating.expected_version = current.version;
        rating.feedback_event_id = Some(feedback.event_id);
        if invalid_proof == "changed response time" {
            rating.response_ms += 1;
        }
        let error = scheduling::submit_at(&pool, rating, now() + 2)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<StudyError>(),
                Some(StudyError::Conflict(_))
            ),
            "{invalid_proof}"
        );
        let after = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
        assert_eq!(after.version, current.version);
        assert_eq!(after.stability, None);
        assert_eq!(after.last_review_at, None);
        assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 0);
        pool.close().await;
    }
}

#[tokio::test]
async fn expired_feedback_cannot_grade_but_a_fresh_direct_attempt_can() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let mut feedback = submission(&question, ReviewOutcome::Good);
    feedback.kind = EventKind::Exposure;
    feedback.outcome = None;
    let feedback = scheduling::submit_at(&pool, feedback, now()).await.unwrap();

    let mut expired = submission(&question, ReviewOutcome::Good);
    expired.expected_version = feedback.schedule.version;
    expired.feedback_event_id = Some(feedback.event_id);
    let error = scheduling::submit_at(&pool, expired, now() + 901)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StudyError>(),
        Some(StudyError::Conflict(_))
    ));
    let current = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(current.version, feedback.schedule.version);
    assert_eq!(current.stability, None);

    let mut direct = submission(&question, ReviewOutcome::Good);
    direct.expected_version = current.version;
    assert_eq!(direct.feedback_event_id, None);
    let accepted = scheduling::submit_at(&pool, direct, now() + 902)
        .await
        .unwrap();
    assert!(accepted.qualifying);
    assert_eq!(accepted.schedule.last_review_at, Some(now() + 902));
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 1);
    pool.close().await;
}

#[tokio::test]
async fn new_feedback_cannot_launder_a_previous_answer_exposure() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    scheduling::expose_at(
        &pool,
        ExposureSubmission {
            submission_id: uuid::Uuid::new_v4().to_string(),
            fact_ids: vec![saved.id],
            method: ReviewMethod::Flashcard,
        },
        now(),
    )
    .await
    .unwrap();
    let exposed = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    let mut feedback = submission(&question, ReviewOutcome::Good);
    feedback.expected_version = exposed.version;
    feedback.kind = EventKind::Exposure;
    feedback.outcome = None;
    let error = scheduling::submit_at(&pool, feedback.clone(), now() + 1)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StudyError>(),
        Some(StudyError::Conflict(_))
    ));
    let unchanged = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(unchanged.version, exposed.version);
    assert_eq!(unchanged.blocked_until, Some(now() + 600));

    feedback.submission_id = uuid::Uuid::new_v4().to_string();
    let fresh_feedback = scheduling::submit_at(&pool, feedback, now() + 600)
        .await
        .unwrap();
    let mut rating = submission(&question, ReviewOutcome::Good);
    rating.expected_version = fresh_feedback.schedule.version;
    rating.feedback_event_id = Some(fresh_feedback.event_id);
    let accepted = scheduling::submit_at(&pool, rating, now() + 601)
        .await
        .unwrap();
    assert!(accepted.qualifying);
    assert_eq!(accepted.schedule.last_review_at, Some(now() + 600));
    let events = scheduling::export_events(&pool, 0, 100).await.unwrap();
    assert_eq!(events.items.len(), 3);
    assert_eq!(events.items[1].occurred_at, now() + 600);
    assert_eq!(events.items[2].occurred_at, now() + 601);
    assert_eq!(events.items[2].elapsed_seconds, 0);
    let after: Value = serde_json::from_str(&events.items[2].after_json).unwrap();
    assert_eq!(after["last_review_at"], json!(now() + 600));
    pool.close().await;
}

#[tokio::test]
async fn attempted_scheduled_feedback_cannot_expose_a_review_before_it_is_due() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let accepted = scheduling::submit_at(&pool, submission(&question, ReviewOutcome::Good), now())
        .await
        .unwrap();
    let mut early_feedback = submission(&question, ReviewOutcome::Good);
    early_feedback.expected_version = accepted.schedule.version;
    early_feedback.kind = EventKind::Exposure;
    early_feedback.outcome = None;
    let error = scheduling::submit_at(&pool, early_feedback, now() + 60)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StudyError>(),
        Some(StudyError::Conflict(_))
    ));
    let unchanged = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(unchanged.version, accepted.schedule.version);
    assert_eq!(unchanged.blocked_until, None);
    assert_eq!(unchanged.due_at, accepted.schedule.due_at);
    assert_eq!(
        scheduling::export_events(&pool, 0, 100)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    pool.close().await;
}

#[tokio::test]
async fn daily_new_intake_uses_local_days_across_both_london_dst_transitions() {
    for (morning, same_day_evening, next_midnight) in [
        (
            "2026-03-29T00:30:00Z",
            "2026-03-29T22:30:00Z",
            "2026-03-29T23:01:00Z",
        ),
        (
            "2026-10-25T00:30:00Z",
            "2026-10-25T23:30:00Z",
            "2026-10-26T00:01:00Z",
        ),
    ] {
        let (_directory, pool) = temporary_pool().await;
        facts::create_many(&pool, (0..10).map(fact).collect())
            .await
            .unwrap();
        let morning = timestamp(morning);
        for _ in 0..5 {
            let question = first_question(&pool, morning).await;
            scheduling::submit_at(&pool, submission(&question, ReviewOutcome::Good), morning)
                .await
                .unwrap();
        }
        let same_day =
            scheduling::queue_at(&pool, &QueueQuery::default(), timestamp(same_day_evening))
                .await
                .unwrap();
        assert_eq!(same_day.new_remaining, 0);
        assert!(same_day.items.is_empty());
        let next_day =
            scheduling::queue_at(&pool, &QueueQuery::default(), timestamp(next_midnight))
                .await
                .unwrap();
        assert_eq!(next_day.new_remaining, 5);
        assert_eq!(next_day.items.len(), 5);
        assert_eq!(next_day.timezone, "Europe/London");
        pool.close().await;
    }
}

#[tokio::test]
async fn settings_validate_bounds_and_reject_stale_updates() {
    let (_directory, pool) = temporary_pool().await;
    let original = scheduling::settings(&pool).await.unwrap();
    assert_eq!(original.new_limit, 5);
    assert_eq!(original.daily_budget_seconds, 900);
    assert_eq!(original.algorithm, "fsrs-6.6.2");
    assert_eq!(original.parameters.len(), 21);
    for (timezone, new_limit, budget) in [
        ("Not/A_Timezone", 5, 900),
        ("Europe/London", -1, 900),
        ("Europe/London", 101, 900),
        ("Europe/London", 5, 59),
        ("Europe/London", 5, 14_401),
    ] {
        let error = scheduling::update_settings(
            &pool,
            SettingsUpdate {
                timezone: timezone.into(),
                new_limit,
                daily_budget_seconds: budget,
                expected_version: original.version,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<StudyError>(),
            Some(StudyError::Invalid(_))
        ));
    }
    let update = SettingsUpdate {
        timezone: "Pacific/Auckland".into(),
        new_limit: 0,
        daily_budget_seconds: 60,
        expected_version: original.version,
    };
    let changed = scheduling::update_settings(&pool, update.clone())
        .await
        .unwrap();
    assert_eq!(changed.version, original.version + 1);
    assert_eq!(changed.timezone, "Pacific/Auckland");
    assert_eq!(changed.desired_retention, original.desired_retention);
    assert_eq!(changed.parameters, original.parameters);
    let stale = scheduling::update_settings(&pool, update)
        .await
        .unwrap_err();
    assert!(matches!(
        stale.downcast_ref::<StudyError>(),
        Some(StudyError::Conflict(_))
    ));
    pool.close().await;
}

#[tokio::test]
async fn the_daily_budget_is_a_soft_notice_and_does_not_move_due_dates() {
    let (_directory, pool) = temporary_pool().await;
    facts::create_many(&pool, vec![fact(1), fact(2)])
        .await
        .unwrap();
    let settings = scheduling::settings(&pool).await.unwrap();
    scheduling::update_settings(
        &pool,
        SettingsUpdate {
            timezone: settings.timezone,
            new_limit: settings.new_limit,
            daily_budget_seconds: 60,
            expected_version: settings.version,
        },
    )
    .await
    .unwrap();
    let question = first_question(&pool, now()).await;
    let mut request = submission(&question, ReviewOutcome::Good);
    request.response_ms = 60_000;
    let accepted = scheduling::submit_at(&pool, request, now()).await.unwrap();

    let queue = scheduling::queue_at(&pool, &QueueQuery::default(), now())
        .await
        .unwrap();
    assert!(queue.budget_reached);
    assert_eq!(queue.spent_seconds, 60);
    assert_eq!(queue.budget_seconds, 60);
    assert_eq!(queue.items.len(), 1);
    assert_ne!(queue.items[0].target_id, question.target_id);
    assert_eq!(
        scheduling::target(&pool, question.fact.id)
            .await
            .unwrap()
            .unwrap()
            .due_at,
        accepted.schedule.due_at
    );
    pool.close().await;
}

#[tokio::test]
async fn queues_are_bounded_and_paused_intake_does_not_create_obligations() {
    let (_directory, pool) = temporary_pool().await;
    facts::create_many(&pool, (0..20).map(fact).collect())
        .await
        .unwrap();
    facts::create_many(&pool, (20..30).map(fact).collect())
        .await
        .unwrap();
    let original = scheduling::settings(&pool).await.unwrap();
    let changed = scheduling::update_settings(
        &pool,
        SettingsUpdate {
            timezone: original.timezone,
            new_limit: 100,
            daily_budget_seconds: original.daily_budget_seconds,
            expected_version: original.version,
        },
    )
    .await
    .unwrap();
    for limit in [1, 7, 20] {
        let queue = scheduling::queue_at(&pool, &QueueQuery { limit }, now())
            .await
            .unwrap();
        assert_eq!(queue.items.len(), limit as usize);
        assert!(queue.items.iter().all(|item| item.schedule_version == 1));
    }
    for limit in [0, 21] {
        assert!(
            scheduling::queue_at(&pool, &QueueQuery { limit }, now())
                .await
                .is_err()
        );
    }
    scheduling::update_settings(
        &pool,
        SettingsUpdate {
            timezone: changed.timezone,
            new_limit: 0,
            daily_budget_seconds: changed.daily_budget_seconds,
            expected_version: changed.version,
        },
    )
    .await
    .unwrap();
    assert!(
        scheduling::queue_at(&pool, &QueueQuery::default(), now())
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(counts(&pool).await, (30, 30, 30, 30));
    assert!(
        scheduling::export_events(&pool, 0, 100)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    pool.close().await;
}

#[tokio::test]
async fn exported_events_are_cursor_bounded_and_preserve_replay_material() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    for _ in 0..3 {
        scheduling::expose_at(
            &pool,
            ExposureSubmission {
                submission_id: uuid::Uuid::new_v4().to_string(),
                fact_ids: vec![saved.id],
                method: ReviewMethod::Flashcard,
            },
            now(),
        )
        .await
        .unwrap();
    }
    let first = scheduling::export_events(&pool, 0, 2).await.unwrap();
    assert_eq!(first.items.len(), 2);
    let cursor = first.next_cursor.unwrap();
    assert_eq!(cursor, first.items.last().unwrap().id);
    let second = scheduling::export_events(&pool, cursor, 2).await.unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.next_cursor, None);
    assert!(second.items[0].id > cursor);
    for event in first.items.iter().chain(&second.items) {
        let before: Value = serde_json::from_str(&event.before_json).unwrap();
        let after: Value = serde_json::from_str(&event.after_json).unwrap();
        assert_eq!(
            after["version"],
            json!(before["version"].as_i64().unwrap() + 1)
        );
        assert!(serde_json::from_str::<Value>(&event.request_json).is_ok());
        assert!(serde_json::from_str::<Value>(&event.result_json).is_ok());
    }
    pool.close().await;
}
