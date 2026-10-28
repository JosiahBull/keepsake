//! Keepsake for agents: one profile's library over the Model Context Protocol.
//!
//! A second front door beside the HTTP API, built on `keepsake-core` and nothing else
//! in the workspace. The server crate decides whether and where it is mounted.
//!
//! # Not the API with a different name
//!
//! The tools are shaped like tasks an agent has, such as finding facts or saving a
//! reviewed batch, rather than one per route. Reviews, approvals, sharing, settings,
//! backups and anything that removes data are out of reach at every access level: the
//! learner rates their own attempts, and the rest are household decisions.
//!
//! # What crosses this boundary
//!
//! Everything a tool returns goes to whichever model the connecting client runs. That
//! is why the surface is off unless `KEEPSAKE_MCP` allows it, why each profile opts in
//! separately, and why writing needs a second, higher setting. See `docs/mcp.md`.

pub mod config;
mod convert;
mod error;
pub mod http;
pub mod manifest;
pub mod server;
pub mod tools;

pub use config::{McpConfig, McpMode};
pub use http::http_service;
pub use server::KeepsakeMcp;
