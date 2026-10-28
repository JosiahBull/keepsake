//! Reading the library and its study state. Nothing here can change either.

use base64ct::{Base64, Encoding};
use keepsake_core::{
    facts,
    media::{self, MediaKind, QuestionMedia},
    paging::{self, FactQuery},
    scheduling,
};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde::Deserialize;

use crate::config::MAX_ROWS;
use crate::convert::{clip, table};
use crate::error::{Failure, Outcome, finish, refused};
use crate::server::KeepsakeMcp;

/// Long enough to recognise a fact in a list; `get_fact` has the rest.
const CELL_CHARS: usize = 160;

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct SearchFactsParams {
    /// Text to find in questions, answers, context and tags, matched literally and
    /// ignoring case. Omit to list the newest facts.
    #[serde(default)]
    pub search: Option<String>,
    /// Facts per page, 1 to 50. Defaults to 20.
    #[serde(default)]
    pub limit: Option<i64>,
    /// Continue a search from the `before_id` its previous page gave.
    #[serde(default)]
    pub before_id: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FactParams {
    pub fact_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MediaParams {
    /// A `media_id` from `get_fact`.
    pub media_id: i64,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct NoParams {}

fn text(value: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(value)])
}

fn media_summary(media: &[QuestionMedia]) -> String {
    let count = |kind: MediaKind| media.iter().filter(|item| item.kind == kind).count();
    match (count(MediaKind::Image), count(MediaKind::Audio)) {
        (0, 0) => String::new(),
        (images, 0) => format!("{images} image(s)"),
        (0, clips) => format!("{clips} clip(s)"),
        (images, clips) => format!("{images} image(s), {clips} clip(s)"),
    }
}

fn measured(item: &QuestionMedia) -> String {
    match (item.width, item.height, item.duration_ms) {
        (Some(width), Some(height), _) => format!("{width}×{height} px"),
        (_, _, Some(ms)) => format!("{:.1} s", ms as f64 / 1000.0),
        _ => String::new(),
    }
}

#[tool_router(router = library_router, vis = "pub")]
impl KeepsakeMcp {
    #[tool(
        name = "search_facts",
        description = "Search this library's facts, newest first. Returns ids, questions, \
                       answers (cut at 160 characters), tags and media counts. Use get_fact \
                       for a fact's full text, context, source and practice variants.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn search_facts(
        &self,
        Parameters(params): Parameters<SearchFactsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        finish(self.search(params).await)
    }

    #[tool(
        name = "get_fact",
        description = "One fact in full: question, answer, context, source, tags, the images \
                       and clips asked with its question, and its practice variants.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn get_fact(
        &self,
        Parameters(params): Parameters<FactParams>,
    ) -> Result<CallToolResult, ErrorData> {
        finish(self.fact(params.fact_id).await)
    }

    #[tool(
        name = "view_media",
        description = "Return an image or audio clip from a fact's question, by the media_id \
                       get_fact lists, as the learner is served it: AVIF for still images, the \
                       original GIF, WebP or PNG for animations, AAC in MP4 for clips.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn view_media(
        &self,
        Parameters(params): Parameters<MediaParams>,
    ) -> Result<CallToolResult, ErrorData> {
        finish(self.media(params.media_id).await)
    }

    #[tool(
        name = "list_tags",
        description = "Every tag in this library with how many facts carry it, most used \
                       first. Reuse these tags when adding facts: matching practice draws \
                       related facts from one shared tag.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn list_tags(
        &self,
        Parameters(_): Parameters<NoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        finish(self.tags().await)
    }

    #[tool(
        name = "study_overview",
        description = "What is ready to study now (due reviews and new facts within today's \
                       limit), whether today's study budget is spent, and the study settings. \
                       Reviews themselves happen in the Keepsake app.",
        annotations(read_only_hint = true, idempotent_hint = true)
    )]
    pub async fn study_overview(
        &self,
        Parameters(_): Parameters<NoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        finish(self.overview().await)
    }
}

impl KeepsakeMcp {
    async fn search(&self, params: SearchFactsParams) -> Outcome {
        let query = FactQuery {
            limit: params.limit.unwrap_or(20),
            before_id: params.before_id,
            search: params.search.unwrap_or_default(),
        };
        if !(1..=MAX_ROWS).contains(&query.limit) {
            return Err(refused(format!("limit must be between 1 and {MAX_ROWS}")));
        }
        query
            .validate()
            .map_err(|error| refused(error.to_string()))?;
        let page = paging::facts_for(&self.pool, self.profile.id, &query).await?;
        let rows: Vec<Vec<String>> = page
            .items
            .iter()
            .map(|fact| {
                vec![
                    fact.id.to_string(),
                    clip(&fact.prompt, CELL_CHARS),
                    clip(&fact.answer, CELL_CHARS),
                    fact.tags.join(", "),
                    media_summary(&fact.media),
                ]
            })
            .collect();
        let mut out = if rows.is_empty() {
            "No facts match.".to_string()
        } else {
            table(&["id", "question", "answer", "tags", "media"], &rows)
        };
        out.push_str(&format!(
            "\n\n{} of {} matching facts.",
            rows.len(),
            page.total
        ));
        if let Some(cursor) = page.next_cursor {
            out.push_str(&format!(" More: call again with before_id={cursor}."));
        }
        Ok(text(out))
    }

    async fn fact(&self, fact_id: i64) -> Outcome {
        let fact = facts::get_for(&self.pool, self.profile.id, fact_id)
            .await?
            .ok_or_else(|| refused(format!("Fact {fact_id} is not in this library.")))?;
        let mut out = format!(
            "Fact {}\nQuestion: {}\nAnswer: {}\n",
            fact.id, fact.prompt, fact.answer
        );
        for (label, value) in [
            ("Context", fact.context.as_str()),
            ("Source", fact.source.as_str()),
        ] {
            if !value.is_empty() {
                out.push_str(&format!("{label}: {value}\n"));
            }
        }
        if !fact.tags.is_empty() {
            out.push_str(&format!("Tags: {}\n", fact.tags.join(", ")));
        }
        if let Some(sender) = &fact.shared_by {
            out.push_str(&format!("Shared by: {sender}\n"));
        }
        out.push_str(&format!("Created: {}\n", fact.created_at));
        if !fact.media.is_empty() {
            let rows: Vec<Vec<String>> = fact
                .media
                .iter()
                .map(|item| {
                    vec![
                        item.media_id.to_string(),
                        item.kind.as_str().into(),
                        item.content_type.clone(),
                        measured(item),
                        item.alt.clone(),
                    ]
                })
                .collect();
            out.push_str("\nAsked with the question (view_media shows each):\n");
            out.push_str(&table(
                &["media_id", "kind", "type", "size", "description"],
                &rows,
            ));
            out.push('\n');
        }
        let rows: Vec<Vec<String>> = fact
            .practices
            .iter()
            .map(|practice| {
                vec![
                    practice.id.to_string(),
                    practice.kind.as_str().into(),
                    practice.prompt.clone(),
                    practice.answer.clone(),
                    practice.choices.join(" / "),
                ]
            })
            .collect();
        out.push_str("\nPractice variants:\n");
        out.push_str(&table(
            &["practice_id", "kind", "prompt", "answer", "choices"],
            &rows,
        ));
        Ok(text(out))
    }

    async fn media(&self, media_id: i64) -> Outcome {
        let file = media::locate(&self.pool, self.profile.id, media_id)
            .await?
            .ok_or_else(|| refused(format!("Media {media_id} is not in this library.")))?;
        let bytes = tokio::fs::read(&file.path)
            .await
            .map_err(|error| Failure::Internal(error.into()))?;
        let data = Base64::encode_string(&bytes);
        Ok(CallToolResult::success(vec![
            if file.content_type.starts_with("image/") {
                ContentBlock::image(data, file.content_type)
            } else {
                ContentBlock::audio(data, file.content_type)
            },
        ]))
    }

    async fn tags(&self) -> Outcome {
        let tags = paging::tags_for(&self.pool, self.profile.id).await?;
        if tags.is_empty() {
            return Ok(text("No facts are tagged yet.".into()));
        }
        let rows: Vec<Vec<String>> = tags
            .into_iter()
            .map(|tag| vec![tag.tag, tag.facts.to_string()])
            .collect();
        Ok(text(table(&["tag", "facts"], &rows)))
    }

    async fn overview(&self) -> Outcome {
        let summary = scheduling::summary_for(&self.pool, self.profile.id).await?;
        let settings = scheduling::settings_for(&self.pool, self.profile.id).await?;
        Ok(text(format!(
            "Ready now: {} due for review, {} new within today's limit.\n\
             Today's study budget: {}.\n\
             Settings: up to {} new facts a day, a {}-minute daily budget, timezone {}, \
             desired retention {:.0}%.",
            summary.due,
            summary.new_available,
            if summary.budget_reached {
                "spent"
            } else {
                "not yet spent"
            },
            settings.new_limit,
            settings.daily_budget_seconds / 60,
            settings.timezone,
            settings.desired_retention * 100.0,
        )))
    }
}
