//! What the MCP surface may do, as plain data. Reading the environment is the server's
//! job; it builds this and hands it over.

pub use keepsake_core::agent_access::McpMode;

#[derive(Clone, Debug, Default)]
pub struct McpConfig {
    /// The most this process will ever serve, from `KEEPSAKE_MCP`.
    ///
    /// A ceiling rather than the mode itself. Each profile chooses its own mode in the
    /// app, and what is served is that choice clamped to this. Unset means
    /// [`McpMode::Off`], so no setting in the app can turn anything on: enabling agent
    /// access takes someone with access to the host, not just to the app.
    pub ceiling: McpMode,
    /// `Host` authorities accepted besides loopback, from `KEEPSAKE_MCP_ALLOWED_HOSTS`.
    pub allowed_hosts: Vec<String>,
}

/// The most facts a search returns at once. Enough to answer "what do I have about X",
/// short of a dump that costs more context than the question was worth.
pub const MAX_ROWS: i64 = 50;

/// Room for one 5 MiB file as base64 inside a JSON-RPC request.
pub const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
