-- Whether an image or clip asked with a question was made by a generative model. It is
-- shown wherever the question is, and shares keep it. Generated images in the household
-- lists also carry a visible watermark; this flag does not depend on one.
ALTER TABLE fact_media ADD COLUMN ai_generated INTEGER NOT NULL DEFAULT 0
    CHECK (ai_generated IN (0, 1));

-- Facts the server added to a library from one of the household's default lists, by the
-- item's key in that list. Each item is added once: a later version of a list adds only
-- its new items, and an item edited after it was added is not added again.
CREATE TABLE default_list_items (
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
    list TEXT NOT NULL CHECK (length(list) BETWEEN 1 AND 100),
    item TEXT NOT NULL CHECK (length(item) BETWEEN 1 AND 200),
    fact_id INTEGER NOT NULL REFERENCES facts(id),
    added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (profile_id, list, item)
) STRICT;
