// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

use repo_tasks::archive::{
    archive_completed, find, move_to_archive, read_archive, run_startup, startup_candidate, Left,
    LeftKind,
};
use repo_tasks::clock::{rfc3339, Clock};
use repo_tasks::error::Error;
use repo_tasks::model::{List, Status, Task};
use repo_tasks::resolve::{FileKind, Store};
use repo_tasks::store::{mutate, read_list, Tuning};
use serde_json::Map;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "repo-tasks-archive-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn store_in(root: &Path) -> Store {
    Store {
        dir: root.join("repo-tasks"),
        list: "default".to_string(),
        common_dir: Some(root.to_path_buf()),
        start: root.join("start"),
    }
}

fn quick() -> Tuning {
    Tuning {
        lock_deadline: Duration::from_millis(300),
        rename_retry: Duration::from_millis(300),
        read_retry: Duration::from_millis(100),
        poll: Duration::from_millis(5),
    }
}

struct Fixed(i64);

impl Clock for Fixed {
    fn now_secs(&self) -> i64 {
        self.0
    }
}

const NOW: i64 = 1_795_000_000;
const HOUR: i64 = 3600;

fn task(id: u64, status: Status, updated: i64, blocked_by: &[u64]) -> Task {
    Task {
        id,
        subject: format!("task {id}"),
        description: None,
        status,
        owner: None,
        blocked_by: blocked_by.to_vec(),
        created_at: rfc3339(updated),
        updated_at: rfc3339(updated),
        updated_by: "test".to_string(),
        extra: Map::new(),
    }
}

fn list_of(tasks: Vec<Task>) -> List {
    let next_id = tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    List {
        format: 1,
        next_id,
        tasks,
        extra: Map::new(),
    }
}

fn seed(store: &Store, list: &List) {
    mutate(store, &quick(), |l| {
        *l = list.clone();
        Ok(())
    })
    .unwrap();
}

fn archive_text(store: &Store) -> String {
    fs::read_to_string(store.file(FileKind::Archive)).unwrap()
}

#[test]
fn append_after_torn_line_starts_new_line() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    fs::create_dir_all(&store.dir).unwrap();
    fs::write(store.file(FileKind::Archive), b"{\"id\":9,\"subj").unwrap();
    let mut list = list_of(vec![task(1, Status::Completed, NOW, &[])]);
    move_to_archive(
        &store,
        &quick(),
        &mut list,
        &[1],
        LeftKind::Archived,
        "2026-11-18T12:00:00Z",
    )
    .unwrap();
    let text = archive_text(&store);
    assert!(text.starts_with("{\"id\":9,\"subj\n{"), "{text:?}");
    assert!(text.ends_with("}\n"));
    let entries = read_archive(&store, &List::empty()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].task.id, 1);
    assert_eq!(
        entries[0].how,
        Left::Archived("2026-11-18T12:00:00Z".to_string())
    );
    assert!(list.tasks.is_empty());
}

#[test]
fn no_leading_line_feed_when_file_ends_in_one_or_is_empty() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    let mut list = list_of(vec![
        task(1, Status::Completed, NOW, &[]),
        task(2, Status::Completed, NOW, &[]),
    ]);
    fs::create_dir_all(&store.dir).unwrap();
    move_to_archive(&store, &quick(), &mut list, &[1], LeftKind::Archived, "t").unwrap();
    move_to_archive(&store, &quick(), &mut list, &[2], LeftKind::Deleted, "t").unwrap();
    let text = archive_text(&store);
    assert_eq!(text.lines().count(), 2, "{text:?}");
    assert!(!text.starts_with('\n'));
    assert!(!text.contains("\n\n"));
    assert!(text.lines().nth(1).unwrap().contains("\"deleted_at\""));
}

#[test]
fn reader_rules() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    fs::create_dir_all(&store.dir).unwrap();
    // Build lines with the real writer, then damage the file by hand.
    let mut all = list_of(vec![
        task(1, Status::Completed, NOW, &[]),
        task(2, Status::Completed, NOW, &[]),
        task(3, Status::Completed, NOW, &[]),
    ]);
    move_to_archive(
        &store,
        &quick(),
        &mut all,
        &[1, 2],
        LeftKind::Archived,
        "A1",
    )
    .unwrap();
    let mut text = archive_text(&store);
    text.push_str("{\"id\":7,\"subj\n"); // torn line in the middle
    text.push_str("not json at all\n");
    text.push('\n');
    text.push_str("[1,2]\n"); // complete JSON, not an object
    fs::write(store.file(FileKind::Archive), &text).unwrap();
    move_to_archive(&store, &quick(), &mut all, &[3], LeftKind::Deleted, "D3").unwrap();
    // Task 2 again, later, as deleted: last line wins.
    let mut again = list_of(vec![task(2, Status::Pending, NOW, &[])]);
    move_to_archive(&store, &quick(), &mut again, &[2], LeftKind::Deleted, "D2").unwrap();

    // The list holds 1: the list wins, so 1 is ignored.
    let in_list = list_of(vec![task(1, Status::Pending, NOW, &[])]);
    let entries = read_archive(&store, &in_list).unwrap();
    let ids: Vec<u64> = entries.iter().map(|e| e.task.id).collect();
    assert_eq!(ids, vec![2, 3]);
    assert_eq!(entries[0].how, Left::Deleted("D2".to_string()));
    assert_eq!(entries[1].how, Left::Deleted("D3".to_string()));

    let found = find(&store, &List::empty(), 1).unwrap().unwrap();
    assert_eq!(found.how, Left::Archived("A1".to_string()));
    assert!(find(&store, &in_list, 1).unwrap().is_none());
    assert!(find(&store, &List::empty(), 99).unwrap().is_none());
}

#[test]
fn missing_archive_is_empty() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    assert!(read_archive(&store, &List::empty()).unwrap().is_empty());
    assert!(!store.dir.exists());
}

#[test]
fn crash_between_steps_leaves_task_in_both() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    let list = list_of(vec![
        task(1, Status::Completed, NOW, &[]),
        task(2, Status::Completed, NOW, &[]),
    ]);
    seed(&store, &list);
    // Step 1 happens, step 2 never does: the moved copy is dropped unsaved.
    let mut scratch = list.clone();
    move_to_archive(
        &store,
        &quick(),
        &mut scratch,
        &[1],
        LeftKind::Archived,
        "first",
    )
    .unwrap();
    drop(scratch);
    let on_disk = read_list(&store, &quick()).unwrap();
    assert_eq!(on_disk, list);
    assert!(read_archive(&store, &on_disk).unwrap().is_empty());
    // A later complete move: one line counts.
    mutate(&store, &quick(), |l| {
        move_to_archive(&store, &quick(), l, &[1], LeftKind::Archived, "second")
    })
    .unwrap();
    let after = read_list(&store, &quick()).unwrap();
    assert_eq!(after.tasks.len(), 1);
    let entries = read_archive(&store, &after).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].how, Left::Archived("second".to_string()));
    assert_eq!(archive_text(&store).lines().count(), 2);
}

#[test]
fn startup_condition_table() {
    let old = NOW - 13 * HOUR;
    let done = |u: i64| task(1, Status::Completed, u, &[]);
    assert!(!startup_candidate(&List::empty(), NOW));
    assert!(!startup_candidate(
        &list_of(vec![task(1, Status::Pending, old, &[])]),
        NOW
    ));
    assert!(!startup_candidate(
        &list_of(vec![task(1, Status::InProgress, old, &[])]),
        NOW
    ));
    // 11 h 59 min ago.
    assert!(!startup_candidate(
        &list_of(vec![done(NOW - 11 * HOUR - 59 * 60)]),
        NOW
    ));
    // One old, one fresh.
    let mixed = list_of(vec![done(old), {
        let mut t = done(NOW - HOUR);
        t.id = 2;
        t
    }]);
    assert!(!startup_candidate(&mixed, NOW));
    // One completed, one pending old.
    let mixed2 = list_of(vec![done(old), task(2, Status::Pending, old, &[])]);
    assert!(!startup_candidate(&mixed2, NOW));
    // Exactly 12 h and older: true.
    assert!(startup_candidate(
        &list_of(vec![done(NOW - 12 * HOUR)]),
        NOW
    ));
    assert!(startup_candidate(&list_of(vec![done(old)]), NOW));
    // The future.
    assert!(!startup_candidate(&list_of(vec![done(NOW + HOUR)]), NOW));
}

#[test]
fn startup_reads_before_locking() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    let n = run_startup(&store, &quick(), &Fixed(NOW)).unwrap();
    assert_eq!(n, 0);
    assert!(!store.dir.exists(), "no folder may be created");
    assert!(!store.file(FileKind::Lock).exists());
}

#[test]
fn startup_leaves_a_list_that_is_not_a_candidate_untouched() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    seed(
        &store,
        &list_of(vec![task(1, Status::Completed, NOW - HOUR, &[])]),
    );
    let before = fs::read(store.file(FileKind::List)).unwrap();
    assert_eq!(run_startup(&store, &quick(), &Fixed(NOW)).unwrap(), 0);
    assert_eq!(fs::read(store.file(FileKind::List)).unwrap(), before);
    assert!(!store.file(FileKind::Archive).exists());
}

#[test]
fn startup_archives_old_completed_list() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    seed(
        &store,
        &list_of(vec![
            task(1, Status::Completed, NOW - 13 * HOUR, &[]),
            task(2, Status::Completed, NOW - 14 * HOUR, &[1]),
        ]),
    );
    assert_eq!(run_startup(&store, &quick(), &Fixed(NOW)).unwrap(), 2);
    let after = read_list(&store, &quick()).unwrap();
    assert!(after.tasks.is_empty());
    assert_eq!(after.next_id, 3);
    let entries = read_archive(&store, &after).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].how, Left::Archived(rfc3339(NOW)));
}

#[test]
fn startup_one_attempt_when_locked() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    seed(
        &store,
        &list_of(vec![task(1, Status::Completed, NOW - 13 * HOUR, &[])]),
    );
    let before = fs::read(store.file(FileKind::List)).unwrap();
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(store.file(FileKind::Lock))
        .unwrap();
    lock.lock().unwrap();
    // A generous default tuning: the call must still not wait for it.
    let tuning = Tuning {
        lock_deadline: Duration::from_secs(5),
        ..quick()
    };
    let started = Instant::now();
    let r = std::thread::scope(|s| {
        s.spawn(|| run_startup(&store, &tuning, &Fixed(NOW)))
            .join()
            .unwrap()
    });
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(matches!(r, Err(Error::LockTimeout { .. })), "{r:?}");
    assert_eq!(fs::read(store.file(FileKind::List)).unwrap(), before);
    assert!(!store.file(FileKind::Archive).exists());
    drop(lock);
    // Free again: the same call now acts.
    assert_eq!(run_startup(&store, &tuning, &Fixed(NOW)).unwrap(), 1);
}

#[test]
fn deleted_ids_leave_other_blockers() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    seed(
        &store,
        &list_of(vec![
            task(1, Status::Pending, NOW, &[]),
            task(2, Status::Pending, NOW, &[1]),
            task(3, Status::Pending, NOW, &[1, 2]),
            task(4, Status::Pending, NOW, &[2]),
        ]),
    );
    mutate(&store, &quick(), |l| {
        move_to_archive(&store, &quick(), l, &[2], LeftKind::Deleted, "D")
    })
    .unwrap();
    let after = read_list(&store, &quick()).unwrap();
    assert_eq!(after.next_id, 5);
    let ids: Vec<u64> = after.tasks.iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![1, 3, 4]);
    assert!(after.tasks.iter().all(|t| !t.blocked_by.contains(&2)));
    assert_eq!(after.tasks[1].blocked_by, vec![1]);
    // The archived copy keeps the task exactly as it stood.
    let e = find(&store, &after, 2).unwrap().unwrap();
    assert_eq!(e.task.blocked_by, vec![1]);
}

#[test]
fn archive_completed_takes_every_completed_task_whatever_its_age() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    seed(
        &store,
        &list_of(vec![
            task(1, Status::Completed, NOW, &[]),
            task(2, Status::Pending, NOW, &[1]),
            task(3, Status::Completed, NOW, &[]),
        ]),
    );
    assert_eq!(archive_completed(&store, &quick(), &Fixed(NOW)).unwrap(), 2);
    let after = read_list(&store, &quick()).unwrap();
    assert_eq!(after.tasks.len(), 1);
    assert_eq!(after.tasks[0].blocked_by, Vec::<u64>::new());
    assert_eq!(archive_completed(&store, &quick(), &Fixed(NOW)).unwrap(), 0);
}

#[test]
fn archive_completed_without_a_list_creates_nothing() {
    let tmp = Tmp::new();
    let store = store_in(&tmp.0);
    assert_eq!(archive_completed(&store, &quick(), &Fixed(NOW)).unwrap(), 0);
    assert!(!store.dir.exists());
}
