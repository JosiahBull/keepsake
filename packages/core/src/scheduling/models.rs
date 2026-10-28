use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    facts::{Practice, Review, ReviewMethod, ReviewOutcome},
    paging::FactSummary,
};

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct QueueQuery {
    pub limit: i64,
}

impl Default for QueueQuery {
    fn default() -> Self {
        Self { limit: 20 }
    }
}
impl QueueQuery {
    pub fn validate(&self) -> anyhow::Result<()> {
        invalid(
            (1..=20).contains(&self.limit),
            "limit must be between 1 and 20",
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct Schedule {
    pub target_id: i64,
    pub fact_id: i64,
    pub content_revision: i64,
    pub phase: String,
    pub due_at: Option<i64>,
    pub last_review_at: Option<i64>,
    pub stability: Option<f32>,
    pub difficulty: Option<f32>,
    pub lapses: i64,
    pub version: i64,
    pub introduced_at: Option<i64>,
    pub blocked_until: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct RatingIntervals {
    pub again: i64,
    pub hard: i64,
    pub good: i64,
    pub easy: i64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct StudyQuestion {
    pub fact: FactSummary,
    pub practice: Practice,
    pub target_id: i64,
    pub content_revision: i64,
    pub schedule_version: i64,
    pub intervals: RatingIntervals,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct StudyQueue {
    pub items: Vec<StudyQuestion>,
    pub total_due: i64,
    pub new_remaining: i64,
    pub next_due_at: Option<i64>,
    pub timezone: String,
    pub budget_seconds: i64,
    pub spent_seconds: i64,
    pub budget_reached: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct StudySummary {
    /// Learning, relearning and review targets ready now.
    pub due: i64,
    /// New targets waiting within today's remaining intake. The queue offers them after due work.
    pub new_available: i64,
    pub budget_reached: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Attempt,
    Exposure,
}

impl EventKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Attempt => "attempt",
            Self::Exposure => "exposure",
        }
    }

    /// A rated attempt at recall, rather than feedback that showed the answer.
    pub(crate) const fn is_attempt(self) -> bool {
        match self {
            Self::Attempt => true,
            Self::Exposure => false,
        }
    }

    pub(crate) const fn is_exposure(self) -> bool {
        match self {
            Self::Attempt => false,
            Self::Exposure => true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StudyContext {
    Scheduled,
    Supplementary,
    Audit,
}

impl StudyContext {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Supplementary => "supplementary",
            Self::Audit => "audit",
        }
    }

    /// Practice from the due queue: it must be due, and it spends the day's new items.
    pub(crate) const fn is_scheduled(self) -> bool {
        match self {
            Self::Scheduled => true,
            Self::Supplementary | Self::Audit => false,
        }
    }

    /// Practice whose unaided attempts count as recall evidence.
    pub(crate) const fn assesses_recall(self) -> bool {
        match self {
            Self::Scheduled | Self::Audit => true,
            Self::Supplementary => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StudySubmission {
    pub submission_id: String,
    pub target_id: i64,
    pub practice_id: Option<i64>,
    pub content_revision: i64,
    pub expected_version: i64,
    #[serde(default)]
    pub feedback_event_id: Option<i64>,
    pub kind: EventKind,
    pub context: StudyContext,
    pub method: ReviewMethod,
    pub outcome: Option<ReviewOutcome>,
    pub response_ms: i64,
    pub attempted: bool,
    pub assisted: bool,
    pub prior_exposure: bool,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct StudyResult {
    pub event_id: i64,
    pub submission_id: String,
    pub schedule: Schedule,
    pub qualifying: bool,
    pub next_due_at: Option<i64>,
    pub review: Option<Review>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ExposureSubmission {
    pub submission_id: String,
    pub fact_ids: Vec<i64>,
    pub method: ReviewMethod,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ExposureResult {
    pub recorded: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct SchedulerSettings {
    pub desired_retention: f32,
    pub new_limit: i64,
    pub daily_budget_seconds: i64,
    pub timezone: String,
    pub algorithm: String,
    pub parameter_version: i64,
    pub parameters: Vec<f32>,
    pub version: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SettingsUpdate {
    pub timezone: String,
    pub new_limit: i64,
    pub daily_budget_seconds: i64,
    pub expected_version: i64,
}

#[derive(Debug, Deserialize, Serialize, ToSchema, sqlx::FromRow)]
pub struct StudyEvent {
    pub id: i64,
    pub submission_id: String,
    pub target_id: i64,
    pub content_revision: i64,
    pub occurred_at: i64,
    pub kind: String,
    pub context: String,
    pub method: String,
    pub outcome: Option<String>,
    pub response_ms: i64,
    pub qualifying: bool,
    pub algorithm: String,
    pub parameter_version: i64,
    pub elapsed_seconds: i64,
    pub model_interval_days: Option<f32>,
    pub policy_override_seconds: Option<i64>,
    pub request_json: String,
    pub before_json: String,
    pub after_json: String,
    pub result_json: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct StudyEventPage {
    pub items: Vec<StudyEvent>,
    pub next_cursor: Option<i64>,
}

#[derive(Debug)]
pub enum StudyError {
    Invalid(String),
    Conflict(String),
    Missing(String),
}
impl std::fmt::Display for StudyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) | Self::Conflict(message) | Self::Missing(message) => {
                f.write_str(message)
            }
        }
    }
}
impl std::error::Error for StudyError {}

pub(super) fn invalid(condition: bool, message: &str) -> anyhow::Result<()> {
    if !condition {
        return Err(StudyError::Invalid(message.into()).into());
    }
    Ok(())
}

pub(super) fn conflict(condition: bool, message: &str) -> anyhow::Result<()> {
    if !condition {
        return Err(StudyError::Conflict(message.into()).into());
    }
    Ok(())
}
