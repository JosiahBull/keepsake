//! The tool contract, committed as `tool-manifest.json` so a change to it is a diff.
//!
//! Regenerate with `cargo test -p keepsake-mcp -- --ignored update_the_tool_manifest`.

use serde::Serialize;

use crate::config::McpMode;
use crate::server::KeepsakeMcp;

#[derive(Debug, Serialize)]
pub struct ToolSummary {
    pub name: String,
    /// `read` for a tool served with read access, `write` for one added by write access.
    pub tier: &'static str,
    pub description: String,
    pub read_only: Option<bool>,
    pub destructive: Option<bool>,
    pub idempotent: Option<bool>,
    /// Argument names, sorted. Full schemas churn with schemars and drown real changes.
    pub arguments: Vec<String>,
    pub required: Vec<String>,
}

fn names(value: Option<&serde_json::Value>) -> Vec<String> {
    let mut names: Vec<String> = match value {
        Some(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    names.sort();
    names
}

pub fn manifest() -> Vec<ToolSummary> {
    let read: std::collections::HashSet<String> = KeepsakeMcp::router_for(McpMode::Read)
        .list_all()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    let mut tools: Vec<ToolSummary> = KeepsakeMcp::router_for(McpMode::Write)
        .list_all()
        .into_iter()
        .map(|tool| {
            let annotations = tool.annotations.as_ref();
            ToolSummary {
                tier: if read.contains(tool.name.as_ref()) {
                    "read"
                } else {
                    "write"
                },
                name: tool.name.to_string(),
                description: tool.description.map(|d| d.to_string()).unwrap_or_default(),
                read_only: annotations.and_then(|a| a.read_only_hint),
                destructive: annotations.and_then(|a| a.destructive_hint),
                idempotent: annotations.and_then(|a| a.idempotent_hint),
                arguments: names(tool.input_schema.get("properties")),
                required: names(tool.input_schema.get("required")),
            }
        })
        .collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../tool-manifest.json");

    fn rendered() -> String {
        let mut json = serde_json::to_string_pretty(&manifest()).expect("manifest serialises");
        json.push('\n');
        json
    }

    #[test]
    fn the_committed_manifest_matches_the_registered_tools() {
        assert_eq!(
            rendered(),
            MANIFEST,
            "the tool surface changed. Review it, then regenerate with:\n  \
             cargo test -p keepsake-mcp -- --ignored update_the_tool_manifest"
        );
    }

    /// The failure this design exists to prevent: a tool that writes, served read-only.
    #[test]
    fn read_access_serves_only_tools_that_cannot_write() {
        for tool in KeepsakeMcp::router_for(McpMode::Read).list_all() {
            let read_only = tool.annotations.as_ref().and_then(|a| a.read_only_hint);
            assert_eq!(read_only, Some(true), "{} is served read-only", tool.name);
        }
    }

    #[test]
    fn write_access_adds_tools_and_each_admits_it_writes() {
        let read = KeepsakeMcp::router_for(McpMode::Read).list_all().len();
        let write = manifest();
        assert!(write.len() > read);
        for tool in write.iter().filter(|tool| tool.tier == "write") {
            assert_eq!(tool.read_only, Some(false), "{}", tool.name);
            assert_eq!(tool.destructive, Some(false), "{} only adds", tool.name);
        }
    }

    #[test]
    fn off_serves_nothing_at_all() {
        assert!(KeepsakeMcp::router_for(McpMode::Off).list_all().is_empty());
    }

    #[test]
    fn every_tool_describes_itself() {
        for tool in manifest() {
            assert!(
                tool.description.len() > 60,
                "{} needs a description",
                tool.name
            );
        }
    }

    #[test]
    #[ignore = "regenerates the committed manifest; run deliberately"]
    fn update_the_tool_manifest() {
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tool-manifest.json"),
            rendered(),
        )
        .expect("manifest is writable");
    }
}
