-- The server's VAPID identity. Replacing it invalidates every push subscription.
CREATE TABLE push_keys (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    private_key BLOB NOT NULL CHECK (length(private_key) = 32),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

-- One browser subscription can receive reminders for several household profiles.
CREATE TABLE push_devices (
    id INTEGER PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES profiles(id) ON DELETE RESTRICT,
    endpoint TEXT NOT NULL CHECK (length(endpoint) BETWEEN 1 AND 2048),
    -- Notification links open this origin, as seen by the browser that subscribed.
    origin TEXT NOT NULL CHECK (length(origin) BETWEEN 1 AND 255),
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (profile_id, endpoint)
) STRICT;
CREATE INDEX push_devices_endpoint ON push_devices(endpoint);

-- The local date of the last reminder makes delivery at most once per profile per day.
CREATE TABLE reminder_settings (
    profile_id INTEGER PRIMARY KEY REFERENCES profiles(id) ON DELETE RESTRICT,
    minute_of_day INTEGER NOT NULL CHECK (minute_of_day BETWEEN 0 AND 1439),
    last_sent_on TEXT
) STRICT;
