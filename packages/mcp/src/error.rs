//! What a failed tool call tells the agent.
//!
//! A refusal the agent can act on, such as a missing fact or an invalid draft, comes back
//! as a tool result marked as an error, which is the only error text clients show to the
//! model. Anything else is logged and replaced with a fixed sentence: an agent pastes
//! what it is given into a transcript, and a database path or SQL error has no place there.

use keepsake_core::{db, profiles::ProfileError, scheduling::StudyError};
use rmcp::ErrorData;
use rmcp::model::{CallToolResult, ContentBlock};

const INTERNAL_MESSAGE: &str = "Keepsake hit an internal error; its log has the detail.";

pub(crate) enum Failure {
    /// The call was wrong or met a state it cannot change; say why.
    Refused(String),
    Internal(anyhow::Error),
}

pub(crate) type Outcome = Result<CallToolResult, Failure>;

impl From<anyhow::Error> for Failure {
    fn from(error: anyhow::Error) -> Self {
        if let Some(error) = error.downcast_ref::<StudyError>() {
            return Self::Refused(error.to_string());
        }
        if let Some(error) = error.downcast_ref::<ProfileError>() {
            return Self::Refused(error.to_string());
        }
        if db::is_overloaded(&error) {
            return Self::Refused("The library is busy. Try again shortly.".into());
        }
        Self::Internal(error)
    }
}

pub(crate) fn refused(message: impl Into<String>) -> Failure {
    Failure::Refused(message.into())
}

pub(crate) fn finish(outcome: Outcome) -> Result<CallToolResult, ErrorData> {
    match outcome {
        Ok(result) => Ok(result),
        Err(Failure::Refused(message)) => {
            Ok(CallToolResult::error(vec![ContentBlock::text(message)]))
        }
        Err(Failure::Internal(error)) => {
            tracing::error!(%error, "mcp tool failed");
            Err(ErrorData::internal_error(INTERNAL_MESSAGE, None))
        }
    }
}

pub(crate) fn internal(error: anyhow::Error) -> ErrorData {
    tracing::error!(%error, "mcp request failed");
    ErrorData::internal_error(INTERNAL_MESSAGE, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(result: &CallToolResult) -> &str {
        result.content[0]
            .as_text()
            .map(|text| text.text.as_str())
            .unwrap()
    }

    #[test]
    fn a_refusal_keeps_the_message_that_lets_the_agent_fix_its_call() {
        let missing = anyhow::Error::from(StudyError::Missing("Fact not found".into()));
        let result = finish(Err(missing.into())).unwrap();
        assert_eq!(result.is_error, Some(true));
        assert_eq!(text(&result), "Fact not found");
    }

    #[test]
    fn an_internal_fault_says_nothing_about_itself() {
        let error = finish(Err(anyhow::anyhow!(
            "no such table: facts in /home/someone/keepsake.db"
        )
        .into()))
        .unwrap_err();
        assert_eq!(error.message, INTERNAL_MESSAGE);
        assert!(error.data.is_none());
    }
}
