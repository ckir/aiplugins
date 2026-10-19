//! `gc`: the dead-seen grace, the shared sweep and the safety rules (Task 17).

mod common;

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use common::{env_unpaired, SID, SID2};
use opencode_ctl::clock::TestClock;
use opencode_ctl::engine::Engine;
use opencode_ctl::error::ErrorCode;
use opencode_ctl::gc::{gc, GcReport};
use opencode_ctl::host::Host;
use opencode_ctl::liveness::{ProcessProbe, Sentinel};
use opencode_ctl::pairing::{write_record_new, write_sentinel, Record};
use opencode_ctl::settings::Settings;
use opencode_ctl::testing::fake::FakeTransport;
use opencode_ctl::testing::{temp_home, FakeProbe, TempHome};
use opencode_ctl::transport::TransportError;
use serde_json::json;

const NOW: i64 = 1_800_000_000_000;
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;
const A: &str = "aaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbb";

struct Rig {
    home: TempHome,
    probe: Arc<FakeProbe>,
    fake: FakeTransport,
    engine: Engine<FakeTransport, TestClock>,
}

impl Rig {
    fn new() -> Self {
        let home = temp_home();
        let probe = Arc::new(FakeProbe::new("boot"));
        let fake = FakeTransport::new();
        fake.on("session.active", vec![Ok(json!({"data": {}}))]);
        let engine = Engine::new(
            Settings::default(),
            Vec::new(),
            Host::Claude,
            env_unpaired(&home),
            fake.clone(),
            TestClock::new(NOW),
            probe.clone() as Arc<dyn ProcessProbe>,
        );
        Rig {
            home,
            probe,
            fake,
            engine,
        }
    }

    fn path(&self) -> &Path {
        self.home.path()
    }

    fn pf(&self, id: &str, suffix: &str) -> PathBuf {
        self.path().join(format!("pair.{id}.{suffix}"))
    }

    fn active(&self, sessions: &[&str]) {
        let data: serde_json::Map<String, serde_json::Value> = sessions
            .iter()
            .map(|s| (s.to_string(), json!({})))
            .collect();
        self.fake
            .on("session.active", vec![Ok(json!({ "data": data }))]);
    }

    async fn run(&self, now: i64) -> GcReport {
        gc(&self.engine, &*self.probe, now, None).await
    }

    /// A record with a tab and a controller whose sentinels are written for dead pids.
    fn dead_pair(&self, id: &str, session: &str) {
        write_record_new(
            self.path(),
            &rec(id, session, true, Some("claude"), NOW - 10 * DAY),
        )
        .unwrap();
        self.sentinel(id, "claude-pid", 9001, false);
        self.sentinel(id, "g1.tui-pid", 9002, false);
    }

    /// A pair with neither side: no tab, no controller.
    fn none_pair(&self, id: &str, session: &str) {
        write_record_new(self.path(), &rec(id, session, false, None, NOW - 10 * DAY)).unwrap();
    }

    fn sentinel(&self, id: &str, suffix: &str, pid: u32, alive: bool) {
        if alive {
            self.probe.set_alive(pid, "st");
        }
        write_sentinel(
            &self.pf(id, suffix),
            &Sentinel {
                pid,
                boot_id: "boot".into(),
                starttime: "st".into(),
            },
        )
        .unwrap();
    }

    fn dead_seen(&self, id: &str, at: i64) {
        fs::write(self.pf(id, "dead-seen"), at.to_string()).unwrap();
        // the file mtime must never matter: make it look ancient
        set_mtime(&self.pf(id, "dead-seen"), 1_000);
    }
}

fn rec(id: &str, session: &str, tab: bool, controller: Option<&str>, created: i64) -> Record {
    Record {
        pair_id: id.to_string(),
        session_id: session.to_string(),
        directory: PathBuf::from("/work/dir"),
        server_url: None,
        created,
        controller: controller.map(str::to_string),
        generation: 1,
        tab,
    }
}

fn set_mtime(path: &Path, ms: i64) {
    let f = OpenOptions::new().write(true).open(path).expect("open");
    f.set_modified(UNIX_EPOCH + Duration::from_millis(ms as u64))
        .expect("set mtime");
}

fn files_of(rig: &Rig, id: &str) -> Vec<String> {
    let prefix = format!("pair.{id}.");
    let mut v: Vec<String> = fs::read_dir(rig.path())
        .unwrap()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with(&prefix))
        .collect();
    v.sort();
    v
}

fn terr() -> TransportError {
    TransportError {
        code: ErrorCode::ServiceDown,
        message: "down".into(),
        hint: None,
        details: None,
    }
}

#[tokio::test]
async fn gc_keeps_active_session() {
    let rig = Rig::new();
    rig.dead_pair(A, SID);
    rig.dead_seen(A, NOW - 25 * HOUR);
    rig.active(&[SID, SID2]);
    let r = rig.run(NOW).await;
    assert!(r.pairs_deleted.is_empty(), "{r:?}");
    assert!(!r.skipped_service_down);
    assert!(rig.pf(A, "json").exists());
    assert!(rig.pf(A, "dead-seen").exists(), "grace file is kept");
    // the same pair is collected once its session is no longer active
    rig.active(&[SID2]);
    let r = rig.run(NOW).await;
    assert_eq!(r.pairs_deleted, vec![A.to_string()]);
}

#[tokio::test]
async fn gc_skips_when_service_down() {
    let rig = Rig::new();
    rig.fake.on("session.active", vec![Err(terr())]);
    rig.dead_pair(A, SID);
    rig.dead_seen(A, NOW - 25 * HOUR);
    rig.dead_pair(B, SID2); // would get a dead-seen
    let live = "cccccccccccc";
    write_record_new(
        rig.path(),
        &rec(live, SID, true, Some("claude"), NOW - 10 * DAY),
    )
    .unwrap();
    rig.sentinel(live, "claude-pid", 9100, true);
    rig.dead_seen(live, NOW - 2 * DAY); // would be cleared
    let stdout = rig.path().join("stdout").join("7000-600");
    fs::create_dir_all(&stdout).unwrap();
    let r = rig.run(NOW).await;
    assert!(r.skipped_service_down);
    assert!(r.pairs_deleted.is_empty());
    assert!(r.dead_seen_written.is_empty());
    assert!(r.dead_seen_cleared.is_empty());
    assert!(rig.pf(A, "json").exists());
    assert!(!rig.pf(B, "dead-seen").exists());
    assert!(rig.pf(live, "dead-seen").exists());
    // only the sweep ran
    assert!(!stdout.exists());
    assert!(r.swept.contains(&stdout));
}

#[tokio::test]
async fn gc_dead_seen_grace() {
    let rig = Rig::new();
    rig.dead_pair(A, SID);
    // first sight: written, content is the decimal now_ms, nothing deleted
    let r = rig.run(NOW).await;
    assert_eq!(r.dead_seen_written, vec![A.to_string()]);
    assert!(r.pairs_deleted.is_empty());
    assert_eq!(
        fs::read_to_string(rig.pf(A, "dead-seen")).unwrap(),
        NOW.to_string()
    );
    // a second pass does not write again
    let r = rig.run(NOW + HOUR).await;
    assert!(r.dead_seen_written.is_empty(), "{r:?}");
    assert_eq!(
        fs::read_to_string(rig.pf(A, "dead-seen")).unwrap(),
        NOW.to_string()
    );
    // exactly 24 h: kept; 1 ms more: deleted
    let r = rig.run(NOW + DAY).await;
    assert!(r.pairs_deleted.is_empty(), "{r:?}");
    assert!(rig.pf(A, "json").exists());
    let r = rig.run(NOW + DAY + 1).await;
    assert_eq!(r.pairs_deleted, vec![A.to_string()]);
    assert!(files_of(&rig, A).is_empty(), "{:?}", files_of(&rig, A));
}

#[tokio::test]
async fn gc_dead_seen_age_is_its_content_not_its_mtime() {
    let rig = Rig::new();
    rig.dead_pair(A, SID);
    rig.dead_seen(A, NOW); // content fresh, mtime ancient
    let r = rig.run(NOW + HOUR).await;
    assert!(r.pairs_deleted.is_empty(), "{r:?}");
    assert!(rig.pf(A, "json").exists());
}

#[tokio::test]
async fn gc_living_side_clears_dead_seen() {
    let rig = Rig::new();
    rig.dead_pair(A, SID);
    rig.sentinel(A, "claude-pid", 9300, true); // controller alive
    rig.dead_seen(A, NOW - 3 * DAY);
    let r = rig.run(NOW).await;
    assert_eq!(r.dead_seen_cleared, vec![A.to_string()]);
    assert!(r.pairs_deleted.is_empty());
    assert!(!rig.pf(A, "dead-seen").exists());
    // a starting side clears it too (no sentinel, wrapper younger than 30 s)
    rig.dead_pair(B, SID2);
    fs::remove_file(rig.pf(B, "g1.tui-pid")).unwrap();
    fs::write(rig.pf(B, "g1.wrapper.sh"), "#!/bin/sh").unwrap();
    set_mtime(&rig.pf(B, "g1.wrapper.sh"), NOW - 10_000);
    rig.dead_seen(B, NOW - 3 * DAY);
    let r = rig.run(NOW).await;
    assert_eq!(r.dead_seen_cleared, vec![B.to_string()]);
    assert!(!rig.pf(B, "dead-seen").exists());
    // clearing is not reported when there was nothing to clear
    let r = rig.run(NOW).await;
    assert!(r.dead_seen_cleared.is_empty(), "{r:?}");
}

#[tokio::test]
async fn gc_unknown_after_grace() {
    // unparsable controller sentinel: age = its mtime, strictly more than 1 h counts as dead
    let rig = Rig::new();
    write_record_new(
        rig.path(),
        &rec(A, SID, false, Some("claude"), NOW - 10 * DAY),
    )
    .unwrap();
    fs::write(rig.pf(A, "claude-pid"), "garbage").unwrap();
    set_mtime(&rig.pf(A, "claude-pid"), NOW - HOUR);
    let r = rig.run(NOW).await;
    assert!(
        r.dead_seen_written.is_empty(),
        "exactly 1 h is still alive: {r:?}"
    );
    assert!(!rig.pf(A, "dead-seen").exists());
    let r = rig.run(NOW + 1).await;
    assert_eq!(r.dead_seen_written, vec![A.to_string()]);
    // and then the pair is collected after the 24 h grace
    let r = rig.run(NOW + 1 + DAY + 1).await;
    assert_eq!(r.pairs_deleted, vec![A.to_string()]);
}

#[tokio::test]
async fn gc_unknown_without_sentinel_ages_from_created() {
    let rig = Rig::new();
    // no claude-pid, no resume-lock: unknown (record mtime is old); age = record `created`
    write_record_new(rig.path(), &rec(A, SID, false, Some("claude"), NOW - HOUR)).unwrap();
    set_mtime(&rig.pf(A, "json"), NOW - 5 * DAY);
    let r = rig.run(NOW).await;
    assert!(r.dead_seen_written.is_empty(), "{r:?}");
    let r = rig.run(NOW + 1).await;
    assert_eq!(r.dead_seen_written, vec![A.to_string()]);
}

#[tokio::test]
async fn gc_unknown_tui_sentinel_uses_tui_pid_mtime() {
    let rig = Rig::new();
    write_record_new(rig.path(), &rec(A, SID, true, None, NOW - 10 * DAY)).unwrap();
    fs::write(rig.pf(A, "g1.tui-pid"), "").unwrap();
    set_mtime(&rig.pf(A, "g1.tui-pid"), NOW - HOUR - 1);
    let r = rig.run(NOW).await;
    assert_eq!(r.dead_seen_written, vec![A.to_string()]);
}

#[tokio::test]
async fn gc_rechecks_liveness_before_delete() {
    let rig = Rig::new();
    rig.dead_pair(A, SID);
    // the sentinel names pid 9001, dead until the failpoint revives it
    rig.dead_seen(A, NOW - 2 * DAY);
    let calls = AtomicUsize::new(0);
    let probe = rig.probe.clone();
    let revive = || {
        calls.fetch_add(1, Ordering::SeqCst);
        probe.set_alive(9001, "st");
    };
    let r = gc(&rig.engine, &*rig.probe, NOW, Some(&revive)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(r.pairs_deleted.is_empty(), "{r:?}");
    assert!(rig.pf(A, "json").exists());
    assert!(rig.pf(A, "claude-pid").exists());
    assert_eq!(r.dead_seen_cleared, vec![A.to_string()]);
    assert!(!rig.pf(A, "dead-seen").exists());
}

#[tokio::test]
async fn gc_sweeps_old_exe_and_stdout_dirs() {
    let rig = Rig::new();
    rig.probe.set_alive(7001, "700");
    let stdout = rig.path().join("stdout");
    fs::create_dir_all(stdout.join("7000-600")).unwrap();
    fs::create_dir_all(stdout.join("7001-700")).unwrap();
    let old = rig.path().join("created.aaaaaaaaaaaa.json");
    fs::write(&old, "[]").unwrap();
    set_mtime(&old, NOW - 31 * DAY);
    let r = rig.run(NOW).await;
    assert!(r.swept.contains(&stdout.join("7000-600")));
    assert!(r.swept.contains(&old));
    assert!(stdout.join("7001-700").exists());
    assert!(!old.exists());
    // the Windows leftover is swept from the launcher directory by the shared rule
    let dir = rig.path().join("launcher");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("opencode-pair.exe.old"), "x").unwrap();
    let mut report = opencode_ctl::sweep::SweepReport::default();
    opencode_ctl::sweep::sweep_old_launcher(&dir, &mut report);
    assert!(!dir.join("opencode-pair.exe.old").exists());
}

#[tokio::test]
async fn gc_deletes_none_sided_pair() {
    let rig = Rig::new();
    rig.none_pair(A, SID);
    rig.dead_pair(B, SID2);
    rig.sentinel(B, "claude-pid", 9400, true); // B lives
    fs::write(rig.pf(A, "lock"), "").unwrap();
    fs::write(rig.pf(A, "g1.wrapper.sh"), "#!/bin/sh").unwrap();
    rig.dead_seen(A, NOW - 25 * HOUR);
    let before_b = files_of(&rig, B);
    let r = rig.run(NOW).await;
    assert_eq!(r.pairs_deleted, vec![A.to_string()]);
    assert!(files_of(&rig, A).is_empty());
    assert_eq!(files_of(&rig, B), before_b);
}

#[tokio::test]
async fn gc_none_sided_pair_gets_the_grace_first() {
    let rig = Rig::new();
    rig.none_pair(A, SID);
    let r = rig.run(NOW).await;
    assert_eq!(r.dead_seen_written, vec![A.to_string()]);
    assert!(r.pairs_deleted.is_empty());
    assert!(rig.pf(A, "json").exists());
}

#[tokio::test]
async fn gc_log_retention() {
    let rig = Rig::new();
    let logs = rig.path().join("logs");
    fs::create_dir_all(&logs).unwrap();
    // 22 dead-pid serve logs, all older than 14 days: the 2 oldest are beyond the newest 20
    for i in 0..22u32 {
        let p = logs.join(format!("serve-{}.log", 50_000 + i));
        fs::write(&p, "x").unwrap();
        set_mtime(&p, NOW - 20 * DAY - i64::from(i) * 1000);
    }
    let r = rig.run(NOW).await;
    assert!(r.swept.contains(&logs.join("serve-50021.log")));
    assert!(r.swept.contains(&logs.join("serve-50020.log")));
    assert!(logs.join("serve-50019.log").exists());
    assert_eq!(fs::read_dir(&logs).unwrap().count(), 20);
}

#[tokio::test]
async fn gc_orphan_sentinels_24h() {
    let rig = Rig::new();
    let f = |name: &str, age: i64| {
        let p = rig.path().join(name);
        fs::write(&p, "1:boot:1").unwrap();
        set_mtime(&p, NOW - age);
        p
    };
    let old = f(&format!("pair.{A}.claude-pid"), DAY + 1);
    let edge = f(&format!("pair.{A}.g1.tui-pid"), DAY);
    let tmp_old = f(&format!("pair.{A}.json.tmp-1"), DAY + 1);
    let r = rig.run(NOW).await;
    assert!(!old.exists() && r.swept.contains(&old));
    assert!(!tmp_old.exists());
    assert!(edge.exists(), "exactly 24 h is kept");
    // with a record the sentinel is never an orphan
    write_record_new(rig.path(), &rec(B, SID, true, Some("claude"), NOW)).unwrap();
    let guarded = f(&format!("pair.{B}.claude-exited"), 5 * DAY);
    rig.run(NOW).await;
    assert!(guarded.exists());
}

#[tokio::test]
async fn gc_dead_resume_lock_swept() {
    let rig = Rig::new();
    rig.probe.set_alive(8001, "st");
    fs::write(rig.pf(A, "resume-lock"), "8000:boot:st").unwrap();
    fs::write(rig.pf(B, "resume-lock"), "8001:boot:st").unwrap();
    let r = rig.run(NOW).await;
    assert!(!rig.pf(A, "resume-lock").exists());
    assert!(r.swept.contains(&rig.pf(A, "resume-lock")));
    assert!(rig.pf(B, "resume-lock").exists());
}

// ---------------------------------------------------------------- safety

#[tokio::test]
async fn gc_keys_on_the_file_name_not_the_inner_id() {
    let rig = Rig::new();
    // B is a live pair
    write_record_new(
        rig.path(),
        &rec(B, SID2, true, Some("claude"), NOW - 10 * DAY),
    )
    .unwrap();
    rig.sentinel(B, "claude-pid", 9500, true);
    rig.dead_seen(B, NOW - 3 * DAY);
    // A's file claims to be B and is dead
    let mut lie = rec(A, SID, true, Some("claude"), NOW - 10 * DAY);
    write_record_new(rig.path(), &lie).unwrap();
    lie.pair_id = B.to_string();
    fs::write(rig.pf(A, "json"), serde_json::to_string(&lie).unwrap()).unwrap();
    rig.sentinel(A, "claude-pid", 9501, false);
    rig.sentinel(A, "g1.tui-pid", 9502, false);
    rig.dead_seen(A, NOW - 25 * HOUR);
    let before_b = files_of(&rig, B);
    let r = rig.run(NOW).await;
    assert_eq!(r.pairs_deleted, vec![A.to_string()]);
    assert!(files_of(&rig, A).is_empty());
    // B was judged by its own files: alive -> its dead-seen is cleared, nothing else touched
    assert_eq!(r.dead_seen_cleared, vec![B.to_string()]);
    let after_b = files_of(&rig, B);
    assert!(before_b
        .iter()
        .filter(|n| !n.ends_with("dead-seen"))
        .all(|n| after_b.contains(n)));
}

#[tokio::test]
async fn gc_leaves_unparsable_records_alone() {
    let rig = Rig::new();
    fs::write(rig.pf(A, "json"), "{not json").unwrap();
    rig.dead_seen(A, NOW - 3 * DAY);
    let r = rig.run(NOW).await;
    assert!(r.pairs_deleted.is_empty());
    assert!(rig.pf(A, "json").exists());
    assert!(rig.pf(A, "dead-seen").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn gc_never_follows_a_symlinked_record() {
    let rig = Rig::new();
    let elsewhere = temp_home();
    let target = elsewhere.path().join("real.json");
    fs::write(
        &target,
        serde_json::to_string(&rec(A, SID, false, None, NOW - 10 * DAY)).unwrap(),
    )
    .unwrap();
    std::os::unix::fs::symlink(&target, rig.pf(A, "json")).unwrap();
    rig.dead_seen(A, NOW - 3 * DAY);
    let r = rig.run(NOW).await;
    assert!(r.pairs_deleted.is_empty(), "{r:?}");
    assert!(target.exists());
    assert!(rig.pf(A, "dead-seen").exists());
}

#[tokio::test]
async fn gc_unknown_fresh_keeps_the_pair_and_the_grace_untouched() {
    let rig = Rig::new();
    write_record_new(rig.path(), &rec(A, SID, false, Some("claude"), NOW)).unwrap();
    fs::write(rig.pf(A, "claude-pid"), "garbage").unwrap();
    set_mtime(&rig.pf(A, "claude-pid"), NOW - 10 * 60_000);
    rig.dead_seen(A, NOW - 3 * DAY);
    let r = rig.run(NOW).await;
    assert!(r.pairs_deleted.is_empty(), "{r:?}");
    assert!(rig.pf(A, "json").exists());
}
