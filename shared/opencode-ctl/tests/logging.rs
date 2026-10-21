//! File logging: scopes, size cap and retention (Task 16, part A).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use opencode_ctl::logging::{init_file_logging, retain, CappedWriter, LogScope};
use opencode_ctl::testing::{temp_home, FakeProbe};

const T0: i64 = 1_800_000_000_000;
const DAY: i64 = 86_400_000;

fn set_mtime(path: &Path, ms: i64) {
    let f = OpenOptions::new().write(true).open(path).expect("open");
    f.set_modified(UNIX_EPOCH + Duration::from_millis(ms as u64))
        .expect("set mtime");
}

fn make(dir: &Path, name: &str, age_days: i64) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, b"x").unwrap();
    set_mtime(&p, T0 - age_days * DAY);
    p
}

#[tokio::test]
async fn scoped_log_receives_spawned_task_event() {
    let h = temp_home();
    let path = h.path().join("logs").join("serve-1.log");
    let handle = init_file_logging(&path, false, LogScope::Scoped).expect("open log");
    tokio::spawn(async {
        tracing::info!("hello-from-spawned-task");
    })
    .await
    .unwrap();
    drop(handle);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("hello-from-spawned-task"), "{text}");
    assert!(!text.contains('\u{1b}'), "no ANSI escapes: {text:?}");
}

#[test]
fn scoped_log_level_info_drops_debug_and_debug_flag_keeps_it() {
    let h = temp_home();
    for (debug, name) in [(false, "a.log"), (true, "b.log")] {
        let path = h.path().join("logs").join(name);
        let handle = init_file_logging(&path, debug, LogScope::Scoped).unwrap();
        tracing::debug!("dbg-line");
        tracing::info!("info-line");
        drop(handle);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("info-line"), "{text}");
        assert_eq!(text.contains("dbg-line"), debug, "{text}");
    }
}

#[test]
fn scoped_handle_drop_restores_no_subscriber() {
    // after the handle drops, an event reaches no file (no leaked default subscriber)
    let h = temp_home();
    let path = h.path().join("logs").join("c.log");
    let handle = init_file_logging(&path, false, LogScope::Scoped).unwrap();
    drop(handle);
    tracing::warn!("after-drop");
    assert!(!fs::read_to_string(&path).unwrap().contains("after-drop"));
}

#[cfg(unix)]
#[test]
fn log_file_is_private_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let h = temp_home();
    let path = h.path().join("logs").join("serve-2.log");
    drop(init_file_logging(&path, false, LogScope::Scoped).unwrap());
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn log_not_creatable_is_an_error() {
    let h = temp_home();
    fs::write(h.path().join("logs"), b"a file").unwrap();
    let r = init_file_logging(
        &h.path().join("logs").join("serve-3.log"),
        false,
        LogScope::Scoped,
    );
    assert!(r.is_err());
}

#[test]
fn log_size_cap_rolls_once() {
    let h = temp_home();
    let path = h.path().join("w.log");
    let mut w = CappedWriter::open(&path, 100).unwrap();
    for _ in 0..5 {
        w.write_all(&[b'a'; 60]).unwrap();
    }
    w.flush().unwrap();
    let rolled = h.path().join("w.log.1");
    assert_eq!(fs::metadata(&path).unwrap().len(), 60);
    assert_eq!(fs::metadata(&rolled).unwrap().len(), 60);
    assert!(!h.path().join("w.log.2").exists());

    // through init_file_logging: a file already at the 5 MiB cap rolls on the next event
    let big = h.path().join("logs").join("serve-9.log");
    fs::create_dir_all(big.parent().unwrap()).unwrap();
    fs::write(&big, vec![b'z'; 5 * 1024 * 1024]).unwrap();
    let handle = init_file_logging(&big, false, LogScope::Scoped).unwrap();
    tracing::info!("after-roll");
    drop(handle);
    let rolled = big.with_file_name("serve-9.log.1");
    assert_eq!(fs::metadata(&rolled).unwrap().len(), 5 * 1024 * 1024);
    assert!(fs::read_to_string(&big).unwrap().contains("after-roll"));
}

#[test]
fn log_retention_and_not_open() {
    let h = temp_home();
    let dir = h.path();
    let probe = FakeProbe::new("b");
    // 25 serve logs: 3 young, 22 older than 14 days (ages 18..=39 days), ranked newest first by index
    let mut paths = Vec::new();
    for i in 0..25i64 {
        let age = if i < 3 { 1 + i } else { 15 + i };
        paths.push(make(dir, &format!("serve-{}.log", 1000 + i), age));
    }
    // the oldest one belongs to a LIVE pid: beyond 20 and old, but not dead, so kept
    probe.set_alive(1024, "s");
    // .1 siblings: index 21 (follows a deleted base), 5 (kept base), 24 (live base)
    let sib21 = make(dir, "serve-1021.log.1", 36);
    let sib5 = make(dir, "serve-1005.log.1", 20);
    let sib24 = make(dir, "serve-1024.log.1", 39);
    // files of other shapes are left alone
    let other = make(dir, "notes.txt", 100);
    let odd = make(dir, "serve-abc.log", 100);

    let mut removed = retain(dir, T0, &probe, &|_| false);
    removed.sort();
    let mut want: Vec<PathBuf> = (20..24).map(|i| paths[i].clone()).collect();
    want.push(sib21.clone());
    want.sort();
    assert_eq!(removed, want);
    for kept in [
        &paths[24], &sib5, &sib24, &other, &odd, &paths[0], &paths[19],
    ] {
        assert!(kept.exists(), "{kept:?}");
    }
    for gone in &want {
        assert!(!gone.exists(), "{gone:?}");
    }
}

#[test]
fn log_retention_counts_kinds_separately() {
    let h = temp_home();
    let dir = h.path();
    let probe = FakeProbe::new("b");
    // 21 old pair logs (one beyond 20) and 5 old serve logs (all within 20 of THEIR kind)
    let mut pairs = Vec::new();
    for i in 0..21i64 {
        pairs.push(make(dir, &format!("pair-{i:012x}.log"), 15 + i));
    }
    for i in 0..5i64 {
        make(dir, &format!("serve-{}.log", 2000 + i), 40 + i);
    }
    // the beyond-20 pair log is kept while its pair still has a live side
    let last_id = format!("{:012x}", 20);
    let kept = retain(dir, T0, &probe, &|id| id != last_id);
    assert!(kept.is_empty(), "{kept:?}");
    let removed = retain(dir, T0, &probe, &|_| true);
    assert_eq!(removed, vec![pairs[20].clone()]);
}

#[test]
fn log_retention_keeps_young_files_beyond_twenty() {
    let h = temp_home();
    let probe = FakeProbe::new("b");
    for i in 0..23i64 {
        make(h.path(), &format!("serve-{}.log", 3000 + i), 1);
    }
    assert!(retain(h.path(), T0, &probe, &|_| true).is_empty());
}
