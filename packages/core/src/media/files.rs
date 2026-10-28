//! Media files beside the database: `tmp/` for work in progress, and `media/images/` and
//! `media/audio/` for what is served, each named by the SHA-256 of its contents.
//!
//! One Keepsake database per directory. The server removes files no row references, so
//! two databases sharing a directory would remove each other's.

use std::{
    collections::HashSet,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use sqlx::SqlitePool;

use super::{MediaKind, sha256_hex};

#[derive(Clone, Debug)]
pub struct MediaFiles {
    root: PathBuf,
}

fn kind_directory(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Image => "images",
        MediaKind::Audio => "audio",
    }
}

/// The file extension for each served content type.
pub(crate) fn extension(content_type: &str) -> Option<&'static str> {
    Some(match content_type {
        "image/avif" => "avif",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/png" => "png",
        "audio/mp4" => "m4a",
        _ => return None,
    })
}

impl MediaFiles {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The directory holding a database file.
    pub fn for_database(path: &Path) -> Self {
        Self::new(
            path.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
    }

    /// The directory holding this pool's database file.
    pub fn for_pool(pool: &SqlitePool) -> Self {
        Self::for_database(pool.connect_options().get_filename())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub fn media(&self) -> PathBuf {
        self.root.join("media")
    }

    pub(crate) fn kind_dir(&self, kind: MediaKind) -> PathBuf {
        self.media().join(kind_directory(kind))
    }

    pub(crate) fn path(&self, kind: MediaKind, file_sha256: &str, content_type: &str) -> PathBuf {
        self.media()
            .join(Self::relative(kind, file_sha256, content_type))
    }

    /// A served file's path within `media/`, such as `images/<sha256>.avif`.
    pub(crate) fn relative(kind: MediaKind, file_sha256: &str, content_type: &str) -> PathBuf {
        let extension = extension(content_type).unwrap_or("bin");
        Path::new(kind_directory(kind)).join(format!("{file_sha256}.{extension}"))
    }

    /// A private file in `tmp/` for an upload in progress, removed when dropped.
    pub fn partial_upload(&self) -> io::Result<tempfile::NamedTempFile> {
        fs::create_dir_all(self.tmp())?;
        tempfile::Builder::new()
            .suffix(".upload")
            .tempfile_in(self.tmp())
    }

    /// Write a served file and return its SHA-256. It is written in `tmp/`, synced, then
    /// renamed into place, so a reader never sees part of it. An identical file is
    /// replaced, which also renews its age for [`MediaFiles::sweep`].
    pub(crate) fn place(
        &self,
        kind: MediaKind,
        content_type: &str,
        data: &[u8],
    ) -> io::Result<String> {
        let directory = self.kind_dir(kind);
        fs::create_dir_all(&directory)?;
        fs::create_dir_all(self.tmp())?;
        let sha256 = sha256_hex(data);
        let mut partial = tempfile::Builder::new()
            .suffix(".part")
            .tempfile_in(self.tmp())?;
        partial.write_all(data)?;
        partial.as_file().sync_all()?;
        partial
            .persist(self.path(kind, &sha256, content_type))
            .map_err(|error| error.error)?;
        #[cfg(unix)]
        fs::File::open(&directory)?.sync_all()?;
        Ok(sha256)
    }

    /// Remove everything in `tmp/`. Only the server calls this, as it starts.
    pub fn clean_tmp(&self) -> io::Result<usize> {
        let entries = match fs::read_dir(self.tmp()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error),
        };
        let mut removed = 0;
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                fs::remove_dir_all(entry.path())?;
            } else {
                fs::remove_file(entry.path())?;
            }
            removed += 1;
        }
        Ok(removed)
    }

    /// Remove served files no row references, once they are older than `min_age`. The
    /// age spares a file written moments before the row that will reference it.
    pub(crate) fn sweep(
        &self,
        referenced: &HashSet<String>,
        min_age: Duration,
    ) -> io::Result<usize> {
        let now = SystemTime::now();
        let mut removed = 0;
        for kind in [MediaKind::Image, MediaKind::Audio] {
            let entries = match fs::read_dir(self.kind_dir(kind)) {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                // Only files this module names are candidates; anything else is left alone.
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                if stem.len() != 64
                    || !stem.bytes().all(|byte| byte.is_ascii_hexdigit())
                    || referenced.contains(stem)
                {
                    continue;
                }
                let modified = entry.metadata()?.modified()?;
                if now.duration_since(modified).unwrap_or_default() >= min_age {
                    fs::remove_file(&path)?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}
