mod support;
use support as db;
#[path = "support/media_files.rs"]
mod files;

use keepsake_core::{
    facts::{self, NewFact},
    media::{self, MediaKind, NewQuestionMedia},
    paging::{self, FactQuery, SampleQuery},
    profiles::{self, NewProfile},
    sharing::{self, ShareRequest},
};

fn png(seed: u8) -> Vec<u8> {
    files::png(seed)
}

fn fact(prompt: &str, answer: &str, media: Vec<NewQuestionMedia>) -> NewFact {
    NewFact {
        prompt: prompt.into(),
        answer: answer.into(),
        context: String::new(),
        source: String::new(),
        tags: vec!["birds".into()],
        practices: vec![],
        media,
    }
}

fn attach(media_id: i64, alt: &str) -> NewQuestionMedia {
    NewQuestionMedia {
        media_id,
        alt: alt.into(),
    }
}

async fn pool(directory: &tempfile::TempDir) -> sqlx::SqlitePool {
    let url = format!("sqlite:{}", directory.path().join("media.db").display());
    db::connect(&url).await.unwrap()
}

#[tokio::test]
async fn questions_keep_their_images_and_clips_in_order_across_reads() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let image = media::upload(&pool, 1, "image/png", png(1)).await.unwrap();
    let clip = media::upload(&pool, 1, "audio/wav", files::wav(300, 1))
        .await
        .unwrap();
    assert_eq!(
        (image.kind, image.content_type.as_str()),
        (MediaKind::Image, "image/png")
    );
    assert_eq!(
        (clip.kind, clip.content_type.as_str()),
        (MediaKind::Audio, "audio/wav")
    );
    assert_eq!(
        media::upload(&pool, 1, "image/png", png(1))
            .await
            .unwrap()
            .media_id,
        image.media_id,
        "identical bytes reuse the stored file"
    );

    let saved = facts::create_for(
        &pool,
        1,
        fact(
            "Which bird is this?",
            "Kererū",
            vec![
                attach(clip.media_id, ""),
                attach(image.media_id, "A large green and white pigeon"),
            ],
        ),
    )
    .await
    .unwrap();
    let order = |media: &[media::QuestionMedia]| {
        media
            .iter()
            .map(|item| (item.media_id, item.alt.clone()))
            .collect::<Vec<_>>()
    };
    let expected = vec![
        (clip.media_id, String::new()),
        (image.media_id, "A large green and white pigeon".into()),
    ];
    assert_eq!(order(&saved.media), expected);
    let page = paging::facts_for(&pool, 1, &FactQuery::default())
        .await
        .unwrap();
    assert_eq!(order(&page.items[0].media), expected);
    let file = media::read(&pool, 1, image.media_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (file.content_type.as_str(), file.data),
        ("image/png", png(1))
    );
    pool.close().await;
}

#[tokio::test]
async fn uploads_are_checked_before_they_are_stored() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    for (declared, bytes, message) in [
        ("image/svg+xml", b"<svg/>".to_vec(), "Unsupported file type"),
        ("image/jpeg", png(1), "do not match"),
        ("image/png", png(1)[..20].to_vec(), "damaged"),
        ("image/png", Vec::new(), "empty"),
        (
            "image/png",
            [png(1), vec![0; media::MAX_MEDIA_BYTES]].concat(),
            "5 MiB",
        ),
    ] {
        let error = media::upload(&pool, 1, declared, bytes).await.unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM media")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 0);

    let image = media::upload(&pool, 1, "image/png", png(2)).await.unwrap();
    for (attached, message) in [
        (
            (0..5)
                .map(|index| attach(image.media_id + index, ""))
                .collect(),
            "at most 4",
        ),
        (
            vec![attach(image.media_id, ""), attach(image.media_id, "")],
            "once",
        ),
        (vec![attach(image.media_id + 99, "")], "not in this library"),
    ] {
        let error = facts::create_for(&pool, 1, fact("Which bird?", "Tūī", attached))
            .await
            .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
    assert_eq!(facts::list_for(&pool, 1).await.unwrap().total, 0);
    pool.close().await;
}

#[tokio::test]
async fn another_profile_can_neither_read_nor_attach_an_upload() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let guest = profiles::create(
        &pool,
        NewProfile {
            name: "Guest".into(),
            handle: "guest".into(),
        },
    )
    .await
    .unwrap();
    let image = media::upload(&pool, 1, "image/png", png(3)).await.unwrap();
    assert!(
        media::read(&pool, guest.id, image.media_id)
            .await
            .unwrap()
            .is_none()
    );
    let error = facts::create_for(
        &pool,
        guest.id,
        fact(
            "Whose picture?",
            "Not mine",
            vec![attach(image.media_id, "")],
        ),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("not in this library"), "{error}");
    pool.close().await;
}

#[tokio::test]
async fn accepting_a_share_gives_the_recipient_their_own_copy_of_its_media() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let guest = profiles::create(
        &pool,
        NewProfile {
            name: "Guest".into(),
            handle: "guest".into(),
        },
    )
    .await
    .unwrap();
    let image = media::upload(&pool, 1, "image/png", png(4)).await.unwrap();
    let source = facts::create_for(
        &pool,
        1,
        fact(
            "Which bird is this?",
            "Tūī",
            vec![attach(image.media_id, "A dark bird")],
        ),
    )
    .await
    .unwrap();
    let share = sharing::send(
        &pool,
        1,
        ShareRequest {
            submission_id: uuid::Uuid::new_v4().to_string(),
            recipient_id: guest.id,
            fact_ids: vec![source.id],
        },
    )
    .await
    .unwrap()
    .remove(0);
    let accepted = sharing::accept(&pool, guest.id, share.id).await.unwrap();
    let copy = facts::get_for(&pool, guest.id, accepted.accepted_fact_id.unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(copy.media.len(), 1);
    assert_ne!(copy.media[0].media_id, image.media_id);
    assert_eq!(copy.media[0].alt, "A dark bird");
    let bytes = media::read(&pool, guest.id, copy.media[0].media_id)
        .await
        .unwrap()
        .unwrap()
        .data;
    assert_eq!(bytes, png(4));
    pool.close().await;
}

#[tokio::test]
async fn uploads_never_attached_are_removed_after_a_day() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let attached = media::upload(&pool, 1, "image/png", png(5)).await.unwrap();
    facts::create_for(
        &pool,
        1,
        fact("Which bird?", "Kākā", vec![attach(attached.media_id, "")]),
    )
    .await
    .unwrap();
    let insert = |seed: u8, created_at: String| {
        let bytes = png(seed);
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO media(profile_id, kind, content_type, sha256, byte_size, width, height,
               data, created_at)
             VALUES(1, 'image', 'image/png', ?, ?, 4, 3, ?, ?) RETURNING id",
        )
        .bind(format!("{seed:0>64}"))
        .bind(bytes.len() as i64)
        .bind(bytes)
        .bind(created_at)
    };
    let abandoned = insert(7, "2026-01-01T00:00:00.000Z".into())
        .fetch_one(&pool)
        .await
        .unwrap();
    // Matches SQLite's strftime('%Y-%m-%dT%H:%M:%fZ', 'now').
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
    let in_progress = insert(8, now.to_string()).fetch_one(&pool).await.unwrap();

    let new = media::upload(&pool, 1, "image/png", png(6)).await.unwrap();
    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM media ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!remaining.contains(&abandoned));
    assert_eq!(remaining, [attached.media_id, in_progress, new.media_id]);
    pool.close().await;
}

#[tokio::test]
async fn the_same_words_asked_of_different_pictures_can_be_matched() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    for (seed, bird) in [(10, "Kererū"), (11, "Tūī"), (12, "Pīwakawaka")] {
        let image = media::upload(&pool, 1, "image/png", png(seed))
            .await
            .unwrap();
        facts::create_for(
            &pool,
            1,
            fact("What is this?", bird, vec![attach(image.media_id, "")]),
        )
        .await
        .unwrap();
    }
    let sample = paging::sample_for(&pool, 1, &SampleQuery::default())
        .await
        .unwrap();
    let mut answers: Vec<_> = sample
        .items
        .iter()
        .map(|fact| fact.answer.as_str())
        .collect();
    answers.sort();
    assert_eq!(answers, ["Kererū", "Pīwakawaka", "Tūī"]);
    assert!(sample.items.iter().all(|fact| fact.media.len() == 1));
    pool.close().await;
}

#[tokio::test]
async fn the_schema_refuses_what_inspection_would_have_refused() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    for (kind, content_type, width, duration_ms) in [
        ("image", "image/svg+xml", Some(4), None),
        ("image", "text/html", Some(4), None),
        ("audio", "image/png", None, Some(10)),
        ("image", "audio/wav", Some(4), None),
        ("image", "image/png", None, None),
        ("image", "image/png", Some(4097), None),
        ("audio", "audio/wav", None, Some(300_001)),
    ] {
        let inserted = sqlx::query(
            "INSERT INTO media(profile_id, kind, content_type, sha256, byte_size, width, height,
               duration_ms, data)
             VALUES(1, ?, ?, ?, 1, ?, ?, ?, x'00')",
        )
        .bind(kind)
        .bind(content_type)
        .bind("0".repeat(64))
        .bind(width)
        .bind(width)
        .bind(duration_ms)
        .execute(&pool)
        .await;
        assert!(inserted.is_err(), "{kind} {content_type} was stored");
    }
    pool.close().await;
}
