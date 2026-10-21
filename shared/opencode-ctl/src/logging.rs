//! File logging (spec "Logging"): `<home>/logs/serve-<pid>.log` and `pair-<pair-id>.log`, level
//! `info` (`debug` on request), a 5 MiB cap that rolls once to `.1`, and retention. Nothing here
//! ever writes to stdout: stdout is the MCP channel.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tracing::subscriber::DefaultGuard;
use tracing_appender::non_blocking::WorkerGuard;

use crate::liveness::ProcessProbe;
use crate::pairing::valid_pair_id;

/// The size at which a log file rolls to `.1`.
pub const LOG_CAP_BYTES: u64 = 5 * 1024 * 1024;
/// Retention: files beyond the newest 20 of their kind ...
pub const KEEP_NEWEST: usize = 20;
/// ... AND older than this.
pub const RETAIN_MAX_AGE_MS: i64 = 14 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogScope {
    /// The process-wide subscriber, installed once per process (`serve`, the launcher subcommands).
    Global,
    /// A thread-local default subscriber, uninstalled when the handle drops (tests).
    Scoped,
}

/// Keeps the log alive: dropping it flushes the worker and (Scoped) uninstalls the subscriber.
pub struct LogHandle {
    worker: WorkerGuard,
    default: Option<DefaultGuard>,
}

impl LogHandle {
    pub fn is_scoped(&self) -> bool {
        self.default.is_some()
    }

    pub fn worker(&self) -> &WorkerGuard {
        &self.worker
    }
}

/// An append-only file that moves itself to `<name>.1` (replacing any earlier one) before a write
/// that would take it past the cap. Whole writes stay whole; there is never a `.2`.
pub struct CappedWriter {
    path: PathBuf,
    cap: u64,
    file: Option<File>,
    size: u64,
}

fn open_append(path: &Path) -> io::Result<File> {
    let mut o = OpenOptions::new();
    o.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(path)
}

fn create_logs_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)
}

fn rolled_name(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

impl CappedWriter {
    pub fn open(path: &Path, cap: u64) -> io::Result<Self> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            create_logs_dir(dir)?;
        }
        let file = open_append(path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            path: path.to_path_buf(),
            cap,
            file: Some(file),
            size,
        })
    }

    fn roll(&mut self) -> io::Result<()> {
        if let Some(mut f) = self.file.take() {
            let _ = f.flush();
        }
        // A failed rename keeps appending to the same file rather than losing lines.
        let _ = std::fs::rename(&self.path, rolled_name(&self.path));
        let file = open_append(&self.path)?;
        self.size = file.metadata()?.len();
        self.file = Some(file);
        Ok(())
    }
}

impl Write for CappedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.size > 0 && self.size + buf.len() as u64 > self.cap {
            self.roll()?;
        }
        let f = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("log file closed"))?;
        let n = f.write(buf)?;
        self.size += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

/// Open `path` as a non-blocking, ANSI-free file log. `Global` installs the process subscriber
/// with `try_init` (a second call returns a handle and leaves the first subscriber in place);
/// `Scoped` installs a thread-local default whose guard lives in the handle.
pub fn init_file_logging(
    path: &Path,
    debug: bool,
    scope: LogScope,
) -> Result<LogHandle, io::Error> {
    let writer = CappedWriter::open(path, LOG_CAP_BYTES)?;
    let (non_blocking, worker) = tracing_appender::non_blocking(writer);
    let level = if debug {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let builder = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_max_level(level);
    let default = match scope {
        LogScope::Global => {
            let _ = builder.try_init();
            None
        }
        LogScope::Scoped => Some(tracing::subscriber::set_default(builder.finish())),
    };
    Ok(LogHandle { worker, default })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Serve,
    Pair,
}

/// `serve-<pid>.log[.1]` / `pair-<12 hex>.log[.1]` -> (kind, id, is_rolled).
fn classify(name: &str) -> Option<(Kind, &str, bool)> {
    let (stem, rolled) = match name.strip_suffix(".log.1") {
        Some(s) => (s, true),
        None => (name.strip_suffix(".log")?, false),
    };
    if let Some(pid) = stem.strip_prefix("serve-") {
        if !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()) {
            return Some((Kind::Serve, pid, rolled));
        }
    }
    if let Some(id) = stem.strip_prefix("pair-") {
        if valid_pair_id(id) {
            return Some((Kind::Pair, id, rolled));
        }
    }
    None
}

fn mtime_ms(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    let d = t.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_millis() as i64)
}

struct Group {
    id: String,
    files: Vec<PathBuf>,
    newest_ms: i64,
}

/// Log retention (spec Logging). A base log and its `.1` form one group of their kind (`serve-*`
/// and `pair-*` are counted separately); a group is deleted only when it is beyond the newest 20
/// of its kind AND its newest file is older than 14 days AND its owner is gone: for `serve-<pid>`
/// the pid is dead, for `pair-<id>` `pair_is_dead(id)`. Decisions key on the FILE NAME's id.
/// Returns the files removed.
pub fn retain(
    logs_dir: &Path,
    now_ms: i64,
    probe: &dyn ProcessProbe,
    pair_is_dead: &dyn Fn(&str) -> bool,
) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(logs_dir) else {
        return Vec::new();
    };
    let mut groups: BTreeMap<(Kind, String), Group> = BTreeMap::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some((kind, id, _)) = name.to_str().and_then(classify) else {
            continue;
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let m = mtime_ms(&path).unwrap_or(now_ms);
        let g = groups
            .entry((kind, id.to_string()))
            .or_insert_with(|| Group {
                id: id.to_string(),
                files: Vec::new(),
                newest_ms: i64::MIN,
            });
        g.files.push(path);
        g.newest_ms = g.newest_ms.max(m);
    }
    let mut removed = Vec::new();
    for kind in [Kind::Serve, Kind::Pair] {
        let mut of_kind: Vec<&Group> = groups
            .iter()
            .filter(|((k, _), _)| *k == kind)
            .map(|(_, g)| g)
            .collect();
        of_kind.sort_by(|a, b| b.newest_ms.cmp(&a.newest_ms).then_with(|| b.id.cmp(&a.id)));
        for g in of_kind.into_iter().skip(KEEP_NEWEST) {
            if now_ms - g.newest_ms <= RETAIN_MAX_AGE_MS {
                continue;
            }
            let gone = match kind {
                Kind::Serve => g.id.parse::<u32>().is_ok_and(|pid| !probe.is_alive(pid)),
                Kind::Pair => pair_is_dead(&g.id),
            };
            if !gone {
                continue;
            }
            for f in &g.files {
                if std::fs::remove_file(f).is_ok() {
                    removed.push(f.clone());
                }
            }
        }
    }
    removed
}
