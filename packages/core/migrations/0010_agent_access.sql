-- How much of each library an MCP client may reach, chosen in that profile's settings.
-- KEEPSAKE_MCP is a ceiling this value can only sit at or below (off < read < write):
-- with it unset, nothing is served whatever is stored here, so turning agent access on
-- needs someone with access to the host as well as the app. Off by default, because
-- serving a library sends its questions and answers to whichever model a client runs.
ALTER TABLE profiles ADD COLUMN mcp_mode TEXT NOT NULL DEFAULT 'off'
    CHECK (mcp_mode IN ('off', 'read', 'write'));
