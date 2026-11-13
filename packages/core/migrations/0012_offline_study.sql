-- An attempt made offline can reach the server after its target changed, for example
-- after another device reviewed it. It is kept as history with the reason it was not
-- applied, and leaves the schedule as it was. NULL means the event was applied.
ALTER TABLE study_events ADD COLUMN ignored TEXT;
