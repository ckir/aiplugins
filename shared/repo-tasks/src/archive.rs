// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The archive (8.5): append-only lines of tasks that left the list, and start-up archiving.

use crate::clock::{parse_rfc3339, rfc3339, Clock};
use crate::error::{Error, Result};
use crate::model::{List, Status, Task};
use crate::resolve::{FileKind, Store};
use crate::store::{mutate, read_list, Tuning};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// How long every task must have been untouched before start-up archiving acts (8.5).
const STARTUP_AGE_SECS: i64 = 43_200;

/// How a task left the list; the value is the RFC 3339 time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Left {
    Archived(String),
    Deleted(String),
}

/// Which member a move writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftKind {
    Archived,
    Deleted,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub task: Task,
    pub how: Left,
}

fn io_err(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// One archive line: the task as it stood plus exactly one of the two time members.
fn line_for(task: &Task, how: LeftKind, now: &str) -> Result<String> {
    let mut value = serde_json::to_value(task).map_err(|e| Error::ListInvalid {
        reason: format!("task {} cannot be written: {e}", task.id),
    })?;
    if let Value::Object(map) = &mut value {
        map.remove("archived_at");
        map.remove("deleted_at");
        let key = match how {
            LeftKind::Archived => "archived_at",
            LeftKind::Deleted => "deleted_at",
        };
        map.insert(key.to_string(), Value::String(now.to_string()));
    }
    serde_json::to_string(&value).map_err(|e| Error::ListInvalid {
        reason: format!("task {} cannot be written: {e}", task.id),
    })
}

/// Whether the file is non-empty and its last byte is not a line feed.
fn ends_in_torn_line(file: &mut std::fs::File) -> io::Result<bool> {
    let len = file.seek(SeekFrom::End(0))?;
    if len == 0 {
        return Ok(false);
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

fn append_lines(path: &Path, lines: &[String]) -> Result<()> {
    // Never follow a symlink at the archive path: the append would write wherever it points.
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::Refused {
            reason: format!(
                "the archive path {} is a symlink; remove it so the archive can be written",
                path.display()
            ),
        });
    }
    let mut o = OpenOptions::new();
    o.read(true).append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut file = o.open(path).map_err(|e| io_err(path, e))?;
    let mut lead = ends_in_torn_line(&mut file).map_err(|e| io_err(path, e))?;
    for line in lines {
        // One write per whole line, so a line is never empty or partial by our doing.
        let mut buf = String::with_capacity(line.len() + 2);
        if lead {
            buf.push('\n');
            lead = false;
        }
        buf.push_str(line);
        buf.push('\n');
        file.write_all(buf.as_bytes())
            .map_err(|e| io_err(path, e))?;
    }
    file.flush().map_err(|e| io_err(path, e))?;
    file.sync_all().map_err(|e| io_err(path, e))
}

/// Step 1 and the in-memory half of step 2 of 8.5, for use inside a `mutate` closure:
/// append the lines of the tasks with these ids and flush, then remove them from
/// `list.tasks` and from every remaining `blocked_by`. An id not in the list is ignored.
/// `next_id` is unchanged.
pub fn move_to_archive(
    store: &Store,
    _tuning: &Tuning,
    list: &mut List,
    ids: &[u64],
    how: LeftKind,
    now: &str,
) -> Result<()> {
    let moving: Vec<&Task> = list.tasks.iter().filter(|t| ids.contains(&t.id)).collect();
    if moving.is_empty() {
        return Ok(());
    }
    let lines = moving
        .iter()
        .map(|t| line_for(t, how, now))
        .collect::<Result<Vec<_>>>()?;
    append_lines(&store.file(FileKind::Archive), &lines)?;
    list.tasks.retain(|t| !ids.contains(&t.id));
    for t in &mut list.tasks {
        t.blocked_by.retain(|b| !ids.contains(b));
    }
    Ok(())
}

/// One line as an entry, or `None` when it is not a complete archive object.
fn parse_line(line: &[u8]) -> Option<Entry> {
    let Value::Object(mut map) = serde_json::from_slice::<Value>(line).ok()? else {
        return None;
    };
    let archived = map.remove("archived_at");
    let deleted = map.remove("deleted_at");
    let how = match (archived, deleted) {
        (Some(Value::String(t)), None) => Left::Archived(t),
        (None, Some(Value::String(t))) => Left::Deleted(t),
        _ => return None,
    };
    let task: Task = serde_json::from_value(Value::Object(Map::from_iter(map))).ok()?;
    if task.id == 0 {
        return None;
    }
    Some(Entry { task, how })
}

/// The archive's readable entries in id order (8.5): a line whose id is in `list` is
/// ignored, the last line wins for any other id, and a line that is not a complete
/// archive object is skipped without an error. No archive file is an empty vec.
pub fn read_archive(store: &Store, list: &List) -> Result<Vec<Entry>> {
    let path = store.file(FileKind::Archive);
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err(&path, e)),
    };
    let mut by_id: BTreeMap<u64, Entry> = BTreeMap::new();
    for line in bytes.split(|b| *b == b'\n') {
        if let Some(entry) = parse_line(line) {
            if !list.tasks.iter().any(|t| t.id == entry.task.id) {
                by_id.insert(entry.task.id, entry);
            }
        }
    }
    Ok(by_id.into_values().collect())
}

pub fn find(store: &Store, list: &List, id: u64) -> Result<Option<Entry>> {
    Ok(read_archive(store, list)?
        .into_iter()
        .find(|e| e.task.id == id))
}

/// The start-up condition of 8.5: at least one task, all completed, every `updated_at`
/// at least 12 hours before `now_secs`. A time that is unreadable or in the future fails it.
pub fn startup_candidate(list: &List, now_secs: i64) -> bool {
    !list.tasks.is_empty()
        && list.tasks.iter().all(|t| {
            t.status == Status::Completed
                && parse_rfc3339(&t.updated_at)
                    .is_some_and(|u| now_secs.saturating_sub(u) >= STARTUP_AGE_SECS)
        })
}

fn move_all(store: &Store, tuning: &Tuning, list: &mut List, now: &str) -> Result<usize> {
    let ids: Vec<u64> = list.tasks.iter().map(|t| t.id).collect();
    move_to_archive(store, tuning, list, &ids, LeftKind::Archived, now)?;
    Ok(ids.len())
}

/// Start-up archiving (8.5). Reads without the lock and returns `Ok(0)` without touching
/// the disk unless the condition holds; then one non-blocking lock attempt, a re-read
/// under the lock, and the move if the condition still holds. A failure is returned for
/// the caller to print to standard error.
pub fn run_startup(store: &Store, tuning: &Tuning, clock: &dyn Clock) -> Result<usize> {
    let now = clock.now_secs();
    if !startup_candidate(&read_list(store, tuning)?, now) {
        return Ok(0);
    }
    let once = Tuning {
        lock_deadline: std::time::Duration::ZERO,
        ..tuning.clone()
    };
    mutate(store, &once, |list| {
        if !startup_candidate(list, now) {
            return Ok(0);
        }
        move_all(store, tuning, list, &rfc3339(now))
    })
}

/// The `archive` subcommand: every completed task, whatever its age, with `archived_at`.
pub fn archive_completed(store: &Store, tuning: &Tuning, clock: &dyn Clock) -> Result<usize> {
    if !read_list(store, tuning)?
        .tasks
        .iter()
        .any(|t| t.status == Status::Completed)
    {
        return Ok(0);
    }
    let now = rfc3339(clock.now_secs());
    mutate(store, tuning, |list| {
        let ids: Vec<u64> = list
            .tasks
            .iter()
            .filter(|t| t.status == Status::Completed)
            .map(|t| t.id)
            .collect();
        move_to_archive(store, tuning, list, &ids, LeftKind::Archived, &now)?;
        Ok(ids.len())
    })
}
