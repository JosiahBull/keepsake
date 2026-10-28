use anyhow::Context;
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use sqlx::{SqliteConnection, SqlitePool};

use crate::facts::{PracticeKind, ReviewMethod};

use super::{
    models::{conflict, invalid},
    *,
};

pub(super) async fn load_settings(
    connection: &mut SqliteConnection,
    profile_id: i64,
) -> anyhow::Result<SchedulerSettings> {
    crate::profiles::require(connection, profile_id).await?;
    let json = sqlx::query_scalar!(
        "SELECT value_json FROM scheduler_settings WHERE profile_id = ?",
        profile_id
    )
    .fetch_one(connection)
    .await?;
    let settings: SchedulerSettings = serde_json::from_str(&json)?;
    anyhow::ensure!(
        settings.algorithm == adapter::ALGORITHM
            && settings.parameters == adapter::default_parameters(),
        "unsupported scheduler parameter version"
    );
    Ok(settings)
}

pub async fn settings_for(pool: &SqlitePool, profile_id: i64) -> anyhow::Result<SchedulerSettings> {
    load_settings(&mut *pool.acquire().await?, profile_id).await
}

pub async fn update_settings_for(
    pool: &SqlitePool,
    profile_id: i64,
    update: SettingsUpdate,
) -> anyhow::Result<SchedulerSettings> {
    invalid(
        update.timezone.parse::<Tz>().is_ok(),
        "timezone must be an IANA timezone",
    )?;
    invalid(
        (0..=100).contains(&update.new_limit),
        "new_limit must be between 0 and 100",
    )?;
    invalid(
        (60..=14_400).contains(&update.daily_budget_seconds),
        "daily_budget_seconds must be between 60 and 14400",
    )?;
    crate::db::write_transaction(pool, move |connection| {
        let update = update.clone();
        Box::pin(async move {
            let mut saved = load_settings(connection, profile_id).await?;
            conflict(
                saved.version == update.expected_version,
                "Settings changed. Refresh and try again.",
            )?;
            saved.timezone = update.timezone;
            saved.new_limit = update.new_limit;
            saved.daily_budget_seconds = update.daily_budget_seconds;
            saved.version += 1;
            let json = serde_json::to_string(&saved)?;
            sqlx::query!(
                "UPDATE scheduler_settings SET value_json = ? WHERE profile_id = ?",
                json,
                profile_id
            )
            .execute(connection)
            .await?;
            Ok(saved)
        })
    })
    .await
}

pub(super) async fn load_schedule(
    connection: &mut SqliteConnection,
    target_id: i64,
) -> anyhow::Result<Schedule> {
    let json = sqlx::query_scalar!(
        "SELECT state_json FROM schedule_states WHERE target_id = ?",
        target_id
    )
    .fetch_optional(connection)
    .await?
    .ok_or_else(|| StudyError::Missing("Learning target not found".into()))?;
    Ok(serde_json::from_str(&json)?)
}

pub async fn target_for(
    pool: &SqlitePool,
    profile_id: i64,
    fact_id: i64,
) -> anyhow::Result<Option<Schedule>> {
    let mut connection = pool.acquire().await?;
    crate::profiles::require_fact(&mut connection, profile_id, fact_id).await?;
    let id = sqlx::query_scalar!(
        "SELECT id FROM learning_targets WHERE fact_id = ? AND active = 1",
        fact_id
    )
    .fetch_optional(&mut *connection)
    .await?;
    match id {
        Some(id) => Ok(Some(load_schedule(&mut connection, id).await?)),
        None => Ok(None),
    }
}

pub(super) async fn save_schedule(
    connection: &mut SqliteConnection,
    schedule: &Schedule,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(schedule)?;
    sqlx::query!(
        "INSERT INTO schedule_states(target_id, phase, due_at, blocked_until, introduced_at, version, state_json)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(target_id) DO UPDATE SET phase=excluded.phase, due_at=excluded.due_at,
           blocked_until=excluded.blocked_until, introduced_at=excluded.introduced_at,
           version=excluded.version, state_json=excluded.state_json",
        schedule.target_id, schedule.phase, schedule.due_at, schedule.blocked_until,
        schedule.introduced_at, schedule.version, json
    ).execute(connection).await?;
    Ok(())
}

pub(crate) async fn register_fact(
    connection: &mut SqliteConnection,
    fact_id: i64,
) -> anyhow::Result<()> {
    let id = sqlx::query_scalar!(
        "INSERT INTO learning_targets(fact_id, prompt, answer)
         SELECT id, prompt, answer FROM facts WHERE id = ? RETURNING id AS \"id!\"",
        fact_id
    )
    .fetch_one(&mut *connection)
    .await?;
    let schedule = Schedule {
        target_id: id,
        fact_id,
        content_revision: 1,
        phase: "new".into(),
        due_at: None,
        last_review_at: None,
        stability: None,
        difficulty: None,
        lapses: 0,
        version: 1,
        introduced_at: None,
        blocked_until: None,
    };
    save_schedule(connection, &schedule).await?;
    map_practices(connection, fact_id).await
}

pub(crate) async fn map_practices(
    connection: &mut SqliteConnection,
    fact_id: i64,
) -> anyhow::Result<()> {
    sqlx::query!(
        "INSERT INTO target_practices(practice_id, target_id, qualifying)
         SELECT p.id, t.id, p.kind = 'flashcard' AND p.prompt = t.prompt AND p.answer = t.answer
         FROM practices p JOIN learning_targets t ON t.fact_id = p.fact_id AND t.active = 1
         WHERE p.fact_id = ? AND NOT EXISTS (SELECT 1 FROM target_practices m WHERE m.practice_id = p.id)", fact_id
    ).execute(connection).await?;
    Ok(())
}

pub async fn approve_practice_for(
    pool: &SqlitePool,
    profile_id: i64,
    practice_id: i64,
) -> anyhow::Result<()> {
    crate::db::write_transaction(pool, move |connection| {
        Box::pin(async move {
            let mapping = sqlx::query!(
                "SELECT p.kind, p.answer, t.answer AS target_answer FROM target_practices m
             JOIN practices p ON p.id=m.practice_id JOIN learning_targets t ON t.id=m.target_id
             WHERE m.practice_id = ? AND t.active=1 AND t.fact_id IN (SELECT id FROM facts WHERE profile_id=?)",
                practice_id, profile_id
            )
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| StudyError::Missing("Practice not found".into()))?;
            invalid(
                ReviewMethod::from(PracticeKind::parse(&mapping.kind)?).assesses_recall(),
                "Only recall or cloze prompts can assess this recall target",
            )?;
            invalid(
                mapping.answer.trim() == mapping.target_answer.trim(),
                "Practice answer must match the target's canonical answer",
            )?;
            sqlx::query!(
                "UPDATE target_practices SET qualifying=1 WHERE practice_id=?",
                practice_id
            )
            .execute(connection)
            .await?;
            Ok(())
        })
    })
    .await
}

pub async fn export_events_for(
    pool: &SqlitePool,
    profile_id: i64,
    after: i64,
    limit: i64,
) -> anyhow::Result<StudyEventPage> {
    invalid(
        after >= 0 && (1..=1000).contains(&limit),
        "after must be nonnegative and limit between 1 and 1000",
    )?;
    crate::profiles::require(&mut *pool.acquire().await?, profile_id).await?;
    let fetch_limit = limit + 1;
    let mut items = sqlx::query_as!(StudyEvent,
        "SELECT id AS \"id!\", submission_id, target_id, content_revision, occurred_at, kind, context,
         method, outcome, response_ms, qualifying AS \"qualifying: bool\", algorithm, parameter_version,
         elapsed_seconds, model_interval_days AS \"model_interval_days: f32\", policy_override_seconds,
         request_json, before_json, after_json, result_json
         FROM study_events WHERE profile_id=? AND id > ? ORDER BY id LIMIT ?", profile_id, after, fetch_limit
    ).fetch_all(pool).await?;
    let next_cursor = if items.len() > limit as usize {
        items.truncate(limit as usize);
        items.last().map(|event| event.id)
    } else {
        None
    };
    Ok(StudyEventPage { items, next_cursor })
}

/// Local calendar boundaries, including 23/25-hour days and skipped midnights.
pub(super) fn day_bounds(now: i64, timezone: &str) -> anyhow::Result<(i64, i64)> {
    let timezone: Tz = timezone.parse().context("invalid stored timezone")?;
    let date = Utc
        .timestamp_opt(now, 0)
        .single()
        .context("timestamp out of range")?
        .with_timezone(&timezone)
        .date_naive();
    fn midnight(date: NaiveDate, timezone: Tz) -> anyhow::Result<i64> {
        let midnight = date.and_hms_opt(0, 0, 0).context("invalid calendar day")?;
        for minutes in 0..=2880 {
            if let Some(time) = timezone
                .from_local_datetime(&(midnight + Duration::minutes(minutes)))
                .earliest()
            {
                return Ok(time.timestamp());
            }
        }
        anyhow::bail!("could not resolve local day boundary")
    }
    Ok((
        midnight(date, timezone)?,
        midnight(date.succ_opt().context("calendar overflow")?, timezone)?,
    ))
}

pub(super) async fn introduced_today(
    connection: &mut SqliteConnection,
    profile_id: i64,
    start: i64,
    end: i64,
) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!(
        "SELECT count(*) FROM schedule_states WHERE target_id IN (SELECT t.id FROM learning_targets t JOIN facts f ON f.id=t.fact_id WHERE f.profile_id=?) AND introduced_at >= ? AND introduced_at < ?",
        profile_id, start,
        end
    )
    .fetch_one(connection)
    .await? as i64)
}

pub async fn settings(pool: &SqlitePool) -> anyhow::Result<SchedulerSettings> {
    settings_for(pool, crate::profiles::sole(pool).await?.id).await
}

pub async fn update_settings(
    pool: &SqlitePool,
    update: SettingsUpdate,
) -> anyhow::Result<SchedulerSettings> {
    update_settings_for(pool, crate::profiles::sole(pool).await?.id, update).await
}

pub async fn target(pool: &SqlitePool, fact_id: i64) -> anyhow::Result<Option<Schedule>> {
    target_for(pool, crate::profiles::sole(pool).await?.id, fact_id).await
}

pub async fn approve_practice(pool: &SqlitePool, practice_id: i64) -> anyhow::Result<()> {
    approve_practice_for(pool, crate::profiles::sole(pool).await?.id, practice_id).await
}

pub async fn export_events(
    pool: &SqlitePool,
    after: i64,
    limit: i64,
) -> anyhow::Result<StudyEventPage> {
    export_events_for(pool, crate::profiles::sole(pool).await?.id, after, limit).await
}
