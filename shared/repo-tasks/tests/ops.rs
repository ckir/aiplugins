// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

use repo_tasks::archive::{read_archive, Left};
use repo_tasks::clock::{rfc3339, Clock};
use repo_tasks::host::Host;
use repo_tasks::model::{List, Status};
use repo_tasks::ops::{self, CreateArgs, GetArgs, ListHeader, UpdateArgs};
use repo_tasks::resolve::{FileKind, Store};
use repo_tasks::store::{read_list, Tuning};
use serde_json::{json, Value};
use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "repo-tasks-ops-{}-{}",
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
        common_dir: Some(root.join(".git")),
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

struct Clk(Cell<i64>);

impl Clock for Clk {
    fn now_secs(&self) -> i64 {
        self.0.get()
    }
}

const T0: i64 = 1_795_000_000;

fn clk() -> Clk {
    Clk(Cell::new(T0))
}

fn cargs(v: Value) -> CreateArgs {
    serde_json::from_value(v).unwrap()
}

fn uargs(v: Value) -> UpdateArgs {
    serde_json::from_value(v).unwrap()
}

fn create(s: &Store, c: &Clk, v: Value) -> Result<String, String> {
    ops::create(s, &quick(), Host::Claude, c, cargs(v)).map_err(|e| e.to_string())
}

fn update(s: &Store, c: &Clk, v: Value) -> Result<String, String> {
    ops::update(s, &quick(), Host::Qwen, c, uargs(v)).map_err(|e| e.to_string())
}

fn get(s: &Store, id: i64) -> Result<String, String> {
    ops::get(s, &quick(), GetArgs { id }).map_err(|e| e.to_string())
}

fn list_text(s: &Store) -> String {
    ops::list(s, &quick(), &ListHeader::for_store(s)).unwrap()
}

fn subjects(n: usize) -> Value {
    Value::Array(
        (1..=n)
            .map(|i| json!({"subject": format!("t{i}")}))
            .collect(),
    )
}

fn one_line(e: &str) {
    assert!(!e.is_empty() && !e.contains('\n'), "{e:?}");
}

#[test]
fn create_five_with_earlier() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let reply = create(
        &s,
        &clk(),
        json!({"tasks": [
            {"subject": "Reproduce"},
            {"subject": "Fix", "blocked_by_earlier": [1]},
            {"subject": "Test", "blocked_by_earlier": [2]},
            {"subject": "Docs", "blocked_by_earlier": [2, 3]},
            {"subject": "Ship", "blocked_by_earlier": [1, 4]},
        ]}),
    )
    .unwrap();
    assert_eq!(
        reply,
        "created #1 Reproduce\ncreated #2 Fix\ncreated #3 Test\ncreated #4 Docs\ncreated #5 Ship"
    );
    let l = read_list(&s, &quick()).unwrap();
    assert_eq!(l.next_id, 6);
    assert_eq!(l.tasks[3].blocked_by, vec![2, 3]);
    assert_eq!(l.tasks[4].blocked_by, vec![1, 4]);
    assert!(l.tasks.iter().all(|t| t.status == Status::Pending));
    assert_eq!(l.tasks[0].updated_by, "claude-code");
    assert_eq!(l.tasks[0].created_at, rfc3339(T0));
}

#[test]
fn create_limits() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let e = create(&s, &clk(), json!({"tasks": []})).unwrap_err();
    one_line(&e);
    let e = create(&s, &clk(), json!({"tasks": subjects(51)})).unwrap_err();
    one_line(&e);
    assert!(read_list(&s, &quick()).unwrap().tasks.is_empty());
    let reply = create(&s, &clk(), json!({"tasks": subjects(50)})).unwrap();
    assert_eq!(reply.lines().count(), 50);
    // blocked_by_earlier must be lower than the entry's own position.
    for bad in [json!([2]), json!([3]), json!([0]), json!([-1])] {
        let e = create(
            &s,
            &clk(),
            json!({"tasks": [{"subject": "a"}, {"subject": "b", "blocked_by_earlier": bad}]}),
        )
        .unwrap_err();
        one_line(&e);
    }
    let e = create(
        &s,
        &clk(),
        json!({"tasks": [{"subject": "a", "blocked_by_earlier": [1]}]}),
    )
    .unwrap_err();
    one_line(&e);
    assert_eq!(read_list(&s, &quick()).unwrap().tasks.len(), 50);
}

#[test]
fn create_blocker_left_the_list_prints_satisfied_line() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(&s, &c, json!({"tasks": subjects(3)})).unwrap();
    assert_eq!(
        update(&s, &c, json!({"id": 1, "delete": true})).unwrap(),
        "deleted #1"
    );
    assert_eq!(
        update(&s, &c, json!({"id": 2, "delete": true})).unwrap(),
        "deleted #2"
    );
    let reply = create(
        &s,
        &c,
        json!({"tasks": [
            {"subject": "a", "blocked_by": [2, 3, 1]},
            {"subject": "b", "blocked_by": [1]},
        ]}),
    )
    .unwrap();
    assert_eq!(
        reply,
        "created #4 a\ncreated #5 b\n\
         #4: blocker #1 has left the list and counts as satisfied\n\
         #4: blocker #2 has left the list and counts as satisfied\n\
         #5: blocker #1 has left the list and counts as satisfied"
    );
    let l = read_list(&s, &quick()).unwrap();
    assert_eq!(
        l.tasks.iter().find(|t| t.id == 4).unwrap().blocked_by,
        vec![3]
    );
    assert!(l
        .tasks
        .iter()
        .find(|t| t.id == 5)
        .unwrap()
        .blocked_by
        .is_empty());
    // never issued: the call fails
    for bad in [0, -3, 6, 99] {
        let e = create(
            &s,
            &c,
            json!({"tasks": [{"subject": "x", "blocked_by": [bad]}]}),
        )
        .unwrap_err();
        one_line(&e);
    }
}

#[test]
fn get_template() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(
        &s,
        &c,
        json!({"tasks": [
            {"subject": "one"}, {"subject": "two"},
            {"subject": "Add a regression test", "description": "The description, exactly as stored.\nSecond line.",
             "owner": "ck", "blocked_by_earlier": [2]},
            {"subject": "four"},
            {"subject": "five", "blocked_by_earlier": [3]},
        ]}),
    )
    .unwrap();
    update(&s, &c, json!({"id": 3, "add_blocked_by": [4]})).unwrap();
    let want = format!(
        "id: 3\nsubject: Add a regression test\nstatus: pending\nowner: ck\nblocked_by: #2, #4\nblocks: #5\n\
         created_at: {t}\nupdated_at: {t}\nupdated_by: qwen\n\nThe description, exactly as stored.\nSecond line.",
        t = rfc3339(T0)
    );
    assert_eq!(get(&s, 3).unwrap(), want);
    // empty members are left out, and the blank line and description with them
    let want = format!(
        "id: 1\nsubject: one\nstatus: pending\ncreated_at: {t}\nupdated_at: {t}\nupdated_by: claude-code",
        t = rfc3339(T0)
    );
    assert_eq!(get(&s, 1).unwrap(), want);
}

#[test]
fn get_from_archive() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(
        &s,
        &c,
        json!({"tasks": [{"subject": "one"}, {"subject": "two", "blocked_by_earlier": [1], "description": "why"}]}),
    )
    .unwrap();
    c.0.set(T0 + 60);
    update(&s, &c, json!({"id": 2, "delete": true})).unwrap();
    // task 1 is still in the list and blocked nothing now; task 2 is archived
    let want = format!(
        "deleted {d}\nid: 2\nsubject: two\nstatus: pending\nblocked_by: #1\ncreated_at: {t}\n\
         updated_at: {t}\nupdated_by: claude-code\n\nwhy",
        d = rfc3339(T0 + 60),
        t = rfc3339(T0)
    );
    assert_eq!(get(&s, 2).unwrap(), want);
    let entries = read_archive(&s, &read_list(&s, &quick()).unwrap()).unwrap();
    assert_eq!(entries[0].how, Left::Deleted(rfc3339(T0 + 60)));
}

#[test]
fn get_from_archive_when_not_in_list() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(&s, &c, json!({"tasks": subjects(3)})).unwrap();
    // Archive 3 by hand-built line, then drop it from the list through a delete of another path:
    let mut l = read_list(&s, &quick()).unwrap();
    let t3 = l.tasks.pop().unwrap();
    let line = {
        let mut v = serde_json::to_value(&t3).unwrap();
        v["archived_at"] = json!(rfc3339(T0 + 5));
        v.to_string()
    };
    fs::write(s.file(FileKind::Archive), format!("{line}\n")).unwrap();
    fs::write(s.file(FileKind::List), l.to_bytes()).unwrap();
    let want = format!(
        "archived {a}\nid: 3\nsubject: t3\nstatus: pending\ncreated_at: {t}\nupdated_at: {t}\nupdated_by: claude-code",
        t = rfc3339(T0),
        a = rfc3339(T0 + 5)
    );
    assert_eq!(get(&s, 3).unwrap(), want);
}

#[test]
fn get_errors() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    // no list at all: never issued
    let e = get(&s, 1).unwrap_err();
    one_line(&e);
    assert!(e.contains("never issued"), "{e}");
    create(&s, &c, json!({"tasks": subjects(2)})).unwrap();
    for bad in [0, -1, 3, 100] {
        let e = get(&s, bad).unwrap_err();
        one_line(&e);
        assert!(e.contains("never issued"), "{e}");
    }
    // left the list, archive holds no record
    let mut l = read_list(&s, &quick()).unwrap();
    l.tasks.pop();
    fs::write(s.file(FileKind::List), l.to_bytes()).unwrap();
    let e = get(&s, 2).unwrap_err();
    one_line(&e);
    assert!(e.contains("archive holds no record"), "{e}");
}

#[test]
fn json_string_id_is_rejected_in_one_line() {
    for text in [r#"{"id":"3"}"#, r#"{"id":3.5}"#] {
        let e = serde_json::from_str::<GetArgs>(text)
            .unwrap_err()
            .to_string();
        one_line(&e);
        let e = serde_json::from_str::<UpdateArgs>(text)
            .unwrap_err()
            .to_string();
        one_line(&e);
    }
}

#[test]
fn update_no_op_writes_nothing() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(
        &s,
        &c,
        json!({"tasks": [{"subject": "a"}, {"subject": "b", "blocked_by_earlier": [1]}]}),
    )
    .unwrap();
    let before = fs::read(s.file(FileKind::List)).unwrap();
    c.0.set(T0 + 500);
    assert_eq!(
        update(&s, &c, json!({"id": 1, "status": "pending"})).unwrap(),
        "updated #1"
    );
    assert_eq!(
        update(&s, &c, json!({"id": 2, "add_blocked_by": [1]})).unwrap(),
        "updated #2"
    );
    assert_eq!(
        update(&s, &c, json!({"id": 2, "remove_blocked_by": [2]})).unwrap(),
        "updated #2"
    );
    assert_eq!(
        update(&s, &c, json!({"id": 1, "subject": "  a "})).unwrap(),
        "updated #1"
    );
    assert_eq!(
        update(&s, &c, json!({"id": 1, "owner": ""})).unwrap(),
        "updated #1"
    );
    assert_eq!(
        update(
            &s,
            &c,
            json!({"id": 1, "delete": false, "status": "pending"})
        )
        .unwrap(),
        "updated #1"
    );
    assert_eq!(fs::read(s.file(FileKind::List)).unwrap(), before);
}

#[test]
fn update_requires_a_member_besides_id() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(&s, &c, json!({"tasks": subjects(1)})).unwrap();
    for v in [json!({"id": 1}), json!({"id": 1, "delete": false})] {
        let e = update(&s, &c, v).unwrap_err();
        one_line(&e);
    }
}

#[test]
fn update_sets_updated_by_and_time() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(&s, &c, json!({"tasks": subjects(2)})).unwrap();
    c.0.set(T0 + 90);
    let r = update(
        &s,
        &c,
        json!({"id": 2, "status": "in_progress", "subject": " new ", "owner": "ck", "description": "d"}),
    )
    .unwrap();
    assert_eq!(r, "updated #2");
    let l = read_list(&s, &quick()).unwrap();
    let t = &l.tasks[1];
    assert_eq!(t.status, Status::InProgress);
    assert_eq!(t.subject, "new");
    assert_eq!(t.owner.as_deref(), Some("ck"));
    assert_eq!(t.updated_at, rfc3339(T0 + 90));
    assert_eq!(t.created_at, rfc3339(T0));
    assert_eq!(t.updated_by, "qwen");
    assert_eq!(l.tasks[0].updated_by, "claude-code");
    // empty owner and description remove the member
    update(&s, &c, json!({"id": 2, "owner": "", "description": ""})).unwrap();
    let l = read_list(&s, &quick()).unwrap();
    assert_eq!(l.tasks[1].owner, None);
    assert_eq!(l.tasks[1].description, None);
    // any status may follow any status, even with an open blocker
    update(&s, &c, json!({"id": 1, "status": "completed"})).unwrap();
    update(&s, &c, json!({"id": 1, "status": "pending"})).unwrap();
}

#[test]
fn update_id_errors() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(&s, &c, json!({"tasks": subjects(2)})).unwrap();
    for bad in [0, -1, 3] {
        let e = update(&s, &c, json!({"id": bad, "status": "completed"})).unwrap_err();
        one_line(&e);
        assert!(e.contains("never issued"), "{e}");
    }
    update(&s, &c, json!({"id": 2, "delete": true})).unwrap();
    let e = update(&s, &c, json!({"id": 2, "status": "completed"})).unwrap_err();
    one_line(&e);
    assert!(e.contains("has left the list"), "{e}");
}

#[test]
fn update_on_missing_list_creates_nothing() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    let e = update(&s, &c, json!({"id": 1, "status": "completed"})).unwrap_err();
    one_line(&e);
    assert!(e.contains("never issued"), "{e}");
    let e = get(&s, 1).unwrap_err();
    one_line(&e);
    let e = update(&s, &c, json!({"id": 1, "delete": true})).unwrap_err();
    one_line(&e);
    assert!(!s.dir.exists(), "the store folder must not be created");
    let t = list_text(&s);
    assert!(t.ends_with("\nno tasks"), "{t}");
    assert!(!s.dir.exists());
}

#[test]
fn update_remove_blocked_by_rules() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(
        &s,
        &c,
        json!({"tasks": [{"subject": "a"}, {"subject": "b"}, {"subject": "c", "blocked_by_earlier": [1, 2]}]}),
    )
    .unwrap();
    for bad in [0, -2, 4, 50] {
        let e = update(&s, &c, json!({"id": 3, "remove_blocked_by": [bad]})).unwrap_err();
        one_line(&e);
    }
    // same id in add and remove fails, even when nothing would change
    let e = update(
        &s,
        &c,
        json!({"id": 3, "add_blocked_by": [1], "remove_blocked_by": [1]}),
    )
    .unwrap_err();
    one_line(&e);
    // an issued id that is not a stored blocker is ignored (3 is the task itself, 1 is stored)
    assert_eq!(
        update(&s, &c, json!({"id": 1, "remove_blocked_by": [2, 3]})).unwrap(),
        "updated #1"
    );
    assert_eq!(
        update(&s, &c, json!({"id": 3, "remove_blocked_by": [1]})).unwrap(),
        "updated #3"
    );
    assert_eq!(
        read_list(&s, &quick()).unwrap().tasks[2].blocked_by,
        vec![2]
    );
    // add: never issued fails
    let e = update(&s, &c, json!({"id": 3, "add_blocked_by": [9]})).unwrap_err();
    one_line(&e);
    assert_eq!(
        read_list(&s, &quick()).unwrap().tasks[2].blocked_by,
        vec![2]
    );
}

#[test]
fn update_add_blocker_that_left_prints_satisfied_line() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(&s, &c, json!({"tasks": subjects(3)})).unwrap();
    update(&s, &c, json!({"id": 1, "delete": true})).unwrap();
    let r = update(
        &s,
        &c,
        json!({"id": 3, "add_blocked_by": [1, 2], "status": "in_progress"}),
    )
    .unwrap();
    assert_eq!(
        r,
        "updated #3\n#3: blocker #1 has left the list and counts as satisfied"
    );
    assert_eq!(
        read_list(&s, &quick()).unwrap().tasks[1].blocked_by,
        vec![2]
    );
}

#[test]
fn delete_rules() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(
        &s,
        &c,
        json!({"tasks": [{"subject": "a"}, {"subject": "b", "blocked_by_earlier": [1]}, {"subject": "c", "blocked_by_earlier": [1, 2]}]}),
    )
    .unwrap();
    c.0.set(T0 + 10);
    // combined with another member: fails
    for v in [
        json!({"id": 1, "delete": true, "status": "completed"}),
        json!({"id": 1, "delete": true, "subject": "x"}),
        json!({"id": 1, "delete": true, "add_blocked_by": [2]}),
    ] {
        one_line(&update(&s, &c, v).unwrap_err());
    }
    assert_eq!(read_list(&s, &quick()).unwrap().tasks.len(), 3);
    assert!(!s.file(FileKind::Archive).exists());
    // open task moves, with deleted_at; blockers lose it
    assert_eq!(
        update(&s, &c, json!({"id": 1, "delete": true})).unwrap(),
        "deleted #1"
    );
    let l = read_list(&s, &quick()).unwrap();
    assert_eq!(l.tasks.len(), 2);
    assert_eq!(l.tasks[0].blocked_by, Vec::<u64>::new());
    assert_eq!(l.tasks[1].blocked_by, vec![2]);
    let text = fs::read_to_string(s.file(FileKind::Archive)).unwrap();
    let v: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(v["id"], 1);
    assert_eq!(v["deleted_at"], rfc3339(T0 + 10));
    assert!(v.get("archived_at").is_none());
    // completed fails
    update(&s, &c, json!({"id": 2, "status": "completed"})).unwrap();
    let e = update(&s, &c, json!({"id": 2, "delete": true})).unwrap_err();
    one_line(&e);
    assert_eq!(read_list(&s, &quick()).unwrap().tasks.len(), 2);
    // in_progress is open
    update(&s, &c, json!({"id": 3, "status": "in_progress"})).unwrap();
    assert_eq!(
        update(&s, &c, json!({"id": 3, "delete": true})).unwrap(),
        "deleted #3"
    );
}

#[cfg(unix)]
#[test]
fn delete_archive_write_failure_keeps_the_task() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(&s, &c, json!({"tasks": subjects(2)})).unwrap();
    // a directory where the archive file belongs makes the append fail
    fs::create_dir(s.file(FileKind::Archive)).unwrap();
    let before = fs::read(s.file(FileKind::List)).unwrap();
    let e = update(&s, &c, json!({"id": 1, "delete": true})).unwrap_err();
    one_line(&e);
    assert_eq!(fs::read(s.file(FileKind::List)).unwrap(), before);
}

#[test]
fn cycle_and_self_block_rejected() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(
        &s,
        &c,
        json!({"tasks": [{"subject": "a"}, {"subject": "b", "blocked_by_earlier": [1]}, {"subject": "c", "blocked_by_earlier": [2]}]}),
    )
    .unwrap();
    let before = fs::read(s.file(FileKind::List)).unwrap();
    for v in [
        json!({"id": 2, "add_blocked_by": [2]}),
        json!({"id": 1, "add_blocked_by": [2]}),
        json!({"id": 1, "add_blocked_by": [3]}),
    ] {
        one_line(&update(&s, &c, v).unwrap_err());
    }
    assert_eq!(fs::read(s.file(FileKind::List)).unwrap(), before);
}

#[test]
fn list_format() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    let header = format!(
        "repo-tasks: {} (list default, from {})",
        tmp.0.join(".git").display(),
        tmp.0.join("start").display()
    );
    assert_eq!(list_text(&s), format!("{header}\nno tasks"));
    create(
        &s,
        &c,
        json!({"tasks": [
            {"subject": "Reproduce the failing upload", "owner": "ck"},
            {"subject": "Fix the chunk size"},
            {"subject": "Add a regression test", "blocked_by_earlier": [1, 2]},
            {"subject": "Both", "owner": "x", "blocked_by_earlier": [2]},
        ]}),
    )
    .unwrap();
    update(&s, &c, json!({"id": 1, "status": "completed"})).unwrap();
    c.0.set(T0 + 7);
    update(&s, &c, json!({"id": 2, "status": "in_progress"})).unwrap();
    update(&s, &c, json!({"id": 4, "status": "in_progress"})).unwrap();
    let t = rfc3339(T0 + 7);
    assert_eq!(
        list_text(&s),
        format!(
            "{header}\n\
             #1 [x] Reproduce the failing upload [owner ck]\n\
             #2 [>] Fix the chunk size [qwen {t}]\n\
             #3 [ ] Add a regression test [blocked by #2]\n\
             #4 [>] Both [owner x] [blocked by #2] [qwen {t}]"
        )
    );
    // an archive file adds the line
    fs::write(s.file(FileKind::Archive), b"").unwrap();
    assert!(list_text(&s)
        .ends_with("\narchived tasks exist: the \"list --archived\" subcommand prints them"));
}

#[test]
fn list_outside_git_and_empty_with_archive() {
    let tmp = Tmp::new();
    let mut s = store_in(&tmp.0);
    s.common_dir = None;
    fs::create_dir_all(&s.dir).unwrap();
    fs::write(s.file(FileKind::Archive), b"").unwrap();
    assert_eq!(
        list_text(&s),
        format!(
            "repo-tasks: {} (list default, not a git repository)\nno tasks\n\
             archived tasks exist: the \"list --archived\" subcommand prints them",
            tmp.0.join("start").display()
        )
    );
}

#[test]
fn ids_never_reused() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    create(&s, &c, json!({"tasks": subjects(3)})).unwrap();
    update(&s, &c, json!({"id": 3, "delete": true})).unwrap();
    let r = create(&s, &c, json!({"tasks": [{"subject": "again"}]})).unwrap();
    assert_eq!(r, "created #4 again");
    let l: List = read_list(&s, &quick()).unwrap();
    assert_eq!(l.next_id, 5);
}

#[test]
fn create_validates_text() {
    let tmp = Tmp::new();
    let s = store_in(&tmp.0);
    let c = clk();
    for v in [
        json!({"tasks": [{"subject": "  "}]}),
        json!({"tasks": [{"subject": "a\nb"}]}),
        json!({"tasks": [{"subject": "a", "owner": "x\u{1b}"}]}),
        json!({"tasks": [{"subject": "a", "description": "x\u{1b}"}]}),
    ] {
        let e = create(&s, &c, v).unwrap_err();
        one_line(&e);
    }
    assert!(read_list(&s, &quick()).unwrap().tasks.is_empty());
}
