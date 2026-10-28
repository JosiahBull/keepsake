//! MCP over Streamable HTTP, as a plain tower service for one profile.
//!
//! The server crate decides where it is mounted, so requests pass through the same
//! middleware as the API: request IDs, panic recovery, admission limits and deadlines.

use std::sync::Arc;

use keepsake_core::profiles::Profile;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use sqlx::SqlitePool;

use crate::config::{MAX_REQUEST_BYTES, McpConfig};
use crate::server::KeepsakeMcp;

/// The endpoint for one profile's library.
///
/// Stateless JSON: every call is one POST and one JSON body, with no held-open stream
/// for a graceful shutdown to wait on.
///
/// Two checks guard a server with no authentication. `Host` must be loopback or listed
/// in `KEEPSAKE_MCP_ALLOWED_HOSTS`, which defeats DNS rebinding. Any request carrying a
/// browser `Origin` is refused, so a web page cannot drive the tools; MCP clients send
/// none.
pub fn http_service(
    pool: SqlitePool,
    config: &McpConfig,
    profile: Profile,
) -> StreamableHttpService<KeepsakeMcp, LocalSessionManager> {
    let mut server_config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(MAX_REQUEST_BYTES)
        .enforce_origin_validation();
    // Extended, not replaced: loopback stays reachable for local clients.
    server_config
        .allowed_hosts
        .extend(config.allowed_hosts.iter().cloned());
    let ceiling = config.ceiling;
    StreamableHttpService::new(
        move || Ok(KeepsakeMcp::new(pool.clone(), ceiling, profile.clone())),
        Arc::new(LocalSessionManager::default()),
        server_config,
    )
}
