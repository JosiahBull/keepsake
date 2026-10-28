mod support;
use support as db;
#[path = "support/media_files.rs"]
mod files;

use std::path::{Path, PathBuf};

use keepsake_core::{
    default_lists, facts,
    media::{self, UploadFormat},
    profiles::{self, NewProfile},
};
use serde_json::json;

fn item(key: &str, prompt: &str, answer: &str, media: serde_json::Value) -> serde_json::Value {
    json!({
        "key": key,
        "prompt": prompt,
        "answer": answer,
        "context": "Photo: A. Photographer, CC BY 4.0.",
        "source": "https://www.nzbirdsonline.org.nz/",
        "tags": ["nz-birds"],
        "media": media,
    })
}

fn photo(file: &str, ai_generated: bool) -> serde_json::Value {
    json!({
        "file": file,
        "alt": "A dark bird on a branch",
        "ai_generated": ai_generated,
        "credit": "A. Photographer, CC BY 4.0, https://commons.wikimedia.org/",
    })
}

/// A directory of default lists holding one list, `birds`, with these items.
fn write_list(root: &Path, items: Vec<serde_json::Value>) -> PathBuf {
    let directory = root.join("lists");
    let list = directory.join("birds");
    std::fs::create_dir_all(list.join("media")).unwrap();
    std::fs::write(list.join("media/tui.png"), files::png(1)).unwrap();
    std::fs::write(list.join("media/drawn.png"), files::png(2)).unwrap();
    std::fs::write(list.join("media/call.wav"), files::wav(300, 1)).unwrap();
    std::fs::write(
        list.join("list.json"),
        serde_json::to_vec(&json!({"id": "birds", "title": "Birds", "items": items})).unwrap(),
    )
    .unwrap();
    directory
}

async fn member(pool: &sqlx::SqlitePool, name: &str, handle: &str) -> i64 {
    profiles::create(
        pool,
        NewProfile {
            name: name.into(),
            handle: handle.into(),
        },
    )
    .await
    .unwrap()
    .id
}

async fn count(pool: &sqlx::SqlitePool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn each_member_receives_each_item_once_sharing_one_converted_file() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite:{}", directory.path().join("lists.db").display());
    let pool = db::connect(&url).await.unwrap();
    let josiah = member(&pool, "Josiah", "josiah").await;
    // Found by handle after a rename.
    let ansam = member(&pool, "Sam", "ansam").await;
    let lists = write_list(
        directory.path(),
        vec![
            item(
                "tui-photo",
                "Which bird is this?",
                "Tūī",
                json!([
                    photo("media/tui.png", false),
                    photo("media/call.wav", false)
                ]),
            ),
            item(
                "tui-drawn",
                "Which bird is shown?",
                "Tūī",
                json!([photo("media/drawn.png", true)]),
            ),
        ],
    );

    let loaded = default_lists::load(&lists).unwrap();
    assert_eq!((loaded.len(), loaded[0].len()), (1, 2));
    let installed = default_lists::install(&pool, &loaded, || false)
        .await
        .unwrap();
    assert_eq!(installed.added, 4);
    assert!(installed.missing.is_empty());
    assert!(installed.failed.is_empty(), "{:?}", installed.failed);

    for profile in [josiah, ansam] {
        let page = facts::list_for(&pool, profile).await.unwrap();
        assert_eq!(page.total, 2);
        let drawn = page
            .items
            .iter()
            .find(|fact| fact.prompt == "Which bird is shown?");
        let drawn = drawn.unwrap();
        assert!(drawn.media[0].ai_generated);
        assert_eq!(drawn.media[0].alt, "A dark bird on a branch");
        let photo = page
            .items
            .iter()
            .find(|fact| fact.prompt == "Which bird is this?");
        let photo = photo.unwrap();
        assert_eq!(photo.media.len(), 2);
        assert!(photo.media.iter().all(|item| !item.ai_generated));
        assert_eq!(photo.tags, ["nz-birds"]);
    }
    assert_eq!(
        facts::list_for(&pool, 1).await.unwrap().total,
        0,
        "not a member"
    );
    // Each file was converted once; the second library's records share it.
    assert_eq!(count(&pool, "SELECT count(*) FROM media").await, 6);
    assert_eq!(
        count(&pool, "SELECT count(DISTINCT file_sha256) FROM media").await,
        3
    );

    // Starting again adds nothing.
    let again = default_lists::install(&pool, &loaded, || false)
        .await
        .unwrap();
    assert_eq!(again.added, 0);

    // A later version adds its new item, and leaves an edited one as it was added.
    let lists = write_list(
        directory.path(),
        vec![
            item(
                "tui-photo",
                "Name this bird.",
                "Tūī",
                json!([photo("media/tui.png", false)]),
            ),
            item(
                "tui-drawn",
                "Which bird is shown?",
                "Tūī",
                json!([photo("media/drawn.png", true)]),
            ),
            item(
                "tui-call",
                "Which bird is calling?",
                "Tūī",
                json!([photo("media/call.wav", false)]),
            ),
        ],
    );
    let loaded = default_lists::load(&lists).unwrap();
    let later = default_lists::install(&pool, &loaded, || false)
        .await
        .unwrap();
    assert_eq!(later.added, 2);
    for profile in [josiah, ansam] {
        let prompts: Vec<String> = facts::list_for(&pool, profile)
            .await
            .unwrap()
            .items
            .into_iter()
            .map(|fact| fact.prompt)
            .collect();
        assert_eq!(prompts.len(), 3);
        assert!(prompts.contains(&"Which bird is this?".to_string()));
        assert!(!prompts.contains(&"Name this bird.".to_string()));
    }
    pool.close().await;
}

#[tokio::test]
async fn a_member_without_a_profile_is_reported_and_others_still_receive_lists() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite:{}", directory.path().join("lists.db").display());
    let pool = db::connect(&url).await.unwrap();
    let josiah = member(&pool, "Josiah", "default-josiah").await;
    let lists = write_list(
        directory.path(),
        vec![item(
            "tui-photo",
            "Which bird is this?",
            "Tūī",
            json!([photo("media/tui.png", false)]),
        )],
    );
    let loaded = default_lists::load(&lists).unwrap();
    // A shutdown before the first item adds nothing; the next start finishes.
    let stopped = default_lists::install(&pool, &loaded, || true)
        .await
        .unwrap();
    assert_eq!(stopped.added, 0);
    let installed = default_lists::install(&pool, &loaded, || false)
        .await
        .unwrap();
    assert_eq!(installed.missing, ["ansam"]);
    assert_eq!(installed.added, 1);
    assert_eq!(facts::list_for(&pool, josiah).await.unwrap().total, 1);
    pool.close().await;
}

#[tokio::test]
async fn lists_take_turns_so_each_reaches_study_from_the_start() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite:{}", directory.path().join("lists.db").display());
    let pool = db::connect(&url).await.unwrap();
    let josiah = member(&pool, "Josiah", "josiah").await;
    let lists = write_list(
        directory.path(),
        ["tui", "kea", "ruru"]
            .map(|bird| item(bird, &format!("Is this a {bird}?"), "Yes", json!([])))
            .to_vec(),
    );
    let plants = lists.join("plants");
    std::fs::create_dir(&plants).unwrap();
    let items =
        ["kowhai"].map(|plant| item(plant, &format!("Is this a {plant}?"), "Yes", json!([])));
    std::fs::write(
        plants.join("list.json"),
        serde_json::to_vec(&json!({"id": "plants", "title": "Plants", "items": items})).unwrap(),
    )
    .unwrap();

    let loaded = default_lists::load(&lists).unwrap();
    default_lists::install(&pool, &loaded, || false)
        .await
        .unwrap();
    // Newest first.
    let prompts: Vec<String> = facts::list_for(&pool, josiah)
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|fact| fact.prompt)
        .rev()
        .collect();
    assert_eq!(
        prompts,
        [
            "Is this a tui?",
            "Is this a kowhai?",
            "Is this a kea?",
            "Is this a ruru?"
        ]
    );
    pool.close().await;
}

#[test]
fn lists_that_could_not_be_installed_are_refused_when_loaded() {
    let refused = |items: Vec<serde_json::Value>| {
        let directory = tempfile::tempdir().unwrap();
        let lists = write_list(directory.path(), items);
        format!("{:#}", default_lists::load(&lists).unwrap_err())
    };
    let tui = |key: &str, media: serde_json::Value| item(key, "Which bird?", "Tūī", media);

    let twice = refused(vec![tui("tui", json!([])), tui("tui", json!([]))]);
    assert!(twice.contains("listed twice"), "{twice}");
    let outside = refused(vec![tui("tui", json!([photo("../outside.png", false)]))]);
    assert!(outside.contains("inside the list's directory"), "{outside}");
    let missing = refused(vec![tui("tui", json!([photo("media/kea.png", false)]))]);
    assert!(missing.contains("media/kea.png is missing"), "{missing}");
    let uncredited = refused(vec![tui(
        "tui",
        json!([{"file": "media/tui.png", "alt": "", "credit": " "}]),
    )]);
    assert!(uncredited.contains("needs a credit"), "{uncredited}");
    let key = refused(vec![tui("Tūī", json!([]))]);
    assert!(key.contains("item key"), "{key}");
    let blank = refused(vec![item("tui", " ", "Tūī", json!([]))]);
    assert!(blank.contains("prompt must not be empty"), "{blank}");
}

/// The lists the server ships load, and every file they name decodes as its extension
/// says, so none is refused when it is installed.
#[test]
fn shipped_default_lists_load_and_every_file_decodes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../default-lists");
    let lists = default_lists::load(&root).unwrap();
    assert!(!lists.is_empty());
    let mut files = Vec::new();
    for list in &lists {
        let json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(&list.id).join("list.json")).unwrap())
                .unwrap();
        for item in json["items"].as_array().unwrap() {
            for file in item["media"].as_array().unwrap() {
                let name = format!("{}/{}", list.id, file["file"].as_str().unwrap());
                assert!(
                    !file["alt"].as_str().unwrap_or_default().trim().is_empty(),
                    "{name} needs a description"
                );
                files.push(name);
            }
        }
    }
    files.sort();
    files.dedup();
    // Hundreds of files: decode them on every core.
    let threads = std::thread::available_parallelism().map_or(4, |count| count.get());
    std::thread::scope(|scope| {
        for chunk in files.chunks(files.len().div_ceil(threads)) {
            let root = &root;
            scope.spawn(move || {
                for name in chunk {
                    let path = root.join(name);
                    let extension = path.extension().unwrap().to_str().unwrap();
                    let format = UploadFormat::from_extension(extension).unwrap();
                    let bytes = std::fs::read(&path).unwrap();
                    media::inspect(format, &bytes)
                        .unwrap_or_else(|error| panic!("{name}: {error:#}"));
                }
            });
        }
    });
}
