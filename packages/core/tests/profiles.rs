mod support;
use support as db;

use keepsake_core::{
    backup,
    facts::{self, NewFact, NewPractice, PracticeKind, ReviewMethod, ReviewOutcome},
    paging,
    profiles::{self, NewProfile, UpdateProfile},
    scheduling::{
        self, EventKind, ExposureSubmission, QueueQuery, SettingsUpdate, StudyContext,
        StudySubmission,
    },
    sharing::{self, ShareBatch, ShareQuery, ShareRequest},
};
use sqlx::{Row, sqlite::SqliteConnectOptions};

const NOW: i64 = 1_800_000_000;
async fn fixture() -> (tempfile::TempDir, sqlx::SqlitePool, i64, i64) {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::connect(&format!(
        "sqlite:{}",
        dir.path().join("profiles.sqlite3").display()
    ))
    .await
    .unwrap();
    let first = profiles::sole(&pool).await.unwrap();
    assert!(!first.setup_complete);
    profiles::rename(
        &pool,
        first.id,
        UpdateProfile {
            name: "Josiah".into(),
        },
    )
    .await
    .unwrap();
    let second = profiles::create(
        &pool,
        NewProfile {
            name: "Partner".into(),
            handle: "partner".into(),
        },
    )
    .await
    .unwrap();
    (dir, pool, first.id, second.id)
}
fn fact(prompt: &str, answer: &str) -> NewFact {
    NewFact {
        prompt: prompt.into(),
        answer: answer.into(),
        context: "Context".into(),
        source: "https://example.com/source".into(),
        tags: vec!["food".into()],
        practices: vec![],
        media: vec![],
    }
}
async fn add(pool: &sqlx::SqlitePool, owner: i64, prompt: &str, answer: &str) -> facts::Fact {
    facts::create_for(pool, owner, fact(prompt, answer))
        .await
        .unwrap()
}
fn request(recipient: i64, ids: Vec<i64>) -> ShareRequest {
    ShareRequest {
        submission_id: uuid::Uuid::new_v4().to_string(),
        recipient_id: recipient,
        fact_ids: ids,
    }
}
async fn attempt(pool: &sqlx::SqlitePool, owner: i64, time: i64) -> StudySubmission {
    let item = scheduling::queue_at_for(pool, owner, &QueueQuery::default(), time)
        .await
        .unwrap()
        .items
        .remove(0);
    StudySubmission {
        submission_id: uuid::Uuid::new_v4().to_string(),
        target_id: item.target_id,
        practice_id: Some(item.practice.id),
        content_revision: item.content_revision,
        expected_version: item.schedule_version,
        feedback_event_id: None,
        kind: EventKind::Attempt,
        context: StudyContext::Scheduled,
        method: match item.practice.kind {
            PracticeKind::Cloze => ReviewMethod::Cloze,
            _ => ReviewMethod::Flashcard,
        },
        outcome: Some(ReviewOutcome::Good),
        response_ms: 60_000,
        attempted: true,
        assisted: false,
        prior_exposure: false,
    }
}

#[tokio::test]
async fn profiles_validate_names_handles_and_ambiguous_compatibility() {
    let (_dir, pool, a, b) = fixture().await;
    assert!(
        profiles::sole(&pool)
            .await
            .unwrap_err()
            .to_string()
            .contains("profile_required")
    );
    assert!(facts::create(&pool, fact("bad", "bad")).await.is_err());
    assert!(facts::list(&pool).await.is_err());
    assert!(
        scheduling::queue(&pool, &QueueQuery::default())
            .await
            .is_err()
    );
    assert!(scheduling::settings(&pool).await.is_err());
    assert_eq!(profiles::resolve(&pool, "PARTNER").await.unwrap().id, b);
    assert_eq!(
        profiles::resolve(&pool, &a.to_string()).await.unwrap().id,
        a
    );
    assert!(
        profiles::create(
            &pool,
            NewProfile {
                name: "  ".into(),
                handle: "blank".into()
            }
        )
        .await
        .is_err()
    );
    for handle in ["UPPER", "has space", "123", "", "--"] {
        assert!(
            profiles::create(
                &pool,
                NewProfile {
                    name: "Name".into(),
                    handle: handle.into()
                }
            )
            .await
            .is_err()
        );
    }
    assert!(
        profiles::create(
            &pool,
            NewProfile {
                name: "Copy".into(),
                handle: "partner".into()
            }
        )
        .await
        .is_err()
    );
    assert!(
        profiles::rename(
            &pool,
            a,
            UpdateProfile {
                name: "Bad\nname".into()
            }
        )
        .await
        .is_err()
    );
    assert!(
        profiles::rename(
            &pool,
            999,
            UpdateProfile {
                name: "Missing".into()
            }
        )
        .await
        .is_err()
    );
    assert_eq!(profiles::list(&pool).await.unwrap().len(), 2);
}

#[tokio::test]
async fn reads_search_pagination_and_matching_only_use_owned_content() {
    let (_dir, pool, a, b) = fixture().await;
    for (q, ans) in [
        ("Food A", "Apple"),
        ("Food B", "Bread"),
        ("Food C", "Carrot"),
    ] {
        add(&pool, a, q, ans).await;
    }
    let private = add(&pool, b, "Secret food", "Secret answer").await;
    assert!(
        facts::get_for(&pool, a, private.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!paging::fact_exists_for(&pool, a, private.id).await.unwrap());
    assert!(
        paging::reviews_for(&pool, a, private.id, &paging::ReviewQuery::default())
            .await
            .is_err()
    );
    assert!(
        facts::add_practices_for(
            &pool,
            a,
            private.id,
            vec![NewPractice {
                kind: PracticeKind::Flashcard,
                prompt: "Secret".into(),
                answer: "Secret answer".into(),
                choices: vec![],
                explanation: String::new()
            }]
        )
        .await
        .is_err()
    );
    let page = paging::facts_for(
        &pool,
        a,
        &paging::FactQuery {
            limit: 1,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 1);
    assert!(page.next_cursor.is_some());
    assert_eq!(
        paging::facts_for(
            &pool,
            a,
            &paging::FactQuery {
                search: "Secret".into(),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .total,
        0
    );
    assert_eq!(
        paging::facts_for(&pool, b, &paging::FactQuery::default())
            .await
            .unwrap()
            .total,
        1
    );
    let session = paging::practice_for(&pool, a, &paging::PracticeQuery::default())
        .await
        .unwrap();
    assert_eq!(session.total, 3);
    assert!(session.items.iter().all(|i| i.fact.id != private.id));
    assert_eq!(
        paging::sample_for(&pool, a, &paging::SampleQuery::default())
            .await
            .unwrap()
            .items
            .len(),
        3
    );
    assert!(
        paging::sample_for(&pool, b, &paging::SampleQuery::default())
            .await
            .unwrap()
            .items
            .is_empty()
    );
}

#[tokio::test]
async fn settings_intake_budget_due_and_event_exports_are_personal() {
    let (_dir, pool, a, b) = fixture().await;
    add(&pool, a, "A", "A answer").await;
    add(&pool, b, "B", "B answer").await;
    let settings = scheduling::settings_for(&pool, a).await.unwrap();
    scheduling::update_settings_for(
        &pool,
        a,
        SettingsUpdate {
            timezone: "Pacific/Auckland".into(),
            new_limit: 1,
            daily_budget_seconds: 60,
            expected_version: settings.version,
        },
    )
    .await
    .unwrap();
    let before_b =
        serde_json::to_string(&scheduling::settings_for(&pool, b).await.unwrap()).unwrap();
    let input = attempt(&pool, a, NOW).await;
    scheduling::submit_at_for(&pool, a, input, NOW)
        .await
        .unwrap();
    let aq = scheduling::queue_at_for(&pool, a, &QueueQuery::default(), NOW + 1)
        .await
        .unwrap();
    let bq = scheduling::queue_at_for(&pool, b, &QueueQuery::default(), NOW + 1)
        .await
        .unwrap();
    assert_eq!(aq.new_remaining, 0);
    assert!(aq.budget_reached);
    assert_eq!(aq.spent_seconds, 60);
    assert_eq!(aq.timezone, "Pacific/Auckland");
    assert_eq!(bq.new_remaining, 5);
    assert_eq!(bq.spent_seconds, 0);
    assert_eq!(bq.items.len(), 1);
    assert!(!bq.budget_reached);
    assert_eq!(
        before_b,
        serde_json::to_string(&scheduling::settings_for(&pool, b).await.unwrap()).unwrap()
    );
    assert_eq!(
        scheduling::export_events_for(&pool, a, 0, 100)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(
        scheduling::export_events_for(&pool, b, 0, 100)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    let due = scheduling::queue_at_for(&pool, a, &QueueQuery::default(), NOW + 100_000_000)
        .await
        .unwrap();
    assert_eq!(due.total_due, 1);
    assert_eq!(
        scheduling::queue_at_for(&pool, b, &QueueQuery::default(), NOW + 100_000_000)
            .await
            .unwrap()
            .total_due,
        0
    );
}

#[tokio::test]
async fn foreign_targets_practices_feedback_and_uuid_replays_cannot_cross_profiles() {
    let (_dir, pool, a, b) = fixture().await;
    let fa = add(&pool, a, "A", "A answer").await;
    let fb = add(&pool, b, "B", "B answer").await;
    let mut input = attempt(&pool, a, NOW).await;
    assert!(
        scheduling::submit_at_for(&pool, b, input.clone(), NOW)
            .await
            .is_err()
    );
    assert!(scheduling::target_for(&pool, b, fa.id).await.is_err());
    assert!(
        scheduling::approve_practice_for(&pool, b, fa.practices[0].id)
            .await
            .is_err()
    );
    input.kind = EventKind::Exposure;
    input.outcome = None;
    let feedback = scheduling::submit_at_for(&pool, a, input.clone(), NOW)
        .await
        .unwrap();
    let mut other = attempt(&pool, b, NOW).await;
    other.feedback_event_id = Some(feedback.event_id);
    assert!(
        scheduling::submit_at_for(&pool, b, other.clone(), NOW + 1)
            .await
            .is_err()
    );
    other.feedback_event_id = None;
    other.submission_id = input.submission_id.clone();
    assert!(
        scheduling::submit_at_for(&pool, b, other, NOW + 1)
            .await
            .is_err()
    );
    let exposure = ExposureSubmission {
        submission_id: uuid::Uuid::new_v4().to_string(),
        fact_ids: vec![fa.id],
        method: ReviewMethod::Flashcard,
    };
    scheduling::expose_at_for(&pool, a, exposure.clone(), NOW)
        .await
        .unwrap();
    let mut stolen = exposure.clone();
    stolen.fact_ids = vec![fb.id];
    assert!(
        scheduling::expose_at_for(&pool, b, stolen, NOW)
            .await
            .is_err()
    );
    let before =
        serde_json::to_string(&scheduling::target_for(&pool, b, fb.id).await.unwrap()).unwrap();
    let mixed = ExposureSubmission {
        submission_id: uuid::Uuid::new_v4().to_string(),
        fact_ids: vec![fb.id, fa.id],
        method: ReviewMethod::Matching,
    };
    assert!(
        scheduling::expose_at_for(&pool, b, mixed, NOW)
            .await
            .is_err()
    );
    assert_eq!(
        before,
        serde_json::to_string(&scheduling::target_for(&pool, b, fb.id).await.unwrap()).unwrap()
    );
    assert!(
        scheduling::export_events_for(&pool, b, 0, 100)
            .await
            .unwrap()
            .items
            .is_empty()
    );
}

#[tokio::test]
async fn sharing_is_an_immutable_question_only_offer_then_an_independent_fresh_copy() {
    let (_dir, pool, a, b) = fixture().await;
    let mut original = fact("What does wai mean?", "Water");
    original.practices = vec![NewPractice {
        kind: PracticeKind::Cloze,
        prompt: "Wai means ___".into(),
        answer: "Water".into(),
        choices: vec![],
        explanation: "Drink".into(),
    }];
    let source = facts::create_for(&pool, a, original).await.unwrap();
    let extra = source
        .practices
        .iter()
        .find(|p| p.kind == PracticeKind::Cloze)
        .unwrap();
    scheduling::approve_practice_for(&pool, a, extra.id)
        .await
        .unwrap();
    let input = attempt(&pool, a, NOW).await;
    scheduling::submit_at_for(&pool, a, input, NOW)
        .await
        .unwrap();
    let req = request(b, vec![source.id]);
    let sent = sharing::send(&pool, a, req.clone()).await.unwrap();
    let json = serde_json::to_string(&sent).unwrap();
    assert!(!json.contains("Water"));
    assert!(!json.contains("snapshot"));
    assert!(facts::list_for(&pool, b).await.unwrap().items.is_empty());
    assert!(
        scheduling::queue_at_for(&pool, b, &QueueQuery::default(), NOW)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    sqlx::query("UPDATE facts SET answer='Changed after sharing' WHERE id=?")
        .bind(source.id)
        .execute(&pool)
        .await
        .unwrap();
    let accepted = sharing::accept(&pool, b, sent[0].id).await.unwrap();
    let copy = facts::get_for(&pool, b, accepted.accepted_fact_id.unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_ne!(copy.id, source.id);
    assert_eq!(copy.answer, "Water");
    assert_eq!(copy.source, source.source);
    assert_eq!(copy.tags, source.tags);
    assert_eq!(copy.practices.len(), 2);
    assert_eq!(copy.shared_by.as_deref(), Some("Josiah"));
    let state = scheduling::target_for(&pool, b, copy.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.phase, "new");
    assert_eq!(state.version, 1);
    assert!(state.stability.is_none());
    assert!(state.difficulty.is_none());
    assert!(state.due_at.is_none());
    assert!(
        facts::reviews_for(&pool, b, copy.id)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    let qualifying: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM target_practices WHERE target_id=? AND qualifying=1",
    )
    .bind(state.target_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(qualifying, 1);
    assert_eq!(
        sharing::accept(&pool, b, sent[0].id)
            .await
            .unwrap()
            .accepted_fact_id,
        Some(copy.id)
    );
    assert_eq!(
        sharing::send(&pool, a, req).await.unwrap()[0].accepted_fact_id,
        Some(copy.id)
    );
    assert_eq!(facts::list_for(&pool, b).await.unwrap().total, 1);
}

#[tokio::test]
async fn send_deduplicates_pending_offers_and_is_atomic_and_owner_bound() {
    let (_dir, pool, a, b) = fixture().await;
    let source = add(&pool, a, "A", "Answer").await;
    let foreign = add(&pool, b, "B", "Private").await;
    let req = request(b, vec![source.id]);
    let first = sharing::send(&pool, a, req.clone()).await.unwrap();
    assert_eq!(
        sharing::send(&pool, a, req.clone()).await.unwrap()[0].id,
        first[0].id
    );
    assert_eq!(
        sharing::send(&pool, a, request(b, vec![source.id]))
            .await
            .unwrap()[0]
            .id,
        first[0].id
    );
    let mut changed = req.clone();
    changed.fact_ids = vec![foreign.id];
    assert!(sharing::send(&pool, a, changed).await.is_err());
    let mut stolen = req;
    stolen.recipient_id = a;
    stolen.fact_ids = vec![foreign.id];
    assert!(sharing::send(&pool, b, stolen).await.is_err());
    assert!(
        sharing::send(&pool, a, request(b, vec![source.id, foreign.id]))
            .await
            .is_err()
    );
    assert!(
        sharing::send(&pool, a, request(a, vec![source.id]))
            .await
            .is_err()
    );
    assert_eq!(
        sharing::page(&pool, b, &ShareQuery::default())
            .await
            .unwrap()
            .total,
        1
    );
    assert!(sharing::accept(&pool, a, first[0].id).await.is_err());
    assert!(sharing::decline(&pool, a, first[0].id).await.is_err());
    assert!(sharing::withdraw(&pool, b, first[0].id).await.is_err());
}

#[tokio::test]
async fn accept_batches_rollback_and_concurrent_acceptance_creates_one_copy() {
    let (_dir, pool, a, b) = fixture().await;
    let source = add(&pool, a, "A", "Answer").await;
    let share = sharing::send(&pool, a, request(b, vec![source.id]))
        .await
        .unwrap()
        .remove(0);
    assert!(
        sharing::accept_batch(
            &pool,
            b,
            ShareBatch {
                share_ids: vec![share.id, 999]
            }
        )
        .await
        .is_err()
    );
    assert_eq!(facts::list_for(&pool, b).await.unwrap().total, 0);
    assert_eq!(sharing::list(&pool, b).await.unwrap()[0].status, "pending");
    let (one, two) = tokio::join!(
        sharing::accept(&pool, b, share.id),
        sharing::accept(&pool, b, share.id)
    );
    assert_eq!(one.unwrap().accepted_fact_id, two.unwrap().accepted_fact_id);
    assert_eq!(facts::list_for(&pool, b).await.unwrap().total, 1);
}

#[tokio::test]
async fn accept_withdraw_races_serialize_and_terminal_actions_are_repeatable() {
    let (_dir, pool, a, b) = fixture().await;
    let source = add(&pool, a, "A", "Answer").await;
    for _ in 0..10 {
        let share = sharing::send(&pool, a, request(b, vec![source.id]))
            .await
            .unwrap()
            .remove(0);
        let (accepted, withdrawn) = tokio::join!(
            sharing::accept(&pool, b, share.id),
            sharing::withdraw(&pool, a, share.id)
        );
        let withdrawn = withdrawn.unwrap();
        if withdrawn.status == "accepted" {
            assert!(accepted.is_ok());
        } else {
            assert_eq!(withdrawn.status, "withdrawn");
            assert!(accepted.is_err());
        }
        assert_eq!(
            sharing::withdraw(&pool, a, share.id).await.unwrap().status,
            withdrawn.status
        );
    }
    let share = sharing::send(&pool, a, request(b, vec![source.id]))
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        sharing::decline(&pool, b, share.id).await.unwrap().status,
        "declined"
    );
    assert_eq!(
        sharing::decline(&pool, b, share.id).await.unwrap().status,
        "declined"
    );
    assert!(sharing::accept(&pool, b, share.id).await.is_err());
}

#[tokio::test]
async fn sharing_pages_reach_every_offer_without_disclosing_foreign_offers() {
    let (_dir, pool, a, b) = fixture().await;
    let third = profiles::create(
        &pool,
        NewProfile {
            name: "Third".into(),
            handle: "third".into(),
        },
    )
    .await
    .unwrap();
    for n in 0..31 {
        let f = add(&pool, a, &format!("Question {n}"), "Answer").await;
        sharing::send(&pool, a, request(b, vec![f.id]))
            .await
            .unwrap();
    }
    let private = add(&pool, third.id, "Secret", "Private answer").await;
    sharing::send(&pool, third.id, request(a, vec![private.id]))
        .await
        .unwrap();
    let mut before = None;
    let mut ids = vec![];
    loop {
        let page = sharing::page(
            &pool,
            b,
            &ShareQuery {
                limit: 7,
                before_id: before,
                direction: Some("inbox".into()),
                status: Some("pending".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(page.total, 31);
        ids.extend(page.items.iter().map(|s| s.id));
        before = page.next_cursor;
        if before.is_none() {
            break;
        }
    }
    assert_eq!(ids.len(), 31);
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 31);
    assert!(
        sharing::page(
            &pool,
            b,
            &ShareQuery {
                direction: Some("all SQL".into()),
                ..Default::default()
            }
        )
        .await
        .is_err()
    );
    assert_eq!(
        sharing::page(
            &pool,
            b,
            &ShareQuery {
                direction: Some("sent".into()),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .total,
        0
    );
}

#[tokio::test]
async fn full_household_backup_restores_profiles_offers_and_learning_state() {
    let (dir, pool, a, b) = fixture().await;
    let source = add(&pool, a, "A", "Answer").await;
    let input = attempt(&pool, a, NOW).await;
    scheduling::submit_at_for(&pool, a, input, NOW)
        .await
        .unwrap();
    sharing::send(&pool, a, request(b, vec![source.id]))
        .await
        .unwrap();
    let backup_path = dir.path().join("backup.sqlite3");
    backup::create(
        &format!("sqlite:{}", dir.path().join("profiles.sqlite3").display()),
        &backup_path,
    )
    .await
    .unwrap();
    let restored_path = dir.path().join("restored.sqlite3");
    backup::restore(&backup_path, &restored_path).await.unwrap();
    let restored = db::connect(&format!("sqlite:{}", restored_path.display()))
        .await
        .unwrap();
    assert_eq!(profiles::list(&restored).await.unwrap().len(), 2);
    assert_eq!(sharing::list(&restored, b).await.unwrap().len(), 1);
    assert_eq!(
        serde_json::to_string(&scheduling::target_for(&pool, a, source.id).await.unwrap()).unwrap(),
        serde_json::to_string(
            &scheduling::target_for(&restored, a, source.id)
                .await
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        scheduling::export_events_for(&restored, a, 0, 100)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&restored)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn migration_preserves_all_v030_content_and_evidence_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let old = dir.path().join("old-migrations");
    std::fs::create_dir(&old).unwrap();
    for (name, sql) in [
        (
            "0002_facts_and_reviews.sql",
            include_str!("../migrations/0002_facts_and_reviews.sql"),
        ),
        (
            "0003_query_indexes.sql",
            include_str!("../migrations/0003_query_indexes.sql"),
        ),
        (
            "0004_scheduling.sql",
            include_str!("../migrations/0004_scheduling.sql"),
        ),
    ] {
        std::fs::write(old.join(name), sql).unwrap();
    }
    let path = dir.path().join("legacy.sqlite3");
    let pool = sqlx::SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::migrate::Migrator::new(old)
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO facts(id,prompt,answer) VALUES(41,'Legacy','Evidence'); INSERT INTO practices(id,fact_id,kind,prompt,answer) VALUES(51,41,'flashcard','Legacy','Evidence'); INSERT INTO learning_targets(id,fact_id,prompt,answer) VALUES(61,41,'Legacy','Evidence'); INSERT INTO target_practices(practice_id,target_id,qualifying) VALUES(51,61,1); INSERT INTO schedule_states(target_id,phase,due_at,blocked_until,introduced_at,version,state_json) VALUES(61,'review',1800001234,1800001111,1700000000,7,'{\"target_id\":61,\"fact_id\":41,\"content_revision\":1,\"phase\":\"review\",\"due_at\":1800001234,\"last_review_at\":1700000000,\"stability\":2.5,\"difficulty\":4.2,\"lapses\":1,\"version\":7,\"introduced_at\":1700000000,\"blocked_until\":1800001111}'); INSERT INTO reviews(id,fact_id,practice_id,method,outcome,response_ms) VALUES(71,41,51,'flashcard','good',1234); INSERT INTO study_events(id,submission_id,target_id,content_revision,occurred_at,kind,context,method,outcome,response_ms,qualifying,algorithm,parameter_version,elapsed_seconds,request_json,before_json,after_json,result_json) VALUES(81,'legacy-event',61,1,1700000000,'attempt','scheduled','flashcard','good',1234,1,'fsrs-6.6.2',1,100,'{\"historical\":true}','{\"before\":1}','{\"after\":2}','{\"result\":3}'); INSERT INTO study_exposure_batches VALUES('legacy-batch','{\"facts\":[41]}','{\"recorded\":1}'); UPDATE scheduler_settings SET legacy_review_cutoff=71;").execute(&pool).await.unwrap();
    let queries = [
        "SELECT json_array(id,prompt,answer,context,source,tags,created_at) FROM facts",
        "SELECT json_array(id,fact_id,kind,prompt,answer,choices,explanation) FROM practices",
        "SELECT json_array(id,fact_id,ability,direction,content_revision,prompt,answer,active) FROM learning_targets",
        "SELECT json_array(practice_id,target_id,qualifying) FROM target_practices",
        "SELECT json_array(target_id,phase,due_at,blocked_until,introduced_at,version,state_json) FROM schedule_states",
        "SELECT json_array(id,fact_id,practice_id,method,outcome,response_ms,reviewed_at) FROM reviews",
        "SELECT json_array(id,submission_id,target_id,content_revision,occurred_at,kind,context,method,outcome,response_ms,qualifying,algorithm,parameter_version,elapsed_seconds,model_interval_days,policy_override_seconds,request_json,before_json,after_json,result_json) FROM study_events",
        "SELECT json_array(submission_id,request_json,result_json) FROM study_exposure_batches",
        "SELECT json_array(value_json,legacy_review_cutoff) FROM scheduler_settings",
    ];
    // All statements are fixed source-controlled literals, preserving SQLx's SQL safety contract.
    async fn rows(pool: &sqlx::SqlitePool, queries: &[&'static str]) -> Vec<Vec<String>> {
        let mut result = vec![];
        for query in queries {
            result.push(sqlx::query_scalar(*query).fetch_all(pool).await.unwrap());
        }
        result
    }
    let before = rows(&pool, &queries).await;
    pool.close().await;
    let pool = db::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    let after = rows(&pool, &queries).await;
    for (original_rows, migrated_rows) in before.iter().zip(&after) {
        for original in original_rows {
            assert!(
                migrated_rows.contains(original),
                "historical row changed: {original}"
            );
        }
    }
    assert_eq!(
        profiles::get(&pool, 1).await.unwrap().unwrap().handle,
        "default"
    );
    let owner: i64 = sqlx::query_scalar("SELECT profile_id FROM facts WHERE id=41")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, 1);
    let new = profiles::create(
        &pool,
        NewProfile {
            name: "Partner".into(),
            handle: "partner".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(facts::list_for(&pool, new.id).await.unwrap().total, 0);
    assert!(
        scheduling::export_events_for(&pool, new.id, 0, 100)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        sqlx::query("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get::<String, _>(0),
        "ok"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        sqlx::query("INSERT INTO facts(prompt,answer) VALUES('No owner','Rejected')")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE facts SET profile_id=? WHERE id=41")
            .bind(new.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE reviews SET profile_id=? WHERE id=71")
            .bind(new.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE study_events SET profile_id=? WHERE id=81")
            .bind(new.id)
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn copied_facts_keep_all_practices_accumulated_across_append_requests() {
    let (_directory, pool, sender, recipient) = fixture().await;
    let fact = add(&pool, sender, "Canonical question", "Canonical answer").await;
    for batch in 0..2 {
        let practices = (0..20)
            .map(|number| NewPractice {
                kind: PracticeKind::Flashcard,
                prompt: format!("Alternate {batch} {number}"),
                answer: "Canonical answer".into(),
                choices: vec![],
                explanation: String::new(),
            })
            .collect();
        facts::add_practices_for(&pool, sender, fact.id, practices)
            .await
            .unwrap();
    }
    let offer = sharing::send(&pool, sender, request(recipient, vec![fact.id]))
        .await
        .unwrap()
        .remove(0);
    let accepted = sharing::accept(&pool, recipient, offer.id).await.unwrap();
    let copied = facts::get_for(&pool, recipient, accepted.accepted_fact_id.unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(copied.practices.len(), 41);
    let state = scheduling::target_for(&pool, recipient, copied.id)
        .await
        .unwrap()
        .unwrap();
    let qualifying: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM target_practices WHERE target_id=? AND qualifying=1",
    )
    .bind(state.target_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(qualifying, 1);
}

#[tokio::test]
async fn two_people_can_grade_their_own_schedule_versions_concurrently() {
    let (_directory, pool, first, second) = fixture().await;
    let first_fact = add(&pool, first, "First question", "First answer").await;
    let second_fact = add(&pool, second, "Second question", "Second answer").await;
    let first_input = attempt(&pool, first, NOW).await;
    let second_input = attempt(&pool, second, NOW).await;
    let (first_result, second_result) = tokio::join!(
        scheduling::submit_at_for(&pool, first, first_input, NOW),
        scheduling::submit_at_for(&pool, second, second_input, NOW)
    );
    assert_eq!(first_result.unwrap().schedule.version, 2);
    assert_eq!(second_result.unwrap().schedule.version, 2);
    assert_eq!(
        facts::reviews_for(&pool, first, first_fact.id)
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        facts::reviews_for(&pool, second, second_fact.id)
            .await
            .unwrap()
            .total,
        1
    );
}
