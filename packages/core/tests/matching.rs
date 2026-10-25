mod support;
use support as db;

use keepsake_core::{
    facts::{self, NewFact},
    paging::{self, FactSummary, SampleQuery},
};
use std::collections::HashSet;

async fn temporary_pool() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("matching.sqlite3").display()
    ))
    .await
    .unwrap();
    (directory, pool)
}

fn fact(prompt: &str, answer: &str, tags: &[&str]) -> NewFact {
    NewFact {
        prompt: prompt.into(),
        answer: answer.into(),
        context: String::new(),
        source: String::new(),
        tags: tags.iter().map(|tag| (*tag).into()).collect(),
        practices: vec![],
    }
}

async fn insert(pool: &sqlx::SqlitePool, values: Vec<NewFact>) -> Vec<i64> {
    facts::create_many(pool, values)
        .await
        .unwrap()
        .into_iter()
        .map(|fact| fact.id)
        .collect()
}

fn assert_one_bank(items: &[FactSummary], banks: &[&[i64]]) {
    assert!(
        banks
            .iter()
            .any(|bank| items.iter().all(|item| bank.contains(&item.id))),
        "a matching round must contain facts from one eligible bank"
    );
    assert_eq!(
        items
            .iter()
            .map(|item| item.id)
            .collect::<HashSet<_>>()
            .len(),
        items.len(),
        "a matching round must not repeat a fact"
    );
}

#[tokio::test]
async fn every_round_stays_within_one_related_bank() {
    let (_directory, pool) = temporary_pool().await;
    let food = insert(
        &pool,
        vec![
            fact("What does wai mean?", "Water", &["māori food"]),
            fact("What does kai mean?", "Food", &["māori food"]),
            fact("What does miraka mean?", "Milk", &["māori food"]),
            fact("What does parāoa mean?", "Bread", &["māori food"]),
        ],
    )
    .await;
    let cargo = insert(
        &pool,
        vec![
            fact("Which command compiles Rust?", "cargo build", &["cargo"]),
            fact("Which command runs Rust tests?", "cargo test", &["cargo"]),
            fact("Which command checks Rust?", "cargo check", &["cargo"]),
            fact("Which command formats Rust?", "cargo fmt", &["cargo"]),
        ],
    )
    .await;

    for _ in 0..12 {
        let sample = paging::sample(&pool, &SampleQuery::default())
            .await
            .unwrap();
        assert_eq!(sample.items.len(), 4);
        assert_one_bank(&sample.items, &[&food, &cargo]);
    }
    pool.close().await;
}

#[tokio::test]
async fn narrow_eligible_banks_take_priority_over_shared_broad_tags() {
    let (_directory, pool) = temporary_pool().await;
    let food = insert(
        &pool,
        vec![
            fact("Food word one?", "Kai", &["beginner", "household", "food"]),
            fact("Food word two?", "Wai", &["beginner", "household", "food"]),
            fact(
                "Food word three?",
                "Miraka",
                &["beginner", "household", "food"],
            ),
        ],
    )
    .await;
    let actions = insert(
        &pool,
        vec![
            fact(
                "Action word one?",
                "Horoi",
                &["beginner", "household", "actions"],
            ),
            fact(
                "Action word two?",
                "Inu",
                &["beginner", "household", "actions"],
            ),
            fact(
                "Action word three?",
                "Tunu",
                &["beginner", "household", "actions"],
            ),
        ],
    )
    .await;

    for _ in 0..12 {
        let sample = paging::sample(&pool, &SampleQuery::default())
            .await
            .unwrap();
        assert_eq!(sample.items.len(), 3);
        assert_one_bank(&sample.items, &[&food, &actions]);
    }
    pool.close().await;
}

#[tokio::test]
async fn three_related_pairs_are_not_padded_with_unrelated_facts() {
    let (_directory, pool) = temporary_pool().await;
    let related = insert(
        &pool,
        vec![
            fact("Food one?", "Kai", &["food"]),
            fact("Food two?", "Wai", &["food"]),
            fact("Food three?", "Miraka", &["food"]),
        ],
    )
    .await;
    insert(
        &pool,
        vec![
            fact("Compile command?", "cargo build", &["rust"]),
            fact("A garden tool?", "Spade", &["gardening"]),
        ],
    )
    .await;

    for _ in 0..12 {
        let sample = paging::sample(&pool, &SampleQuery::default())
            .await
            .unwrap();
        assert_eq!(sample.items.len(), 3);
        assert_one_bank(&sample.items, &[&related]);
    }
    pool.close().await;
}

#[tokio::test]
async fn fewer_than_three_related_pairs_never_form_a_bank_even_with_a_small_limit() {
    let (_directory, pool) = temporary_pool().await;
    insert(
        &pool,
        vec![
            fact("Food one?", "Kai", &["food"]),
            fact("Food two?", "Wai", &["food"]),
            fact("Rust command one?", "cargo build", &["rust"]),
            fact("Rust command two?", "cargo test", &["rust"]),
            fact("An untagged question?", "An unrelated answer", &[]),
        ],
    )
    .await;

    for limit in 1..=4 {
        let sample = paging::sample(&pool, &SampleQuery { limit }).await.unwrap();
        assert!(sample.items.is_empty(), "limit {limit}");
    }
    pool.close().await;
}

#[tokio::test]
async fn duplicate_answers_and_ambiguous_prompts_do_not_make_a_bank_eligible() {
    let (_directory, pool) = temporary_pool().await;
    insert(
        &pool,
        vec![
            fact("Water word one?", "WAI", &["duplicates"]),
            fact("Water word two?", " wai ", &["duplicates"]),
            fact("Food word?", "Kai", &["duplicates"]),
            fact("What does kai mean?", "Food", &["ambiguous"]),
            fact(" WHAT DOES KAI MEAN? ", "Water", &["ambiguous"]),
            fact("What does miraka mean?", "Milk", &["ambiguous"]),
        ],
    )
    .await;

    for _ in 0..12 {
        let sample = paging::sample(&pool, &SampleQuery::default())
            .await
            .unwrap();
        assert!(sample.items.is_empty());
    }
    pool.close().await;
}

#[tokio::test]
async fn answer_deduplication_is_scoped_to_each_bank() {
    let (_directory, pool) = temporary_pool().await;
    // Older facts in an ineligible bank must not take the matching answers away
    // from a later bank that has three distinct, usable question–answer pairs.
    insert(
        &pool,
        vec![
            fact("Earlier water question?", "Water", &["incomplete"]),
            fact("Earlier food question?", "Food", &["incomplete"]),
        ],
    )
    .await;
    let eligible = insert(
        &pool,
        vec![
            fact("What does wai mean?", "Water", &["māori food"]),
            fact("What does kai mean?", "Food", &["māori food"]),
            fact("What does miraka mean?", "Milk", &["māori food"]),
        ],
    )
    .await;

    for _ in 0..12 {
        let sample = paging::sample(&pool, &SampleQuery::default())
            .await
            .unwrap();
        assert_eq!(sample.items.len(), 3);
        assert_one_bank(&sample.items, &[&eligible]);
    }
    pool.close().await;
}

#[tokio::test]
async fn bank_tags_normalize_whitespace_ascii_case_and_maori_macron_case() {
    let (_directory, pool) = temporary_pool().await;
    let bank = insert(
        &pool,
        vec![
            fact("First vowel word?", "One", &["  MĀORI ĀĒĪŌŪ  "]),
            fact("Second vowel word?", "Two", &["māori āēīōū"]),
            fact("Third vowel word?", "Three", &["Māori ĀēĪōŪ"]),
        ],
    )
    .await;

    for _ in 0..12 {
        let sample = paging::sample(&pool, &SampleQuery::default())
            .await
            .unwrap();
        assert_eq!(sample.items.len(), 3);
        assert_one_bank(&sample.items, &[&bank]);
    }
    pool.close().await;
}

#[tokio::test]
async fn unicode_case_duplicates_never_make_a_phantom_bank_eligible() {
    let (_directory, pool) = temporary_pool().await;
    insert(
        &pool,
        vec![
            fact("Capital sigma?", "Σ", &["greek"]),
            fact("Lowercase sigma?", "σ", &["greek"]),
            fact("Another Greek letter?", "Delta", &["greek"]),
        ],
    )
    .await;

    // Small limits still require a bank with three distinct answers. This also
    // catches inconsistent SQL eligibility that a later Rust filter would hide
    // when requesting a full four-pair round.
    for limit in 1..=4 {
        let sample = paging::sample(&pool, &SampleQuery { limit }).await.unwrap();
        assert!(sample.items.is_empty(), "limit {limit}");
    }
    pool.close().await;
}

#[tokio::test]
async fn bank_tags_normalize_tabs_newlines_and_unicode_whitespace() {
    let (_directory, pool) = temporary_pool().await;
    let bank = insert(
        &pool,
        vec![
            fact("First food word?", "Kai", &["\t MĀORI ĀĒĪŌŪ \n"]),
            fact("Second food word?", "Wai", &["\u{2003}māori āēīōū\u{00a0}"]),
            fact("Third food word?", "Miraka", &["\rMāori ĀēĪōŪ\t"]),
        ],
    )
    .await;

    for _ in 0..12 {
        let sample = paging::sample(&pool, &SampleQuery::default())
            .await
            .unwrap();
        assert_eq!(sample.items.len(), 3);
        assert_one_bank(&sample.items, &[&bank]);
    }
    pool.close().await;
}

#[tokio::test]
async fn conflicting_answers_to_the_same_prompt_are_scoped_to_their_bank() {
    let (_directory, pool) = temporary_pool().await;
    insert(
        &pool,
        vec![
            fact("What is a shell?", "A protective covering", &["biology"]),
            fact("What is a leaf?", "A plant organ", &["biology"]),
        ],
    )
    .await;
    let software = insert(
        &pool,
        vec![
            fact("What is a shell?", "A command interpreter", &["software"]),
            fact("Which command builds Rust?", "cargo build", &["software"]),
            fact("Which command tests Rust?", "cargo test", &["software"]),
        ],
    )
    .await;

    for _ in 0..12 {
        let sample = paging::sample(&pool, &SampleQuery::default())
            .await
            .unwrap();
        assert_eq!(sample.items.len(), 3);
        assert_one_bank(&sample.items, &[&software]);
    }
    pool.close().await;
}

#[tokio::test]
async fn matching_limits_stay_between_one_and_four_without_changing_bank_eligibility() {
    let (_directory, pool) = temporary_pool().await;
    let bank = insert(
        &pool,
        vec![
            fact("Food one?", "Kai", &["food"]),
            fact("Food two?", "Wai", &["food"]),
            fact("Food three?", "Miraka", &["food"]),
            fact("Food four?", "Parāoa", &["food"]),
        ],
    )
    .await;

    for limit in 1..=4 {
        let sample = paging::sample(&pool, &SampleQuery { limit }).await.unwrap();
        assert_eq!(sample.items.len(), limit as usize);
        assert_one_bank(&sample.items, &[&bank]);
    }
    for limit in [0, 5, -1, i64::MAX] {
        assert!(paging::sample(&pool, &SampleQuery { limit }).await.is_err());
    }
    pool.close().await;
}
