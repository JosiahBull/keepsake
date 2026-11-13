mod support;

use support as db;

use chrono::DateTime;
use keepsake_core::{
    facts::{self, NewFact, PracticeKind, ReviewMethod, ReviewOutcome},
    scheduling::{
        self, EventKind, QueueQuery, StudyContext, StudyError, StudyQuestion, StudySubmission,
    },
};

const DAY: i64 = 86_400;

fn now() -> i64 {
    DateTime::parse_from_rfc3339("2026-10-20T12:00:00Z")
        .unwrap()
        .timestamp()
}

async fn temporary_pool() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("offline.sqlite3").display()
    ))
    .await
    .unwrap();
    (directory, pool)
}

fn fact(index: usize) -> NewFact {
    NewFact {
        prompt: format!("Synthetic offline question {index}?"),
        answer: format!("Offline answer {index}"),
        context: String::new(),
        source: String::new(),
        tags: vec![],
        practices: vec![],
        media: vec![],
    }
}

async fn first_question(pool: &sqlx::SqlitePool, at: i64) -> StudyQuestion {
    scheduling::queue_at(pool, &QueueQuery::default(), at)
        .await
        .unwrap()
        .items
        .into_iter()
        .next()
        .expect("a question must be available")
}

/// Feedback for an attempt made on a device at `at`, as an offline device sends it.
fn feedback(question: &StudyQuestion, at: i64) -> StudySubmission {
    StudySubmission {
        submission_id: uuid::Uuid::new_v4().to_string(),
        target_id: question.target_id,
        practice_id: Some(question.practice.id),
        feedback_event_id: None,
        feedback_submission_id: None,
        occurred_at: Some(at),
        content_revision: question.content_revision,
        expected_version: question.schedule_version,
        kind: EventKind::Exposure,
        context: StudyContext::Scheduled,
        method: match question.practice.kind {
            PracticeKind::Flashcard => ReviewMethod::Flashcard,
            PracticeKind::Cloze => ReviewMethod::Cloze,
            other => panic!("scheduled recall cannot use {other}"),
        },
        outcome: None,
        response_ms: 2_000,
        attempted: true,
        assisted: false,
        prior_exposure: false,
    }
}

/// The rating that follows `feedback`, made `after` seconds later on the same device.
fn rating(feedback: &StudySubmission, outcome: ReviewOutcome, after: i64) -> StudySubmission {
    StudySubmission {
        submission_id: uuid::Uuid::new_v4().to_string(),
        feedback_submission_id: Some(feedback.submission_id.clone()),
        occurred_at: feedback.occurred_at.map(|at| at + after),
        expected_version: feedback.expected_version + 1,
        kind: EventKind::Attempt,
        outcome: Some(outcome),
        ..feedback.clone()
    }
}

#[tokio::test]
async fn an_attempt_made_offline_counts_from_when_it_happened() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let shown = feedback(&question, now());
    let rated = rating(&shown, ReviewOutcome::Good, 30);

    // Both reach the server two days later, in order.
    scheduling::submit_at(&pool, shown, now() + 2 * DAY)
        .await
        .unwrap();
    let result = scheduling::submit_at(&pool, rated.clone(), now() + 2 * DAY + 5)
        .await
        .unwrap();
    assert!(result.qualifying);
    assert_eq!(result.ignored, None);
    // The model measures from the attempt's feedback, not from when it arrived.
    assert_eq!(result.schedule.last_review_at, Some(now()));
    assert_eq!(result.schedule.introduced_at, Some(now()));
    let review = facts::reviews(&pool, saved.id).await.unwrap();
    assert_eq!(review.total, 1);
    assert_eq!(review.items[0].reviewed_at, "2026-10-20T12:00:00.000Z");

    // Sending it again returns the same result.
    let again = scheduling::submit_at(&pool, rated, now() + 3 * DAY)
        .await
        .unwrap();
    assert_eq!(again.event_id, result.event_id);
    pool.close().await;
}

#[tokio::test]
async fn an_offline_attempt_overtaken_by_another_device_is_kept_but_ignored() {
    let (_directory, pool) = temporary_pool().await;
    let saved = facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let shown = feedback(&question, now());
    let rated = rating(&shown, ReviewOutcome::Again, 20);

    // Another device reviews the same target before the offline attempt arrives.
    let mut online = feedback(&question, now());
    online.occurred_at = None;
    online.kind = EventKind::Attempt;
    online.outcome = Some(ReviewOutcome::Good);
    let applied = scheduling::submit_at(&pool, online, now() + 60)
        .await
        .unwrap();

    let ignored_feedback = scheduling::submit_at(&pool, shown, now() + DAY)
        .await
        .unwrap();
    assert!(ignored_feedback.ignored.is_some());
    let ignored = scheduling::submit_at(&pool, rated.clone(), now() + DAY + 1)
        .await
        .unwrap();
    assert!(!ignored.qualifying);
    assert!(ignored.review.is_none());
    // Its feedback was ignored, so whichever rule it fails first, it cannot apply.
    assert!(ignored.ignored.is_some());
    let current = scheduling::target(&pool, saved.id).await.unwrap().unwrap();
    assert_eq!(current.version, applied.schedule.version);
    assert_eq!(current.due_at, applied.schedule.due_at);
    assert_eq!(facts::reviews(&pool, saved.id).await.unwrap().total, 1);

    // Both are in the history, marked as ignored, and a retry finds the same event.
    let events = scheduling::export_events(&pool, 0, 100)
        .await
        .unwrap()
        .items;
    assert_eq!(events.len(), 3);
    assert!(events[0].ignored.is_none());
    assert!(events[1].ignored.is_some() && events[2].ignored.is_some());
    assert_eq!(events[2].occurred_at, now() + 20);
    let retried = scheduling::submit_at(&pool, rated, now() + 2 * DAY)
        .await
        .unwrap();
    assert_eq!(retried.event_id, ignored.event_id);
    pool.close().await;
}

#[tokio::test]
async fn offline_times_are_bounded() {
    let (_directory, pool) = temporary_pool().await;
    facts::create(&pool, fact(1)).await.unwrap();
    let question = first_question(&pool, now()).await;

    let stale = feedback(&question, now() - 31 * DAY);
    let error = scheduling::submit_at(&pool, stale, now())
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<StudyError>(),
        Some(StudyError::Invalid(_))
    ));

    // A device clock ahead of the server's counts as now.
    let ahead = feedback(&question, now() + 3_600);
    let result = scheduling::submit_at(&pool, ahead, now()).await.unwrap();
    let events = scheduling::export_events(&pool, 0, 10).await.unwrap().items;
    assert_eq!(events[0].occurred_at, now());
    assert_eq!(result.schedule.blocked_until, Some(now() + 600));
    pool.close().await;
}

#[tokio::test]
async fn new_targets_studied_offline_count_against_the_day_they_were_studied() {
    let (_directory, pool) = temporary_pool().await;
    for index in 0..7 {
        facts::create(&pool, fact(index)).await.unwrap();
    }
    let prepared = scheduling::prepared_at_for(&pool, 1, now()).await.unwrap();
    assert_eq!(prepared.new.len(), 7);
    assert_eq!(prepared.new_limit, 5);
    // Five new facts on the first day and two the next, all sent on the third.
    for (index, question) in prepared.new.iter().enumerate() {
        let at = now()
            + if index < 5 {
                index as i64
            } else {
                DAY + index as i64
            };
        let shown = feedback(question, at);
        let rated = rating(&shown, ReviewOutcome::Good, 10);
        scheduling::submit_at(&pool, shown, now() + 2 * DAY)
            .await
            .unwrap();
        let result = scheduling::submit_at(&pool, rated, now() + 2 * DAY)
            .await
            .unwrap();
        assert!(result.qualifying, "new fact {index}");
    }
    pool.close().await;
}

#[tokio::test]
async fn prepared_study_reaches_a_week_ahead() {
    let (_directory, pool) = temporary_pool().await;
    facts::create(&pool, fact(1)).await.unwrap();
    facts::create(&pool, fact(2)).await.unwrap();
    let question = first_question(&pool, now()).await;
    let shown = feedback(&question, now());
    let rated = rating(&shown, ReviewOutcome::Good, 5);
    scheduling::submit_at(&pool, shown, now()).await.unwrap();
    let result = scheduling::submit_at(&pool, rated, now() + 5)
        .await
        .unwrap();
    let due_at = result.schedule.due_at.unwrap();
    assert!(due_at < now() + 7 * 86_400);

    let prepared = scheduling::prepared_at_for(&pool, 1, now() + 10)
        .await
        .unwrap();
    assert_eq!(prepared.due.len(), 1);
    assert_eq!(prepared.due[0].question.target_id, question.target_id);
    assert_eq!(prepared.due[0].available_at, due_at);
    assert_eq!(
        prepared.due[0].question.schedule_version,
        result.schedule.version
    );
    assert_eq!(prepared.new.len(), 1);
    assert_eq!(prepared.introduced_today, 1);

    // A week later it is past due, so it is available straight away.
    let later = scheduling::prepared_at_for(&pool, 1, due_at + 7 * 86_400)
        .await
        .unwrap();
    assert_eq!(later.due[0].available_at, due_at + 7 * 86_400);
    pool.close().await;
}
