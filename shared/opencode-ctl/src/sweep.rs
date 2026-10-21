//! The startup sweep: the one list of housekeeping that `serve` runs at startup and `gc` reuses
//! (spec "gc"). Every failure is collected, never fatal.

use std::path::{Path, PathBuf};

use crate::created_set::{CreatedSet, SWEEP_MAX_AGE_MS};
use crate::liveness::{self, sentinel_alive, side_liveness, Liveness, ProcessProbe, Side};
use crate::pairing::{self, valid_pair_id};

/// Sentinel files and claim temps are removed when untouched for this long.
pub const STALE_FILE_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Default)]
pub struct SweepReport {
    pub removed: Vec<PathBuf>,
    pub errors: Vec<String>,
}

impl SweepReport {
    fn remove_file(&mut self, path: &Path) {
        match std::fs::remove_file(path) {
            Ok(()) => self.removed.push(path.to_path_buf()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => self.errors.push(format!("{}: {e}", path.display())),
        }
    }
}

fn age_ms(path: &Path, now_ms: i64) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    let ms = t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
    Some(now_ms - ms)
}

/// The pair's record is gone, or both its sides read `dead` or `none`. The record is looked up by
/// the id in the FILE NAME, and liveness is derived for that id even if the record's contents
/// disagree. An unreadable record is not "dead".
pub fn pair_is_dead(home: &Path, id: &str, now_ms: i64, probe: &dyn ProcessProbe) -> bool {
    match pairing::read_record(home, id) {
        Ok(None) => !pairing::record_path(home, id).exists(),
        Ok(Some(mut r)) => {
            r.pair_id = id.to_string();
            [Side::Tui, Side::Controller].into_iter().all(|side| {
                matches!(
                    side_liveness(side, &r, home, probe, now_ms),
                    Liveness::Dead | Liveness::None
                )
            })
        }
        Err(_) => false,
    }
}

/// `pair.<id>.<rest>` -> (id, rest) for a valid id.
fn pair_file_parts(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("pair.")?;
    let (id, rest) = rest.split_once('.')?;
    valid_pair_id(id).then_some((id, rest))
}

fn is_sentinel_name(rest: &str) -> bool {
    rest == "claude-pid"
        || rest == "claude-exited"
        || (rest.starts_with('g') && (rest.ends_with(".tui-pid") || rest.ends_with(".tui-exited")))
}

/// Windows launcher leftover: `opencode-pair.exe.old` under the launcher install directory.
pub fn sweep_old_launcher(dir: &Path, report: &mut SweepReport) {
    let old = dir.join("opencode-pair.exe.old");
    if old.exists() {
        report.remove_file(&old);
    }
}

/// Log retention, dead stdout dirs, aged `created.*`, `resume-lock` files whose owner is dead,
/// `opencode-pair.exe.old` (Windows), sentinel files without a record older than 24 h and
/// `*.tmp-*` claim temps older than 24 h.
pub fn startup_sweep(home: &Path, now_ms: i64, probe: &dyn ProcessProbe) -> SweepReport {
    let mut report = SweepReport::default();
    report.removed.extend(crate::logging::retain(
        &home.join("logs"),
        now_ms,
        probe,
        &|id| pair_is_dead(home, id, now_ms, probe),
    ));
    report
        .removed
        .extend(liveness::sweep_stdout_dirs(home, probe));
    report
        .removed
        .extend(CreatedSet::new(home.to_path_buf()).sweep(now_ms, SWEEP_MAX_AGE_MS));

    if let Ok(entries) = std::fs::read_dir(home) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !path.is_file() {
                continue;
            }
            if name.contains(".tmp-") {
                if age_ms(&path, now_ms).is_some_and(|a| a > STALE_FILE_MS) {
                    report.remove_file(&path);
                }
                continue;
            }
            let Some((id, rest)) = pair_file_parts(name) else {
                continue;
            };
            if rest == "resume-lock" {
                // An empty or unparsable lock may be mid-creation: left alone.
                if pairing::read_sentinel(&path).is_some_and(|s| !sentinel_alive(&s, probe)) {
                    report.remove_file(&path);
                }
            } else if is_sentinel_name(rest)
                && !pairing::record_path(home, id).exists()
                && age_ms(&path, now_ms).is_some_and(|a| a > STALE_FILE_MS)
            {
                report.remove_file(&path);
            }
        }
    }

    #[cfg(windows)]
    if let Some(local) = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
        sweep_old_launcher(&PathBuf::from(local).join("opencode-ctl"), &mut report);
    }
    report
}
