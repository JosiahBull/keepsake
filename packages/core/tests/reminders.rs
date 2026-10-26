mod support;
use support as db;

use chrono::DateTime;
use keepsake_core::{
    facts::{self, NewFact, ReviewMethod, ReviewOutcome},
    profiles::{self, NewProfile},
    reminders::{self, DeviceKeys, NewDevice, ReminderUpdate},
    scheduling::{
        self, EventKind, QueueQuery, SettingsUpdate, StudyContext, StudyError, StudySubmission,
    },
};

fn timestamp(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value).unwrap().timestamp()
}

async fn temporary_pool() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        directory.path().join("reminders.sqlite3").display()
    ))
    .await
    .unwrap();
    (directory, pool)
}

fn fact(index: usize) -> NewFact {
    NewFact {
        prompt: format!("Synthetic reminder question {index}?"),
        answer: format!("Synthetic reminder answer {index}"),
        context: "Synthetic reminder test data".into(),
        source: String::new(),
        tags: vec![],
        practices: vec![],
    }
}

fn device(endpoint: &str, p256dh: &str) -> NewDevice {
    NewDevice {
        endpoint: endpoint.into(),
        keys: DeviceKeys {
            p256dh: p256dh.into(),
            auth: "c3ludGhldGljLWF1dGg".into(),
        },
    }
}

const ORIGIN: &str = "https://keepsake.example";

async fn set_timezone(pool: &sqlx::SqlitePool, timezone: &str, daily_budget_seconds: i64) {
    let settings = scheduling::settings_for(pool, 1).await.unwrap();
    scheduling::update_settings_for(
        pool,
        1,
        SettingsUpdate {
            timezone: timezone.into(),
            new_limit: settings.new_limit,
            daily_budget_seconds,
            expected_version: settings.version,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn summary_counts_the_queues_ready_work_without_overstating_new_intake() {
    let (_directory, pool) = temporary_pool().await;
    let now = timestamp("2026-10-20T12:00:00Z");
    let empty = scheduling::summary_at_for(&pool, 1, now).await.unwrap();
    assert_eq!((empty.due, empty.new_available), (0, 0));

    facts::create_many(&pool, (1..=7).map(fact).collect())
        .await
        .unwrap();
    let fresh = scheduling::summary_at_for(&pool, 1, now).await.unwrap();
    assert_eq!((fresh.due, fresh.new_available), (0, 5));

    let queue = scheduling::queue_at_for(&pool, 1, &QueueQuery::default(), now)
        .await
        .unwrap();
    let question = &queue.items[0];
    scheduling::submit_at_for(
        &pool,
        1,
        StudySubmission {
            submission_id: uuid::Uuid::new_v4().to_string(),
            target_id: question.target_id,
            practice_id: Some(question.practice.id),
            feedback_event_id: None,
            content_revision: question.content_revision,
            expected_version: question.schedule_version,
            kind: EventKind::Attempt,
            context: StudyContext::Scheduled,
            method: ReviewMethod::Flashcard,
            outcome: Some(ReviewOutcome::Again),
            response_ms: 2_000,
            attempted: true,
            assisted: false,
            prior_exposure: false,
        },
        now,
    )
    .await
    .unwrap();
    let waiting = scheduling::summary_at_for(&pool, 1, now + 599)
        .await
        .unwrap();
    assert_eq!((waiting.due, waiting.new_available), (0, 4));
    let retry = scheduling::summary_at_for(&pool, 1, now + 600)
        .await
        .unwrap();
    assert_eq!((retry.due, retry.new_available), (1, 4));
    let queued = scheduling::queue_at_for(&pool, 1, &QueueQuery::default(), now + 600)
        .await
        .unwrap();
    assert_eq!(queued.total_due, retry.due);
    pool.close().await;
}

#[tokio::test]
async fn devices_are_idempotent_shared_across_profiles_and_bounded() {
    let (_directory, pool) = temporary_pool().await;
    let partner = profiles::create(
        &pool,
        NewProfile {
            name: "Partner".into(),
            handle: "partner".into(),
        },
    )
    .await
    .unwrap();
    let endpoint = "https://push.example/subscription/shared";
    for _ in 0..2 {
        let saved =
            reminders::register_device_for(&pool, 1, ORIGIN.into(), device(endpoint, "old"))
                .await
                .unwrap();
        assert_eq!(saved.devices, 1);
        assert_eq!(saved.minute_of_day, reminders::DEFAULT_MINUTE_OF_DAY);
    }
    reminders::register_device_for(&pool, partner.id, ORIGIN.into(), device(endpoint, "new"))
        .await
        .unwrap();
    // Re-subscribing rotates keys; every profile must encrypt with the latest ones.
    for profile_id in [1, partner.id] {
        let devices = reminders::devices_for(&pool, profile_id).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].p256dh, "new");
        assert_eq!(devices[0].origin, ORIGIN);
    }
    assert_eq!(
        reminders::subscribed_profiles(&pool).await.unwrap(),
        vec![1, partner.id]
    );

    let removed = reminders::remove_device_for(&pool, 1, endpoint)
        .await
        .unwrap();
    assert_eq!(removed.devices, 0);
    assert_eq!(
        reminders::devices_for(&pool, partner.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        reminders::forget_endpoint(&pool, endpoint).await.unwrap(),
        1
    );
    assert!(
        reminders::subscribed_profiles(&pool)
            .await
            .unwrap()
            .is_empty()
    );

    for index in 0..reminders::MAX_DEVICES + 3 {
        reminders::register_device_for(
            &pool,
            1,
            ORIGIN.into(),
            device(&format!("https://push.example/subscription/{index}"), "key"),
        )
        .await
        .unwrap();
    }
    let devices = reminders::devices_for(&pool, 1).await.unwrap();
    assert_eq!(devices.len() as i64, reminders::MAX_DEVICES);
    assert_eq!(devices[0].endpoint, "https://push.example/subscription/3");
    pool.close().await;
}

#[tokio::test]
async fn invalid_reminder_input_is_rejected_before_storage() {
    let (_directory, pool) = temporary_pool().await;
    for (origin, candidate) in [
        (ORIGIN, device("", "key")),
        (ORIGIN, device("https://push.example/with space", "key")),
        (ORIGIN, device("https://push.example/ok", "not base64!")),
        ("", device("https://push.example/ok", "key")),
    ] {
        let error = reminders::register_device_for(&pool, 1, origin.into(), candidate)
            .await
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<StudyError>(),
            Some(StudyError::Invalid(_))
        ));
    }
    for minute_of_day in [-1, 1440] {
        let error = reminders::update_settings_for(&pool, 1, ReminderUpdate { minute_of_day })
            .await
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<StudyError>(),
            Some(StudyError::Invalid(_))
        ));
    }
    assert!(
        reminders::register_device_for(
            &pool,
            99,
            ORIGIN.into(),
            device("https://push.example/a", "k")
        )
        .await
        .is_err()
    );
    assert!(reminders::devices_for(&pool, 1).await.unwrap().is_empty());
    pool.close().await;
}

#[tokio::test]
async fn the_server_keeps_its_first_vapid_key() {
    let (_directory, pool) = temporary_pool().await;
    let first = reminders::vapid_key(&pool, [1; 32]).await.unwrap();
    let second = reminders::vapid_key(&pool, [2; 32]).await.unwrap();
    assert_eq!(first, vec![1; 32]);
    assert_eq!(second, first);
    pool.close().await;
}

#[tokio::test]
async fn reminders_follow_the_local_day_and_are_claimed_once() {
    let (_directory, pool) = temporary_pool().await;
    set_timezone(&pool, "Pacific/Auckland", 900).await;
    reminders::register_device_for(
        &pool,
        1,
        ORIGIN.into(),
        device("https://push.example/subscription/1", "key"),
    )
    .await
    .unwrap();
    let settings = reminders::update_settings_for(
        &pool,
        1,
        ReminderUpdate {
            minute_of_day: 7 * 60 + 30,
        },
    )
    .await
    .unwrap();
    assert_eq!(settings.timezone, "Pacific/Auckland");

    // 07:30 on 21 October in New Zealand daylight time is 18:30 UTC the previous day.
    let morning = timestamp("2026-10-20T18:30:00Z");
    assert!(
        reminders::claim_for(&pool, 1, morning)
            .await
            .unwrap()
            .is_none(),
        "nothing is ready in an empty library"
    );
    facts::create_many(&pool, vec![fact(1), fact(2)])
        .await
        .unwrap();
    assert!(
        reminders::claim_for(&pool, 1, morning - 60)
            .await
            .unwrap()
            .is_none(),
        "07:29 local time is before the chosen time"
    );
    let reminder = reminders::claim_for(&pool, 1, morning)
        .await
        .unwrap()
        .expect("practice is ready at the chosen time");
    assert_eq!(reminder.name, "Existing profile");
    assert_eq!(
        (reminder.summary.due, reminder.summary.new_available),
        (0, 2)
    );
    assert!(
        reminders::claim_for(&pool, 1, morning + 12 * 3600)
            .await
            .unwrap()
            .is_none(),
        "one reminder per local day"
    );
    assert!(
        reminders::claim_for(&pool, 1, morning + 86_400)
            .await
            .unwrap()
            .is_some(),
        "the next local day can be reminded"
    );
    pool.close().await;
}

#[tokio::test]
async fn a_spent_daily_budget_defers_the_reminder_without_claiming_it() {
    let (_directory, pool) = temporary_pool().await;
    set_timezone(&pool, "Europe/London", 60).await;
    facts::create_many(&pool, vec![fact(1), fact(2)])
        .await
        .unwrap();
    reminders::register_device_for(
        &pool,
        1,
        ORIGIN.into(),
        device("https://push.example/subscription/1", "key"),
    )
    .await
    .unwrap();
    let now = timestamp("2026-10-20T12:00:00Z");
    let queue = scheduling::queue_at_for(&pool, 1, &QueueQuery::default(), now)
        .await
        .unwrap();
    let question = &queue.items[0];
    scheduling::submit_at_for(
        &pool,
        1,
        StudySubmission {
            submission_id: uuid::Uuid::new_v4().to_string(),
            target_id: question.target_id,
            practice_id: Some(question.practice.id),
            feedback_event_id: None,
            content_revision: question.content_revision,
            expected_version: question.schedule_version,
            kind: EventKind::Attempt,
            context: StudyContext::Scheduled,
            method: ReviewMethod::Flashcard,
            outcome: Some(ReviewOutcome::Good),
            response_ms: 60_000,
            attempted: true,
            assisted: false,
            prior_exposure: false,
        },
        now,
    )
    .await
    .unwrap();
    assert!(
        reminders::claim_for(&pool, 1, now + 60)
            .await
            .unwrap()
            .is_none()
    );
    // The budget belongs to the local day, so the next morning is unaffected.
    assert!(
        reminders::claim_for(&pool, 1, timestamp("2026-10-21T08:00:00Z"))
            .await
            .unwrap()
            .is_some()
    );
    pool.close().await;
}
