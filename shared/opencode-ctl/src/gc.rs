//! `gc`: the shared startup sweep, then the pair-record collector with the dead-seen grace
//! (spec "Pairing contract", `gc`).
//!
//! Every decision and every path keys on the id in the FILE NAME (`pair.<id>.json`), validated
//! first: `pairing::read_record` and `list_records` key on it too, and this module deletes files.
//! `gc` never opens a log: the caller (the `gc` subcommand, or `pair` with its own log) owns that.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::clock::Clock;
use crate::engine::Engine;
use crate::liveness::{side_liveness, Liveness, ProcessProbe, Side};
use crate::pairing::{self, valid_pair_id, Record};
use crate::sweep::{startup_sweep, STALE_FILE_MS};
use crate::transport::Transport;

/// A side `unknown` for strictly more than this counts as dead.
pub const UNKNOWN_GRACE_MS: i64 = 60 * 60 * 1000;
/// A pair is collected only when its `dead-seen` is strictly older than this.
pub const DEAD_SEEN_GRACE_MS: i64 = STALE_FILE_MS;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GcReport {
    pub swept: Vec<PathBuf>,
    pub pairs_deleted: Vec<String>,
    pub dead_seen_written: Vec<String>,
    pub dead_seen_cleared: Vec<String>,
    pub skipped_service_down: bool,
}

/// What the two sides add up to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    /// Some side is `alive` or `starting`.
    Living,
    /// Some side is `unknown` for less than the grace and none is living: keeps the pair, changes nothing.
    Undecided,
    /// Every side is `dead`, `none` or unknown-past-1-h.
    AllDead,
}

fn mtime_ms(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    let d = t.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_millis() as i64)
}

fn pair_file(home: &Path, id: &str, suffix: &str) -> PathBuf {
    home.join(format!("pair.{id}.{suffix}"))
}

/// The pid sentinel that produced an `unknown` TUI reading: the record's generation when it has a
/// `tui-pid`, else the highest generation that has one (the rule `side_liveness` applies).
fn tui_pid_file(home: &Path, record: &Record) -> Option<PathBuf> {
    let id = &record.pair_id;
    let wanted = pair_file(home, id, &format!("g{}.tui-pid", record.generation));
    if wanted.exists() {
        return Some(wanted);
    }
    let prefix = format!("pair.{id}.g");
    std::fs::read_dir(home)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let g = name
                .strip_prefix(&prefix)?
                .strip_suffix(".tui-pid")?
                .parse::<u32>()
                .ok()?;
            Some((g, e.path()))
        })
        .max_by_key(|(g, _)| *g)
        .map(|(_, p)| p)
}

/// Age of an `unknown` side: the mtime of its sentinel, else the record's `created`.
fn unknown_age_ms(side: Side, record: &Record, home: &Path, now_ms: i64) -> i64 {
    let sentinel = match side {
        Side::Tui => tui_pid_file(home, record),
        Side::Controller => Some(pair_file(home, &record.pair_id, "claude-pid")),
    };
    let origin = sentinel
        .and_then(|p| mtime_ms(&p))
        .unwrap_or(record.created);
    now_ms - origin
}

fn verdict(record: &Record, home: &Path, probe: &dyn ProcessProbe, now_ms: i64) -> Verdict {
    let mut undecided = false;
    for side in [Side::Tui, Side::Controller] {
        match side_liveness(side, record, home, probe, now_ms) {
            Liveness::Alive | Liveness::Starting => return Verdict::Living,
            Liveness::Dead | Liveness::None => {}
            Liveness::Unknown => {
                if unknown_age_ms(side, record, home, now_ms) <= UNKNOWN_GRACE_MS {
                    undecided = true;
                }
            }
        }
    }
    if undecided {
        Verdict::Undecided
    } else {
        Verdict::AllDead
    }
}

/// The record behind `pair.<id>.json`, keyed on the file name: a regular file (never a symlink)
/// that parses (`read_record` keys it on the file name's id).
fn load_record(home: &Path, id: &str) -> Option<Record> {
    if !valid_pair_id(id) {
        return None;
    }
    let meta = std::fs::symlink_metadata(pairing::record_path(home, id)).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    pairing::read_record(home, id).ok().flatten()
}

/// `dead-seen` content as `<epoch ms>`; absent or unparsable is `None`.
fn read_dead_seen(home: &Path, id: &str) -> Option<i64> {
    std::fs::read_to_string(pair_file(home, id, "dead-seen"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn remove_dead_seen(home: &Path, id: &str) -> bool {
    std::fs::remove_file(pair_file(home, id, "dead-seen")).is_ok()
}

fn write_dead_seen(home: &Path, id: &str, now_ms: i64) -> bool {
    let path = pair_file(home, id, "dead-seen");
    if path.exists() {
        // present but unreadable: restart the grace rather than trust it
        let _ = std::fs::remove_file(&path);
    }
    pairing::claim_file(&path, now_ms.to_string().as_bytes(), 0o600).unwrap_or(false)
}

/// Every `pair.<id>.*` entry must be a regular file directly in the home (no symlink, no directory).
fn pair_files_deletable(home: &Path, id: &str) -> Option<Vec<PathBuf>> {
    let prefix = format!("pair.{id}.");
    let mut files = Vec::new();
    for e in std::fs::read_dir(home).ok()?.flatten() {
        let name = e.file_name().into_string().ok()?;
        if !name.starts_with(&prefix) {
            continue;
        }
        let path = e.path();
        if path.parent() != Some(home) || !std::fs::symlink_metadata(&path).ok()?.is_file() {
            return None;
        }
        files.push(path);
    }
    Some(files)
}

/// Delete every file of the pair, the record last. Nothing is deleted when any entry is not a
/// regular file.
fn delete_pair(home: &Path, id: &str) -> bool {
    let record = pairing::record_path(home, id);
    let Some(files) = pair_files_deletable(home, id) else {
        tracing::warn!("gc: pair {id} has a non-regular entry; left alone");
        return false;
    };
    for f in files.iter().filter(|f| **f != record) {
        if let Err(e) = std::fs::remove_file(f) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("gc: cannot delete {}: {e}", f.display());
                return false;
            }
        }
    }
    pairing::delete_pair_files(home, id).is_ok()
}

/// `session.active` read once: the set of active session ids, or `None` when it cannot be read.
async fn active_sessions<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
) -> Option<std::collections::HashSet<String>> {
    let (data, _, _) = engine
        .call_json("session.active", Vec::new(), None)
        .await
        .ok()?;
    Some(data.as_object()?.keys().cloned().collect())
}

/// The sweep, then the record loop (skipped when `session.active` cannot be read).
/// `before_delete` is the test failpoint, called after the grace has elapsed and before the
/// liveness is re-read for the deletion decision; every real caller passes `None`.
pub async fn gc<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    probe: &dyn ProcessProbe,
    now_ms: i64,
    before_delete: Option<&dyn Fn()>,
) -> GcReport {
    let home = engine.env.home.as_path();
    let mut report = GcReport {
        swept: startup_sweep(home, now_ms, probe).removed,
        ..GcReport::default()
    };
    let Some(active) = active_sessions(engine).await else {
        tracing::warn!("gc: session.active unreadable; no pair is touched");
        report.skipped_service_down = true;
        return report;
    };
    for id in pairing::record_ids(home) {
        let Some(record) = load_record(home, &id) else {
            continue;
        };
        match verdict(&record, home, probe, now_ms) {
            Verdict::Living => {
                if remove_dead_seen(home, &id) {
                    tracing::info!("gc: pair {id} is alive again; dead-seen cleared");
                    report.dead_seen_cleared.push(id);
                }
            }
            Verdict::Undecided => {}
            Verdict::AllDead => {
                let Some(seen) = read_dead_seen(home, &id) else {
                    if write_dead_seen(home, &id, now_ms) {
                        tracing::info!("gc: pair {id} reads dead; dead-seen written");
                        report.dead_seen_written.push(id);
                    }
                    continue;
                };
                if now_ms - seen <= DEAD_SEEN_GRACE_MS || active.contains(&record.session_id) {
                    continue;
                }
                if let Some(f) = before_delete {
                    f();
                }
                // Re-read everything for the decision: record, liveness, the grace file.
                let Some(record) = load_record(home, &id) else {
                    continue;
                };
                match verdict(&record, home, probe, now_ms) {
                    Verdict::Living => {
                        if remove_dead_seen(home, &id) {
                            tracing::info!(
                                "gc: pair {id} revived before deletion; dead-seen cleared"
                            );
                            report.dead_seen_cleared.push(id);
                        }
                    }
                    Verdict::Undecided => {}
                    Verdict::AllDead => {
                        let still_old = read_dead_seen(home, &id)
                            .is_some_and(|s| now_ms - s > DEAD_SEEN_GRACE_MS);
                        if still_old
                            && !active.contains(&record.session_id)
                            && delete_pair(home, &id)
                        {
                            tracing::info!("gc: pair {id} deleted");
                            report.pairs_deleted.push(id);
                        }
                    }
                }
            }
        }
    }
    report
}
