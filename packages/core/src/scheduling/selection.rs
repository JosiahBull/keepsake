use anyhow::Context;
use sqlx::SqlitePool;

use super::{
    models::{conflict, invalid},
    storage::*,
    *,
};

pub async fn queue_for(
    pool: &SqlitePool,
    profile_id: i64,
    query: &QueueQuery,
) -> anyhow::Result<StudyQueue> {
    queue_at_for(pool, profile_id, query, now()).await
}

pub async fn queue_at_for(
    pool: &SqlitePool,
    profile_id: i64,
    query: &QueueQuery,
    now: i64,
) -> anyhow::Result<StudyQueue> {
    query.validate()?;
    invalid(now >= 0, "timestamp must be nonnegative")?;
    let mut transaction = pool.begin().await?;
    crate::profiles::require(&mut transaction, profile_id).await?;
    let settings = load_settings(&mut transaction, profile_id).await?;
    let (start, end) = day_bounds(now, &settings.timezone)?;
    let introduced = introduced_today(&mut transaction, profile_id, start, end).await?;
    let spent_ms = sqlx::query_scalar!(
        "SELECT coalesce(sum(response_ms), 0) AS \"spent!: i64\" FROM study_events
         WHERE profile_id=? AND occurred_at >= ? AND occurred_at < ?
           AND (kind <> 'attempt' OR json_extract(request_json, '$.feedback_event_id') IS NULL)",
        profile_id,
        start,
        end
    )
    .fetch_one(&mut *transaction)
    .await?;
    let total_due = sqlx::query_scalar!(
        "SELECT count(*) AS \"count!: i64\" FROM schedule_states s JOIN learning_targets t ON t.id=s.target_id
         WHERE t.fact_id IN (SELECT id FROM facts WHERE profile_id=?) AND t.active=1 AND s.phase <> 'new'
           AND max(coalesce(s.due_at, 0), coalesce(s.blocked_until, 0)) <= ?", profile_id, now
    ).fetch_one(&mut *transaction).await?;
    let mut ids = sqlx::query_scalar!(
        "SELECT s.target_id FROM schedule_states s JOIN learning_targets t ON t.id=s.target_id
         WHERE t.fact_id IN (SELECT id FROM facts WHERE profile_id=?) AND t.active=1 AND s.phase <> 'new'
           AND max(coalesce(s.due_at, 0), coalesce(s.blocked_until, 0)) <= ?
         ORDER BY CASE WHEN s.phase IN ('learning', 'relearning') THEN 0 ELSE 1 END,
           coalesce(s.due_at, s.blocked_until, 0) / 3600, random() LIMIT ?",
        profile_id, now,
        query.limit
    )
    .fetch_all(&mut *transaction)
    .await?;

    let new_remaining = if total_due > 0 {
        0
    } else {
        (settings.new_limit - introduced).max(0)
    };
    let new_slots = new_remaining.min(query.limit - ids.len() as i64);
    if new_slots > 0 {
        // Stable intake prevents reloads/tabs from showing a different five new facts.
        ids.extend(
            sqlx::query_scalar!(
            "SELECT s.target_id FROM schedule_states s JOIN learning_targets t ON t.id=s.target_id
             WHERE t.fact_id IN (SELECT id FROM facts WHERE profile_id=?) AND t.active=1 AND s.phase='new' AND coalesce(s.blocked_until, 0) <= ?
             ORDER BY s.target_id LIMIT ?", profile_id, now, new_slots
        )
            .fetch_all(&mut *transaction)
            .await?,
        );
    }

    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        let schedule = load_schedule(&mut transaction, id).await?;
        let fact = crate::facts::load_id(&mut transaction, schedule.fact_id)
            .await?
            .context("scheduled fact missing")?;
        validate_content(&mut transaction, &schedule, &fact).await?;
        let practice_id = sqlx::query_scalar!(
            "SELECT practice_id FROM target_practices WHERE target_id=? AND qualifying=1
             ORDER BY random() LIMIT 1",
            id
        )
        .fetch_one(&mut *transaction)
        .await?;
        let practice = fact
            .practices
            .into_iter()
            .find(|practice| practice.id == practice_id)
            .context("scheduled practice missing")?;
        let transitions = transitions(&schedule, &settings, now)?;
        items.push(StudyQuestion {
            fact: crate::paging::FactSummary {
                shared_by: fact.shared_by,
                id: fact.id,
                prompt: fact.prompt,
                answer: fact.answer,
                context: fact.context,
                source: fact.source,
                tags: fact.tags,
                created_at: fact.created_at,
            },
            practice,
            target_id: id,
            content_revision: schedule.content_revision,
            schedule_version: schedule.version,
            intervals: RatingIntervals {
                again: RETRY_SECONDS,
                hard: transitions.hard.interval_seconds,
                good: transitions.good.interval_seconds,
                easy: transitions.easy.interval_seconds,
            },
        });
    }
    let mut next_due_at = sqlx::query_scalar!(
        "SELECT min(max(coalesce(s.due_at, 0), coalesce(s.blocked_until, 0))) AS \"due: i64\"
         FROM schedule_states s JOIN learning_targets t ON t.id=s.target_id
         WHERE t.fact_id IN (SELECT id FROM facts WHERE profile_id=?) AND t.active=1 AND s.phase <> 'new'",
        profile_id
    )
    .fetch_one(&mut *transaction)
    .await?;
    let next_new = sqlx::query_scalar!(
        "SELECT min(coalesce(s.blocked_until, 0)) AS \"due: i64\"
         FROM schedule_states s JOIN learning_targets t ON t.id=s.target_id WHERE t.fact_id IN (SELECT id FROM facts WHERE profile_id=?) AND t.active=1 AND s.phase='new'", profile_id
    ).fetch_one(&mut *transaction).await?;
    if let Some(next_new) = next_new {
        // A zero intake limit intentionally disables new items; do not promise tomorrow.
        if settings.new_limit > 0 && total_due == 0 {
            let intake_time = if introduced >= settings.new_limit {
                end.max(next_new)
            } else {
                now.max(next_new)
            };
            next_due_at = Some(next_due_at.map_or(intake_time, |due| due.min(intake_time)));
        }
    }
    transaction.commit().await?;
    let spent_seconds = spent_ms / 1000;
    Ok(StudyQueue {
        items,
        total_due,
        new_remaining,
        next_due_at,
        timezone: settings.timezone,
        budget_seconds: settings.daily_budget_seconds,
        spent_seconds,
        budget_reached: spent_seconds >= settings.daily_budget_seconds,
    })
}

pub(super) async fn validate_content(
    connection: &mut sqlx::SqliteConnection,
    schedule: &Schedule,
    fact: &crate::facts::Fact,
) -> anyhow::Result<()> {
    let target = sqlx::query!(
        "SELECT prompt, answer, content_revision, active FROM learning_targets WHERE id=?",
        schedule.target_id
    )
    .fetch_one(connection)
    .await?;
    conflict(
        target.active == 1 && target.content_revision == schedule.content_revision,
        "This learning target changed. Refresh practice.",
    )?;
    conflict(
        target.prompt == fact.prompt && target.answer == fact.answer,
        "The fact's answer changed. Create a new target revision before studying it.",
    )
}

pub async fn queue(pool: &SqlitePool, query: &QueueQuery) -> anyhow::Result<StudyQueue> {
    queue_for(pool, crate::profiles::sole(pool).await?.id, query).await
}

pub async fn queue_at(
    pool: &SqlitePool,
    query: &QueueQuery,
    now: i64,
) -> anyhow::Result<StudyQueue> {
    queue_at_for(pool, crate::profiles::sole(pool).await?.id, query, now).await
}
