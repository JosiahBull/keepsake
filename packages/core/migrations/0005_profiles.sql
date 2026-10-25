-- Ownership is added without rewriting historical IDs or JSON evidence.
CREATE TABLE profiles (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
    handle TEXT NOT NULL UNIQUE COLLATE NOCASE,
    setup_complete INTEGER NOT NULL CHECK (setup_complete IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
INSERT INTO profiles(id, name, handle, setup_complete) VALUES (1, 'Existing profile', 'default', 0);

ALTER TABLE facts ADD COLUMN profile_id INTEGER REFERENCES profiles(id) ON DELETE RESTRICT;
UPDATE facts SET profile_id = 1;
CREATE INDEX facts_profile_page ON facts(profile_id, id DESC);
CREATE TRIGGER facts_require_owner BEFORE INSERT ON facts WHEN NEW.profile_id IS NULL
BEGIN SELECT RAISE(ABORT, 'fact requires profile'); END;
CREATE TRIGGER facts_owner_immutable BEFORE UPDATE OF profile_id ON facts WHEN NEW.profile_id IS NOT OLD.profile_id
BEGIN SELECT RAISE(ABORT, 'fact owner is immutable'); END;

ALTER TABLE reviews ADD COLUMN profile_id INTEGER REFERENCES profiles(id) ON DELETE RESTRICT;
UPDATE reviews SET profile_id = 1;
CREATE INDEX reviews_profile ON reviews(profile_id, fact_id, id);
CREATE TRIGGER reviews_require_owner BEFORE INSERT ON reviews
WHEN NEW.profile_id IS NULL OR NOT EXISTS (SELECT 1 FROM facts WHERE id=NEW.fact_id AND profile_id=NEW.profile_id)
BEGIN SELECT RAISE(ABORT, 'review owner must match fact'); END;

ALTER TABLE study_events ADD COLUMN profile_id INTEGER REFERENCES profiles(id) ON DELETE RESTRICT;
UPDATE study_events SET profile_id = 1;
CREATE INDEX study_events_profile_time ON study_events(profile_id, occurred_at, id);
CREATE TRIGGER events_require_owner BEFORE INSERT ON study_events
WHEN NEW.profile_id IS NULL OR NOT EXISTS (SELECT 1 FROM learning_targets t JOIN facts f ON f.id=t.fact_id WHERE t.id=NEW.target_id AND f.profile_id=NEW.profile_id)
BEGIN SELECT RAISE(ABORT, 'event owner must match target'); END;

ALTER TABLE study_exposure_batches ADD COLUMN profile_id INTEGER REFERENCES profiles(id) ON DELETE RESTRICT;
UPDATE study_exposure_batches SET profile_id = 1;
CREATE TRIGGER batches_require_owner BEFORE INSERT ON study_exposure_batches WHEN NEW.profile_id IS NULL
BEGIN SELECT RAISE(ABORT, 'exposure batch requires profile'); END;

ALTER TABLE scheduler_settings RENAME TO legacy_scheduler_settings;
CREATE TABLE scheduler_settings (
    profile_id INTEGER PRIMARY KEY REFERENCES profiles(id) ON DELETE RESTRICT,
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    legacy_review_cutoff INTEGER NOT NULL
) STRICT;
INSERT INTO scheduler_settings SELECT 1, value_json, legacy_review_cutoff FROM legacy_scheduler_settings;
DROP TABLE legacy_scheduler_settings;

CREATE TABLE fact_shares (
    id INTEGER PRIMARY KEY,
    sender_id INTEGER NOT NULL REFERENCES profiles(id),
    recipient_id INTEGER NOT NULL REFERENCES profiles(id),
    source_fact_id INTEGER NOT NULL,
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    fingerprint TEXT NOT NULL,
    question TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'declined', 'withdrawn')),
    accepted_fact_id INTEGER REFERENCES facts(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    resolved_at TEXT,
    CHECK (sender_id <> recipient_id)
) STRICT;
CREATE UNIQUE INDEX shares_pending_unique ON fact_shares(sender_id, recipient_id, source_fact_id, fingerprint) WHERE status='pending';
CREATE INDEX shares_recipient ON fact_shares(recipient_id, id DESC);
CREATE INDEX shares_sender ON fact_shares(sender_id, id DESC);
CREATE TABLE share_submissions (
    submission_id TEXT PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES profiles(id),
    request_json TEXT NOT NULL CHECK (json_valid(request_json)),
    result_json TEXT NOT NULL CHECK (json_valid(result_json))
) STRICT;

CREATE TRIGGER reviews_owner_immutable BEFORE UPDATE OF profile_id, fact_id ON reviews
WHEN NEW.profile_id IS NOT OLD.profile_id OR NEW.fact_id IS NOT OLD.fact_id
BEGIN SELECT RAISE(ABORT, 'review owner is immutable'); END;
CREATE TRIGGER events_owner_immutable BEFORE UPDATE OF profile_id, target_id ON study_events
WHEN NEW.profile_id IS NOT OLD.profile_id OR NEW.target_id IS NOT OLD.target_id
BEGIN SELECT RAISE(ABORT, 'event owner is immutable'); END;
CREATE TRIGGER batches_owner_immutable BEFORE UPDATE OF profile_id ON study_exposure_batches
WHEN NEW.profile_id IS NOT OLD.profile_id
BEGIN SELECT RAISE(ABORT, 'exposure owner is immutable'); END;
CREATE TRIGGER shares_owner_required BEFORE INSERT ON fact_shares
WHEN NOT EXISTS (SELECT 1 FROM facts WHERE id=NEW.source_fact_id AND profile_id=NEW.sender_id)
BEGIN SELECT RAISE(ABORT, 'share source must belong to sender'); END;
CREATE TRIGGER shares_snapshot_immutable BEFORE UPDATE OF sender_id, recipient_id, source_fact_id, snapshot_json, fingerprint, question ON fact_shares
WHEN NEW.sender_id IS NOT OLD.sender_id OR NEW.recipient_id IS NOT OLD.recipient_id
 OR NEW.source_fact_id IS NOT OLD.source_fact_id OR NEW.snapshot_json IS NOT OLD.snapshot_json
 OR NEW.fingerprint IS NOT OLD.fingerprint OR NEW.question IS NOT OLD.question
BEGIN SELECT RAISE(ABORT, 'share snapshot is immutable'); END;
CREATE TRIGGER shares_copy_required BEFORE UPDATE OF accepted_fact_id, status ON fact_shares
WHEN (NEW.status='accepted' AND NOT EXISTS (SELECT 1 FROM facts WHERE id=NEW.accepted_fact_id AND profile_id=NEW.recipient_id))
 OR (NEW.status<>'accepted' AND NEW.accepted_fact_id IS NOT NULL)
 OR (OLD.status<>'pending' AND NEW.status<>OLD.status)
BEGIN SELECT RAISE(ABORT, 'invalid share resolution'); END;
CREATE TRIGGER practices_fact_immutable BEFORE UPDATE OF fact_id ON practices WHEN NEW.fact_id IS NOT OLD.fact_id
BEGIN SELECT RAISE(ABORT, 'practice owner is immutable'); END;
CREATE TRIGGER targets_fact_immutable BEFORE UPDATE OF fact_id ON learning_targets WHEN NEW.fact_id IS NOT OLD.fact_id
BEGIN SELECT RAISE(ABORT, 'target owner is immutable'); END;
CREATE TRIGGER accepted_copy_immutable BEFORE UPDATE OF accepted_fact_id ON fact_shares
WHEN OLD.status<>'pending' AND NEW.accepted_fact_id IS NOT OLD.accepted_fact_id
BEGIN SELECT RAISE(ABORT, 'accepted copy is immutable'); END;
