mod support;
use support as db;
#[path = "support/media_files.rs"]
mod files;

fn format(declared: &str) -> media::UploadFormat {
    media::UploadFormat::from_content_type(declared).unwrap()
}

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
        ai_generated: false,
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
    let image = media::upload(&pool, 1, format("image/png"), png(1))
        .await
        .unwrap();
    let clip = media::upload(&pool, 1, format("audio/wav"), files::wav(300, 1))
        .await
        .unwrap();
    // Served as AVIF and AAC in MP4, whatever was uploaded.
    assert_eq!(
        (image.kind, image.content_type.as_str()),
        (MediaKind::Image, "image/avif")
    );
    assert_eq!(
        (clip.kind, clip.content_type.as_str(), clip.duration_ms),
        (MediaKind::Audio, "audio/mp4", Some(300))
    );
    assert_eq!(
        media::upload(&pool, 1, format("image/png"), png(1))
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
    let file = media::locate(&pool, 1, image.media_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(file.content_type, "image/avif");
    assert!(file.path.starts_with(directory.path().join("media/images")));
    assert_eq!(
        std::fs::read(&file.path).unwrap().len() as i64,
        file.byte_size
    );
    // Only what is served is kept: no upload, no blob in SQLite, nothing left in tmp/.
    assert_eq!(files_in(&directory.path().join("media")), 2);
    assert_eq!(files_in(&directory.path().join("tmp")), 0);
    pool.close().await;
}

fn files_in(directory: &std::path::Path) -> usize {
    walk(directory).len()
}

fn walk(directory: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path)
            } else {
                vec![path]
            }
        })
        .collect()
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
        let error = match media::UploadFormat::parse_content_type(declared) {
            Ok(format) => media::upload(&pool, 1, format, bytes).await.unwrap_err(),
            Err(error) => error,
        };
        assert!(error.to_string().contains(message), "{error}");
    }
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM media")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 0);
    assert_eq!(files_in(&directory.path().join("media")), 0);
    assert_eq!(files_in(&directory.path().join("tmp")), 0);

    let image = media::upload(&pool, 1, format("image/png"), png(2))
        .await
        .unwrap();
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
    let image = media::upload(&pool, 1, format("image/png"), png(3))
        .await
        .unwrap();
    assert!(
        media::locate(&pool, guest.id, image.media_id)
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
    let image = media::upload(&pool, 1, format("image/png"), png(4))
        .await
        .unwrap();
    let source = facts::create_for(
        &pool,
        1,
        fact(
            "Which bird is this?",
            "Tūī",
            vec![NewQuestionMedia {
                ai_generated: true,
                ..attach(image.media_id, "A dark bird")
            }],
        ),
    )
    .await
    .unwrap();
    assert!(source.media[0].ai_generated);
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
    assert!(copy.media[0].ai_generated, "the copy is still labelled");
    // Files are named by their contents, so the copy shares the sender's file.
    let path = |profile, id| {
        let pool = pool.clone();
        async move {
            media::locate(&pool, profile, id)
                .await
                .unwrap()
                .unwrap()
                .path
        }
    };
    assert_eq!(
        path(guest.id, copy.media[0].media_id).await,
        path(1, image.media_id).await
    );
    pool.close().await;
}

#[test]
fn snapshots_mention_generated_media_only_when_it_is() {
    // Pending offers made before the flag existed keep matching new offers of the same fact.
    let photographed = serde_json::to_value(attach(1, "A dark bird")).unwrap();
    assert_eq!(
        photographed,
        serde_json::json!({"media_id": 1, "alt": "A dark bird"})
    );
    let generated = NewQuestionMedia {
        ai_generated: true,
        ..attach(1, "A dark bird")
    };
    assert_eq!(
        serde_json::to_value(generated).unwrap()["ai_generated"],
        true
    );
}

#[tokio::test]
async fn uploads_never_attached_are_removed_after_a_day() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let attached = media::upload(&pool, 1, format("image/png"), png(5))
        .await
        .unwrap();
    facts::create_for(
        &pool,
        1,
        fact("Which bird?", "Kākā", vec![attach(attached.media_id, "")]),
    )
    .await
    .unwrap();
    let insert = |seed: u8, created_at: String| {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO media(profile_id, kind, upload_sha256, file_sha256,
               byte_size, width, height, created_at)
             VALUES(1, 'image', ?, ?, 10, 4, 3, ?) RETURNING id",
        )
        .bind(format!("{seed:0>64}"))
        .bind(format!("{seed:0>64}"))
        .bind(created_at)
    };
    let abandoned = insert(7, "2026-01-01T00:00:00.000Z".into())
        .fetch_one(&pool)
        .await
        .unwrap();
    // Matches SQLite's strftime('%Y-%m-%dT%H:%M:%fZ', 'now').
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ");
    let in_progress = insert(8, now.to_string()).fetch_one(&pool).await.unwrap();

    let new = media::upload(&pool, 1, format("image/png"), png(6))
        .await
        .unwrap();
    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM media ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(!remaining.contains(&abandoned));
    assert_eq!(remaining, [attached.media_id, in_progress, new.media_id]);
    pool.close().await;
}

#[tokio::test]
async fn housekeeping_empties_tmp_and_removes_only_old_unreferenced_files() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    let kept = media::upload(&pool, 1, format("image/png"), png(9))
        .await
        .unwrap();
    let kept = media::locate(&pool, 1, kept.media_id)
        .await
        .unwrap()
        .unwrap()
        .path;
    let images = directory.path().join("media/images");
    let aged = |name: &str, hours: u64| {
        let path = images.join(name);
        let file = std::fs::File::create(&path).unwrap();
        file.set_modified(
            std::time::SystemTime::now() - std::time::Duration::from_secs(hours * 3600),
        )
        .unwrap();
        path
    };
    let orphan = aged(&format!("{}.avif", "a".repeat(64)), 2);
    let just_written = aged(&format!("{}.avif", "b".repeat(64)), 0);
    let not_ours = aged("notes.txt", 2);
    let tmp = directory.path().join("tmp");
    std::fs::create_dir_all(tmp.join("nested")).unwrap();
    std::fs::write(tmp.join("interrupted.upload"), b"half a file").unwrap();

    let tidied = media::housekeeping(&pool).await.unwrap();
    assert_eq!((tidied.tmp_removed, tidied.orphans_removed), (2, 1));
    assert!(kept.is_file());
    assert!(!orphan.exists());
    assert!(
        just_written.is_file(),
        "a file may be moments ahead of its row"
    );
    assert!(not_ours.is_file());
    assert_eq!(files_in(&tmp), 0);
    pool.close().await;
}

#[tokio::test]
async fn the_same_words_asked_of_different_pictures_can_be_matched() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    for (seed, bird) in [(10, "Kererū"), (11, "Tūī"), (12, "Pīwakawaka")] {
        let image = media::upload(&pool, 1, format("image/png"), png(seed))
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
async fn the_schema_refuses_what_rendering_would_never_produce() {
    let directory = tempfile::tempdir().unwrap();
    let pool = pool(&directory).await;
    for (kind, width, duration_ms) in [
        ("video", None, Some(10)),
        ("gif", Some(4), None),
        ("image", None, None),
        ("image", Some(4), Some(10)),
        ("image", Some(4097), None),
        ("audio", None, None),
        ("audio", Some(4), Some(10)),
        ("audio", None, Some(300_001)),
    ] {
        let inserted = sqlx::query(
            "INSERT INTO media(profile_id, kind, upload_sha256, file_sha256,
               byte_size, width, height, duration_ms)
             VALUES(1, ?, ?, ?, 1, ?, ?, ?)",
        )
        .bind(kind)
        .bind("0".repeat(64))
        .bind("0".repeat(64))
        .bind(width)
        .bind(width)
        .bind(duration_ms)
        .execute(&pool)
        .await;
        assert!(
            inserted.is_err(),
            "{kind} {width:?} {duration_ms:?} was stored"
        );
    }
    pool.close().await;
}
