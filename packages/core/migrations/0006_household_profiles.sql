-- Keep the original library ID and handle when naming its untouched placeholder.
-- Existing named profiles and their settings are retained.
UPDATE profiles
SET name = 'Josiah', setup_complete = 1
WHERE handle = 'default' AND name = 'Existing profile' AND setup_complete = 0
  AND NOT EXISTS (SELECT 1 FROM profiles WHERE lower(trim(name)) = 'josiah');

-- Reuse a named household member; otherwise choose an unused stable handle.
WITH RECURSIVE candidates(suffix, handle) AS (
    VALUES (0, 'josiah')
    UNION ALL
    SELECT suffix + 1, 'josiah-' || (suffix + 1)
    FROM candidates
    WHERE EXISTS (SELECT 1 FROM profiles WHERE profiles.handle = candidates.handle)
)
INSERT INTO profiles(name, handle, setup_complete)
SELECT 'Josiah', handle, 1
FROM candidates
WHERE NOT EXISTS (SELECT 1 FROM profiles WHERE lower(trim(name)) = 'josiah')
  AND NOT EXISTS (SELECT 1 FROM profiles WHERE profiles.handle = candidates.handle)
ORDER BY suffix
LIMIT 1;

-- Reuse a named household member; otherwise choose an unused stable handle.
WITH RECURSIVE candidates(suffix, handle) AS (
    VALUES (0, 'ansam')
    UNION ALL
    SELECT suffix + 1, 'ansam-' || (suffix + 1)
    FROM candidates
    WHERE EXISTS (SELECT 1 FROM profiles WHERE profiles.handle = candidates.handle)
)
INSERT INTO profiles(name, handle, setup_complete)
SELECT 'Ansam', handle, 1
FROM candidates
WHERE NOT EXISTS (SELECT 1 FROM profiles WHERE lower(trim(name)) = 'ansam')
  AND NOT EXISTS (SELECT 1 FROM profiles WHERE profiles.handle = candidates.handle)
ORDER BY suffix
LIMIT 1;

UPDATE profiles SET setup_complete = 1
WHERE lower(trim(name)) IN ('ansam', 'josiah');

-- New members receive default settings, rather than another member's preferences.
INSERT INTO scheduler_settings(profile_id, value_json, legacy_review_cutoff)
SELECT p.id, '{"desired_retention":0.9,"new_limit":5,"daily_budget_seconds":900,"timezone":"Europe/London","algorithm":"fsrs-6.6.2","parameter_version":1,"parameters":[0.212,1.2931,2.3065,8.2956,6.4133,0.8334,3.0194,0.001,1.8722,0.1666,0.796,1.4835,0.0614,0.2629,1.6483,0.6014,1.8729,0.5425,0.0912,0.0658,0.1542],"version":1}', coalesce((SELECT max(id) FROM reviews), 0)
FROM profiles p
WHERE lower(trim(p.name)) IN ('ansam', 'josiah')
  AND NOT EXISTS (SELECT 1 FROM scheduler_settings s WHERE s.profile_id = p.id);
