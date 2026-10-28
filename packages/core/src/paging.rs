//! Bounded library pages and practice samples, independent of the displayed library page.
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::{IntoParams, ToSchema};

use crate::facts::{FactRow, Practice, PracticeRow, Review};
use crate::media::QuestionMedia;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct FactSummary {
    pub id: i64,
    pub prompt: String,
    pub answer: String,
    pub context: String,
    pub source: String,
    pub tags: Vec<String>,
    pub created_at: String,
    #[serde(default)]
    pub shared_by: Option<String>,
    /// Images and audio clips asked with the question, in display order.
    pub media: Vec<QuestionMedia>,
}

impl TryFrom<FactRow> for FactSummary {
    type Error = anyhow::Error;

    fn try_from(row: FactRow) -> anyhow::Result<Self> {
        let FactRow {
            id,
            prompt,
            answer,
            context,
            source,
            tags,
            created_at,
        } = row;
        Ok(Self {
            shared_by: None,
            media: Vec::new(),
            id,
            prompt,
            answer,
            context,
            source,
            tags: serde_json::from_str(&tags)?,
            created_at,
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
    /// Number of related pairs from one topic bank, from 1 to 4 (default 4).
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

pub async fn facts_for(
    pool: &SqlitePool,
    profile_id: i64,
    query: &FactQuery,
) -> anyhow::Result<FactPage> {
    query.validate()?;
    crate::profiles::require(&mut *pool.acquire().await?, profile_id).await?;

    let pattern = search_pattern(&query.search);
    let fetch_limit = query.limit + 1;
    let mut connection = pool.acquire().await?;

    // JSON's table-valued function applies the same fold to each searchable field
    // without joining unrelated field boundaries into a possible substring match.
    let total = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!: i64\" FROM facts
         WHERE profile_id = ? AND EXISTS (
           SELECT 1 FROM json_each(json_array(prompt, answer, context, tags)) AS field
           WHERE lower(replace(replace(replace(replace(replace(
             field.value, 'Ā', 'ā'), 'Ē', 'ē'), 'Ī', 'ī'), 'Ō', 'ō'), 'Ū', 'ū'))
             LIKE ? ESCAPE '#'
         )",
        profile_id,
        pattern
    )
    .fetch_one(&mut *connection)
    .await?;
    let mut rows = sqlx::query_as!(
        FactRow,
        "SELECT id AS \"id!\", prompt, answer, context, source, tags, created_at FROM facts
         WHERE profile_id = ? AND (? IS NULL OR id < ?) AND EXISTS (
           SELECT 1 FROM json_each(json_array(prompt, answer, context, tags)) AS field
           WHERE lower(replace(replace(replace(replace(replace(
             field.value, 'Ā', 'ā'), 'Ē', 'ē'), 'Ī', 'ī'), 'Ō', 'ō'), 'Ū', 'ū'))
             LIKE ? ESCAPE '#'
         )
         ORDER BY id DESC LIMIT ?",
        profile_id,
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

    let mut items = rows
        .into_iter()
        .map(FactSummary::try_from)
        .collect::<anyhow::Result<Vec<_>>>()?;
    for fact in &mut items {
        enrich_summary(&mut connection, fact).await?;
    }
    Ok(FactPage {
        items,
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

pub async fn reviews_for(
    pool: &SqlitePool,
    profile_id: i64,
    fact_id: i64,
    query: &ReviewQuery,
) -> anyhow::Result<ReviewPage> {
    query.validate()?;
    crate::profiles::require(&mut *pool.acquire().await?, profile_id).await?;

    crate::profiles::require_fact(&mut *pool.acquire().await?, profile_id, fact_id).await?;
    let fetch_limit = query.limit + 1;
    let total = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!: i64\" FROM reviews WHERE fact_id = ?",
        fact_id
    )
    .fetch_one(pool)
    .await?;
    let mut rows = sqlx::query_as!(
        ReviewRow,
        "SELECT id AS \"id!\", fact_id, practice_id, method, outcome, response_ms, reviewed_at FROM reviews
         WHERE fact_id = ? AND (? IS NULL OR id > ?) ORDER BY id LIMIT ?",
        fact_id,
        query.after_id,
        query.after_id,
        fetch_limit
    )
    .fetch_all(pool)
    .await?;

    let has_more = rows.len() > query.limit as usize;
    rows.truncate(query.limit as usize);
    let next_cursor = has_more.then(|| rows.last().expect("nonempty page").id);

    let items = rows
        .into_iter()
        .map(|row| {
            let ReviewRow {
                id,
                fact_id,
                practice_id,
                method,
                outcome,
                response_ms,
                reviewed_at,
            } = row;
            Ok(Review {
                id,
                fact_id,
                practice_id,
                method: serde_json::from_value(serde_json::Value::String(method))?,
                outcome: serde_json::from_value(serde_json::Value::String(outcome))?,
                response_ms,
                reviewed_at,
            })
        })
        .collect::<anyhow::Result<_>>()?;

    Ok(ReviewPage {
        items,
        next_cursor,
        total,
    })
}

pub async fn practice_for(
    pool: &SqlitePool,
    profile_id: i64,
    query: &PracticeQuery,
) -> anyhow::Result<PracticeSession> {
    query.validate()?;
    crate::profiles::require(&mut *pool.acquire().await?, profile_id).await?;

    let mut connection = pool.acquire().await?;
    let total = sqlx::query_scalar!("SELECT count(*) AS \"count!: i64\" FROM practices WHERE fact_id IN (SELECT id FROM facts WHERE profile_id=?)", profile_id)
        .fetch_one(&mut *connection)
        .await?;
    let practices = sqlx::query_as!(
        PracticeRow,
        "SELECT id AS \"id!\", fact_id, kind, prompt, answer, choices, explanation FROM practices WHERE fact_id IN (SELECT id FROM facts WHERE profile_id=?) ORDER BY random() LIMIT ?",
        profile_id,
        query.limit
    )
    .fetch_all(&mut *connection)
    .await?;

    let mut items = Vec::with_capacity(practices.len());

    for practice in practices {
        let fact = sqlx::query_as!(
            FactRow,
            "SELECT id AS \"id!\", prompt, answer, context, source, tags, created_at FROM facts WHERE id = ?",
            practice.fact_id
        )
        .fetch_one(&mut *connection)
        .await?;

        let mut summary = fact.try_into()?;
        enrich_summary(&mut connection, &mut summary).await?;
        items.push(PracticeQuestion {
            fact: summary,
            practice: practice.try_into()?,
        });
    }

    Ok(PracticeSession { items, total })
}

pub async fn sample_for(
    pool: &SqlitePool,
    profile_id: i64,
    query: &SampleQuery,
) -> anyhow::Result<FactSample> {
    crate::profiles::require(&mut *pool.acquire().await?, profile_id).await?;
    crate::matching::sample(pool, profile_id, query).await
}

#[derive(Debug, Deserialize, Serialize, ToSchema, sqlx::FromRow)]
pub struct TagCount {
    pub tag: String,
    pub facts: i64,
}

/// Every tag in this library with how many facts carry it, most used first.
pub async fn tags_for(pool: &SqlitePool, profile_id: i64) -> anyhow::Result<Vec<TagCount>> {
    crate::profiles::require(&mut *pool.acquire().await?, profile_id).await?;
    // A runtime query: SQLx cannot describe a grouped json_each column at compile time.
    Ok(sqlx::query_as(
        "SELECT tag.value AS tag, count(DISTINCT f.id) AS facts
         FROM facts f, json_each(f.tags) tag
         WHERE f.profile_id = ? AND tag.type = 'text'
         GROUP BY tag.value ORDER BY facts DESC, tag.value
         LIMIT 500",
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await?)
}

pub async fn fact_exists_for(pool: &SqlitePool, profile_id: i64, id: i64) -> anyhow::Result<bool> {
    crate::profiles::require(&mut *pool.acquire().await?, profile_id).await?;
    Ok(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM facts WHERE id=? AND profile_id=?)")
            .bind(id)
            .bind(profile_id)
            .fetch_one(pool)
            .await?,
    )
}

pub async fn facts(pool: &SqlitePool, query: &FactQuery) -> anyhow::Result<FactPage> {
    facts_for(pool, crate::profiles::sole(pool).await?.id, query).await
}

pub async fn reviews(
    pool: &SqlitePool,
    fact_id: i64,
    query: &ReviewQuery,
) -> anyhow::Result<ReviewPage> {
    reviews_for(pool, crate::profiles::sole(pool).await?.id, fact_id, query).await
}

pub async fn practice(pool: &SqlitePool, query: &PracticeQuery) -> anyhow::Result<PracticeSession> {
    practice_for(pool, crate::profiles::sole(pool).await?.id, query).await
}

pub async fn sample(pool: &SqlitePool, query: &SampleQuery) -> anyhow::Result<FactSample> {
    sample_for(pool, crate::profiles::sole(pool).await?.id, query).await
}

pub async fn fact_exists(pool: &SqlitePool, id: i64) -> anyhow::Result<bool> {
    fact_exists_for(pool, crate::profiles::sole(pool).await?.id, id).await
}

pub(crate) async fn enrich_summary(
    connection: &mut sqlx::SqliteConnection,
    fact: &mut FactSummary,
) -> anyhow::Result<()> {
    fact.shared_by = sqlx::query_scalar(
        "SELECT p.name FROM fact_shares s JOIN profiles p ON p.id=s.sender_id
         WHERE s.accepted_fact_id=? AND s.status='accepted' LIMIT 1",
    )
    .bind(fact.id)
    .fetch_optional(&mut *connection)
    .await?;
    fact.media = crate::media::for_fact(connection, fact.id).await?;
    Ok(())
}
