//! Deleting a fact removes it with its history, and leaves other libraries' copies alone.
mod support;
use support as db;
#[path = "support/media_files.rs"]
mod files;

use keepsake_core::{
    facts::{self, NewFact, NewPractice, NewReview, PracticeKind, ReviewMethod, ReviewOutcome},
    media::{self, NewQuestionMedia, UploadFormat},
    profiles::{self, NewProfile},
    scheduling::{self, EventKind, QueueQuery, StudyContext, StudyError, StudySubmission},
    sharing::{self, ShareRequest},
};

fn fact(prompt: &str, media: Vec<NewQuestionMedia>) -> NewFact {
    NewFact {
        prompt: prompt.into(),
        answer: "Kererū".into(),
        context: "A native wood pigeon.".into(),
        source: String::new(),
        tags: vec!["birds".into()],
        practices: vec![],
        media,
    }
}

async fn pool(directory: &tempfile::TempDir) -> sqlx::SqlitePool {
    let url = format!("sqlite:{}", directory.path().join("deletion.db").display());
    db::connect(&url).await.unwrap()
}

/// Rows still recorded against a fact, its recall target or the profile's study events.
async fn remnants(pool: &sqlx::SqlitePool, fact_id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM facts WHERE id=?)
              + (SELECT count(*) FROM practices WHERE fact_id=?)
              + (SELECT count(*) FROM reviews WHERE fact_id=?)
              + (SELECT count(*) FROM fact_media WHERE fact_id=?)
              + (SELECT count(*) FROM learning_targets WHERE fact_id=?)
              + (SELECT count(*) FROM target_practices)
              + (SELECT count(*) FROM schedule_states)
              + (SELECT count(*) FROM study_events)",
    )
    .bind(fact_id)
    .bind(fact_id)
    .bind(fact_id)
    .bind(fact_id)
    .bind(fact_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn missing(error: anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<StudyError>(),
        Some(StudyError::Missing(_))
    )
}

#[tokio::test]
async fn deleting_a_fact_removes_its_history_and_media_no_other_question_uses() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let format = UploadFormat::from_content_type("image/png").unwrap();
    let shared = media::upload(&pool, 1, format, files::png(1))
        .await
        .unwrap();
    let own = media::upload(&pool, 1, format, files::png(2))
        .await
        .unwrap();
    let attach = |media_id| NewQuestionMedia {
        media_id,
        alt: "A bird on a branch".into(),
    };
    let mut first = fact(
        "Which bird is this?",
        vec![attach(shared.media_id), attach(own.media_id)],
    );
    first.practices.push(NewPractice {
        kind: PracticeKind::Cloze,
        prompt: "This bird is a ___.".into(),
        answer: "Kererū".into(),
        choices: vec![],
        explanation: String::new(),
    });
    let first = facts::create_for(&pool, 1, first).await.unwrap();
    let second = facts::create_for(
        &pool,
        1,
        fact("Name the bird again", vec![attach(shared.media_id)]),
    )
    .await
    .unwrap();

    let question = scheduling::queue_for(&pool, 1, &QueueQuery::default())
        .await
        .unwrap()
        .items
        .into_iter()
        .find(|question| question.fact.id == first.id)
        .unwrap();
    scheduling::submit_for(
        &pool,
        1,
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
    // Extra practice keeps a fact out of the queue for a while, so it comes second.
    facts::record_review_for(
        &pool,
        1,
        NewReview {
            fact_id: first.id,
            practice_id: Some(first.practices[1].id),
            method: ReviewMethod::Cloze,
            outcome: ReviewOutcome::Good,
            response_ms: 2100,
        },
    )
    .await
    .unwrap();

    let deleted = facts::delete_for(&pool, 1, first.id).await.unwrap();
    assert_eq!(deleted.prompt, "Which bird is this?");
    assert!(facts::get_for(&pool, 1, first.id).await.unwrap().is_none());
    // Only the second fact's practice mapping and schedule remain.
    assert_eq!(remnants(&pool, first.id).await, 2);
    assert!(
        media::locate(&pool, 1, own.media_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        media::locate(&pool, 1, shared.media_id)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        facts::get_for(&pool, 1, second.id)
            .await
            .unwrap()
            .unwrap()
            .media[0]
            .media_id,
        shared.media_id
    );

    facts::delete_for(&pool, 1, second.id).await.unwrap();
    assert_eq!(remnants(&pool, second.id).await, 0);
    // The newest fact was deleted, and its ID is not given to the next one.
    let next = facts::create_for(&pool, 1, fact("A new bird", vec![]))
        .await
        .unwrap();
    assert_eq!(next.id, second.id + 1);
    assert!(
        media::locate(&pool, 1, shared.media_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(missing(
        facts::delete_for(&pool, 1, first.id).await.unwrap_err()
    ));
}

#[tokio::test]
async fn a_profile_cannot_delete_another_profiles_fact() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let ansam = profiles::create(
        &pool,
        NewProfile {
            name: "Ansam".into(),
            handle: "ansam".into(),
        },
    )
    .await
    .unwrap();
    let theirs = facts::create_for(&pool, ansam.id, fact("Which bird is this?", vec![]))
        .await
        .unwrap();
    assert!(missing(
        facts::delete_for(&pool, 1, theirs.id).await.unwrap_err()
    ));
    assert!(
        facts::get_for(&pool, ansam.id, theirs.id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn shared_facts_withdraw_pending_offers_and_keep_accepted_ones() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let ansam = profiles::create(
        &pool,
        NewProfile {
            name: "Ansam".into(),
            handle: "ansam".into(),
        },
    )
    .await
    .unwrap();
    let accepted_source = facts::create_for(&pool, ansam.id, fact("Which bird is this?", vec![]))
        .await
        .unwrap();
    let pending_source = facts::create_for(&pool, ansam.id, fact("Name the bird", vec![]))
        .await
        .unwrap();
    let offers = sharing::send(
        &pool,
        ansam.id,
        ShareRequest {
            submission_id: uuid::Uuid::new_v4().to_string(),
            recipient_id: 1,
            fact_ids: vec![accepted_source.id, pending_source.id],
        },
    )
    .await
    .unwrap();
    let copy_id = sharing::accept(&pool, 1, offers[0].id)
        .await
        .unwrap()
        .accepted_fact_id
        .unwrap();

    facts::delete_for(&pool, ansam.id, pending_source.id)
        .await
        .unwrap();
    facts::delete_for(&pool, ansam.id, accepted_source.id)
        .await
        .unwrap();
    let sent = sharing::list(&pool, ansam.id).await.unwrap();
    let status = |id| {
        let share = sent.iter().find(|share| share.id == id).unwrap();
        (share.status.as_str(), share.accepted_fact_id)
    };
    assert_eq!(status(offers[1].id), ("withdrawn", None));
    assert_eq!(status(offers[0].id), ("accepted", Some(copy_id)));
    // The recipient's copy is theirs, so the sender deleting the original leaves it.
    let copy = facts::get_for(&pool, 1, copy_id).await.unwrap().unwrap();
    assert_eq!(copy.shared_by.as_deref(), Some("Ansam"));

    // While the copy exists, its offer cannot stop naming it or name another fact.
    let other = facts::create_for(&pool, 1, fact("Another bird", vec![]))
        .await
        .unwrap();
    for replacement in [None, Some(other.id)] {
        sqlx::query("UPDATE fact_shares SET accepted_fact_id=? WHERE id=?")
            .bind(replacement)
            .bind(offers[0].id)
            .execute(&pool)
            .await
            .unwrap_err();
    }

    facts::delete_for(&pool, 1, copy_id).await.unwrap();
    let offer = sharing::accept(&pool, 1, offers[0].id).await.unwrap();
    assert_eq!(
        (offer.status.as_str(), offer.accepted_fact_id),
        ("accepted", None)
    );
    assert!(facts::get_for(&pool, 1, copy_id).await.unwrap().is_none());
    assert_eq!(facts::list_for(&pool, 1).await.unwrap().total, 1);
}
