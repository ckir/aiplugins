// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

use repo_tasks::error::Error;
use repo_tasks::model::{List, Status, Task};
use repo_tasks::resolve::{FileKind, Store};
use repo_tasks::store::{mutate, read_list, Tuning};
use serde_json::Map;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "repo-tasks-store-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn store_in(root: &Path, git: bool) -> Store {
    Store {
        dir: root.join("repo-tasks"),
        list: "default".to_string(),
        common_dir: if git { Some(root.to_path_buf()) } else { None },
        start: root.join("start"),
    }
}

fn quick() -> Tuning {
    Tuning {
        lock_deadline: Duration::from_millis(300),
        rename_retry: Duration::from_millis(300),
        read_retry: Duration::from_millis(100),
        poll: Duration::from_millis(5),
        retry_log: None,
    }
}

fn long() -> Tuning {
    Tuning {
        lock_deadline: Duration::from_secs(60),
        ..Tuning::default()
    }
}

fn add_task(list: &mut List, subject: &str) -> u64 {
    let id = list.next_id;
    list.next_id += 1;
    list.tasks.push(Task {
        id,
        subject: subject.to_string(),
        description: None,
        status: Status::Pending,
        owner: None,
        blocked_by: Vec::new(),
        created_at: "2026-11-18T10:00:00Z".to_string(),
        updated_at: "2026-11-18T10:00:00Z".to_string(),
        updated_by: "test".to_string(),
        extra: Map::new(),
    });
    id
}

#[test]
fn missing_list_reads_empty_and_creates_nothing() {
    let t = Tmp::new();
    let s = store_in(&t.0, true);
    assert_eq!(read_list(&s, &Tuning::default()).unwrap(), List::empty());
    assert!(!s.dir.exists());
    // A folder that exists but holds no list reads empty too.
    fs::create_dir_all(&s.dir).unwrap();
    assert_eq!(read_list(&s, &Tuning::default()).unwrap(), List::empty());
    assert!(!s.file(FileKind::List).exists());
}

#[cfg(unix)]
#[test]
fn first_write_creates_folder_with_modes() {
    use std::os::unix::fs::PermissionsExt;
    let t = Tmp::new();
    let s = store_in(&t.0, true);
    mutate(&s, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap();
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&s.dir), 0o700);
    assert_eq!(mode(&s.file(FileKind::List)), 0o600);
    assert_eq!(mode(&s.file(FileKind::Lock)), 0o600);
    assert!(!s.file(FileKind::Tmp).exists());
}

#[test]
fn f_error_leaves_file_untouched() {
    let t = Tmp::new();
    let s = store_in(&t.0, true);
    mutate(&s, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap();
    let before = fs::read(s.file(FileKind::List)).unwrap();
    let r: Result<(), Error> = mutate(&s, &Tuning::default(), |l| {
        add_task(l, "b");
        Err(Error::ListInvalid {
            reason: "refused".to_string(),
        })
    });
    assert!(r.is_err());
    assert_eq!(fs::read(s.file(FileKind::List)).unwrap(), before);
    assert!(!s.file(FileKind::Tmp).exists());
}

#[test]
fn no_change_writes_nothing() {
    let t = Tmp::new();
    let s = store_in(&t.0, true);
    mutate(&s, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap();
    let path = s.file(FileKind::List);
    let before = fs::read(&path).unwrap();
    let mtime = fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let n = mutate(&s, &Tuning::default(), |l| Ok(l.tasks.len())).unwrap();
    assert_eq!(n, 1);
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), mtime);
    assert!(!s.file(FileKind::Tmp).exists());
}

#[test]
fn leftover_tmp_is_removed_under_lock() {
    let t = Tmp::new();
    let s = store_in(&t.0, true);
    fs::create_dir_all(&s.dir).unwrap();
    fs::write(s.file(FileKind::Tmp), b"half a write").unwrap();
    mutate(&s, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap();
    assert!(!s.file(FileKind::Tmp).exists());
    assert_eq!(read_list(&s, &Tuning::default()).unwrap().tasks.len(), 1);
}

#[test]
fn malformed_list_is_never_overwritten() {
    let t = Tmp::new();
    let s = store_in(&t.0, true);
    fs::create_dir_all(&s.dir).unwrap();
    let path = s.file(FileKind::List);
    fs::write(&path, b"{ this is not json").unwrap();
    let err = mutate(&s, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap_err();
    assert!(err.to_string().contains(path.to_str().unwrap()), "{err}");
    assert_eq!(fs::read(&path).unwrap(), b"{ this is not json");
    let err = read_list(&s, &Tuning::default()).unwrap_err();
    assert!(err.to_string().contains(path.to_str().unwrap()), "{err}");
    // A newer format and a broken rule are named the same way.
    for body in [
        r#"{"format": 99, "next_id": 1, "tasks": []}"#,
        r#"{"format": 1, "next_id": 1, "tasks": [{"id": 0}]}"#,
    ] {
        fs::write(&path, body).unwrap();
        for e in [
            read_list(&s, &Tuning::default()).unwrap_err(),
            mutate(&s, &Tuning::default(), |_| Ok(())).unwrap_err(),
        ] {
            assert!(e.to_string().contains(path.to_str().unwrap()), "{e}");
        }
        assert_eq!(fs::read(&path).unwrap(), body.as_bytes());
    }
}

#[test]
fn two_threads_allocate_distinct_ids() {
    let t = Tmp::new();
    let s = std::sync::Arc::new(store_in(&t.0, true));
    let handles: Vec<_> = (0..50)
        .map(|i| {
            let s = s.clone();
            std::thread::spawn(move || {
                mutate(&s, &long(), |l| Ok(add_task(l, &format!("t{i}")))).unwrap()
            })
        })
        .collect();
    let mut ids: Vec<u64> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    ids.sort_unstable();
    assert_eq!(ids, (1..=50).collect::<Vec<u64>>());
    let list = read_list(&s, &Tuning::default()).unwrap();
    assert_eq!(list.next_id, 51);
    assert_eq!(list.tasks.len(), 50);
}

fn hold(lock: &Path) -> std::process::Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_hold-lock"))
        .arg(lock)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line.trim(), "locked");
    child
}

#[test]
fn second_process_blocks_then_fails_at_deadline() {
    let t = Tmp::new();
    let s = store_in(&t.0, true);
    mutate(&s, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap();
    let before = fs::read(s.file(FileKind::List)).unwrap();
    let mut child = hold(&s.file(FileKind::Lock));
    let started = std::time::Instant::now();
    let err = mutate(&s, &quick(), |l| Ok(add_task(l, "b"))).unwrap_err();
    let waited = started.elapsed();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(err.to_string().contains("lock"), "{err}");
    assert!(waited >= Duration::from_millis(300), "{waited:?}");
    assert_eq!(fs::read(s.file(FileKind::List)).unwrap(), before);
}

#[test]
fn lock_released_when_holder_dies() {
    let t = Tmp::new();
    let s = store_in(&t.0, true);
    let mut child = {
        fs::create_dir_all(&s.dir).unwrap();
        hold(&s.file(FileKind::Lock))
    };
    assert!(mutate(&s, &quick(), |l| Ok(add_task(l, "x"))).is_err());
    child.kill().unwrap();
    child.wait().unwrap();
    mutate(&s, &quick(), |l| Ok(add_task(l, "a"))).unwrap();
    assert_eq!(read_list(&s, &quick()).unwrap().tasks.len(), 1);
}

#[test]
fn first_write_outside_git_creates_directory_txt() {
    let t = Tmp::new();
    let s = store_in(&t.0, false);
    assert_eq!(read_list(&s, &Tuning::default()).unwrap(), List::empty());
    assert!(!s.dir.exists());
    mutate(&s, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap();
    let txt = fs::read_to_string(s.dir.join("directory.txt")).unwrap();
    assert_eq!(txt, s.start.to_str().unwrap());
    // Inside git there is no such file.
    let g = Tmp::new();
    let gs = store_in(&g.0, true);
    mutate(&gs, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap();
    assert!(!gs.dir.join("directory.txt").exists());
}

#[cfg(unix)]
#[test]
fn store_unwritable_names_path() {
    use std::os::unix::fs::PermissionsExt;
    let t = Tmp::new();
    fs::set_permissions(&t.0, fs::Permissions::from_mode(0o500)).unwrap();
    if fs::create_dir(t.0.join("probe")).is_ok() {
        // Running as a user the mode does not bind (root): nothing to measure.
        return;
    }
    let s = store_in(&t.0, true);
    let err = mutate(&s, &Tuning::default(), |l| Ok(add_task(l, "a"))).unwrap_err();
    let text = err.to_string();
    assert!(text.contains(s.dir.to_str().unwrap()), "{text}");
    assert!(text.to_lowercase().contains("permission denied"), "{text}");
    assert_eq!(read_list(&s, &Tuning::default()).unwrap(), List::empty());
}

#[cfg(windows)]
#[test]
fn windows_reader_and_writer_never_fail() {
    let t = Tmp::new();
    let s = std::sync::Arc::new(store_in(&t.0, true));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let log: repo_tasks::store::RetryLog = Default::default();
    let writer = {
        let (s, stop, log) = (s.clone(), stop.clone(), log.clone());
        std::thread::spawn(move || {
            let mut n = 0;
            let tuning = Tuning {
                retry_log: Some(log),
                ..long()
            };
            while !stop.load(Ordering::SeqCst) {
                mutate(&s, &tuning, |l| Ok(add_task(l, "w"))).expect("writer call failed");
                n += 1;
            }
            n
        })
    };
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let (s, stop, log) = (s.clone(), stop.clone(), log.clone());
            std::thread::spawn(move || {
                let mut n = 0;
                let tuning = Tuning {
                    retry_log: Some(log),
                    ..Tuning::default()
                };
                while !stop.load(Ordering::SeqCst) {
                    read_list(&s, &tuning).expect("reader call failed");
                    n += 1;
                }
                n
            })
        })
        .collect();
    std::thread::sleep(Duration::from_secs(3));
    stop.store(true, Ordering::SeqCst);
    let w: u32 = writer.join().unwrap();
    let r: u32 = readers.into_iter().map(|h| h.join().unwrap()).sum();
    println!("writer calls: {w}, reader calls: {r}");
    if let Some(file) = std::env::var_os("REPO_TASKS_CI_NOTES") {
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(file) {
            let _ = writeln!(f, "windows-rw: writer calls {w}, reader calls {r}");
            let mut counts = std::collections::BTreeMap::<String, u32>::new();
            for kind in log.lock().unwrap().iter() {
                *counts.entry(kind.clone()).or_default() += 1;
            }
            for (kind, n) in counts {
                let _ = writeln!(f, "retry-kind: {kind} x{n}");
            }
        }
    }
    assert!(w > 0 && r > 0);
}
