-- A fact added offline may be sent more than once. Its idempotency key returns the fact
-- the first request created instead of adding another.
CREATE TABLE fact_submissions (
    submission_id TEXT PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    -- Not a foreign key: a deleted fact must not let a late retry add it again.
    fact_id INTEGER NOT NULL,
    request_json TEXT NOT NULL CHECK (json_valid(request_json))
) STRICT;
