// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The four operations and their replies (spec 9.1 to 9.6).

use crate::archive::{self, Left, LeftKind};
use crate::clock::{rfc3339, Clock};
use crate::error::{Error, Result};
use crate::host::Host;
use crate::model::{validate_description, validate_owner, validate_subject, List, Status, Task};
use crate::resolve::{FileKind, Store};
use crate::store::{mutate, read_list, Tuning};
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_CREATE: usize = 50;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CreateArgs {
    pub tasks: Vec<NewTask>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct NewTask {
    pub subject: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub blocked_by: Option<Vec<i64>>,
    pub blocked_by_earlier: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
pub struct GetArgs {
    pub id: i64,
}

fn status_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "enum": ["pending", "in_progress", "completed"] })
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct UpdateArgs {
    pub id: i64,
    #[serde(default)]
    #[schemars(schema_with = "status_schema")]
    pub status: Option<Status>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub add_blocked_by: Option<Vec<i64>>,
    pub remove_blocked_by: Option<Vec<i64>>,
    pub delete: Option<bool>,
}

/// Line 1 of `task_list` (9.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListHeader(pub String);

impl ListHeader {
    pub fn for_store(store: &Store) -> ListHeader {
        let d = store.start.display();
        ListHeader(match &store.common_dir {
            Some(c) => format!(
                "repo-tasks: {} (list {}, from {d})",
                c.display(),
                store.list
            ),
            None => format!(
                "repo-tasks: {d} (list {}, not a git repository)",
                store.list
            ),
        })
    }
}

fn refuse(reason: impl Into<String>) -> Error {
    Error::Refused {
        reason: reason.into(),
    }
}

/// A model-rule failure becomes a plain one-line refusal.
fn plain(e: Error) -> Error {
    match e {
        Error::ListInvalid { reason } => Error::Refused { reason },
        other => other,
    }
}

fn non_empty(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

fn never_issued(id: i64, next_id: u64) -> Error {
    refuse(format!(
        "task #{id} was never issued: ids start at 1 and the next id is {next_id}"
    ))
}

fn blocker_never_issued(id: i64, next_id: u64) -> Error {
    refuse(format!(
        "blocker #{id} was never issued: ids start at 1 and the next id is {next_id}"
    ))
}

fn is_issued(id: i64, next_id: u64) -> bool {
    id >= 1 && (id as u64) < next_id
}

fn satisfied_line(task: u64, blocker: u64) -> String {
    format!("#{task}: blocker #{blocker} has left the list and counts as satisfied")
}

/// 9.6: no cycle, and no task blocks itself.
fn check_acyclic(list: &List) -> Result<()> {
    let edges: BTreeMap<u64, &Vec<u64>> =
        list.tasks.iter().map(|t| (t.id, &t.blocked_by)).collect();
    for t in &list.tasks {
        if t.blocked_by.contains(&t.id) {
            return Err(refuse(format!("task #{} cannot block itself", t.id)));
        }
    }
    // 0 = unseen, 1 = on the path, 2 = done
    let mut mark: BTreeMap<u64, u8> = BTreeMap::new();
    for t in &list.tasks {
        let mut stack: Vec<(u64, usize)> = vec![(t.id, 0)];
        while let Some((id, i)) = stack.pop() {
            if i == 0 {
                match mark.get(&id) {
                    Some(2) => continue,
                    Some(1) => {
                        return Err(refuse(format!(
                            "the change would make tasks block each other in a cycle through #{id}"
                        )))
                    }
                    _ => {}
                }
                mark.insert(id, 1);
            }
            let next = edges.get(&id).and_then(|b| b.get(i)).copied();
            match next {
                Some(b) => {
                    stack.push((id, i + 1));
                    if mark.get(&b) == Some(&1) {
                        return Err(refuse(format!(
                            "the change would make tasks block each other in a cycle through #{b}"
                        )));
                    }
                    if mark.get(&b) != Some(&2) {
                        stack.push((b, 0));
                    }
                }
                None => {
                    mark.insert(id, 2);
                }
            }
        }
    }
    Ok(())
}

/// Validate `ids` as blockers by 9.6 against `list`. Returns the ids to store and
/// the ids accepted as already satisfied.
fn classify(list: &List, ids: &[i64]) -> Result<(BTreeSet<u64>, BTreeSet<u64>)> {
    let (mut stored, mut satisfied) = (BTreeSet::new(), BTreeSet::new());
    for &b in ids {
        if !is_issued(b, list.next_id) {
            return Err(blocker_never_issued(b, list.next_id));
        }
        let b = b as u64;
        if list.tasks.iter().any(|t| t.id == b) {
            stored.insert(b);
        } else {
            satisfied.insert(b);
        }
    }
    Ok((stored, satisfied))
}

/// `task_create` (9.2).
pub fn create(
    store: &Store,
    tuning: &Tuning,
    host: Host,
    clock: &dyn Clock,
    args: CreateArgs,
) -> Result<String> {
    let n = args.tasks.len();
    if n == 0 || n > MAX_CREATE {
        return Err(refuse(format!(
            "tasks holds 1 to {MAX_CREATE} entries, got {n}"
        )));
    }
    // Text and position rules need no list: refuse before touching the disk.
    let mut checked = Vec::with_capacity(n);
    for (i, t) in args.tasks.iter().enumerate() {
        let pos = i + 1;
        let subject = validate_subject(&t.subject).map_err(plain)?;
        let description = match &t.description {
            Some(d) => non_empty(validate_description(d).map_err(plain)?),
            None => None,
        };
        let owner = match &t.owner {
            Some(o) => non_empty(validate_owner(o).map_err(plain)?),
            None => None,
        };
        let mut earlier = Vec::new();
        for &v in t.blocked_by_earlier.as_deref().unwrap_or(&[]) {
            if v < 1 || v as usize >= pos {
                return Err(refuse(format!(
                    "task {pos}: blocked_by_earlier value {v} must be at least 1 and lower than its own position {pos}"
                )));
            }
            earlier.push(v as u64);
        }
        checked.push((subject, description, owner, earlier));
    }
    let now = rfc3339(clock.now_secs());
    mutate(store, tuning, |list| {
        let first = list.next_id;
        let mut satisfied: BTreeSet<(u64, u64)> = BTreeSet::new();
        let mut lines = Vec::with_capacity(n);
        for (i, (subject, description, owner, earlier)) in checked.into_iter().enumerate() {
            let id = first + i as u64;
            let named = args.tasks[i].blocked_by.as_deref().unwrap_or(&[]);
            // `list.next_id` is still `first` here, so only ids issued before the call pass.
            let (mut stored, gone) = classify(list, named).map_err(|e| match e {
                Error::Refused { reason } => refuse(format!("task {}: {reason}", i + 1)),
                other => other,
            })?;
            for g in gone {
                satisfied.insert((id, g));
            }
            stored.extend(earlier.iter().map(|e| first + e - 1));
            lines.push(format!("created #{id} {subject}"));
            list.tasks.push(Task {
                id,
                subject,
                description,
                status: Status::Pending,
                owner,
                blocked_by: stored.into_iter().collect(),
                created_at: now.clone(),
                updated_at: now.clone(),
                updated_by: host.updated_by().to_string(),
                extra: Default::default(),
            });
        }
        list.next_id = first + n as u64;
        check_acyclic(list)?;
        lines.extend(satisfied.into_iter().map(|(t, b)| satisfied_line(t, b)));
        Ok(lines.join("\n"))
    })
}

fn task_block(t: &Task, blocks: Option<&[u64]>, left: Option<&Left>) -> String {
    let ids = |v: &[u64]| {
        v.iter()
            .map(|i| format!("#{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out = String::new();
    match left {
        Some(Left::Archived(at)) => out.push_str(&format!("archived {at}\n")),
        Some(Left::Deleted(at)) => out.push_str(&format!("deleted {at}\n")),
        None => {}
    }
    out.push_str(&format!(
        "id: {}\nsubject: {}\nstatus: {}\n",
        t.id,
        t.subject,
        match t.status {
            Status::Pending => "pending",
            Status::InProgress => "in_progress",
            Status::Completed => "completed",
        }
    ));
    if let Some(o) = &t.owner {
        out.push_str(&format!("owner: {o}\n"));
    }
    if !t.blocked_by.is_empty() {
        out.push_str(&format!("blocked_by: {}\n", ids(&t.blocked_by)));
    }
    if let Some(b) = blocks.filter(|b| !b.is_empty()) {
        out.push_str(&format!("blocks: {}\n", ids(b)));
    }
    out.push_str(&format!(
        "created_at: {}\nupdated_at: {}\nupdated_by: {}",
        t.created_at, t.updated_at, t.updated_by
    ));
    if let Some(d) = &t.description {
        out.push_str(&format!("\n\n{d}"));
    }
    out
}

/// `task_get` (9.3). Reads the list first and the archive second (8.3).
pub fn get(store: &Store, tuning: &Tuning, args: GetArgs) -> Result<String> {
    let list = read_list(store, tuning)?;
    if !is_issued(args.id, list.next_id) {
        return Err(never_issued(args.id, list.next_id));
    }
    let id = args.id as u64;
    if let Some(t) = list.tasks.iter().find(|t| t.id == id) {
        let blocks: Vec<u64> = list
            .tasks
            .iter()
            .filter(|o| o.blocked_by.contains(&id))
            .map(|o| o.id)
            .collect();
        return Ok(task_block(t, Some(&blocks), None));
    }
    match archive::find(store, &list, id)? {
        Some(e) => Ok(task_block(&e.task, None, Some(&e.how))),
        None => Err(refuse(format!(
            "task #{id} has left the list and the archive holds no record of it"
        ))),
    }
}

/// `task_update` (9.4).
pub fn update(
    store: &Store,
    tuning: &Tuning,
    host: Host,
    clock: &dyn Clock,
    args: UpdateArgs,
) -> Result<String> {
    let delete = args.delete == Some(true);
    let others = args.status.is_some()
        || args.subject.is_some()
        || args.description.is_some()
        || args.owner.is_some()
        || args.add_blocked_by.is_some()
        || args.remove_blocked_by.is_some();
    if !others && !delete {
        return Err(refuse(
            "task_update needs at least one member besides id (delete: false counts as absent)",
        ));
    }
    if delete && others {
        return Err(refuse(
            "delete cannot be combined with any other change in one call",
        ));
    }
    let subject = args
        .subject
        .as_deref()
        .map(validate_subject)
        .transpose()
        .map_err(plain)?;
    let description = args
        .description
        .as_deref()
        .map(validate_description)
        .transpose()
        .map_err(plain)?;
    let owner = args
        .owner
        .as_deref()
        .map(validate_owner)
        .transpose()
        .map_err(plain)?;
    let add = args.add_blocked_by.clone().unwrap_or_default();
    let remove = args.remove_blocked_by.clone().unwrap_or_default();
    if let Some(x) = add.iter().find(|a| remove.contains(a)) {
        return Err(refuse(format!(
            "blocker #{x} is named in both add_blocked_by and remove_blocked_by"
        )));
    }
    // Decide existence without `mutate`, which creates the folder and lock file.
    let pre = read_list(store, tuning)?;
    check_target(&pre, args.id)?;
    let id = args.id as u64;
    let now_secs = clock.now_secs();
    mutate(store, tuning, |list| {
        check_target(list, args.id)?;
        let pos = list.tasks.iter().position(|t| t.id == id).unwrap_or(0);
        if delete {
            if list.tasks[pos].status == Status::Completed {
                return Err(refuse(format!(
                    "task #{id} is completed: completed tasks leave the list only by archiving"
                )));
            }
            archive::move_to_archive(
                store,
                tuning,
                list,
                &[id],
                LeftKind::Deleted,
                &rfc3339(now_secs),
            )?;
            return Ok(format!("deleted #{id}"));
        }
        for &r in &remove {
            if !is_issued(r, list.next_id) {
                return Err(blocker_never_issued(r, list.next_id));
            }
        }
        let (stored, gone) = classify(list, &add)?;
        let before = list.tasks[pos].clone();
        let t = &mut list.tasks[pos];
        if let Some(s) = args.status {
            t.status = s;
        }
        if let Some(s) = subject {
            t.subject = s;
        }
        if let Some(d) = description {
            t.description = non_empty(d);
        }
        if let Some(o) = owner {
            t.owner = non_empty(o);
        }
        let mut blockers: BTreeSet<u64> = t.blocked_by.iter().copied().collect();
        blockers.extend(stored);
        for r in &remove {
            blockers.remove(&(*r as u64));
        }
        t.blocked_by = blockers.into_iter().collect();
        if *t != before {
            t.updated_at = rfc3339(now_secs);
            t.updated_by = host.updated_by().to_string();
        }
        check_acyclic(list)?;
        let mut out = vec![format!("updated #{id}")];
        out.extend(gone.into_iter().map(|b| satisfied_line(id, b)));
        Ok(out.join("\n"))
    })
}

/// 9.1: which of the three cases applies when the id is not in the list.
fn check_target(list: &List, id: i64) -> Result<()> {
    if !is_issued(id, list.next_id) {
        return Err(never_issued(id, list.next_id));
    }
    if !list.tasks.iter().any(|t| t.id == id as u64) {
        return Err(refuse(format!("task #{id} has left the list")));
    }
    Ok(())
}

/// `task_list` (9.5).
pub fn list(store: &Store, tuning: &Tuning, header: &ListHeader) -> Result<String> {
    let list = read_list(store, tuning)?;
    let mut lines = vec![header.0.clone()];
    if list.tasks.is_empty() {
        lines.push("no tasks".to_string());
    }
    for t in &list.tasks {
        let mark = match t.status {
            Status::Pending => "[ ]",
            Status::InProgress => "[>]",
            Status::Completed => "[x]",
        };
        let mut line = format!("#{} {mark} {}", t.id, t.subject);
        if let Some(o) = &t.owner {
            line.push_str(&format!(" [owner {o}]"));
        }
        let open: Vec<String> = t
            .blocked_by
            .iter()
            .filter(|b| {
                list.tasks
                    .iter()
                    .any(|o| o.id == **b && o.status != Status::Completed)
            })
            .map(|b| format!("#{b}"))
            .collect();
        if !open.is_empty() {
            line.push_str(&format!(" [blocked by {}]", open.join(", ")));
        }
        if t.status == Status::InProgress {
            line.push_str(&format!(" [{} {}]", t.updated_by, t.updated_at));
        }
        lines.push(line);
    }
    if std::fs::metadata(store.file(FileKind::Archive)).is_ok() {
        lines.push(
            "archived tasks exist: the \"list --archived\" subcommand prints them".to_string(),
        );
    }
    Ok(lines.join("\n"))
}
