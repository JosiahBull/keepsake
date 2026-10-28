-- Empty fixture for repository/API behavior tests. Production defaults are
-- tested separately; this does not remove schema or the migration ledger.
DELETE FROM share_submissions;
DELETE FROM fact_shares;
DELETE FROM study_exposure_batches;
DELETE FROM study_events;
DELETE FROM reviews;
DELETE FROM target_practices;
DELETE FROM schedule_states;
DELETE FROM learning_targets;
DELETE FROM practices;
DELETE FROM fact_media;
DELETE FROM facts;
DELETE FROM deleted_facts;
DELETE FROM media;
DELETE FROM scheduler_settings;
DELETE FROM push_devices;
DELETE FROM reminder_settings;
DELETE FROM profiles;
INSERT INTO profiles(id, name, handle, setup_complete)
VALUES (1, 'Existing profile', 'default', 0);
INSERT INTO scheduler_settings(profile_id, value_json, legacy_review_cutoff)
VALUES (
    1,
    '{"desired_retention":0.9,"new_limit":5,"daily_budget_seconds":900,"timezone":"Europe/London","algorithm":"fsrs-6.6.2","parameter_version":1,"parameters":[0.212,1.2931,2.3065,8.2956,6.4133,0.8334,3.0194,0.001,1.8722,0.1666,0.796,1.4835,0.0614,0.2629,1.6483,0.6014,1.8729,0.5425,0.0912,0.0658,0.1542],"version":1}',
    0
);
