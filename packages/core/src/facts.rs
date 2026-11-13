use std::{collections::HashSet, fmt};

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use strum::EnumIter;
use utoipa::ToSchema;

use crate::media::{NewQuestionMedia, QuestionMedia};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema, EnumIter)]
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

    /// The stored text of a kind, as [`PracticeKind::as_str`] writes it.
    pub(crate) fn parse(value: &str) -> anyhow::Result<Self> {
        Ok(match value {
            "flashcard" => Self::Flashcard,
            "multiple_choice" => Self::MultipleChoice,
            "cloze" => Self::Cloze,
            "explain" => Self::Explain,
            "apply" => Self::Apply,
            other => anyhow::bail!("unknown practice kind {other}"),
        })
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
    /// Uploaded images and audio clips asked with the question, in display order.
    /// Omitted when empty, so share snapshots of text-only facts are unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<NewQuestionMedia>,
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
    #[serde(default)]
    pub shared_by: Option<String>,
    /// Images and audio clips asked with the question, in display order.
    pub media: Vec<QuestionMedia>,
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
    crate::media::validate(&fact.media)?;
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
pub(crate) struct FactRow {
    pub(crate) id: i64,
    pub(crate) prompt: String,
    pub(crate) answer: String,
    pub(crate) context: String,
    pub(crate) source: String,
    pub(crate) tags: String,
    pub(crate) created_at: String,
}

#[derive(FromRow)]
pub(crate) struct PracticeRow {
    pub(crate) id: i64,
    pub(crate) fact_id: i64,
    pub(crate) kind: String,
    pub(crate) prompt: String,
    pub(crate) answer: String,
    pub(crate) choices: String,
    pub(crate) explanation: String,
}

impl TryFrom<PracticeRow> for Practice {
    type Error = anyhow::Error;
    fn try_from(row: PracticeRow) -> anyhow::Result<Self> {
        let PracticeRow {
            id,
            fact_id,
            kind,
            prompt,
            answer,
            choices,
            explanation,
        } = row;
        Ok(Self {
            id,
            fact_id,
            kind: PracticeKind::parse(&kind)?,
            prompt,
            answer,
            choices: serde_json::from_str(&choices)?,
            explanation,
        })
    }
}

async fn load(connection: &mut SqliteConnection, row: FactRow) -> anyhow::Result<Fact> {
    let FactRow {
        id,
        prompt,
        answer,
        context,
        source,
        tags,
        created_at,
    } = row;
    let practices = sqlx::query_as!(
        PracticeRow,
        "SELECT id AS \"id!\", fact_id, kind, prompt, answer, choices, explanation
         FROM practices
         WHERE fact_id = ?
         ORDER BY id",
        id
    )
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(Practice::try_from)
    .collect::<anyhow::Result<Vec<_>>>()?;

    let shared_by = sqlx::query_scalar::<_, String>(
        "SELECT p.name FROM fact_shares s JOIN profiles p ON p.id=s.sender_id
         WHERE s.accepted_fact_id=? AND s.status='accepted' LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&mut *connection)
    .await?;
    let media = crate::media::for_fact(connection, id).await?;
    Ok(Fact {
        shared_by,
        media,
        id,
        prompt,
        answer,
        context,
        source,
        tags: serde_json::from_str(&tags)?,
        created_at,
        practices,
    })
}

pub(crate) async fn load_id(
    connection: &mut SqliteConnection,
    id: i64,
) -> anyhow::Result<Option<Fact>> {
    let row = sqlx::query_as!(
        FactRow,
        "SELECT id AS \"id!\", prompt, answer, context, source, tags, created_at FROM facts WHERE id = ?",
        id
    )
    .fetch_optional(&mut *connection)
    .await?;
    match row {
        Some(row) => Ok(Some(load(connection, row).await?)),
        None => Ok(None),
    }
}

pub async fn get_for(pool: &SqlitePool, profile_id: i64, id: i64) -> anyhow::Result<Option<Fact>> {
    let mut connection = pool.acquire().await?;
    crate::profiles::require(&mut connection, profile_id).await?;
    let owned: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM facts WHERE id=? AND profile_id=?)")
            .bind(id)
            .bind(profile_id)
            .fetch_one(&mut *connection)
            .await?;
    if !owned {
        return Ok(None);
    }
    load_id(&mut connection, id).await
}

pub async fn list_for(
    pool: &SqlitePool,
    profile_id: i64,
) -> anyhow::Result<crate::paging::FactPage> {
    crate::paging::facts_for(pool, profile_id, &crate::paging::FactQuery::default()).await
}

async fn insert_practice(
    connection: &mut SqliteConnection,
    fact_id: i64,
    practice: &NewPractice,
) -> anyhow::Result<()> {
    sqlx::query!(
        "INSERT INTO practices (fact_id, kind, prompt, answer, choices, explanation)
         VALUES (?, ?, ?, ?, ?, ?)",
        fact_id,
        practice.kind.as_str(),
        practice.prompt,
        practice.answer,
        serde_json::to_string(&practice.choices)?,
        practice.explanation
    )
    .execute(connection)
    .await?;
    Ok(())
}

/// A request that conflicts with what the library already holds.
#[derive(Debug)]
pub enum FactError {
    Conflict(String),
    Invalid(String),
}

impl fmt::Display for FactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict(message) | Self::Invalid(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for FactError {}

/// Creates a fact once per submission ID, so a retried request returns the fact the first
/// one created. Reusing the ID for a different fact is a conflict, as is retrying after
/// the fact was deleted.
pub async fn create_once_for(
    pool: &SqlitePool,
    profile_id: i64,
    submission_id: &str,
    fact: NewFact,
) -> anyhow::Result<Fact> {
    ensure!(
        uuid::Uuid::parse_str(submission_id).is_ok(),
        "Idempotency-Key must be a UUID"
    );
    validate_new_fact(&fact)?;
    let submission_id = submission_id.to_owned();
    crate::db::write_transaction(pool, move |connection| {
        let fact = fact.clone();
        let submission_id = submission_id.clone();
        Box::pin(async move {
            crate::profiles::require(connection, profile_id).await?;
            let request_json = serde_json::to_string(&fact)?;
            let earlier: Option<(i64, i64, String)> = sqlx::query_as(
                "SELECT profile_id, fact_id, request_json FROM fact_submissions WHERE submission_id=?",
            )
            .bind(&submission_id)
            .fetch_optional(&mut *connection)
            .await?;
            if let Some((owner, fact_id, earlier_json)) = earlier {
                if owner != profile_id || earlier_json != request_json {
                    return Err(FactError::Conflict(
                        "Idempotency-Key already used for a different fact".into(),
                    )
                    .into());
                }
                let owned: Option<i64> =
                    sqlx::query_scalar("SELECT id FROM facts WHERE id=? AND profile_id=?")
                        .bind(fact_id)
                        .bind(profile_id)
                        .fetch_optional(&mut *connection)
                        .await?;
                if owned.is_none() {
                    return Err(FactError::Conflict(
                        "This fact was saved and has since been deleted".into(),
                    )
                    .into());
                }
                return load_id(connection, fact_id)
                    .await?
                    .context("saved fact missing");
            }
            let saved = insert_fact(connection, profile_id, fact).await?;
            sqlx::query(
                "INSERT INTO fact_submissions(submission_id, profile_id, fact_id, request_json)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&submission_id)
            .bind(profile_id)
            .bind(saved.id)
            .bind(&request_json)
            .execute(&mut *connection)
            .await?;
            Ok(saved)
        })
    })
    .await
}

pub async fn create_for(pool: &SqlitePool, profile_id: i64, fact: NewFact) -> anyhow::Result<Fact> {
    create_many_for(pool, profile_id, vec![fact])
        .await?
        .pop()
        .context("inserted fact missing")
}

/// Validate the whole import before writing, then commit all facts together.
pub async fn create_many_for(
    pool: &SqlitePool,
    profile_id: i64,
    facts: Vec<NewFact>,
) -> anyhow::Result<Vec<Fact>> {
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
            crate::profiles::require(connection, profile_id).await?;
            let mut saved = Vec::with_capacity(facts.len());
            for fact in facts {
                saved.push(insert_fact(connection, profile_id, fact).await?);
            }
            Ok(saved)
        })
    })
    .await
}

pub(crate) async fn insert_fact(
    connection: &mut SqliteConnection,
    profile_id: i64,
    mut fact: NewFact,
) -> anyhow::Result<Fact> {
    let canonical = |practice: &NewPractice| match practice.kind {
        PracticeKind::Flashcard => practice.prompt == fact.prompt && practice.answer == fact.answer,
        PracticeKind::MultipleChoice
        | PracticeKind::Cloze
        | PracticeKind::Explain
        | PracticeKind::Apply => false,
    };
    if !fact.practices.iter().any(canonical) {
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
    // An ID past every deleted fact's; NULL, so SQLite chooses, in an empty library.
    let result = sqlx::query!(
        "INSERT INTO facts (id, profile_id, prompt, answer, context, source, tags)
         VALUES ((SELECT max(id) + 1 FROM (
                   SELECT max(id) AS id FROM facts UNION ALL SELECT max(id) FROM deleted_facts
                 )), ?, ?, ?, ?, ?, ?)",
        profile_id,
        fact.prompt,
        fact.answer,
        fact.context,
        fact.source,
        serde_json::to_string(&fact.tags)?
    )
    .execute(&mut *connection)
    .await?;
    let id = result.last_insert_rowid();
    crate::media::attach(connection, profile_id, id, &fact.media).await?;
    for practice in &fact.practices {
        insert_practice(connection, id, practice).await?;
    }
    crate::scheduling::register_fact(connection, id).await?;
    load_id(connection, id)
        .await?
        .context("inserted fact missing")
}

pub async fn add_practices_for(
    pool: &SqlitePool,
    profile_id: i64,
    id: i64,
    practices: Vec<NewPractice>,
) -> anyhow::Result<Fact> {
    ensure!(!practices.is_empty(), "provide at least one practice");
    validate_practices(&practices)?;
    crate::db::write_transaction(pool, move |connection| {
        let practices = practices.clone();
        Box::pin(async move {
            crate::profiles::require_fact(connection, profile_id, id).await?;
            ensure!(
                load_id(connection, id).await?.is_some(),
                "fact {id} does not exist"
            );
            for practice in &practices {
                insert_practice(connection, id, practice).await?;
            }
            crate::scheduling::map_practices(connection, id).await?;
            load_id(connection, id).await?.context("fact missing")
        })
    })
    .await
}

/// Delete a fact with its practices, reviews, schedule and study events, and return what
/// it held. Its pending offers are withdrawn, and media no other question uses is removed.
/// The parts of a fact a person can edit.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FactFields {
    pub prompt: String,
    pub answer: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// An edit, with the fields as the editor last saw them. A field the editor changed that
/// was also changed meanwhile, to something else, is a conflict; edits to different
/// fields both apply.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FactEdit {
    pub base: FactFields,
    pub fields: FactFields,
}

/// One field of an edit: the editor's value unless they left it alone, refusing a field
/// someone else changed meanwhile.
fn merge<T: PartialEq>(
    name: &'static str,
    base: T,
    edited: T,
    current: T,
    conflicts: &mut Vec<&'static str>,
) -> T {
    if edited == base || edited == current {
        current
    } else if current == base {
        edited
    } else {
        conflicts.push(name);
        current
    }
}

/// Applies an edit. Changing the question or answer makes a new revision of the fact's
/// recall target with a fresh schedule: performance on the old wording is not evidence
/// about the new one. Earlier events stay with the retired revision.
pub async fn edit_for(
    pool: &SqlitePool,
    profile_id: i64,
    id: i64,
    edit: FactEdit,
) -> anyhow::Result<Fact> {
    crate::db::write_transaction(pool, move |connection| {
        let edit = edit.clone();
        Box::pin(async move {
            crate::profiles::require_fact(connection, profile_id, id).await?;
            let current = load_id(connection, id).await?.context("fact missing")?;
            let FactEdit { base, fields } = edit;
            let FactFields {
                prompt: base_prompt,
                answer: base_answer,
                context: base_context,
                source: base_source,
                tags: base_tags,
            } = base;
            let FactFields {
                prompt,
                answer,
                context,
                source,
                tags,
            } = fields;
            let Fact {
                id: _,
                prompt: current_prompt,
                answer: current_answer,
                context: current_context,
                source: current_source,
                tags: current_tags,
                created_at: _,
                practices: _,
                shared_by: _,
                media: _,
            } = current;
            let mut conflicts = Vec::new();
            let merged = NewFact {
                prompt: merge(
                    "question",
                    base_prompt,
                    prompt,
                    current_prompt.clone(),
                    &mut conflicts,
                ),
                answer: merge(
                    "answer",
                    base_answer,
                    answer,
                    current_answer.clone(),
                    &mut conflicts,
                ),
                context: merge(
                    "context",
                    base_context,
                    context,
                    current_context,
                    &mut conflicts,
                ),
                source: merge(
                    "source",
                    base_source,
                    source,
                    current_source,
                    &mut conflicts,
                ),
                tags: merge("tags", base_tags, tags, current_tags, &mut conflicts),
                practices: vec![],
                media: vec![],
            };
            if !conflicts.is_empty() {
                return Err(FactError::Conflict(format!(
                    "The {} changed elsewhere since you opened this fact",
                    conflicts.join(" and ")
                ))
                .into());
            }
            validate_new_fact(&merged).map_err(|error| FactError::Invalid(error.to_string()))?;
            let NewFact {
                prompt,
                answer,
                context,
                source,
                tags,
                practices: _,
                media: _,
            } = merged;
            sqlx::query!(
                "UPDATE facts SET prompt=?, answer=?, context=?, source=?, tags=? WHERE id=?",
                prompt,
                answer,
                context,
                source,
                serde_json::to_string(&tags)?,
                id
            )
            .execute(&mut *connection)
            .await?;
            if prompt != current_prompt || answer != current_answer {
                // The canonical flashcard follows the fact; generated variants keep their text.
                let updated = sqlx::query!(
                    "UPDATE practices SET prompt=?, answer=?
                     WHERE fact_id=? AND kind='flashcard' AND prompt=? AND answer=?",
                    prompt,
                    answer,
                    id,
                    current_prompt,
                    current_answer
                )
                .execute(&mut *connection)
                .await?;
                if updated.rows_affected() == 0 {
                    insert_practice(
                        connection,
                        id,
                        &NewPractice {
                            kind: PracticeKind::Flashcard,
                            prompt: prompt.clone(),
                            answer: answer.clone(),
                            choices: vec![],
                            explanation: String::new(),
                        },
                    )
                    .await?;
                }
                crate::scheduling::revise_target(connection, id).await?;
            }
            load_id(connection, id)
                .await?
                .context("edited fact missing")
        })
    })
    .await
}

pub async fn delete_for(pool: &SqlitePool, profile_id: i64, id: i64) -> anyhow::Result<Fact> {
    crate::db::write_transaction(pool, move |connection| {
        Box::pin(async move {
            crate::profiles::require_fact(connection, profile_id, id).await?;
            let fact = load_id(connection, id).await?.context("fact missing")?;
            crate::sharing::withdraw_pending(connection, profile_id, id).await?;
            crate::scheduling::forget_fact(connection, id).await?;
            crate::media::detach(connection, id).await?;
            // An accepted offer names its copy until the copy is gone, so that reference
            // is checked at commit rather than as the fact is deleted.
            sqlx::query("PRAGMA defer_foreign_keys = ON")
                .execute(&mut *connection)
                .await?;
            sqlx::query!("DELETE FROM facts WHERE id = ?", id)
                .execute(&mut *connection)
                .await?;
            sqlx::query!("INSERT INTO deleted_facts (id) VALUES (?)", id)
                .execute(&mut *connection)
                .await?;
            crate::sharing::forget_copy(connection, id).await?;
            Ok(fact)
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

impl From<PracticeKind> for ReviewMethod {
    fn from(kind: PracticeKind) -> Self {
        match kind {
            PracticeKind::Flashcard => Self::Flashcard,
            PracticeKind::MultipleChoice => Self::MultipleChoice,
            PracticeKind::Cloze => Self::Cloze,
            PracticeKind::Explain => Self::Explain,
            PracticeKind::Apply => Self::Apply,
        }
    }
}

impl ReviewMethod {
    /// Unaided recall of the fact's answer: the only evidence the recall schedule uses.
    pub(crate) const fn assesses_recall(self) -> bool {
        match self {
            Self::Flashcard | Self::Cloze => true,
            Self::MultipleChoice | Self::Explain | Self::Apply | Self::Matching => false,
        }
    }

    /// Matching and the canonical flashcard are recorded against the fact rather than one
    /// of its practice variants.
    pub(crate) const fn recorded_without_practice(self) -> bool {
        match self {
            Self::Flashcard | Self::Matching => true,
            Self::MultipleChoice | Self::Cloze | Self::Explain | Self::Apply => false,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
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
    pub(crate) fn as_str(self) -> &'static str {
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

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
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
                ReviewMethod::from(practice.kind) == review.method,
                "method does not match practice"
            );
        }
        None => ensure!(
            review.method.recorded_without_practice(),
            "this method requires a practice_id"
        ),
    }
    Ok(())
}

pub async fn record_review_for(
    pool: &SqlitePool,
    profile_id: i64,
    review: NewReview,
) -> anyhow::Result<Review> {
    crate::db::write_transaction(pool, move |connection| {
        let review = review.clone();
        Box::pin(async move {
            crate::profiles::require_fact(connection, profile_id, review.fact_id).await?;
            let fact = load_id(connection, review.fact_id)
                .await?
                .context("fact does not exist")?;
            if let Some(practice_id) = review.practice_id {
                let owned: bool = sqlx::query_scalar(
                    "SELECT EXISTS(
                       SELECT 1 FROM practices p JOIN facts f ON f.id=p.fact_id
                       WHERE p.id=? AND f.profile_id=?
                     )",
                )
                .bind(practice_id)
                .bind(profile_id)
                .fetch_one(&mut *connection)
                .await?;
                if !owned {
                    return Err(crate::scheduling::StudyError::Missing("Practice not found".into()).into());
                }
            }
            validate_review(&review, &fact)
                .map_err(|error| crate::scheduling::StudyError::Invalid(error.to_string()))?;
            let saved = sqlx::query!(
                "INSERT INTO reviews (profile_id, fact_id, practice_id, method, outcome, response_ms)
                 VALUES (?, ?, ?, ?, ?, ?)
                 RETURNING id AS \"id!\", reviewed_at",
                profile_id,
                review.fact_id,
                review.practice_id,
                review.method.as_str(),
                review.outcome.as_str(),
                review.response_ms
            )
            .fetch_one(&mut *connection)
            .await?;

            let NewReview {
                fact_id,
                practice_id,
                method,
                outcome,
                response_ms,
            } = review;
            let review = Review {
                id: saved.id,
                fact_id,
                practice_id,
                method,
                outcome,
                response_ms,
                reviewed_at: saved.reviewed_at,
            };
            crate::scheduling::record_supplementary(connection, profile_id, &review).await?;
            Ok(review)
        })
    })
    .await
}

pub async fn reviews_for(
    pool: &SqlitePool,
    profile_id: i64,
    fact_id: i64,
) -> anyhow::Result<crate::paging::ReviewPage> {
    crate::paging::reviews_for(
        pool,
        profile_id,
        fact_id,
        &crate::paging::ReviewQuery::default(),
    )
    .await
}

// Compatibility calls are deliberately limited to a one-profile household.
pub async fn get(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Fact>> {
    get_for(pool, crate::profiles::sole(pool).await?.id, id).await
}

pub async fn list(pool: &SqlitePool) -> anyhow::Result<crate::paging::FactPage> {
    list_for(pool, crate::profiles::sole(pool).await?.id).await
}

pub async fn create(pool: &SqlitePool, fact: NewFact) -> anyhow::Result<Fact> {
    create_for(pool, crate::profiles::sole(pool).await?.id, fact).await
}

pub async fn create_many(pool: &SqlitePool, facts: Vec<NewFact>) -> anyhow::Result<Vec<Fact>> {
    create_many_for(pool, crate::profiles::sole(pool).await?.id, facts).await
}

pub async fn add_practices(
    pool: &SqlitePool,
    id: i64,
    practices: Vec<NewPractice>,
) -> anyhow::Result<Fact> {
    add_practices_for(pool, crate::profiles::sole(pool).await?.id, id, practices).await
}

pub async fn record_review(pool: &SqlitePool, review: NewReview) -> anyhow::Result<Review> {
    record_review_for(pool, crate::profiles::sole(pool).await?.id, review).await
}

pub async fn reviews(pool: &SqlitePool, id: i64) -> anyhow::Result<crate::paging::ReviewPage> {
    reviews_for(pool, crate::profiles::sole(pool).await?.id, id).await
}
