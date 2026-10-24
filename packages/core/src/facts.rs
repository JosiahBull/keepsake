use std::{collections::HashSet, fmt};

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PracticeKind {
    Flashcard,
    MultipleChoice,
    Cloze,
    Explain,
    Apply,
}

impl PracticeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flashcard => "flashcard",
            Self::MultipleChoice => "multiple_choice",
            Self::Cloze => "cloze",
            Self::Explain => "explain",
            Self::Apply => "apply",
        }
    }
}

impl fmt::Display for PracticeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewPractice {
    pub kind: PracticeKind,
    pub prompt: String,
    pub answer: String,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub explanation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewFact {
    pub prompt: String,
    pub answer: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub practices: Vec<NewPractice>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Practice {
    pub id: i64,
    pub fact_id: i64,
    pub kind: PracticeKind,
    pub prompt: String,
    pub answer: String,
    pub choices: Vec<String>,
    pub explanation: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Fact {
    pub id: i64,
    pub prompt: String,
    pub answer: String,
    pub context: String,
    pub source: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub practices: Vec<Practice>,
}

fn bounded(value: &str, label: &str, limit: usize, required: bool) -> anyhow::Result<()> {
    ensure!(
        !required || !value.trim().is_empty(),
        "{label} must not be empty"
    );
    ensure!(value.len() <= limit, "{label} exceeds {limit} bytes");
    ensure!(!value.contains('\0'), "{label} must not contain null bytes");
    Ok(())
}

pub fn validate_practice(practice: &NewPractice) -> anyhow::Result<()> {
    bounded(&practice.prompt, "practice prompt", 8_000, true)?;
    bounded(&practice.answer, "practice answer", 32_000, true)?;
    bounded(&practice.explanation, "practice explanation", 32_000, false)?;
    match practice.kind {
        PracticeKind::MultipleChoice => {
            ensure!(
                (2..=8).contains(&practice.choices.len()),
                "multiple choice needs 2 to 8 choices"
            );
            let mut seen = HashSet::new();
            for choice in &practice.choices {
                bounded(choice, "choice", 4_000, true)?;
                ensure!(
                    seen.insert(choice.trim().to_lowercase()),
                    "choices must be distinct"
                );
            }
            ensure!(
                practice
                    .choices
                    .iter()
                    .filter(|choice| *choice == &practice.answer)
                    .count()
                    == 1,
                "multiple choice answer must match exactly one choice"
            );
        }
        PracticeKind::Cloze => {
            ensure!(
                practice.choices.is_empty(),
                "only multiple choice accepts choices"
            );
            ensure!(
                practice.prompt.contains("___"),
                "cloze prompt needs a ___ blank"
            );
        }
        PracticeKind::Flashcard | PracticeKind::Explain | PracticeKind::Apply => {
            ensure!(
                practice.choices.is_empty(),
                "only multiple choice accepts choices"
            );
        }
    }
    Ok(())
}

pub fn validate_new_fact(fact: &NewFact) -> anyhow::Result<()> {
    bounded(&fact.prompt, "prompt", 8_000, true)?;
    bounded(&fact.answer, "answer", 32_000, true)?;
    bounded(&fact.context, "context", 64_000, false)?;
    bounded(&fact.source, "source", 4_000, false)?;
    ensure!(fact.tags.len() <= 32, "at most 32 tags are allowed");
    for tag in &fact.tags {
        bounded(tag, "tag", 100, true)?;
    }
    validate_practices(&fact.practices)?;
    Ok(())
}

fn validate_practices(practices: &[NewPractice]) -> anyhow::Result<()> {
    ensure!(practices.len() <= 32, "at most 32 practices per request");
    for practice in practices {
        validate_practice(practice)?;
    }
    Ok(())
}

#[derive(FromRow)]
struct FactRow {
    id: i64,
    prompt: String,
    answer: String,
    context: String,
    source: String,
    tags: String,
    created_at: String,
}

#[derive(FromRow)]
struct PracticeRow {
    id: i64,
    fact_id: i64,
    kind: String,
    prompt: String,
    answer: String,
    choices: String,
    explanation: String,
}

impl TryFrom<PracticeRow> for Practice {
    type Error = anyhow::Error;
    fn try_from(row: PracticeRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.id,
            fact_id: row.fact_id,
            kind: serde_json::from_value(serde_json::Value::String(row.kind))?,
            prompt: row.prompt,
            answer: row.answer,
            choices: serde_json::from_str(&row.choices)?,
            explanation: row.explanation,
        })
    }
}

async fn load(connection: &mut SqliteConnection, row: FactRow) -> anyhow::Result<Fact> {
    let practices = sqlx::query_as::<_, PracticeRow>(
        "SELECT id, fact_id, kind, prompt, answer, choices, explanation
         FROM practices
         WHERE fact_id = ?
         ORDER BY id",
    )
    .bind(row.id)
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(Practice::try_from)
    .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(Fact {
        id: row.id,
        prompt: row.prompt,
        answer: row.answer,
        context: row.context,
        source: row.source,
        tags: serde_json::from_str(&row.tags)?,
        created_at: row.created_at,
        practices,
    })
}

async fn load_id(connection: &mut SqliteConnection, id: i64) -> anyhow::Result<Option<Fact>> {
    let row = sqlx::query_as::<_, FactRow>(
        "SELECT id, prompt, answer, context, source, tags, created_at FROM facts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *connection)
    .await?;
    match row {
        Some(row) => Ok(Some(load(connection, row).await?)),
        None => Ok(None),
    }
}

pub async fn get(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Fact>> {
    let mut connection = pool.acquire().await?;
    load_id(&mut connection, id).await
}

pub async fn list(pool: &SqlitePool) -> anyhow::Result<Vec<Fact>> {
    let mut connection = pool.acquire().await?;
    let rows = sqlx::query_as::<_, FactRow>(
        "SELECT id, prompt, answer, context, source, tags, created_at FROM facts ORDER BY id DESC",
    )
    .fetch_all(&mut *connection)
    .await?;
    let mut facts = Vec::with_capacity(rows.len());
    for row in rows {
        facts.push(load(&mut connection, row).await?);
    }
    Ok(facts)
}

async fn insert_practice(
    connection: &mut SqliteConnection,
    fact_id: i64,
    practice: &NewPractice,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO practices (fact_id, kind, prompt, answer, choices, explanation)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(fact_id)
    .bind(practice.kind.as_str())
    .bind(&practice.prompt)
    .bind(&practice.answer)
    .bind(serde_json::to_string(&practice.choices)?)
    .bind(&practice.explanation)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn create(pool: &SqlitePool, fact: NewFact) -> anyhow::Result<Fact> {
    create_many(pool, vec![fact])
        .await?
        .pop()
        .context("inserted fact missing")
}

/// Validate the whole import before writing, then commit all facts together.
pub async fn create_many(pool: &SqlitePool, facts: Vec<NewFact>) -> anyhow::Result<Vec<Fact>> {
    ensure!(
        (1..=20).contains(&facts.len()),
        "provide between 1 and 20 facts"
    );
    for fact in &facts {
        validate_new_fact(fact)?;
    }
    crate::db::write_transaction(pool, move |connection| {
        let facts = facts.clone();
        Box::pin(async move {
            let mut saved = Vec::with_capacity(facts.len());
            for fact in facts {
                saved.push(insert_fact(connection, fact).await?);
            }
            Ok(saved)
        })
    })
    .await
}

async fn insert_fact(connection: &mut SqliteConnection, mut fact: NewFact) -> anyhow::Result<Fact> {
    if !fact.practices.iter().any(|p| {
        p.kind == PracticeKind::Flashcard && p.prompt == fact.prompt && p.answer == fact.answer
    }) {
        fact.practices.insert(
            0,
            NewPractice {
                kind: PracticeKind::Flashcard,
                prompt: fact.prompt.clone(),
                answer: fact.answer.clone(),
                choices: vec![],
                explanation: String::new(),
            },
        );
    }
    let result = sqlx::query(
        "INSERT INTO facts (prompt, answer, context, source, tags) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&fact.prompt)
    .bind(&fact.answer)
    .bind(&fact.context)
    .bind(&fact.source)
    .bind(serde_json::to_string(&fact.tags)?)
    .execute(&mut *connection)
    .await?;
    let id = result.last_insert_rowid();
    for practice in &fact.practices {
        insert_practice(connection, id, practice).await?;
    }
    load_id(connection, id)
        .await?
        .context("inserted fact missing")
}

pub async fn add_practices(
    pool: &SqlitePool,
    id: i64,
    practices: Vec<NewPractice>,
) -> anyhow::Result<Fact> {
    ensure!(!practices.is_empty(), "provide at least one practice");
    validate_practices(&practices)?;
    crate::db::write_transaction(pool, move |connection| {
        let practices = practices.clone();
        Box::pin(async move {
            ensure!(
                load_id(connection, id).await?.is_some(),
                "fact {id} does not exist"
            );
            for practice in &practices {
                insert_practice(connection, id, practice).await?;
            }
            load_id(connection, id).await?.context("fact missing")
        })
    })
    .await
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewMethod {
    Flashcard,
    MultipleChoice,
    Cloze,
    Explain,
    Apply,
    Matching,
}

impl ReviewMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::Flashcard => "flashcard",
            Self::MultipleChoice => "multiple_choice",
            Self::Cloze => "cloze",
            Self::Explain => "explain",
            Self::Apply => "apply",
            Self::Matching => "matching",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewOutcome {
    Again,
    Hard,
    Good,
    Easy,
}

impl ReviewOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Again => "again",
            Self::Hard => "hard",
            Self::Good => "good",
            Self::Easy => "easy",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewReview {
    pub fact_id: i64,
    pub practice_id: Option<i64>,
    pub method: ReviewMethod,
    pub outcome: ReviewOutcome,
    pub response_ms: i64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct Review {
    pub id: i64,
    pub fact_id: i64,
    pub practice_id: Option<i64>,
    pub method: ReviewMethod,
    pub outcome: ReviewOutcome,
    pub response_ms: i64,
    pub reviewed_at: String,
}

pub fn validate_review(review: &NewReview, fact: &Fact) -> anyhow::Result<()> {
    ensure!(review.fact_id == fact.id, "fact does not match review");
    ensure!(
        (0..=86_400_000).contains(&review.response_ms),
        "response_ms must be between 0 and 86400000"
    );
    match review.practice_id {
        Some(id) => {
            let practice = fact
                .practices
                .iter()
                .find(|p| p.id == id)
                .context("practice does not belong to this fact")?;
            ensure!(
                practice.kind.as_str() == review.method.as_str(),
                "method does not match practice"
            );
        }
        None => ensure!(
            matches!(
                review.method,
                ReviewMethod::Matching | ReviewMethod::Flashcard
            ),
            "this method requires a practice_id"
        ),
    }
    Ok(())
}

pub async fn record_review(pool: &SqlitePool, review: NewReview) -> anyhow::Result<Review> {
    crate::db::write_transaction(pool, move |connection| {
        let review = review.clone();
        Box::pin(async move {
            let fact = load_id(connection, review.fact_id)
                .await?
                .context("fact does not exist")?;
            validate_review(&review, &fact)?;
            let (id, reviewed_at): (i64, String) = sqlx::query_as(
                "INSERT INTO reviews (fact_id, practice_id, method, outcome, response_ms)
         VALUES (?, ?, ?, ?, ?)
         RETURNING id, reviewed_at",
            )
            .bind(review.fact_id)
            .bind(review.practice_id)
            .bind(review.method.as_str())
            .bind(review.outcome.as_str())
            .bind(review.response_ms)
            .fetch_one(&mut *connection)
            .await?;

            Ok(Review {
                id,
                fact_id: review.fact_id,
                practice_id: review.practice_id,
                method: review.method,
                outcome: review.outcome,
                response_ms: review.response_ms,
                reviewed_at,
            })
        })
    })
    .await
}

#[derive(FromRow)]
struct ReviewRow {
    id: i64,
    fact_id: i64,
    practice_id: Option<i64>,
    method: String,
    outcome: String,
    response_ms: i64,
    reviewed_at: String,
}

pub async fn reviews(pool: &SqlitePool, fact_id: i64) -> anyhow::Result<Vec<Review>> {
    let rows = sqlx::query_as::<_, ReviewRow>(
        "SELECT id, fact_id, practice_id, method, outcome, response_ms, reviewed_at
         FROM reviews
         WHERE fact_id = ?
         ORDER BY id",
    )
    .bind(fact_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(Review {
                id: row.id,
                fact_id: row.fact_id,
                practice_id: row.practice_id,
                method: serde_json::from_value(serde_json::Value::String(row.method))?,
                outcome: serde_json::from_value(serde_json::Value::String(row.outcome))?,
                response_ms: row.response_ms,
                reviewed_at: row.reviewed_at,
            })
        })
        .collect()
}
