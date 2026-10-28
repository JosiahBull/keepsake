//! Frozen, question-only offers; acceptance creates independent owned content.
use anyhow::Context;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqliteConnection, SqlitePool};
use utoipa::{IntoParams, ToSchema};

use crate::{
    facts::{Fact, NewFact, NewPractice, Practice},
    media::{NewQuestionMedia, QuestionMedia},
    scheduling::StudyError,
};

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, FromRow)]
pub struct Share {
    pub id: i64,
    pub sender_id: i64,
    pub recipient_id: i64,
    pub sender_name: String,
    pub recipient_name: String,
    pub source_fact_id: i64,
    pub question: String,
    pub status: String,
    pub accepted_fact_id: Option<i64>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareRequest {
    pub submission_id: String,
    pub recipient_id: i64,
    pub fact_ids: Vec<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareBatch {
    pub share_ids: Vec<i64>,
}

#[derive(Clone, Debug, Deserialize, IntoParams)]
#[serde(default, deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct ShareQuery {
    pub limit: i64,
    pub before_id: Option<i64>,
    pub direction: Option<String>,
    pub status: Option<String>,
}

impl Default for ShareQuery {
    fn default() -> Self {
        Self {
            limit: 25,
            before_id: None,
            direction: None,
            status: None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct SharePage {
    pub items: Vec<Share>,
    pub next_cursor: Option<i64>,
    pub total: i64,
}

fn invalid(condition: bool, message: &str) -> anyhow::Result<()> {
    if !condition {
        return Err(StudyError::Invalid(message.into()).into());
    }
    Ok(())
}

fn validate_ids(ids: &[i64]) -> anyhow::Result<()> {
    invalid(
        (1..=20).contains(&ids.len())
            && ids.iter().all(|id| *id > 0)
            && ids.iter().collect::<std::collections::HashSet<_>>().len() == ids.len(),
        "Provide 1 to 20 distinct positive IDs",
    )
}

fn validate_snapshot(snapshot: &NewFact) -> anyhow::Result<()> {
    // The 32-practice API limit applies per append request, while an existing
    // fact may contain practices collected over several requests.
    let mut fields = snapshot.clone();
    fields.practices.clear();
    crate::facts::validate_new_fact(&fields)?;
    for practice in &snapshot.practices {
        crate::facts::validate_practice(practice)?;
    }
    Ok(())
}

/// This private loader returns metadata only. Every caller verifies the actor's
/// role before returning an offer; snapshots are never part of the public DTO.
async fn load(connection: &mut SqliteConnection, id: i64) -> anyhow::Result<Share> {
    sqlx::query_as!(
        Share,
        "SELECT s.id AS \"id!\", s.sender_id, s.recipient_id,
           a.name AS sender_name, b.name AS recipient_name,
           s.source_fact_id, s.question, s.status, s.accepted_fact_id,
           s.created_at, s.resolved_at
         FROM fact_shares s
         JOIN profiles a ON a.id=s.sender_id
         JOIN profiles b ON b.id=s.recipient_id
         WHERE s.id=?",
        id
    )
    .fetch_optional(connection)
    .await?
    .ok_or_else(|| StudyError::Missing("Share not found".into()).into())
}

async fn snapshot_fact(
    connection: &mut SqliteConnection,
    profile_id: i64,
    fact_id: i64,
) -> anyhow::Result<NewFact> {
    crate::profiles::require_fact(connection, profile_id, fact_id).await?;
    let fact = crate::facts::load_id(connection, fact_id)
        .await?
        .context("owned fact missing")?;
    // Every field is named, so a new one cannot be left out of offers unnoticed.
    let Fact {
        id: _,
        prompt,
        answer,
        context,
        source,
        tags,
        created_at: _,
        practices,
        shared_by: _,
        media,
    } = fact;
    let practice = |practice: Practice| {
        let Practice {
            id: _,
            fact_id: _,
            kind,
            prompt,
            answer,
            choices,
            explanation,
        } = practice;
        NewPractice {
            kind,
            prompt,
            answer,
            choices,
            explanation,
        }
    };
    let attached = |media: QuestionMedia| {
        let QuestionMedia {
            media_id,
            kind: _,
            content_type: _,
            byte_size: _,
            width: _,
            height: _,
            duration_ms: _,
            alt,
            ai_generated,
        } = media;
        NewQuestionMedia {
            media_id,
            alt,
            ai_generated,
        }
    };
    let snapshot = NewFact {
        prompt,
        answer,
        context,
        source,
        tags,
        practices: practices.into_iter().map(practice).collect(),
        media: media.into_iter().map(attached).collect(),
    };
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub async fn send(
    pool: &SqlitePool,
    profile_id: i64,
    input: ShareRequest,
) -> anyhow::Result<Vec<Share>> {
    invalid(
        uuid::Uuid::parse_str(&input.submission_id).is_ok(),
        "submission_id must be a UUID",
    )?;
    validate_ids(&input.fact_ids)?;
    invalid(
        profile_id != input.recipient_id,
        "Choose another profile as recipient",
    )?;

    crate::db::write_transaction(pool, move |connection| {
        let input = input.clone();
        Box::pin(async move {
            crate::profiles::require(connection, profile_id).await?;
            crate::profiles::require(connection, input.recipient_id).await?;
            let request_json = serde_json::to_string(&input)?;

            if let Some(saved) = sqlx::query!(
                "SELECT profile_id, request_json, result_json
                 FROM share_submissions WHERE submission_id=?",
                input.submission_id
            )
            .fetch_optional(&mut *connection)
            .await?
            {
                if saved.profile_id != profile_id {
                    return Err(StudyError::Missing("Share submission not found".into()).into());
                }
                if saved.request_json != request_json {
                    return Err(StudyError::Conflict(
                        "Submission ID already used for a different share".into(),
                    )
                    .into());
                }
                // Preserve offer identity while reflecting its current accepted status.
                let ids: Vec<i64> = serde_json::from_str(&saved.result_json)?;
                let mut shares = Vec::with_capacity(ids.len());
                for id in ids {
                    shares.push(load(connection, id).await?);
                }
                return Ok(shares);
            }

            let mut shares = Vec::with_capacity(input.fact_ids.len());
            for fact_id in &input.fact_ids {
                let snapshot = snapshot_fact(connection, profile_id, *fact_id).await?;
                // Exact canonical content equality avoids fingerprint collisions.
                let snapshot_json = serde_json::to_string(&snapshot)?;
                let existing = sqlx::query_scalar!(
                    "SELECT id AS \"id!\" FROM fact_shares
                     WHERE sender_id=? AND recipient_id=? AND source_fact_id=?
                       AND fingerprint=? AND status='pending'",
                    profile_id,
                    input.recipient_id,
                    fact_id,
                    snapshot_json
                )
                .fetch_optional(&mut *connection)
                .await?;
                let share_id = match existing {
                    Some(id) => id,
                    None => sqlx::query!(
                        "INSERT INTO fact_shares(
                           sender_id, recipient_id, source_fact_id,
                           snapshot_json, fingerprint, question, status
                         ) VALUES(?, ?, ?, ?, ?, ?, 'pending')",
                        profile_id,
                        input.recipient_id,
                        fact_id,
                        snapshot_json,
                        snapshot_json,
                        snapshot.prompt
                    )
                    .execute(&mut *connection)
                    .await?
                    .last_insert_rowid(),
                };
                shares.push(load(connection, share_id).await?);
            }

            let ids = shares.iter().map(|share| share.id).collect::<Vec<_>>();
            let result_json = serde_json::to_string(&ids)?;
            sqlx::query!(
                "INSERT INTO share_submissions(submission_id, profile_id, request_json, result_json)
                 VALUES(?, ?, ?, ?)",
                input.submission_id,
                profile_id,
                request_json,
                result_json
            )
            .execute(connection)
            .await?;
            Ok(shares)
        })
    })
    .await
}

pub async fn page(
    pool: &SqlitePool,
    profile_id: i64,
    query: &ShareQuery,
) -> anyhow::Result<SharePage> {
    invalid(
        (1..=100).contains(&query.limit) && query.before_id.is_none_or(|id| id > 0),
        "Invalid share page limit or cursor",
    )?;
    invalid(
        query
            .direction
            .as_deref()
            .is_none_or(|direction| matches!(direction, "inbox" | "sent")),
        "direction must be inbox or sent",
    )?;
    invalid(
        query.status.as_deref().is_none_or(|status| {
            matches!(status, "pending" | "accepted" | "declined" | "withdrawn")
        }),
        "Invalid share status",
    )?;

    let mut transaction = pool.begin().await?;
    crate::profiles::require(&mut transaction, profile_id).await?;
    let total = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!: i64\" FROM fact_shares
         WHERE (sender_id=? OR recipient_id=?)
           AND (? IS NULL OR (?='inbox' AND recipient_id=?) OR (?='sent' AND sender_id=?))
           AND (? IS NULL OR status=?)",
        profile_id,
        profile_id,
        query.direction,
        query.direction,
        profile_id,
        query.direction,
        profile_id,
        query.status,
        query.status
    )
    .fetch_one(&mut *transaction)
    .await?;

    let fetch_limit = query.limit + 1;
    let mut items = sqlx::query_as!(
        Share,
        "SELECT s.id AS \"id!\", s.sender_id, s.recipient_id,
           a.name AS sender_name, b.name AS recipient_name,
           s.source_fact_id, s.question, s.status, s.accepted_fact_id,
           s.created_at, s.resolved_at
         FROM fact_shares s
         JOIN profiles a ON a.id=s.sender_id
         JOIN profiles b ON b.id=s.recipient_id
         WHERE (s.sender_id=? OR s.recipient_id=?)
           AND (? IS NULL OR (?='inbox' AND s.recipient_id=?) OR (?='sent' AND s.sender_id=?))
           AND (? IS NULL OR s.status=?) AND (? IS NULL OR s.id<?)
         ORDER BY s.id DESC LIMIT ?",
        profile_id,
        profile_id,
        query.direction,
        query.direction,
        profile_id,
        query.direction,
        profile_id,
        query.status,
        query.status,
        query.before_id,
        query.before_id,
        fetch_limit
    )
    .fetch_all(&mut *transaction)
    .await?;

    let has_more = items.len() > query.limit as usize;
    items.truncate(query.limit as usize);
    let next_cursor = has_more.then(|| items.last().expect("nonempty page").id);
    transaction.commit().await?;
    Ok(SharePage {
        items,
        next_cursor,
        total,
    })
}

pub async fn list(pool: &SqlitePool, profile_id: i64) -> anyhow::Result<Vec<Share>> {
    Ok(page(
        pool,
        profile_id,
        &ShareQuery {
            limit: 100,
            ..Default::default()
        },
    )
    .await?
    .items)
}

pub async fn accept(pool: &SqlitePool, profile_id: i64, id: i64) -> anyhow::Result<Share> {
    accept_batch(
        pool,
        profile_id,
        ShareBatch {
            share_ids: vec![id],
        },
    )
    .await?
    .pop()
    .context("accepted share missing")
}

async fn accept_one(
    connection: &mut SqliteConnection,
    profile_id: i64,
    id: i64,
) -> anyhow::Result<Share> {
    let share = load(connection, id).await?;
    if share.recipient_id != profile_id {
        return Err(StudyError::Missing("Share not found".into()).into());
    }
    if share.status == "accepted" {
        return Ok(share);
    }
    if share.status != "pending" {
        return Err(StudyError::Conflict(format!("Share is {}", share.status)).into());
    }

    let json = sqlx::query_scalar!("SELECT snapshot_json FROM fact_shares WHERE id=?", id)
        .fetch_one(&mut *connection)
        .await?;
    let mut snapshot: NewFact = serde_json::from_str(&json)?;
    validate_snapshot(&snapshot)?;
    // The snapshot names the sender's files; the copy owns its own.
    for media in &mut snapshot.media {
        media.media_id =
            crate::media::copy(connection, share.sender_id, media.media_id, profile_id).await?;
    }
    let fact = crate::facts::insert_fact(connection, profile_id, snapshot).await?;

    // Approval is personal: only one canonical recall prompt starts eligible.
    sqlx::query!(
        "UPDATE target_practices SET qualifying=0
         WHERE target_id IN (SELECT id FROM learning_targets WHERE fact_id=?)
           AND practice_id <> (
             SELECT min(id) FROM practices
             WHERE fact_id=? AND kind='flashcard' AND prompt=? AND answer=?
           )",
        fact.id,
        fact.id,
        fact.prompt,
        fact.answer
    )
    .execute(&mut *connection)
    .await?;
    sqlx::query!(
        "UPDATE fact_shares
         SET status='accepted', accepted_fact_id=?, resolved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id=?",
        fact.id,
        id
    )
    .execute(&mut *connection)
    .await?;
    load(connection, id).await
}

pub async fn accept_batch(
    pool: &SqlitePool,
    profile_id: i64,
    input: ShareBatch,
) -> anyhow::Result<Vec<Share>> {
    validate_ids(&input.share_ids)?;
    crate::db::write_transaction(pool, move |connection| {
        let input = input.clone();
        Box::pin(async move {
            crate::profiles::require(connection, profile_id).await?;
            let mut result = Vec::with_capacity(input.share_ids.len());
            for id in input.share_ids {
                result.push(accept_one(connection, profile_id, id).await?);
            }
            Ok(result)
        })
    })
    .await
}

async fn resolve_offer(
    pool: &SqlitePool,
    profile_id: i64,
    id: i64,
    withdraw: bool,
) -> anyhow::Result<Share> {
    crate::db::write_transaction(pool, move |connection| {
        Box::pin(async move {
            let share = load(connection, id).await?;
            let allowed_id = if withdraw {
                share.sender_id
            } else {
                share.recipient_id
            };
            if allowed_id != profile_id {
                return Err(StudyError::Missing("Share not found".into()).into());
            }
            if share.status != "pending" {
                return Ok(share);
            }
            let status = if withdraw { "withdrawn" } else { "declined" };
            sqlx::query!(
                "UPDATE fact_shares SET status=?, resolved_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE id=?",
                status,
                id
            )
            .execute(&mut *connection)
            .await?;
            load(connection, id).await
        })
    })
    .await
}

pub async fn decline(pool: &SqlitePool, profile_id: i64, id: i64) -> anyhow::Result<Share> {
    resolve_offer(pool, profile_id, id, false).await
}

pub async fn withdraw(pool: &SqlitePool, profile_id: i64, id: i64) -> anyhow::Result<Share> {
    resolve_offer(pool, profile_id, id, true).await
}
