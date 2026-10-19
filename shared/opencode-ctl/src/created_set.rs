//! The per-directory set of sessions this controller created (OpenCode host guard).
//!
//! Files in the ctl home: `created.<key>.json` = `[{id, directory}]` and its sidecar
//! `created.<key>.lock`, which carries a `File::lock` for every read-merge-write.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 30 days, the age past which `sweep` removes untouched files.
pub const SWEEP_MAX_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Ids probed with `session.get` per append at most.
pub const PROBE_LIMIT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub directory: String,
}

#[derive(Debug, Clone)]
pub struct CreatedSet {
    home: PathBuf,
}

/// Drop a Windows verbatim prefix: `\\?\C:\x` -> `C:\x`, `\\?\UNC\h\s` -> `\\h\s`.
pub fn strip_verbatim(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.to_string()
    }
}

/// Resolve `.` and `..` without touching the file system.
fn lexical_normalise(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() && !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// 12 hex of sha256 of the canonicalised path (lexically normalised when it does not exist),
/// Windows verbatim prefix stripped before hashing.
pub fn key_for(directory: &Path) -> String {
    let resolved =
        std::fs::canonicalize(directory).unwrap_or_else(|_| lexical_normalise(directory));
    let text = strip_verbatim(&resolved.to_string_lossy());
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

fn read_entries(path: &Path) -> io::Result<Vec<Entry>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

impl CreatedSet {
    pub fn new(home: PathBuf) -> Self {
        Self { home }
    }

    fn data_path(&self, key: &str) -> PathBuf {
        self.home.join(format!("created.{key}.json"))
    }

    fn lock_path(&self, key: &str) -> PathBuf {
        self.home.join(format!("created.{key}.lock"))
    }

    fn open_lock(&self, key: &str) -> io::Result<File> {
        std::fs::create_dir_all(&self.home)?;
        let f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.lock_path(key))?;
        f.lock()?;
        Ok(f)
    }

    /// Read-merge-write one key's file under its lock.
    fn mutate(&self, key: &str, f: impl FnOnce(&mut Vec<Entry>)) -> io::Result<()> {
        let _lock = self.open_lock(key)?;
        let path = self.data_path(key);
        let mut entries = read_entries(&path)?;
        let before = entries.clone();
        f(&mut entries);
        if entries == before && path.exists() {
            return Ok(());
        }
        let tmp = self.home.join(format!(
            "created.{key}.tmp{}-{}",
            std::process::id(),
            TMP_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(
            &tmp,
            serde_json::to_vec(&entries).map_err(io::Error::other)?,
        )?;
        std::fs::rename(&tmp, &path).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    pub fn append(&self, directory: &Path, id: &str) -> io::Result<()> {
        let key = key_for(directory);
        let entry = Entry {
            id: id.to_string(),
            directory: directory.to_string_lossy().into_owned(),
        };
        self.mutate(&key, |v| {
            if !v.iter().any(|e| e.id == id) {
                v.push(entry);
            }
        })
    }

    /// Every `created.<key>.json` in the home, as `(key, path)`.
    fn data_files(&self) -> Vec<(String, PathBuf)> {
        let Ok(rd) = std::fs::read_dir(&self.home) else {
            return Vec::new();
        };
        let mut out: Vec<(String, PathBuf)> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let key = name.strip_prefix("created.")?.strip_suffix(".json")?;
                Some((key.to_string(), e.path()))
            })
            .collect();
        out.sort();
        out
    }

    pub fn prune(&self, id: &str) -> io::Result<()> {
        for (key, path) in self.data_files() {
            // A file that vanishes or does not parse reads as empty: nothing to prune there.
            if read_entries(&path)
                .unwrap_or_default()
                .iter()
                .any(|e| e.id == id)
            {
                self.mutate(&key, |v| v.retain(|e| e.id != id))?;
            }
        }
        Ok(())
    }

    /// The directory recorded for `id`, scanning every file without locking.
    pub fn contains(&self, id: &str) -> io::Result<Option<PathBuf>> {
        for (_, path) in self.data_files() {
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(entries) = serde_json::from_slice::<Vec<Entry>>(&bytes) else {
                continue;
            };
            if let Some(e) = entries.iter().find(|e| e.id == id) {
                return Ok(Some(PathBuf::from(&e.directory)));
            }
        }
        Ok(None)
    }

    /// The ids stored for `directory`'s key, in stored order.
    pub fn ids_for(&self, directory: &Path) -> Vec<String> {
        read_entries(&self.data_path(&key_for(directory)))
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect()
    }

    pub fn ids(&self) -> Vec<String> {
        self.data_files()
            .into_iter()
            .flat_map(|(_, p)| read_entries(&p).unwrap_or_default())
            .map(|e| e.id)
            .collect()
    }

    /// Remove data files older than `max_age_ms` (by the data file's mtime) together with their
    /// `.lock`, holding the lock; a `.lock` alone is removed only when itself older than 30 days.
    pub fn sweep(&self, now_ms: i64, max_age_ms: i64) -> Vec<PathBuf> {
        let age_of = |p: &Path| -> Option<i64> {
            let m = std::fs::metadata(p).ok()?.modified().ok()?;
            let ms = m.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
            Some(now_ms - ms)
        };
        let mut removed = Vec::new();
        for (key, path) in self.data_files() {
            if !age_of(&path).is_some_and(|a| a > max_age_ms) {
                continue;
            }
            let Ok(lock) = self.open_lock(&key) else {
                continue;
            };
            // Re-check under the lock: a writer may have refreshed the file meanwhile.
            if age_of(&path).is_some_and(|a| a > max_age_ms) && std::fs::remove_file(&path).is_ok()
            {
                let _ = std::fs::remove_file(self.lock_path(&key));
                removed.push(path);
            }
            drop(lock);
        }
        if let Ok(rd) = std::fs::read_dir(&self.home) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let Some(key) = name
                    .strip_prefix("created.")
                    .and_then(|r| r.strip_suffix(".lock"))
                else {
                    continue;
                };
                if !self.data_path(key).exists()
                    && age_of(&e.path()).is_some_and(|a| a > SWEEP_MAX_AGE_MS)
                    && std::fs::remove_file(e.path()).is_ok()
                {
                    removed.push(e.path());
                }
            }
        }
        removed
    }
}
