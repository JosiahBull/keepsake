CREATE TABLE learning_targets (
    id INTEGER PRIMARY KEY,
    fact_id INTEGER NOT NULL REFERENCES facts(id) ON DELETE RESTRICT,
    ability TEXT NOT NULL DEFAULT 'recall' CHECK (ability = 'recall'),
    direction TEXT NOT NULL DEFAULT 'forward' CHECK (direction = 'forward'),
    content_revision INTEGER NOT NULL DEFAULT 1 CHECK (content_revision > 0),
    prompt TEXT NOT NULL,
    answer TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    UNIQUE (fact_id, content_revision)
) STRICT;
CREATE UNIQUE INDEX targets_active_fact ON learning_targets(fact_id) WHERE active = 1;

CREATE TABLE target_practices (
    practice_id INTEGER PRIMARY KEY REFERENCES practices(id) ON DELETE RESTRICT,
    target_id INTEGER NOT NULL REFERENCES learning_targets(id) ON DELETE RESTRICT,
    qualifying INTEGER NOT NULL CHECK (qualifying IN (0, 1))
) STRICT;
CREATE INDEX target_practices_target ON target_practices(target_id, qualifying);

CREATE TABLE schedule_states (
    target_id INTEGER PRIMARY KEY REFERENCES learning_targets(id) ON DELETE RESTRICT,
    phase TEXT NOT NULL CHECK (phase IN ('new', 'learning', 'review', 'relearning')),
    due_at INTEGER,
    blocked_until INTEGER,
    introduced_at INTEGER,
    version INTEGER NOT NULL CHECK (version > 0),
    state_json TEXT NOT NULL CHECK (json_valid(state_json)),
    CHECK (json_extract(state_json, '$.version') = version),
    CHECK (json_extract(state_json, '$.phase') = phase),
    CHECK (json_extract(state_json, '$.due_at') IS due_at),
    CHECK (json_extract(state_json, '$.blocked_until') IS blocked_until),
    CHECK (json_extract(state_json, '$.introduced_at') IS introduced_at)
) STRICT;
CREATE INDEX schedules_due ON schedule_states(phase, due_at, blocked_until);
CREATE INDEX schedules_introduced ON schedule_states(introduced_at);

CREATE TABLE scheduler_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    legacy_review_cutoff INTEGER NOT NULL
) STRICT;
INSERT INTO scheduler_settings(id, value_json, legacy_review_cutoff)
SELECT 1, '{"desired_retention":0.9,"new_limit":5,"daily_budget_seconds":900,"timezone":"Europe/London","algorithm":"fsrs-6.6.2","parameter_version":1,"parameters":[0.212,1.2931,2.3065,8.2956,6.4133,0.8334,3.0194,0.001,1.8722,0.1666,0.796,1.4835,0.0614,0.2629,1.6483,0.6014,1.8729,0.5425,0.0912,0.0658,0.1542],"version":1}', coalesce(max(id), 0) FROM reviews;

CREATE TABLE study_events (
    id INTEGER PRIMARY KEY,
    submission_id TEXT NOT NULL UNIQUE,
    target_id INTEGER NOT NULL REFERENCES learning_targets(id) ON DELETE RESTRICT,
    content_revision INTEGER NOT NULL,
    occurred_at INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('attempt', 'exposure')),
    context TEXT NOT NULL CHECK (context IN ('scheduled', 'supplementary', 'audit')),
    method TEXT NOT NULL CHECK (method IN ('flashcard', 'multiple_choice', 'cloze', 'explain', 'apply', 'matching')),
    outcome TEXT CHECK (outcome IN ('again', 'hard', 'good', 'easy')),
    response_ms INTEGER NOT NULL CHECK (response_ms BETWEEN 0 AND 86400000),
    qualifying INTEGER NOT NULL CHECK (qualifying IN (0, 1)),
    algorithm TEXT NOT NULL,
    parameter_version INTEGER NOT NULL,
    elapsed_seconds INTEGER NOT NULL CHECK (elapsed_seconds >= 0),
    model_interval_days REAL,
    policy_override_seconds INTEGER,
    request_json TEXT NOT NULL CHECK (json_valid(request_json)),
    before_json TEXT NOT NULL CHECK (json_valid(before_json)),
    after_json TEXT NOT NULL CHECK (json_valid(after_json)),
    result_json TEXT NOT NULL CHECK (json_valid(result_json))
) STRICT;
CREATE INDEX study_events_target_time ON study_events(target_id, occurred_at, id);
CREATE INDEX study_events_time ON study_events(occurred_at);

CREATE TABLE study_exposure_batches (
    submission_id TEXT PRIMARY KEY,
    request_json TEXT NOT NULL CHECK (json_valid(request_json)),
    result_json TEXT NOT NULL CHECK (json_valid(result_json))
) STRICT;

-- Legacy reviews remain intact and are not treated as verified recall evidence.
INSERT INTO practices(fact_id, kind, prompt, answer)
SELECT f.id, 'flashcard', f.prompt, f.answer FROM facts f
WHERE NOT EXISTS (
    SELECT 1 FROM practices p WHERE p.fact_id = f.id AND p.kind = 'flashcard'
      AND p.prompt = f.prompt AND p.answer = f.answer
);
INSERT INTO learning_targets(fact_id, prompt, answer) SELECT id, prompt, answer FROM facts;
INSERT INTO target_practices(practice_id, target_id, qualifying)
SELECT p.id, t.id, p.kind = 'flashcard' AND p.prompt = t.prompt AND p.answer = t.answer
FROM practices p JOIN learning_targets t ON t.fact_id = p.fact_id;
INSERT INTO schedule_states(target_id, phase, version, state_json)
SELECT id, 'new', 1, json_object(
    'target_id', id, 'fact_id', fact_id, 'content_revision', content_revision,
    'phase', 'new', 'due_at', NULL, 'last_review_at', NULL, 'stability', NULL,
    'difficulty', NULL, 'lapses', 0, 'version', 1, 'introduced_at', NULL, 'blocked_until', NULL
) FROM learning_targets;
