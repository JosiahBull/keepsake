mod support;

use support as db;

use keepsake_core::{
    facts::{
        self, FactEdit, FactError, FactFields, NewFact, PracticeKind, ReviewMethod, ReviewOutcome,
    },
    scheduling::{self, EventKind, QueueQuery, StudyContext, StudySubmission},
};

async fn temporary_pool() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("editing.sqlite3").display()
    ))
    .await
    .unwrap();
    (directory, pool)
}

fn fields() -> FactFields {
    FactFields {
        prompt: "What does kererū mean?".into(),
        answer: "Wood pigeon".into(),
        context: "A large native bird".into(),
        source: String::new(),
        tags: vec!["birds".into()],
    }
}

async fn create(pool: &sqlx::SqlitePool) -> facts::Fact {
    let FactFields {
        prompt,
        answer,
        context,
        source,
        tags,
    } = fields();
    facts::create(
        pool,
        NewFact {
            prompt,
            answer,
            context,
            source,
            tags,
            practices: vec![],
            media: vec![],
        },
    )
    .await
    .unwrap()
}

async fn review(pool: &sqlx::SqlitePool) {
    let question = scheduling::queue(pool, &QueueQuery::default())
        .await
        .unwrap()
        .items
        .remove(0);
    assert_eq!(question.practice.kind, PracticeKind::Flashcard);
    scheduling::submit(
        pool,
        StudySubmission {
            submission_id: uuid::Uuid::new_v4().to_string(),
            target_id: question.target_id,
            practice_id: Some(question.practice.id),
            content_revision: question.content_revision,
            expected_version: question.schedule_version,
            feedback_event_id: None,
            feedback_submission_id: None,
            occurred_at: None,
            kind: EventKind::Attempt,
            context: StudyContext::Scheduled,
            method: ReviewMethod::Flashcard,
            outcome: Some(ReviewOutcome::Good),
            response_ms: 1_000,
            attempted: true,
            assisted: false,
            prior_exposure: false,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn editing_context_keeps_the_schedule() {
    let (_directory, pool) = temporary_pool().await;
    let fact = create(&pool).await;
    review(&pool).await;
    let before = scheduling::target(&pool, fact.id).await.unwrap().unwrap();
    let mut edited = fields();
    edited.context = "The New Zealand wood pigeon".into();
    let saved = facts::edit_for(
        &pool,
        1,
        fact.id,
        FactEdit {
            base: fields(),
            fields: edited,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.context, "The New Zealand wood pigeon");
    let after = scheduling::target(&pool, fact.id).await.unwrap().unwrap();
    assert_eq!(after.version, before.version);
    assert_eq!(after.content_revision, 1);
    assert_eq!(after.stability, before.stability);
}

#[tokio::test]
async fn changing_the_answer_starts_a_new_revision_with_a_fresh_schedule() {
    let (_directory, pool) = temporary_pool().await;
    let fact = create(&pool).await;
    review(&pool).await;
    let mut edited = fields();
    edited.answer = "Kererū: the New Zealand wood pigeon".into();
    let saved = facts::edit_for(
        &pool,
        1,
        fact.id,
        FactEdit {
            base: fields(),
            fields: edited,
        },
    )
    .await
    .unwrap();
    // The canonical flashcard follows the new answer.
    assert_eq!(saved.practices.len(), 1);
    assert_eq!(
        saved.practices[0].answer,
        "Kererū: the New Zealand wood pigeon"
    );
    let target = scheduling::target(&pool, fact.id).await.unwrap().unwrap();
    assert_eq!(target.content_revision, 2);
    assert_eq!(target.phase, "new");
    assert_eq!(target.stability, None);
    // The new revision is studied like a new fact, and its prompt qualifies.
    let queue = scheduling::queue(&pool, &QueueQuery::default())
        .await
        .unwrap();
    assert_eq!(queue.items.len(), 1);
    assert_eq!(queue.items[0].content_revision, 2);
    review(&pool).await;
    // Earlier history stays.
    assert_eq!(
        scheduling::export_events(&pool, 0, 10)
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    assert_eq!(facts::reviews(&pool, fact.id).await.unwrap().total, 2);
}

#[tokio::test]
async fn edits_to_different_fields_merge_and_to_the_same_field_conflict() {
    let (_directory, pool) = temporary_pool().await;
    let fact = create(&pool).await;
    let mut elsewhere = fields();
    elsewhere.source = "A field guide".into();
    facts::edit_for(
        &pool,
        1,
        fact.id,
        FactEdit {
            base: fields(),
            fields: elsewhere,
        },
    )
    .await
    .unwrap();

    // Made from the older version, it changes only the tags, so both apply.
    let mut tagged = fields();
    tagged.tags = vec!["birds".into(), "forest".into()];
    let merged = facts::edit_for(
        &pool,
        1,
        fact.id,
        FactEdit {
            base: fields(),
            fields: tagged,
        },
    )
    .await
    .unwrap();
    assert_eq!(merged.source, "A field guide");
    assert_eq!(merged.tags, ["birds", "forest"]);

    let mut other_source = fields();
    other_source.source = "My notes".into();
    let error = facts::edit_for(
        &pool,
        1,
        fact.id,
        FactEdit {
            base: fields(),
            fields: other_source,
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<FactError>(),
            Some(FactError::Conflict(_))
        ),
        "{error}"
    );
    assert_eq!(
        error.to_string(),
        "The source changed elsewhere since you opened this fact"
    );

    let mut empty = fields();
    empty.prompt = " ".into();
    let error = facts::edit_for(
        &pool,
        1,
        fact.id,
        FactEdit {
            base: fields(),
            fields: empty,
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(
            error.downcast_ref::<FactError>(),
            Some(FactError::Invalid(_))
        ),
        "{error}"
    );
}
