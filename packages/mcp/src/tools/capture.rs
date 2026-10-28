//! Adding to the library: new facts and practice variants. Served only with write access.
//!
//! Nothing here edits, approves, reviews or removes. Approval and ratings stay with the
//! learner, who checks a variant against its source and attempts each answer unaided.

use base64ct::{Base64, Encoding};
use keepsake_core::{
    facts::{self, NewFact, NewPractice, PracticeKind},
    media::{self, NewQuestionMedia},
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::convert::{clip, table};
use crate::error::{Outcome, finish, refused};
use crate::server::KeepsakeMcp;

#[derive(Debug, Clone, Copy, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Recall the answer to a question.
    Flashcard,
    /// Choose the answer from 2 to 8 distinct choices, exactly one of them the answer.
    MultipleChoice,
    /// Fill the gap marked ___ in the prompt.
    Cloze,
    /// Explain a relationship in your own words.
    Explain,
    /// Use the knowledge in a short scenario.
    Apply,
}

impl From<Kind> for PracticeKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Flashcard => Self::Flashcard,
            Kind::MultipleChoice => Self::MultipleChoice,
            Kind::Cloze => Self::Cloze,
            Kind::Explain => Self::Explain,
            Kind::Apply => Self::Apply,
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DraftPractice {
    pub kind: Kind,
    pub prompt: String,
    /// Must agree with the fact's answer.
    pub answer: String,
    /// Multiple choice only.
    #[serde(default)]
    pub choices: Vec<String>,
    /// Feedback shown after an attempt: why the answer is right, or how to tell it apart.
    #[serde(default)]
    pub explanation: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DraftMedia {
    /// The file's bytes, base64-encoded. At most 5 MiB before encoding.
    pub data_base64: String,
    /// The file's type: image/jpeg, image/png, image/gif, image/webp, audio/mpeg,
    /// audio/mp4, audio/aac, audio/wav, audio/ogg, audio/webm or audio/flac. The file
    /// must decode as this type. Still images are stored as AVIF and clips as AAC.
    pub content_type: String,
    /// For an image: what it shows, for screen readers, without giving away the answer.
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DraftFact {
    /// The question, phrased so one answer is correct.
    pub prompt: String,
    pub answer: String,
    /// Background or qualifications that help when reviewing the answer.
    #[serde(default)]
    pub context: String,
    /// Where the fact comes from: a URL, book or document the learner can check.
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Extra ways to practise this fact. The plain question is always included.
    #[serde(default)]
    pub practices: Vec<DraftPractice>,
    /// Up to 4 images or clips asked with the question. Never put the answer in them.
    #[serde(default)]
    pub media: Vec<DraftMedia>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFactsParams {
    /// 1 to 20 facts, saved together or not at all.
    pub facts: Vec<DraftFact>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddPracticesParams {
    pub fact_id: i64,
    /// 1 to 32 variants.
    pub practices: Vec<DraftPractice>,
}

fn practice(draft: DraftPractice) -> NewPractice {
    NewPractice {
        kind: draft.kind.into(),
        prompt: draft.prompt,
        answer: draft.answer,
        choices: draft.choices,
        explanation: draft.explanation,
    }
}

#[tool_router(router = capture_router, vis = "pub")]
impl KeepsakeMcp {
    #[tool(
        name = "add_facts",
        description = "Save 1 to 20 new facts to this library, all together or none. Each has \
                       a question and answer, and may have context, a source, tags, practice \
                       variants and up to 4 images or clips (base64) asked with the question. \
                       Check facts against their source first, and record the source.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    pub async fn add_facts(
        &self,
        Parameters(params): Parameters<AddFactsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        finish(self.save_facts(params).await)
    }

    #[tool(
        name = "add_practices",
        description = "Add 1 to 32 practice variants to an existing fact: flashcard, multiple \
                       choice, cloze, explain or apply. They appear in extra practice; scheduled \
                       recall uses one only after the learner approves it.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false
        )
    )]
    pub async fn add_practices(
        &self,
        Parameters(params): Parameters<AddPracticesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        finish(self.save_practices(params).await)
    }
}

impl KeepsakeMcp {
    async fn save_facts(&self, params: AddFactsParams) -> Outcome {
        if !(1..=20).contains(&params.facts.len()) {
            return Err(refused("Provide between 1 and 20 facts."));
        }
        // Check every draft and decode every file before storing any of them.
        let mut drafts = Vec::with_capacity(params.facts.len());
        for (index, draft) in params.facts.into_iter().enumerate() {
            let number = index + 1;
            let fact = NewFact {
                prompt: draft.prompt,
                answer: draft.answer,
                context: draft.context,
                source: draft.source,
                tags: draft.tags,
                practices: draft.practices.into_iter().map(practice).collect(),
                media: Vec::new(),
            };
            facts::validate_new_fact(&fact)
                .map_err(|error| refused(format!("Fact {number}: {error}")))?;
            if draft.media.len() > media::MAX_QUESTION_MEDIA {
                return Err(refused(format!(
                    "Fact {number}: a question can have at most {} images or clips.",
                    media::MAX_QUESTION_MEDIA
                )));
            }
            let mut files = Vec::with_capacity(draft.media.len());
            for (position, item) in draft.media.into_iter().enumerate() {
                let bytes = Base64::decode_vec(item.data_base64.trim()).map_err(|_| {
                    refused(format!(
                        "Fact {number}, media {}: data_base64 is not valid base64.",
                        position + 1
                    ))
                })?;
                media::check(&item.content_type, bytes.clone().into())
                    .await
                    .map_err(|error| {
                        refused(format!("Fact {number}, media {}: {error}", position + 1))
                    })?;
                files.push((item.content_type, bytes, item.description));
            }
            drafts.push((fact, files));
        }

        let mut proposed = Vec::with_capacity(drafts.len());
        for (mut fact, files) in drafts {
            for (content_type, bytes, description) in files {
                // An upload left unattached by a failed save is removed after a day.
                let upload =
                    media::upload(&self.pool, self.profile.id, &content_type, bytes).await?;
                fact.media.push(NewQuestionMedia {
                    media_id: upload.media_id,
                    alt: description,
                });
            }
            proposed.push(fact);
        }
        let saved = facts::create_many_for(&self.pool, self.profile.id, proposed).await?;
        let rows: Vec<Vec<String>> = saved
            .iter()
            .map(|fact| {
                vec![
                    fact.id.to_string(),
                    clip(&fact.prompt, 120),
                    fact.practices.len().to_string(),
                    fact.media.len().to_string(),
                ]
            })
            .collect();
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "Saved {} fact(s).\n\n{}",
            saved.len(),
            table(&["id", "question", "practices", "media"], &rows)
        ))]))
    }

    async fn save_practices(&self, params: AddPracticesParams) -> Outcome {
        if !(1..=32).contains(&params.practices.len()) {
            return Err(refused("Provide between 1 and 32 practices."));
        }
        let practices: Vec<NewPractice> = params.practices.into_iter().map(practice).collect();
        for (index, draft) in practices.iter().enumerate() {
            facts::validate_practice(draft)
                .map_err(|error| refused(format!("Practice {}: {error}", index + 1)))?;
        }
        if facts::get_for(&self.pool, self.profile.id, params.fact_id)
            .await?
            .is_none()
        {
            return Err(refused(format!(
                "Fact {} is not in this library.",
                params.fact_id
            )));
        }
        let added = practices.len();
        let fact = facts::add_practices_for(&self.pool, self.profile.id, params.fact_id, practices)
            .await?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "Added {added} practice variant(s) to fact {}, which now has {}. They appear in \
             extra practice; the learner approves any that should count towards scheduled recall.",
            fact.id,
            fact.practices.len()
        ))]))
    }
}
