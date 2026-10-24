use std::collections::HashSet;

use anyhow::Context;
use chrono::{SecondsFormat, TimeZone, Utc};
use sqlx::{SqliteConnection, SqlitePool};

use super::{
    models::{conflict, invalid},
    selection::validate_content,
    storage::*,
    *,
};
use crate::facts::{NewReview, Review, ReviewMethod, ReviewOutcome};

pub async fn submit(pool: &SqlitePool, submission: StudySubmission) -> anyhow::Result<StudyResult> {
    submit_at(pool, submission, now()).await
}

pub async fn submit_at(
    pool: &SqlitePool,
    submission: StudySubmission,
    now: i64,
) -> anyhow::Result<StudyResult> {
    validate_submission(&submission, now)?;
    crate::db::write_transaction(pool, move |connection| {
        let submission = submission.clone();
        Box::pin(async move { apply(connection, submission, now, None).await })
    })
    .await
}

fn validate_submission(input: &StudySubmission, now: i64) -> anyhow::Result<()> {
    invalid(
        uuid::Uuid::parse_str(&input.submission_id).is_ok(),
        "submission_id must be a UUID",
    )?;
    invalid(
        now >= 0 && input.target_id > 0 && input.content_revision > 0 && input.expected_version > 0,
        "timestamp, target, revision and version must be valid",
    )?;
    invalid(
        (0..=86_400_000).contains(&input.response_ms),
        "response_ms must be between 0 and 86400000",
    )?;
    invalid(
        input
            .feedback_event_id
            .is_none_or(|id| id > 0 && input.kind == EventKind::Attempt),
        "feedback_event_id must identify feedback for an attempt",
    )?;
    match input.kind {
        EventKind::Attempt => invalid(
            input.attempted && input.outcome.is_some(),
            "An attempt requires an attempted answer and a rating",
        ),
        EventKind::Exposure => invalid(
            input.outcome.is_none()
                && (!input.attempted
                    || (input.context == StudyContext::Scheduled && input.practice_id.is_some())),
            "Feedback cannot include a rating; attempted feedback requires a scheduled prompt",
        ),
    }
}

async fn apply(
    connection: &mut SqliteConnection,
    input: StudySubmission,
    now: i64,
    existing_review: Option<Review>,
) -> anyhow::Result<StudyResult> {
    let request_json = serde_json::to_string(&input)?;
    if let Some(saved) = sqlx::query!(
        "SELECT request_json, result_json FROM study_events WHERE submission_id=?",
        input.submission_id
    )
    .fetch_optional(&mut *connection)
    .await?
    {
        conflict(
            saved.request_json == request_json,
            "Submission ID already used for a different event",
        )?;
        return Ok(serde_json::from_str(&saved.result_json)?);
    }
    let before = load_schedule(connection, input.target_id).await?;
    conflict(
        before.version == input.expected_version
            && before.content_revision == input.content_revision,
        "This target was already reviewed or changed. Refresh practice.",
    )?;
    let fact = crate::facts::load_id(connection, before.fact_id)
        .await?
        .ok_or_else(|| StudyError::Missing("Fact not found".into()))?;
    validate_content(connection, &before, &fact).await?;
    let mapping = match input.practice_id {
        Some(id) => {
            let mapping = sqlx::query!(
                "SELECT m.target_id, m.qualifying, p.kind, p.answer FROM target_practices m
                 JOIN practices p ON p.id=m.practice_id WHERE m.practice_id=?",
                id
            )
            .fetch_optional(&mut *connection)
            .await?
            .ok_or_else(|| StudyError::Invalid("Practice not found".into()))?;
            invalid(
                mapping.target_id == before.target_id && mapping.kind == input.method.as_str(),
                "Practice does not assess this target or method",
            )?;
            // Recheck the answer so editing an approved prompt cannot retain approval silently.
            mapping.qualifying == 1 && mapping.answer.trim() == fact.answer.trim()
        }
        None => {
            invalid(
                input.kind == EventKind::Exposure
                    || matches!(
                        input.method,
                        ReviewMethod::Matching | ReviewMethod::Flashcard
                    ),
                "This method requires a practice_id",
            )?;
            false
        }
    };
    let qualifying = input.kind == EventKind::Attempt
        && matches!(input.context, StudyContext::Scheduled | StudyContext::Audit)
        && matches!(input.method, ReviewMethod::Flashcard | ReviewMethod::Cloze)
        && input.attempted
        && !input.assisted
        && !input.prior_exposure
        && mapping;
    let unaided_feedback = input.kind == EventKind::Exposure
        && input.attempted
        && !input.assisted
        && !input.prior_exposure
        && mapping
        && matches!(input.method, ReviewMethod::Flashcard | ReviewMethod::Cloze);
    let feedback_at = if input.feedback_event_id.is_some() {
        own_feedback(connection, &input, &before, now).await?
    } else {
        None
    };
    let assessed_at = feedback_at.unwrap_or(now);
    if qualifying || unaided_feedback {
        conflict(
            feedback_at.is_some() || before.blocked_until.is_none_or(|until| until <= now),
            "The answer was recently exposed. Try recall later.",
        )?;
        if input.context == StudyContext::Scheduled {
            conflict(
                before.due_at.is_none_or(|due| due <= now),
                "This target is not due yet",
            )?;
        }
    }
    let settings = load_settings(connection).await?;
    if input.context == StudyContext::Scheduled && before.introduced_at.is_none() {
        let (start, end) = day_bounds(now, &settings.timezone)?;
        let introduced = introduced_today(connection, start, end).await?;
        conflict(
            introduced < settings.new_limit,
            "Today's new-item allowance is used. Refresh practice.",
        )?;
        let ready = sqlx::query_scalar!(
            "SELECT count(*) AS \"count!: i64\" FROM schedule_states s JOIN learning_targets t ON t.id=s.target_id
             WHERE t.active=1 AND s.phase <> 'new'
               AND max(coalesce(s.due_at, 0), coalesce(s.blocked_until, 0)) <= ?", now
        ).fetch_one(&mut *connection).await?;
        conflict(
            ready == 0,
            "Due reviews are waiting. Refresh practice before introducing another target.",
        )?;
    }

    let mut after = before.clone();
    after.version += 1;
    let elapsed_seconds = before.last_review_at.map_or(0, |last| assessed_at - last);
    conflict(elapsed_seconds >= 0, "Clock precedes the previous review")?;
    let mut model_interval_days = None;
    let mut policy_override_seconds = None;
    if qualifying {
        let transitions = transitions(&before, &settings, assessed_at)?;
        let outcome = input.outcome.context("attempt rating missing")?;
        let transition = match outcome {
            ReviewOutcome::Again => transitions.again,
            ReviewOutcome::Hard => transitions.hard,
            ReviewOutcome::Good => transitions.good,
            ReviewOutcome::Easy => transitions.easy,
        };
        let interval = if matches!(outcome, ReviewOutcome::Again) {
            policy_override_seconds = Some(RETRY_SECONDS);
            if before.phase == "review" {
                after.lapses += 1;
            }
            after.phase = if before.phase == "review" || before.phase == "relearning" {
                "relearning"
            } else {
                "learning"
            }
            .into();
            RETRY_SECONDS
        } else {
            after.phase = "review".into();
            transition.interval_seconds
        };
        after.stability = Some(transition.memory.stability);
        after.difficulty = Some(transition.memory.difficulty);
        after.due_at = Some(
            assessed_at
                .checked_add(interval)
                .context("due timestamp overflow")?,
        );
        after.last_review_at = Some(assessed_at);
        after.introduced_at = before.introduced_at.or(Some(now));
        after.blocked_until = None;
        model_interval_days = Some(transition.model_interval_days);
    } else {
        // Exposure delays assessment without changing the model's recall due date.
        let until = now
            .checked_add(RETRY_SECONDS)
            .context("exposure timestamp overflow")?;
        after.blocked_until = Some(before.blocked_until.unwrap_or(0).max(until));
        policy_override_seconds = Some(RETRY_SECONDS);
        if input.context == StudyContext::Scheduled && after.introduced_at.is_none() {
            after.introduced_at = Some(now);
            after.phase = "learning".into();
        }
    }
    let review = if let Some(review) = existing_review {
        Some(review)
    } else if let Some(outcome) = input.outcome {
        let review = NewReview {
            fact_id: fact.id,
            practice_id: input.practice_id,
            method: input.method,
            outcome,
            response_ms: input.response_ms,
        };
        crate::facts::validate_review(&review, &fact)
            .map_err(|error| StudyError::Invalid(error.to_string()))?;
        let reviewed_at = Utc
            .timestamp_opt(assessed_at, 0)
            .single()
            .context("timestamp out of range")?
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let result = sqlx::query!(
                "INSERT INTO reviews(fact_id, practice_id, method, outcome, response_ms, reviewed_at) VALUES (?, ?, ?, ?, ?, ?)",
                fact.id, input.practice_id, input.method.as_str(), outcome.as_str(), input.response_ms, reviewed_at
            ).execute(&mut *connection).await?;
        Some(Review {
            id: result.last_insert_rowid(),
            fact_id: fact.id,
            practice_id: input.practice_id,
            method: input.method,
            outcome,
            response_ms: input.response_ms,
            reviewed_at,
        })
    } else {
        None
    };
    save_schedule(connection, &after).await?;
    let before_json = serde_json::to_string(&before)?;
    let after_json = serde_json::to_string(&after)?;
    let kind = if input.kind == EventKind::Attempt {
        "attempt"
    } else {
        "exposure"
    };
    let context = match input.context {
        StudyContext::Scheduled => "scheduled",
        StudyContext::Supplementary => "supplementary",
        StudyContext::Audit => "audit",
    };
    let outcome = input.outcome.map(|outcome| outcome.as_str());
    let saved = sqlx::query!(
        "INSERT INTO study_events(submission_id, target_id, content_revision, occurred_at, kind, context, method, outcome,
           response_ms, qualifying, algorithm, parameter_version, elapsed_seconds, model_interval_days,
           policy_override_seconds, request_json, before_json, after_json, result_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '{}')",
        input.submission_id, input.target_id, input.content_revision, now, kind, context, input.method.as_str(), outcome,
        input.response_ms, qualifying, settings.algorithm, settings.parameter_version, elapsed_seconds,
        model_interval_days, policy_override_seconds, request_json, before_json, after_json
    ).execute(&mut *connection).await?;
    let result = StudyResult {
        event_id: saved.last_insert_rowid(),
        submission_id: input.submission_id,
        next_due_at: available_at(&after),
        schedule: after,
        qualifying,
        review,
    };
    let json = serde_json::to_string(&result)?;
    sqlx::query!(
        "UPDATE study_events SET result_json=? WHERE id=?",
        json,
        result.event_id
    )
    .execute(connection)
    .await?;
    Ok(result)
}

pub async fn expose(
    pool: &SqlitePool,
    submission: ExposureSubmission,
) -> anyhow::Result<ExposureResult> {
    expose_at(pool, submission, now()).await
}

pub async fn expose_at(
    pool: &SqlitePool,
    submission: ExposureSubmission,
    now: i64,
) -> anyhow::Result<ExposureResult> {
    invalid(
        uuid::Uuid::parse_str(&submission.submission_id).is_ok() && now >= 0,
        "submission_id must be a UUID and timestamp nonnegative",
    )?;
    invalid(
        (1..=20).contains(&submission.fact_ids.len())
            && submission.fact_ids.iter().all(|id| *id > 0),
        "Provide between 1 and 20 positive fact IDs",
    )?;
    invalid(
        submission.fact_ids.iter().collect::<HashSet<_>>().len() == submission.fact_ids.len(),
        "fact_ids must be distinct",
    )?;
    crate::db::write_transaction(pool, move |connection| {
        let submission = submission.clone();
        Box::pin(async move {
            let request_json = serde_json::to_string(&submission)?;
            if let Some(batch) = sqlx::query!("SELECT request_json, result_json FROM study_exposure_batches WHERE submission_id=?", submission.submission_id)
                .fetch_optional(&mut *connection).await? {
                conflict(batch.request_json == request_json, "Submission ID already used for different exposures")?;
                return Ok(serde_json::from_str(&batch.result_json)?);
            }
            for fact_id in &submission.fact_ids {
                let target_id = sqlx::query_scalar!("SELECT id FROM learning_targets WHERE fact_id=? AND active=1", fact_id)
                    .fetch_optional(&mut *connection).await?.ok_or_else(|| StudyError::Missing("Fact not found".into()))?;
                let state = load_schedule(connection, target_id).await?;
                // Batch exposures need no assessment prompt; the method describes the source of exposure.
                let event = StudySubmission { submission_id: uuid::Uuid::new_v4().to_string(),
                    target_id, practice_id: None, content_revision: state.content_revision,
                    expected_version: state.version, feedback_event_id: None, kind: EventKind::Exposure, context: StudyContext::Supplementary,
                    method: submission.method, outcome: None, response_ms: 0, attempted: false, assisted: false, prior_exposure: true };
                apply(connection, event, now, None).await?;
            }
            let result = ExposureResult { recorded: submission.fact_ids.len() as i64 };
            let result_json = serde_json::to_string(&result)?;
            sqlx::query!("INSERT INTO study_exposure_batches(submission_id, request_json, result_json) VALUES (?, ?, ?)",
                submission.submission_id, request_json, result_json).execute(connection).await?;
            Ok(result)
        })
    }).await
}

/// Feedback is saved before revealing an answer. Only that same, still-current
/// attempt may be rated during its cooldown; another exposure invalidates it.
async fn own_feedback(
    connection: &mut SqliteConnection,
    input: &StudySubmission,
    schedule: &Schedule,
    now: i64,
) -> anyhow::Result<Option<i64>> {
    let Some(feedback_id) = input.feedback_event_id else {
        return Ok(None);
    };
    let latest = sqlx::query!(
        "SELECT request_json, after_json, occurred_at FROM study_events WHERE id=? AND target_id=?",
        feedback_id,
        input.target_id
    )
    .fetch_optional(connection)
    .await?;
    let latest = latest
        .ok_or_else(|| StudyError::Conflict("Feedback does not belong to this target".into()))?;
    let feedback: StudySubmission = serde_json::from_str(&latest.request_json)?;
    let state: Schedule = serde_json::from_str(&latest.after_json)?;
    let matching = feedback.kind == EventKind::Exposure
        && feedback.context == StudyContext::Scheduled
        && feedback.attempted
        && feedback.context == input.context
        && feedback.assisted == input.assisted
        && feedback.prior_exposure == input.prior_exposure
        && feedback.practice_id == input.practice_id
        && feedback.method == input.method
        && feedback.response_ms == input.response_ms
        && state.version == schedule.version;
    conflict(
        matching,
        "This feedback no longer describes the current attempt. Refresh practice.",
    )?;
    conflict(
        (0..=900).contains(&(now - latest.occurred_at)),
        "This attempt expired. Refresh practice.",
    )?;
    Ok(Some(latest.occurred_at))
}

pub(crate) async fn record_supplementary(
    connection: &mut SqliteConnection,
    review: &Review,
) -> anyhow::Result<()> {
    let target_id = sqlx::query_scalar!(
        "SELECT id FROM learning_targets WHERE fact_id=? AND active=1",
        review.fact_id
    )
    .fetch_optional(&mut *connection)
    .await?
    .context("fact has no learning target")?;
    let schedule = load_schedule(connection, target_id).await?;
    let input = StudySubmission {
        submission_id: uuid::Uuid::new_v4().to_string(),
        target_id,
        practice_id: review.practice_id,
        content_revision: schedule.content_revision,
        expected_version: schedule.version,
        feedback_event_id: None,
        kind: EventKind::Attempt,
        context: StudyContext::Supplementary,
        method: review.method,
        outcome: Some(review.outcome),
        response_ms: review.response_ms,
        attempted: true,
        assisted: false,
        prior_exposure: true,
    };
    apply(connection, input, now(), Some(review.clone())).await?;
    Ok(())
}
