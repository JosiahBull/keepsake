//! The household's default lists: facts asked with images and audio clips, such as New
//! Zealand birds by sight and song, that the server adds to Josiah's and Ansam's
//! libraries. The text-only household facts arrive through a migration; these cannot,
//! because each file must be checked and converted like any upload.
//!
//! A list is a directory holding `list.json` and the files it names. [`load`] reads and
//! checks every list when the server starts, and [`install`] adds what each library is
//! missing. Each item is added once, recorded by its key, so a later version of a list
//! adds only its new items and never repeats or overwrites one already added.
use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, ensure};
use serde::Deserialize;
use sqlx::{SqliteConnection, SqlitePool};

use crate::{
    facts::{self, NewFact, NewPractice},
    media::{self, NewQuestionMedia, UploadFormat},
};

/// The members each list is for, found by name or handle so a renamed profile keeps
/// receiving new items.
const MEMBERS: [&str; 2] = ["josiah", "ansam"];

/// A list read from its directory and checked, ready to install.
#[derive(Clone, Debug)]
pub struct DefaultList {
    pub id: String,
    pub title: String,
    directory: PathBuf,
    items: Vec<ListItem>,
}

impl DefaultList {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// `list.json`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListFile {
    /// Names the list in each library's record of what was added. Never change it.
    id: String,
    title: String,
    items: Vec<ListItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListItem {
    /// Identifies the item within its list. Never reuse one for different content.
    key: String,
    prompt: String,
    answer: String,
    #[serde(default)]
    context: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    practices: Vec<NewPractice>,
    #[serde(default)]
    media: Vec<ListMedia>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListMedia {
    /// Relative to the list's directory. Its extension names its format.
    file: String,
    #[serde(default)]
    alt: String,
    #[serde(default)]
    ai_generated: bool,
    /// Who made the file, its licence and where it came from. Required, so every file a
    /// list ships can be traced; the fact's context carries the credit learners see.
    credit: String,
}

impl ListItem {
    /// The fact this item adds, asked with the given uploads in the item's order.
    fn fact(&self, media_ids: &[i64]) -> NewFact {
        NewFact {
            prompt: self.prompt.clone(),
            answer: self.answer.clone(),
            context: self.context.clone(),
            source: self.source.clone(),
            tags: self.tags.clone(),
            practices: self.practices.clone(),
            media: self
                .media
                .iter()
                .zip(media_ids)
                .map(|(item, &media_id)| NewQuestionMedia {
                    media_id,
                    alt: item.alt.clone(),
                    ai_generated: item.ai_generated,
                })
                .collect(),
        }
    }
}

fn valid_name(value: &str, limit: usize) -> bool {
    (1..=limit).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Read and check every list under `directory`: one subdirectory per list. Nothing is
/// decoded here; a file that does not decode is reported when it is installed.
pub fn load(directory: &Path) -> anyhow::Result<Vec<DefaultList>> {
    let mut entries = std::fs::read_dir(directory)
        .with_context(|| format!("read default lists in {}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort();
    let mut lists: Vec<DefaultList> = Vec::new();
    for path in entries.into_iter().filter(|path| path.is_dir()) {
        let list = load_one(&path).with_context(|| format!("default list {}", path.display()))?;
        ensure!(
            lists.iter().all(|other| other.id != list.id),
            "two default lists have the id {}",
            list.id
        );
        lists.push(list);
    }
    Ok(lists)
}

fn load_one(directory: &Path) -> anyhow::Result<DefaultList> {
    let json = std::fs::read(directory.join("list.json")).context("read list.json")?;
    let ListFile { id, title, items } = serde_json::from_slice(&json).context("parse list.json")?;
    ensure!(
        valid_name(&id, 100),
        "id must be 1 to 100 lowercase letters, digits or hyphens"
    );
    ensure!(
        directory.file_name().and_then(|name| name.to_str()) == Some(id.as_str()),
        "the list's directory must be named after its id, {id}"
    );
    ensure!(!title.trim().is_empty(), "title must not be empty");
    let mut keys = HashSet::new();
    for item in &items {
        let at = || format!("item {}", item.key);
        ensure!(
            valid_name(&item.key, 200),
            "item key {:?} must be 1 to 200 lowercase letters, digits or hyphens",
            item.key
        );
        ensure!(keys.insert(item.key.as_str()), "{} is listed twice", at());
        let placeholders: Vec<i64> = (1..=item.media.len() as i64).collect();
        facts::validate_new_fact(&item.fact(&placeholders)).with_context(at)?;
        for file in &item.media {
            ensure!(
                !file.credit.trim().is_empty(),
                "{}: {} needs a credit",
                at(),
                file.file
            );
            media_path(directory, &file.file).with_context(at)?;
        }
    }
    Ok(DefaultList {
        id,
        title,
        directory: directory.to_path_buf(),
        items,
    })
}

/// A list's file and the format its extension claims, refusing paths that leave the
/// list's directory.
fn media_path(directory: &Path, file: &str) -> anyhow::Result<(PathBuf, UploadFormat)> {
    let relative = Path::new(file);
    ensure!(
        relative
            .components()
            .all(|component| matches!(component, Component::Normal(_))),
        "{file} must be a path inside the list's directory"
    );
    let format = relative
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(UploadFormat::from_extension)
        .with_context(|| format!("{file} has no supported extension"))?;
    let path = directory.join(relative);
    let metadata = std::fs::metadata(&path).with_context(|| format!("{file} is missing"))?;
    ensure!(metadata.is_file(), "{file} is not a file");
    ensure!(
        metadata.len() <= media::MAX_MEDIA_BYTES as u64,
        "{file} is larger than 5 MiB"
    );
    Ok((path, format))
}

/// What [`install`] did.
#[derive(Debug, Default)]
pub struct Installed {
    /// Facts added, counted once for each library they were added to.
    pub added: usize,
    /// Members with no profile, who received nothing.
    pub missing: Vec<&'static str>,
    /// Items that could not be added, by list and key, and why. They are tried again the
    /// next time the server starts.
    pub failed: Vec<(String, anyhow::Error)>,
}

/// Add each list's missing items to each member's library. Uploads are checked and
/// converted as they would be from the app, once for all members. `stopped` is checked
/// between items, so a shutdown leaves whole facts and finishes the rest next time.
///
/// New facts are introduced for study in the order they were added, so lists take turns
/// item by item: every list starts reaching the study queue at once.
pub async fn install(
    pool: &SqlitePool,
    lists: &[DefaultList],
    stopped: impl Fn() -> bool,
) -> anyhow::Result<Installed> {
    let mut installed = Installed::default();
    let mut members = Vec::new();
    for member in MEMBERS {
        match profile_of(pool, member).await? {
            Some(profile_id) => members.push(profile_id),
            None => installed.missing.push(member),
        }
    }
    let longest = lists.iter().map(DefaultList::len).max().unwrap_or(0);
    for index in 0..longest {
        for list in lists {
            let Some(item) = list.items.get(index) else {
                continue;
            };
            if stopped() {
                return Ok(installed);
            }
            match install_item(pool, list, item, &members).await {
                Ok(added) => installed.added += added,
                Err(error) => installed
                    .failed
                    .push((format!("{}/{}", list.id, item.key), error)),
            }
        }
    }
    Ok(installed)
}

async fn profile_of(pool: &SqlitePool, member: &str) -> anyhow::Result<Option<i64>> {
    Ok(sqlx::query_scalar!(
        "SELECT id AS \"id!\" FROM profiles WHERE lower(trim(name)) = ? OR handle = ?
         ORDER BY lower(trim(name)) = ? DESC, id LIMIT 1",
        member,
        member,
        member
    )
    .fetch_optional(pool)
    .await?)
}

async fn added(
    connection: &mut SqliteConnection,
    profile_id: i64,
    list: &str,
    item: &str,
) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar!(
        "SELECT EXISTS(
           SELECT 1 FROM default_list_items WHERE profile_id=? AND list=? AND item=?
         ) AS \"added!: bool\"",
        profile_id,
        list,
        item
    )
    .fetch_one(connection)
    .await?)
}

async fn install_item(
    pool: &SqlitePool,
    list: &DefaultList,
    item: &ListItem,
    members: &[i64],
) -> anyhow::Result<usize> {
    let mut pending = Vec::new();
    {
        let mut connection = pool.acquire().await?;
        for &profile_id in members {
            if !added(&mut connection, profile_id, &list.id, &item.key).await? {
                pending.push(profile_id);
            }
        }
    }
    let Some(&uploader) = pending.first() else {
        return Ok(0);
    };
    // Converted once, for the first library; the others share its files.
    let mut uploaded = Vec::with_capacity(item.media.len());
    for file in &item.media {
        let (path, format) = media_path(&list.directory, &file.file)?;
        let bytes = tokio::fs::read(&path)
            .await
            .with_context(|| format!("read {}", path.display()))?;
        let upload = media::upload(pool, uploader, format, bytes)
            .await
            .with_context(|| format!("store {}", file.file))?;
        uploaded.push(upload.media_id);
    }
    let list_id = list.id.clone();
    let item = item.clone();
    crate::db::write_transaction(pool, move |connection| {
        let (list_id, item) = (list_id.clone(), item.clone());
        let (pending, uploaded) = (pending.clone(), uploaded.clone());
        Box::pin(async move {
            let mut count = 0;
            for profile_id in pending {
                if added(connection, profile_id, &list_id, &item.key).await? {
                    continue;
                }
                let mut media_ids = Vec::with_capacity(uploaded.len());
                for &media_id in &uploaded {
                    media_ids.push(if profile_id == uploader {
                        media_id
                    } else {
                        media::copy(connection, uploader, media_id, profile_id).await?
                    });
                }
                let fact =
                    facts::insert_fact(connection, profile_id, item.fact(&media_ids)).await?;
                sqlx::query!(
                    "INSERT INTO default_list_items(profile_id, list, item, fact_id)
                     VALUES(?, ?, ?, ?)",
                    profile_id,
                    list_id,
                    item.key,
                    fact.id
                )
                .execute(&mut *connection)
                .await?;
                count += 1;
            }
            Ok(count)
        })
    })
    .await
}
