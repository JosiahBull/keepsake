//! Bounded library pages and practice samples, independent of the displayed library page.
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::{IntoParams, ToSchema};

use crate::facts::{FactRow, Practice, PracticeRow, Review};

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct FactSummary {
    pub id: i64,
    pub prompt: String,
    pub answer: String,
    pub context: String,
    pub source: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

impl TryFrom<FactRow> for FactSummary {
    type Error = anyhow::Error;
    fn try_from(row: FactRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.id,
            prompt: row.prompt,
            answer: row.answer,
            context: row.context,
            source: row.source,
            tags: serde_json::from_str(&row.tags)?,
            created_at: row.created_at,
        })
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(default, deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct FactQuery {
    /// Page size, from 1 to 100 (default 25).
    pub limit: i64,
    /// Return facts older than this ID; omit for the newest page.
    pub before_id: Option<i64>,
    /// Literal substring search across question, answer, context, and tags.
    pub search: String,
}

impl Default for FactQuery {
    fn default() -> Self {
        Self {
            limit: 25,
            before_id: None,
            search: String::new(),
        }
    }
}

impl FactQuery {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_limit(self.limit, 100)?;
        ensure!(
            self.before_id.is_none_or(|id| id > 0),
            "before_id must be positive"
        );
        ensure!(
            self.search.len() <= 2000 && !self.search.contains('\0'),
            "search must contain at most 2000 bytes and no nulls"
        );
        Ok(())
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(default, deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct ReviewQuery {
    /// Page size, from 1 to 100 (default 50).
    pub limit: i64,
    /// Return reviews newer than this ID.
    pub after_id: Option<i64>,
}

impl Default for ReviewQuery {
    fn default() -> Self {
        Self {
            limit: 50,
            after_id: None,
        }
    }
}

impl ReviewQuery {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_limit(self.limit, 100)?;
        ensure!(
            self.after_id.is_none_or(|id| id > 0),
            "after_id must be positive"
        );
        Ok(())
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(default, deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct PracticeQuery {
    /// Number of random prompts, from 1 to 50 (default 20).
    pub limit: i64,
}

impl Default for PracticeQuery {
    fn default() -> Self {
        Self { limit: 20 }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(default, deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct SampleQuery {
    /// Number of distinct matching pairs, from 1 to 4 (default 4).
    pub limit: i64,
}

impl Default for SampleQuery {
    fn default() -> Self {
        Self { limit: 4 }
    }
}

impl PracticeQuery {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_limit(self.limit, 50)
    }
}

impl SampleQuery {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_limit(self.limit, 4)
    }
}

fn validate_limit(limit: i64, maximum: i64) -> anyhow::Result<()> {
    ensure!(
        (1..=maximum).contains(&limit),
        "limit must be between 1 and {maximum}"
    );
    Ok(())
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct FactPage {
    pub items: Vec<FactSummary>,
    pub next_cursor: Option<i64>,
    pub total: i64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ReviewPage {
    pub items: Vec<Review>,
    pub next_cursor: Option<i64>,
    pub total: i64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct PracticeQuestion {
    pub fact: FactSummary,
    pub practice: Practice,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct PracticeSession {
    pub items: Vec<PracticeQuestion>,
    pub total: i64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct FactSample {
    pub items: Vec<FactSummary>,
}

fn search_pattern(search: &str) -> String {
    // Match the SQL fold exactly; other Unicode letters remain case-sensitive.
    // Treat wildcard characters as text, including the escape character itself.
    format!(
        "%{}%",
        search
            .to_ascii_lowercase()
            .replace('Ā', "ā")
            .replace('Ē', "ē")
            .replace('Ī', "ī")
            .replace('Ō', "ō")
            .replace('Ū', "ū")
            .replace('#', "##")
            .replace('%', "#%")
            .replace('_', "#_")
    )
}

pub async fn facts(pool: &SqlitePool, query: &FactQuery) -> anyhow::Result<FactPage> {
    query.validate()?;
    let pattern = search_pattern(&query.search);
    let fetch_limit = query.limit + 1;
    let mut connection = pool.acquire().await?;
    // JSON's table-valued function applies the same fold to each searchable field
    // without joining unrelated field boundaries into a possible substring match.
    let total = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!: i64\" FROM facts
         WHERE EXISTS (
           SELECT 1 FROM json_each(json_array(prompt, answer, context, tags)) AS field
           WHERE lower(replace(replace(replace(replace(replace(
             field.value, 'Ā', 'ā'), 'Ē', 'ē'), 'Ī', 'ī'), 'Ō', 'ō'), 'Ū', 'ū'))
             LIKE ? ESCAPE '#'
         )",
        pattern
    )
    .fetch_one(&mut *connection)
    .await?;
    let mut rows = sqlx::query_as!(
        FactRow,
        "SELECT id AS \"id!\", prompt, answer, context, source, tags, created_at FROM facts
         WHERE (? IS NULL OR id < ?) AND EXISTS (
           SELECT 1 FROM json_each(json_array(prompt, answer, context, tags)) AS field
           WHERE lower(replace(replace(replace(replace(replace(
             field.value, 'Ā', 'ā'), 'Ē', 'ē'), 'Ī', 'ī'), 'Ō', 'ō'), 'Ū', 'ū'))
             LIKE ? ESCAPE '#'
         )
         ORDER BY id DESC LIMIT ?",
        query.before_id,
        query.before_id,
        pattern,
        fetch_limit
    )
    .fetch_all(&mut *connection)
    .await?;
    let has_more = rows.len() > query.limit as usize;
    rows.truncate(query.limit as usize);
    let next_cursor = has_more.then(|| rows.last().expect("nonempty page").id);
    Ok(FactPage {
        items: rows
            .into_iter()
            .map(FactSummary::try_from)
            .collect::<anyhow::Result<_>>()?,
        next_cursor,
        total,
    })
}

#[derive(sqlx::FromRow)]
struct ReviewRow {
    id: i64,
    fact_id: i64,
    practice_id: Option<i64>,
    method: String,
    outcome: String,
    response_ms: i64,
    reviewed_at: String,
}

pub async fn reviews(
    pool: &SqlitePool,
    fact_id: i64,
    query: &ReviewQuery,
) -> anyhow::Result<ReviewPage> {
    query.validate()?;
    let fetch_limit = query.limit + 1;
    let total = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!: i64\" FROM reviews WHERE fact_id = ?",
        fact_id
    )
    .fetch_one(pool)
    .await?;
    let mut rows = sqlx::query_as!(ReviewRow,
        "SELECT id AS \"id!\", fact_id, practice_id, method, outcome, response_ms, reviewed_at FROM reviews
         WHERE fact_id = ? AND (? IS NULL OR id > ?) ORDER BY id LIMIT ?",
        fact_id, query.after_id, query.after_id, fetch_limit
    ).fetch_all(pool).await?;
    let has_more = rows.len() > query.limit as usize;
    rows.truncate(query.limit as usize);
    let next_cursor = has_more.then(|| rows.last().expect("nonempty page").id);
    let items = rows
        .into_iter()
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
        .collect::<anyhow::Result<_>>()?;
    Ok(ReviewPage {
        items,
        next_cursor,
        total,
    })
}

pub async fn practice(pool: &SqlitePool, query: &PracticeQuery) -> anyhow::Result<PracticeSession> {
    query.validate()?;
    let mut connection = pool.acquire().await?;
    let total = sqlx::query_scalar!("SELECT count(*) AS \"count!: i64\" FROM practices")
        .fetch_one(&mut *connection)
        .await?;
    let practices = sqlx::query_as!(PracticeRow,
        "SELECT id AS \"id!\", fact_id, kind, prompt, answer, choices, explanation FROM practices ORDER BY random() LIMIT ?",
        query.limit
    ).fetch_all(&mut *connection).await?;
    let mut items = Vec::with_capacity(practices.len());
    for practice in practices {
        let fact = sqlx::query_as!(FactRow,
            "SELECT id AS \"id!\", prompt, answer, context, source, tags, created_at FROM facts WHERE id = ?",
            practice.fact_id
        ).fetch_one(&mut *connection).await?;
        items.push(PracticeQuestion {
            fact: fact.try_into()?,
            practice: practice.try_into()?,
        });
    }
    Ok(PracticeSession { items, total })
}

pub async fn sample(pool: &SqlitePool, query: &SampleQuery) -> anyhow::Result<FactSample> {
    query.validate()?;
    // Ambiguous questions cannot form a fair matching pair. Pick one representative per
    // answer only after removing questions that have conflicting answers.
    let rows = sqlx::query_as!(
        FactRow,
        "WITH eligible AS (
           SELECT f.* FROM facts f WHERE NOT EXISTS (
             SELECT 1 FROM facts other
             WHERE lower(trim(other.prompt)) = lower(trim(f.prompt))
               AND lower(trim(other.answer)) <> lower(trim(f.answer))
           )
         ), representatives AS (
           SELECT min(id) AS id FROM eligible GROUP BY lower(trim(answer))
         )
         SELECT id AS \"id!\", prompt, answer, context, source, tags, created_at
         FROM facts WHERE id IN (SELECT id FROM representatives) ORDER BY random() LIMIT 100"
    )
    .fetch_all(pool)
    .await?;
    let mut prompts = std::collections::HashSet::new();
    let mut answers = std::collections::HashSet::new();
    let mut items = Vec::new();
    for row in rows {
        let prompt = row.prompt.trim().to_lowercase();
        let answer = row.answer.trim().to_lowercase();
        if prompts.contains(&prompt) || answers.contains(&answer) {
            continue;
        }
        prompts.insert(prompt);
        answers.insert(answer);
        items.push(row.try_into()?);
        if items.len() == query.limit as usize {
            break;
        }
    }
    Ok(FactSample { items })
}

pub async fn fact_exists(pool: &SqlitePool, id: i64) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM facts WHERE id = ?) AS \"exists!: bool\"",
        id
    )
    .fetch_one(pool)
    .await?)
}
