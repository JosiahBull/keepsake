//! Persistent recall scheduling shared by the server and independent CLI.
mod adapter;
mod events;
mod models;
mod selection;
mod storage;

pub(crate) use events::record_supplementary;
pub use events::{
    expose, expose_at, expose_at_for, expose_for, submit, submit_at, submit_at_for, submit_for,
};
pub use models::*;
pub use selection::{queue, queue_at, queue_at_for, queue_for, summary_at_for, summary_for};
pub use storage::{
    approve_practice, approve_practice_for, export_events, export_events_for, settings,
    settings_for, target, target_for, update_settings, update_settings_for,
};
pub(crate) use storage::{forget_fact, map_practices, register_fact};

pub(super) const RETRY_SECONDS: i64 = 600;

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn transitions(
    schedule: &Schedule,
    settings: &SchedulerSettings,
    now: i64,
) -> anyhow::Result<adapter::Transitions> {
    let memory = match (schedule.stability, schedule.difficulty) {
        (Some(stability), Some(difficulty)) => Some(adapter::Memory {
            stability,
            difficulty,
        }),
        (None, None) => None,
        (Some(_), None) | (None, Some(_)) => anyhow::bail!("incomplete memory state"),
    };
    let elapsed = match schedule.last_review_at {
        Some(last) => now
            .checked_sub(last)
            .filter(|elapsed| *elapsed >= 0)
            .ok_or_else(|| StudyError::Conflict("Clock precedes the previous review".into()))?,
        None => 0,
    };
    adapter::next(
        memory,
        elapsed,
        settings.desired_retention,
        &settings.parameters,
    )
}

fn available_at(schedule: &Schedule) -> Option<i64> {
    match (schedule.due_at, schedule.blocked_until) {
        (None, None) => None,
        (due, blocked) => Some(due.unwrap_or(0).max(blocked.unwrap_or(0))),
    }
}
