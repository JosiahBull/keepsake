use keepsake_core::{
    db,
    facts::{self, NewFact, NewPractice, NewReview, PracticeKind, ReviewMethod, ReviewOutcome},
};

fn fact() -> NewFact {
    NewFact {
        prompt: "What does WAL stand for?".into(),
        answer: "Write-ahead logging".into(),
        context: "SQLite appends changes to a separate log before checkpointing.".into(),
        source: "https://sqlite.org/wal.html".into(),
        tags: vec!["sqlite".into()],
        practices: vec![],
    }
}

fn choice() -> NewPractice {
    NewPractice {
        kind: PracticeKind::MultipleChoice,
        prompt: "Which expansion names SQLite's WAL?".into(),
        answer: "Write-ahead logging".into(),
        choices: vec![
            "Write-ahead logging".into(),
            "Write-after locking".into(),
            "Weighted access list".into(),
        ],
        explanation: "The WAL records changes before checkpointing into the database.".into(),
    }
}

#[tokio::test]
async fn persists_source_variants_and_separate_review_methods_across_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite:{}", directory.path().join("facts.db").display());
    let pool = db::connect(&url).await.unwrap();
    let mut input = fact();
    input.practices.push(choice());
    let saved = facts::create(&pool, input).await.unwrap();
    assert_eq!(saved.practices.len(), 2);
    assert_eq!(saved.practices[0].kind, PracticeKind::Flashcard);
    let review = facts::record_review(
        &pool,
        NewReview {
            fact_id: saved.id,
            practice_id: Some(saved.practices[1].id),
            method: ReviewMethod::MultipleChoice,
            outcome: ReviewOutcome::Hard,
            response_ms: 3200,
        },
    )
    .await
    .unwrap();
    assert_eq!(review.response_ms, 3200);
    facts::record_review(
        &pool,
        NewReview {
            fact_id: saved.id,
            practice_id: None,
            method: ReviewMethod::Matching,
            outcome: ReviewOutcome::Good,
            response_ms: 2000,
        },
    )
    .await
    .unwrap();
    pool.close().await;
    let reopened = db::connect(&url).await.unwrap();
    let loaded = facts::get(&reopened, saved.id).await.unwrap().unwrap();
    assert_eq!(loaded.source, fact().source);
    assert_eq!(loaded.answer, fact().answer);
    assert_eq!(loaded.tags, vec!["sqlite"]);
    let history = facts::reviews(&reopened, saved.id).await.unwrap().items;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].method, ReviewMethod::MultipleChoice);
    assert_eq!(history[1].method, ReviewMethod::Matching);
    reopened.close().await;
}

#[tokio::test]
async fn rejects_bad_drafts_without_partial_facts_and_checks_review_identity() {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("facts.db").display()
    ))
    .await
    .unwrap();
    let mut input = fact();
    let mut invalid = choice();
    invalid.choices.push(" write-ahead LOGGING ".into());
    input.practices.push(invalid);
    assert!(facts::create(&pool, input).await.is_err());
    assert!(facts::list(&pool).await.unwrap().items.is_empty());
    let first = facts::create(&pool, fact()).await.unwrap();
    let second = facts::create(&pool, fact()).await.unwrap();
    let invalid_review = NewReview {
        fact_id: first.id,
        practice_id: Some(second.practices[0].id),
        method: ReviewMethod::Flashcard,
        outcome: ReviewOutcome::Good,
        response_ms: 50,
    };
    assert!(facts::record_review(&pool, invalid_review).await.is_err());
    let mismatched = NewReview {
        fact_id: first.id,
        practice_id: Some(first.practices[0].id),
        method: ReviewMethod::MultipleChoice,
        outcome: ReviewOutcome::Good,
        response_ms: 50,
    };
    assert!(facts::record_review(&pool, mismatched).await.is_err());
    assert!(
        facts::reviews(&pool, first.id)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    pool.close().await;
}

#[test]
fn choices_need_exactly_one_correct_answer_and_cloze_needs_blank() {
    let mut practice = choice();
    practice.answer = "not in the list".into();
    assert!(facts::validate_practice(&practice).is_err());
    practice.kind = PracticeKind::Cloze;
    practice.choices.clear();
    assert!(facts::validate_practice(&practice).is_err());
    practice.prompt = "WAL stands for ___.".into();
    assert!(facts::validate_practice(&practice).is_ok());
}

#[tokio::test]
async fn canonical_flashcard_preserves_long_context_and_validates() {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("facts.db").display()
    ))
    .await
    .unwrap();
    let mut input = fact();
    input.context = "x".repeat(40_000);
    let mut different = choice();
    different.kind = PracticeKind::Flashcard;
    different.choices.clear();
    different.prompt = "Another cue".into();
    input.practices.push(different);
    let saved = facts::create(&pool, input).await.unwrap();
    assert_eq!(saved.context.len(), 40_000);
    assert_eq!(saved.practices[0].prompt, saved.prompt);
    for practice in saved.practices {
        facts::validate_practice(&NewPractice {
            kind: practice.kind,
            prompt: practice.prompt,
            answer: practice.answer,
            choices: practice.choices,
            explanation: practice.explanation,
        })
        .unwrap();
    }
    pool.close().await;
}

#[test]
fn choice_answer_must_match_the_displayed_option_exactly() {
    let mut practice = choice();
    practice.choices[0] = " Write-ahead logging ".into();
    assert!(facts::validate_practice(&practice).is_err());
}

#[tokio::test]
async fn batch_import_rolls_back_on_database_failure() {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("facts.db").display()
    ))
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER reject_second BEFORE INSERT ON facts
         WHEN NEW.prompt = 'Rejected'
         BEGIN
             SELECT RAISE(ABORT, 'synthetic failure');
         END",
    )
    .execute(&pool)
    .await
    .unwrap();

    let first = fact();
    let mut second = fact();
    second.prompt = "Rejected".into();
    assert!(
        facts::create_many(&pool, vec![first, second])
            .await
            .is_err()
    );
    assert!(facts::list(&pool).await.unwrap().items.is_empty());
    pool.close().await;
}
