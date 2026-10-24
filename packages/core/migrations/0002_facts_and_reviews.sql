CREATE TABLE facts (
    id INTEGER PRIMARY KEY,
    prompt TEXT NOT NULL CHECK (length(trim(prompt)) > 0),
    answer TEXT NOT NULL CHECK (length(trim(answer)) > 0),
    context TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tags)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE practices (
    id INTEGER PRIMARY KEY,
    fact_id INTEGER NOT NULL REFERENCES facts(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('flashcard', 'multiple_choice', 'cloze', 'explain', 'apply')),
    prompt TEXT NOT NULL CHECK (length(trim(prompt)) > 0),
    answer TEXT NOT NULL CHECK (length(trim(answer)) > 0),
    choices TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(choices)),
    explanation TEXT NOT NULL DEFAULT ''
) STRICT;
CREATE INDEX practices_fact ON practices(fact_id);

CREATE TABLE reviews (
    id INTEGER PRIMARY KEY,
    fact_id INTEGER NOT NULL REFERENCES facts(id) ON DELETE CASCADE,
    practice_id INTEGER REFERENCES practices(id) ON DELETE CASCADE,
    method TEXT NOT NULL CHECK (method IN ('flashcard', 'multiple_choice', 'cloze', 'explain', 'apply', 'matching')),
    outcome TEXT NOT NULL CHECK (outcome IN ('again', 'hard', 'good', 'easy')),
    response_ms INTEGER NOT NULL CHECK (response_ms >= 0),
    reviewed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE INDEX reviews_fact_time ON reviews(fact_id, reviewed_at);
