use keepsake_core::{
    db, facts,
    profiles::{self, NewProfile},
    scheduling,
    sharing::{self, ShareRequest},
};
use sqlx::{Row, SqlitePool, sqlite::SqliteConnectOptions};

const KAI: &str = "What does “kai” mean in te reo Māori (noun)?";
const KAI_SOURCE: &str =
    "https://maoridictionary.co.nz/search?idiom=0&keywords=kai&loan=0&phrase=0&proverb=0";

// Real production migrations run on temporary databases; these cases deliberately
// keep the household seed rather than using the empty behavior-test fixture.
async fn legacy() -> (tempfile::TempDir, std::path::PathBuf, SqlitePool) {
    let directory = tempfile::tempdir().unwrap();
    let migrations = directory.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();
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
        (
            "0005_profiles.sql",
            include_str!("../migrations/0005_profiles.sql"),
        ),
        // Repository code used to build the fixture reads question media, including
        // whether each is AI-generated. Adding that schema early still leaves household
        // seeding to run afterwards.
        (
            "0009_question_media.sql",
            include_str!("../migrations/0009_question_media.sql"),
        ),
        (
            "0011_default_lists.sql",
            include_str!("../migrations/0011_default_lists.sql"),
        ),
    ] {
        std::fs::write(migrations.join(name), sql).unwrap();
    }
    let path = directory.path().join("household.sqlite3");
    let pool = SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    sqlx::migrate::Migrator::new(migrations)
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    (directory, path, pool)
}

async fn count(pool: &SqlitePool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn integrity(pool: &SqlitePool) {
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(pool)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(pool)
            .await
            .unwrap(),
        "ok"
    );
}

#[tokio::test]
async fn fresh_household_has_isolated_facts_frozen_offers_and_no_learning_history() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite:{}", directory.path().join("fresh.db").display());
    let pool = db::connect(&url).await.unwrap();
    let people = profiles::list(&pool).await.unwrap();
    assert_eq!(people.len(), 2);
    assert_eq!((people[0].id, people[0].name.as_str()), (1, "Josiah"));
    assert_eq!((people[1].id, people[1].name.as_str()), (2, "Ansam"));
    assert!(people.iter().all(|person| person.setup_complete));
    assert!(profiles::sole(&pool).await.is_err());
    assert_eq!(facts::list_for(&pool, 1).await.unwrap().total, 40);
    assert_eq!(facts::list_for(&pool, 2).await.unwrap().total, 60);
    assert_eq!(count(&pool, "SELECT count(*) FROM practices").await, 220);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM learning_targets").await,
        100
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM reviews").await, 0);
    assert_eq!(count(&pool, "SELECT count(*) FROM study_events").await, 0);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM schedule_states WHERE phase='new' AND version=1 AND due_at IS NULL AND introduced_at IS NULL AND blocked_until IS NULL").await,
        100
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM target_practices WHERE qualifying=1"
        )
        .await,
        100
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM fact_shares WHERE sender_id=2 AND recipient_id=1 AND status='pending' AND accepted_fact_id IS NULL").await,
        60
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM facts f JOIN json_each(f.tags) tag WHERE tag.value='te-reo-māori' AND f.profile_id<>2").await,
        0
    );

    // Seeded snapshots use the normal canonical format: sending again reuses the
    // existing offer, while accepting creates a fresh, independently owned copy.
    let offers = sqlx::query("SELECT id,source_fact_id,snapshot_json FROM fact_shares ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    for offer in &offers {
        let snapshot: facts::NewFact = serde_json::from_str(offer.get("snapshot_json")).unwrap();
        facts::validate_new_fact(&snapshot).unwrap();
        assert!(!snapshot.source.is_empty());
        let repeated = sharing::send(
            &pool,
            2,
            ShareRequest {
                submission_id: uuid::Uuid::new_v4().to_string(),
                recipient_id: 1,
                fact_ids: vec![offer.get("source_fact_id")],
            },
        )
        .await
        .unwrap();
        assert_eq!(repeated[0].id, offer.get::<i64, _>("id"));
    }
    let accepted = sharing::accept(&pool, 1, offers[0].get("id"))
        .await
        .unwrap();
    let copy = accepted.accepted_fact_id.unwrap();
    assert_ne!(copy, accepted.source_fact_id);
    assert_eq!(facts::list_for(&pool, 1).await.unwrap().total, 41);
    assert_eq!(facts::list_for(&pool, 2).await.unwrap().total, 60);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM fact_shares WHERE status='pending'"
        )
        .await,
        59
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM reviews WHERE fact_id=?")
            .bind(copy)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    pool.close().await;
    let reopened = db::connect(&url).await.unwrap();
    assert_eq!(count(&reopened, "SELECT count(*) FROM facts").await, 101);
    assert_eq!(
        count(&reopened, "SELECT count(*) FROM fact_shares").await,
        60
    );
    assert_eq!(profiles::list(&reopened).await.unwrap().len(), 2);
    integrity(&reopened).await;
}

#[tokio::test]
async fn upgrade_moves_originals_without_rewriting_edits_schedules_or_evidence() {
    let (_directory, path, pool) = legacy().await;
    sqlx::query("INSERT INTO facts(id,profile_id,prompt,answer,context,source,tags) VALUES(41,1,?,'My edited answer','My edited context',?,'[\"te-reo\",\"custom\"]')")
        .bind(KAI)
        .bind(KAI_SOURCE)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        r#"
        INSERT INTO facts(id,profile_id,prompt,answer,tags) VALUES
            (42,1,'Private extra fact','Private answer','["rust"]'),
            (43,1,'Extra household word','Custom translation','["te reo"]');
        INSERT INTO practices(id,fact_id,kind,prompt,answer) VALUES
            (51,41,'flashcard','Edited practice','My edited answer'),
            (52,42,'flashcard','Private extra fact','Private answer'),
            (53,43,'flashcard','Extra household word','Custom translation');
        INSERT INTO learning_targets(id,fact_id,prompt,answer) VALUES
            (61,41,'Edited target','My edited answer'),
            (62,42,'Private extra fact','Private answer'),
            (63,43,'Extra household word','Custom translation');
        INSERT INTO target_practices(practice_id,target_id,qualifying) VALUES
            (51,61,1),(52,62,1),(53,63,1);
        INSERT INTO schedule_states(target_id,phase,due_at,version,state_json)
        VALUES (61,'review',1800001234,7,
          '{"target_id":61,"fact_id":41,"content_revision":1,"phase":"review","due_at":1800001234,"last_review_at":1700000000,"stability":2.5,"difficulty":4.2,"lapses":1,"version":7,"introduced_at":null,"blocked_until":null}');
        INSERT INTO reviews(id,profile_id,fact_id,practice_id,method,outcome,response_ms)
        VALUES(71,1,41,51,'flashcard','good',1234);
        INSERT INTO study_events(id,profile_id,submission_id,target_id,content_revision,occurred_at,kind,context,method,outcome,response_ms,qualifying,algorithm,parameter_version,elapsed_seconds,request_json,before_json,after_json,result_json)
        VALUES(81,1,'historical-event',61,1,1700000000,'attempt','scheduled','flashcard','good',1234,1,'fsrs-6.6.2',1,100,'{"historical":true}','{"before":1}','{"after":2}','{"result":3}');
        INSERT INTO study_exposure_batches(profile_id,submission_id,request_json,result_json) VALUES
            (1,'language-batch','{"submission_id":"language-batch","fact_ids":[41,43],"method":"matching"}','{"recorded":2}'),
            (1,'mixed-batch','{"submission_id":"mixed-batch","fact_ids":[41,42],"method":"matching"}','{"recorded":2}');
        UPDATE scheduler_settings SET value_json=json_set(value_json,'$.new_limit',9),legacy_review_cutoff=71 WHERE profile_id=1;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let queries = [
        "SELECT json_array(id,prompt,answer,context,source,tags,created_at) FROM facts WHERE id IN (41,42,43) ORDER BY id",
        "SELECT json_array(id,fact_id,kind,prompt,answer,choices,explanation) FROM practices WHERE fact_id=41 ORDER BY id",
        "SELECT json_array(id,fact_id,prompt,answer,active,content_revision) FROM learning_targets WHERE fact_id=41",
        "SELECT json_array(practice_id,target_id,qualifying) FROM target_practices WHERE target_id=61",
        "SELECT state_json FROM schedule_states WHERE target_id=61",
        "SELECT json_array(id,fact_id,practice_id,method,outcome,response_ms,reviewed_at) FROM reviews WHERE id=71",
        "SELECT json_array(id,submission_id,target_id,request_json,before_json,after_json,result_json) FROM study_events WHERE id=81",
        "SELECT json_array(submission_id,request_json,result_json) FROM study_exposure_batches ORDER BY submission_id",
        "SELECT json_array(value_json,legacy_review_cutoff) FROM scheduler_settings WHERE profile_id=1",
    ];
    let mut before: Vec<Vec<String>> = vec![];
    for query in queries {
        before.push(sqlx::query_scalar(query).fetch_all(&pool).await.unwrap());
    }
    pool.close().await;
    let migrated = db::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    for (query, expected) in queries.into_iter().zip(before) {
        let actual: Vec<String> = sqlx::query_scalar(query)
            .fetch_all(&migrated)
            .await
            .unwrap();
        assert_eq!(actual, expected, "changed historical content: {query}");
    }
    let owners: Vec<(i64, Option<i64>)> =
        sqlx::query_as("SELECT id,profile_id FROM facts WHERE id IN (41,42,43) ORDER BY id")
            .fetch_all(&migrated)
            .await
            .unwrap();
    assert_eq!(owners, vec![(41, Some(2)), (42, Some(1)), (43, Some(2))]);
    assert_eq!(facts::list_for(&migrated, 1).await.unwrap().total, 41);
    assert_eq!(facts::list_for(&migrated, 2).await.unwrap().total, 61);
    assert_eq!(
        count(&migrated, "SELECT profile_id FROM reviews WHERE id=71").await,
        2
    );
    assert_eq!(
        count(&migrated, "SELECT profile_id FROM study_events WHERE id=81").await,
        2
    );
    assert_eq!(
        count(
            &migrated,
            "SELECT profile_id FROM study_exposure_batches WHERE submission_id='language-batch'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &migrated,
            "SELECT profile_id FROM study_exposure_batches WHERE submission_id='mixed-batch'"
        )
        .await,
        1
    );
    // The migration restores ownership guards before it commits.
    for query in [
        "UPDATE facts SET profile_id=1 WHERE id=41",
        "UPDATE reviews SET profile_id=1 WHERE id=71",
        "UPDATE study_events SET profile_id=1 WHERE id=81",
        "UPDATE study_exposure_batches SET profile_id=1 WHERE submission_id='language-batch'",
    ] {
        assert!(sqlx::query(query).execute(&migrated).await.is_err());
    }
    integrity(&migrated).await;
}

#[tokio::test]
async fn existing_named_people_settings_and_unrelated_libraries_are_preserved() {
    let (_directory, path, pool) = legacy().await;
    let outsider = profiles::create(
        &pool,
        NewProfile {
            name: "Someone else".into(),
            handle: "ansam".into(),
        },
    )
    .await
    .unwrap();
    let josiah = profiles::create(
        &pool,
        NewProfile {
            name: "Josiah".into(),
            handle: "personal".into(),
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE scheduler_settings SET value_json=json_set(value_json,'$.new_limit',11) WHERE profile_id=?")
        .bind(josiah.id)
        .execute(&pool)
        .await
        .unwrap();
    let custom = facts::create_for(
        &pool,
        outsider.id,
        facts::NewFact {
            prompt: KAI.into(),
            answer: "Their wording".into(),
            context: "Private".into(),
            source: KAI_SOURCE.into(),
            tags: vec!["te-reo".into()],
            practices: vec![],
            media: vec![],
        },
    )
    .await
    .unwrap();
    pool.close().await;
    let migrated = db::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap();
    let all = profiles::list(&migrated).await.unwrap();
    assert_eq!(all.len(), 4);
    let ansam = all.iter().find(|profile| profile.name == "Ansam").unwrap();
    assert_eq!(ansam.handle, "ansam-1");
    assert_eq!(
        profiles::get(&migrated, outsider.id)
            .await
            .unwrap()
            .unwrap()
            .name,
        "Someone else"
    );
    assert_eq!(
        profiles::get(&migrated, josiah.id)
            .await
            .unwrap()
            .unwrap()
            .handle,
        "personal"
    );
    assert_eq!(
        scheduling::settings_for(&migrated, josiah.id)
            .await
            .unwrap()
            .new_limit,
        11
    );
    assert_eq!(
        scheduling::settings_for(&migrated, ansam.id)
            .await
            .unwrap()
            .new_limit,
        5
    );
    assert_eq!(
        facts::list_for(&migrated, outsider.id).await.unwrap().total,
        1
    );
    assert_eq!(
        facts::get_for(&migrated, outsider.id, custom.id)
            .await
            .unwrap()
            .unwrap()
            .answer,
        "Their wording"
    );
    assert_eq!(
        facts::list_for(&migrated, josiah.id).await.unwrap().total,
        40
    );
    assert_eq!(
        facts::list_for(&migrated, ansam.id).await.unwrap().total,
        60
    );
    integrity(&migrated).await;
}

#[tokio::test]
async fn failed_seed_transaction_restores_original_owners_and_immutability_guards() {
    let (_directory, path, pool) = legacy().await;
    sqlx::raw_sql(
        r#"
        INSERT INTO facts(id,profile_id,prompt,answer,tags) VALUES(41,1,'Keep my state','Answer','["te-reo"]');
        CREATE TRIGGER reject_household_seed BEFORE INSERT ON facts
        BEGIN SELECT RAISE(ABORT, 'deliberate migration failure'); END;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
    let failure = db::connect(&format!("sqlite:{}", path.display()))
        .await
        .unwrap_err();
    assert!(format!("{failure:#}").contains("deliberate migration failure"));
    let pool = SqlitePool::connect_with(
        SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true),
    )
    .await
    .unwrap();
    assert_eq!(
        count(&pool, "SELECT profile_id FROM facts WHERE id=41").await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM facts").await, 1);
    // The fixture already holds 0009; the failed seed must stop the run after 0006.
    assert_eq!(
        count(
            &pool,
            "SELECT max(version) FROM _sqlx_migrations WHERE version < 9"
        )
        .await,
        6
    );
    assert!(
        sqlx::query("UPDATE facts SET profile_id=2 WHERE id=41")
            .execute(&pool)
            .await
            .is_err()
    );
    integrity(&pool).await;
}
