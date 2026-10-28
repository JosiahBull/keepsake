-- Images and audio clips asked with a fact's question. Each is checked, converted to the
-- one format it is served in, and written beside the database as
-- media/images/<file_sha256>.<ext> or media/audio/<file_sha256>.m4a. Rows describe those
-- files; the bytes never enter SQLite, and the upload itself is not kept.
-- AUTOINCREMENT keeps IDs unique forever: browsers cache media by ID as immutable.
CREATE TABLE media (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('image', 'audio')),
    -- What is served: AVIF for still images, the checked upload for animations, and AAC
    -- in MP4 for clips. Nothing else can be stored by any route.
    content_type TEXT NOT NULL CHECK (
        (kind = 'image' AND content_type IN ('image/avif', 'image/gif', 'image/webp', 'image/png'))
        OR (kind = 'audio' AND content_type = 'audio/mp4')
    ),
    -- The upload's hash, so the same file uploaded again is not processed again.
    upload_sha256 TEXT NOT NULL CHECK (length(upload_sha256) = 64),
    -- The served file's hash, which names it on disk.
    file_sha256 TEXT NOT NULL CHECK (length(file_sha256) = 64),
    byte_size INTEGER NOT NULL CHECK (byte_size BETWEEN 1 AND 5242880),
    width INTEGER CHECK (width BETWEEN 1 AND 4096),
    height INTEGER CHECK (height BETWEEN 1 AND 4096),
    duration_ms INTEGER CHECK (duration_ms BETWEEN 0 AND 300000),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    CHECK ((kind = 'image') = (width IS NOT NULL AND height IS NOT NULL)),
    CHECK ((kind = 'audio') = (duration_ms IS NOT NULL)),
    UNIQUE (profile_id, upload_sha256)
) STRICT;
CREATE INDEX media_file ON media(file_sha256);

CREATE TABLE fact_media (
    fact_id INTEGER NOT NULL REFERENCES facts(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 3),
    media_id INTEGER NOT NULL REFERENCES media(id) ON DELETE RESTRICT,
    alt TEXT NOT NULL DEFAULT '' CHECK (length(CAST(alt AS BLOB)) <= 1000),
    PRIMARY KEY (fact_id, position)
) STRICT;
CREATE INDEX fact_media_media ON fact_media(media_id);

CREATE TRIGGER fact_media_owner BEFORE INSERT ON fact_media
WHEN NOT EXISTS (
  SELECT 1 FROM facts f JOIN media m ON m.profile_id = f.profile_id
  WHERE f.id = NEW.fact_id AND m.id = NEW.media_id
)
BEGIN SELECT RAISE(ABORT, 'media owner must match fact'); END;
CREATE TRIGGER fact_media_immutable BEFORE UPDATE ON fact_media
BEGIN SELECT RAISE(ABORT, 'question media is immutable'); END;
CREATE TRIGGER media_immutable BEFORE UPDATE ON media
BEGIN SELECT RAISE(ABORT, 'media is immutable'); END;
