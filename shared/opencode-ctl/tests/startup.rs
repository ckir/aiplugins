//! The `serve` startup: identity application, the startup sweep, the log and the re-bind
//! (Task 16, part A). The brief files these in tests/doctor.rs, which part C owns; the test
//! names are the brief's.

mod common;

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use common::*;
use opencode_ctl::cli::prepare_serve;
use opencode_ctl::engine::EnvSnapshot;
use opencode_ctl::host::Host;
use opencode_ctl::logging::LogScope;
use opencode_ctl::pairing::{
    apply_identity, record_path, resolve_identity, write_record_new, IdentitySource, Record,
};
use opencode_ctl::settings::{Settings, Source};
use opencode_ctl::startup::startup;
use opencode_ctl::testing::{env_map, temp_home, FakeProbe};

const PID: &str = "0123456789ab";
const T0: i64 = 1_800_000_000_000;
const DAY: i64 = 86_400_000;

fn rec(server_url: Option<&str>) -> Record {
    Record {
        pair_id: PID.to_string(),
        session_id: SID.to_string(),
        directory: PathBuf::from("/work/dir"),
        server_url: server_url.map(str::to_string),
        created: T0,
        controller: Some("claude".to_string()),
        generation: 1,
        tab: true,
    }
}

fn env_with(home: &Path, pairs: &[(&str, &str)]) -> EnvSnapshot {
    EnvSnapshot::from_lookup(&env_map(pairs), PathBuf::from("/cwd"), home.to_path_buf())
}

fn user_u() -> Settings {
    let mut s = Settings {
        server_url: Some("http://u".to_string()),
        ..Settings::default()
    };
    s.sources.insert("server_url", Source::UserFile);
    s
}

fn set_mtime(path: &Path, ms: i64) {
    let f = OpenOptions::new().write(true).open(path).expect("open");
    f.set_modified(UNIX_EPOCH + Duration::from_millis(ms as u64))
        .expect("set mtime");
}

fn run_startup(
    home: &Path,
    env: &EnvSnapshot,
    probe: &FakeProbe,
    warnings: &[opencode_ctl::error::ToolError],
) -> opencode_ctl::startup::StartupReport {
    startup(
        home,
        Host::Claude,
        env,
        probe,
        T0,
        false,
        LogScope::Scoped,
        warnings,
    )
}

fn serve_log(home: &Path) -> String {
    fs::read_to_string(
        home.join("logs")
            .join(format!("serve-{}.log", std::process::id())),
    )
    .unwrap_or_default()
}

#[test]
fn apply_identity_pair_only_fills_session() {
    let h = temp_home();
    write_record_new(h.path(), &rec(Some("http://s"))).unwrap();
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, warns) = resolve_identity(&env, h.path());
    assert!(warns.is_empty());
    let mut settings = Settings::default();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(env.session.as_deref(), Some(SID));
    assert_eq!(env.directory, Some(PathBuf::from("/work/dir")));
    assert_eq!(env.server.as_deref(), Some("http://s"));
    assert_eq!(env.pair.as_deref(), Some(PID));
    assert_eq!(settings.server_url.as_deref(), Some("http://s"));
}

#[test]
fn apply_identity_null_record_overrides_user_file() {
    let h = temp_home();
    write_record_new(h.path(), &rec(None)).unwrap();
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, _) = resolve_identity(&env, h.path());
    let mut settings = user_u();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(settings.server_url, None);
    assert_eq!(settings.sources["server_url"], Source::Env);
}

#[test]
fn apply_identity_missing_record_keeps_pair_and_settings() {
    let h = temp_home();
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, warns) = resolve_identity(&env, h.path());
    assert_eq!(warns.len(), 1);
    let mut settings = user_u();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(env.pair.as_deref(), Some(PID));
    assert_eq!(settings.server_url.as_deref(), Some("http://u"));
    assert_eq!(settings.sources["server_url"], Source::UserFile);
}

#[test]
fn apply_identity_env_identity_keeps_user_file_server_url() {
    let h = temp_home();
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_SESSION", SID)]);
    let (id, _) = resolve_identity(&env, h.path());
    assert_eq!(id.source, IdentitySource::Env);
    let mut settings = user_u();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(settings.server_url.as_deref(), Some("http://u"));
    assert_eq!(settings.sources["server_url"], Source::UserFile);
}

#[test]
fn apply_identity_invalid_session_runs_unpaired() {
    let h = temp_home();
    let mut env = env_with(
        h.path(),
        &[
            ("OPENCODE_CTL_SESSION", "nonsense"),
            ("OPENCODE_CTL_PAIR", PID),
            ("OPENCODE_CTL_DIR", "/d"),
            ("OPENCODE_CTL_SERVER", "http://e"),
        ],
    );
    let (id, warns) = resolve_identity(&env, h.path());
    assert_eq!(warns.len(), 1);
    let mut settings = user_u();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(env.session, None);
    assert_eq!(env.directory, None);
    assert_eq!(env.server, None);
    assert_eq!(env.pair, None);
    assert_eq!(settings.server_url.as_deref(), Some("http://u"));
    assert_eq!(settings.sources["server_url"], Source::UserFile);

    // handed to startup, the warning lands in the Scoped log file
    let probe = FakeProbe::new("boot");
    let report = run_startup(h.path(), &env, &probe, &warns);
    assert!(report.log.is_some());
    assert!(report.identity_stderr.is_empty());
    drop(report);
    let text = serve_log(h.path());
    assert!(text.contains("INVALID_ARGUMENT"), "{text}");
    assert!(text.contains("is not a session id"), "{text}");
}

#[test]
fn apply_identity_invalid_session_warning_goes_to_stderr_when_log_unwritable() {
    let h = temp_home();
    fs::write(h.path().join("logs"), b"a regular file").unwrap();
    let env = env_with(h.path(), &[("OPENCODE_CTL_SESSION", "nonsense")]);
    let (_, warns) = resolve_identity(&env, h.path());
    let probe = FakeProbe::new("boot");
    let report = run_startup(h.path(), &env, &probe, &warns);
    assert!(report.log.is_none());
    assert!(report.log_warning.is_some());
    assert_eq!(report.identity_stderr.len(), 1);
    assert!(
        report.identity_stderr[0].contains("is not a session id"),
        "{:?}",
        report.identity_stderr
    );
}

#[test]
fn serve_startup_sweep() {
    let h = temp_home();
    let home = h.path();
    let probe = FakeProbe::new("boot");
    probe.set_alive(7001, "700");
    let stdout = home.join("stdout");
    fs::create_dir_all(stdout.join("7000-600")).unwrap(); // dead pid
    fs::create_dir_all(stdout.join("7001-700")).unwrap(); // live, matching start
    let old = home.join("created.aaaaaaaaaaaa.json");
    let fresh = home.join("created.bbbbbbbbbbbb.json");
    fs::write(&old, "[]").unwrap();
    fs::write(&fresh, "[]").unwrap();
    set_mtime(&old, T0 - 31 * DAY);
    set_mtime(&fresh, T0 - DAY);

    let report = run_startup(home, &env_with(home, &[]), &probe, &[]);
    assert!(report.log.is_some());
    assert!(report.log_warning.is_none());
    assert!(report.rebind.is_none());
    assert!(report.sweep.removed.contains(&stdout.join("7000-600")));
    assert!(report.sweep.removed.contains(&old));
    assert!(!stdout.join("7000-600").exists());
    assert!(!old.exists());
    assert!(stdout.join("7001-700").exists());
    assert!(fresh.exists());
    assert!(report.sweep.errors.is_empty(), "{:?}", report.sweep.errors);
}

#[test]
fn serve_startup_sweep_pair_leftovers() {
    let h = temp_home();
    let home = h.path();
    let probe = FakeProbe::new("boot");
    probe.set_alive(8001, "800");
    let id = "aaaaaaaaaaaa";
    let f = |name: &str, age_days: i64, body: &str| {
        let p = home.join(name);
        fs::write(&p, body).unwrap();
        set_mtime(&p, T0 - age_days * DAY);
        p
    };
    // resume-lock: owner dead -> removed; owner alive -> kept
    let dead_lock = f(&format!("pair.{id}.resume-lock"), 0, "8000:boot:799");
    let live_id = "bbbbbbbbbbbb";
    let live_lock = f(&format!("pair.{live_id}.resume-lock"), 0, "8001:boot:800");
    // sentinels without a record: old removed, young kept; with a record: kept
    let old_sentinel = f(&format!("pair.{id}.claude-pid"), 2, "1:boot:1");
    let young_sentinel = f(&format!("pair.{id}.g1.tui-pid"), 0, "1:boot:1");
    write_record_new(
        home,
        &Record {
            pair_id: live_id.into(),
            ..rec(None)
        },
    )
    .unwrap();
    let guarded = f(&format!("pair.{live_id}.claude-pid"), 5, "8001:boot:800");
    // claim temps older than 24 h
    let old_tmp = f("pair.cccccccccccc.json.tmp-1234", 2, "x");
    let young_tmp = f("pair.cccccccccccc.json.tmp-5678", 0, "x");

    let report = run_startup(home, &env_with(home, &[]), &probe, &[]);
    for gone in [&dead_lock, &old_sentinel, &old_tmp] {
        assert!(!gone.exists(), "{gone:?}");
        assert!(report.sweep.removed.contains(gone), "{gone:?}");
    }
    for kept in [&live_lock, &young_sentinel, &guarded, &young_tmp] {
        assert!(kept.exists(), "{kept:?}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn serve_without_log_dir_still_serves() {
    use std::os::unix::fs::PermissionsExt;
    let h = temp_home();
    let logs = h.path().join("logs");
    fs::create_dir_all(&logs).unwrap();
    fs::set_permissions(&logs, fs::Permissions::from_mode(0o500)).unwrap();
    let writable = fs::File::create(logs.join("probe")).is_ok();
    if writable {
        fs::set_permissions(&logs, fs::Permissions::from_mode(0o700)).unwrap();
        eprintln!("skipped: running as root");
        return;
    }
    let probe = FakeProbe::new("boot");
    let report = run_startup(h.path(), &env_unpaired(&h), &probe, &[]);
    fs::set_permissions(&logs, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(report.log_warning.is_some());
    assert!(report.log.is_none());

    let r = engine(
        opencode_ctl::testing::fake::FakeTransport::new(),
        opencode_ctl::clock::TestClock::new(T0),
        Host::Claude,
        Settings::default(),
        env_unpaired(&h),
    );
    let client = client_for(r).await;
    let tools = client.list_all_tools().await.expect("list_tools");
    assert_eq!(tools.len(), 20);
}

#[test]
fn startup_rebinds_when_pair_set() {
    use opencode_ctl::liveness::RebindOutcome;
    let h = temp_home();
    write_record_new(h.path(), &rec(None)).unwrap();
    let probe = FakeProbe::new("boot");
    let me = std::process::id();
    probe.parents.lock().unwrap().insert(me, 4242);
    probe.set_alive(4242, "77");
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, _) = resolve_identity(&env, h.path());
    let _ = apply_identity(&mut env, &mut Settings::default(), &id);
    let report = run_startup(h.path(), &env, &probe, &[]);
    assert_eq!(report.rebind, Some(RebindOutcome::Rebound));
    let raw = fs::read_to_string(h.path().join(format!("pair.{PID}.claude-pid"))).unwrap();
    assert!(raw.starts_with("4242:boot:77"), "{raw}");
    assert!(record_path(h.path(), PID).exists());
}

#[test]
fn startup_without_pair_does_not_rebind() {
    let h = temp_home();
    let probe = FakeProbe::new("boot");
    let report = run_startup(h.path(), &env_unpaired(&h), &probe, &[]);
    assert!(report.rebind.is_none());
}

#[test]
fn serve_empty_server_env_overrides_user_file() {
    let h = temp_home();
    fs::write(
        h.path().join("settings.local.md"),
        "---\nserver_url: http://u\n---\n",
    )
    .unwrap();
    let cwd = temp_home();
    // the RAW lookup keeps an empty OPENCODE_CTL_SERVER visible to the settings loader
    let get = env_map(&[("OPENCODE_CTL_SERVER", "")]);
    let prep = prepare_serve(Host::Claude, h.path(), cwd.path().to_path_buf(), &get);
    assert_eq!(prep.settings.server_url, None);
    assert_eq!(prep.settings.sources["server_url"], Source::Env);
    assert_eq!(prep.env.server, None);

    // control: unset leaves the user file in force
    let get = env_map(&[]);
    let prep = prepare_serve(Host::Claude, h.path(), cwd.path().to_path_buf(), &get);
    assert_eq!(prep.settings.server_url.as_deref(), Some("http://u"));
}

#[test]
fn prepare_serve_applies_pair_record_before_transport() {
    let h = temp_home();
    write_record_new(h.path(), &rec(Some("http://rec"))).unwrap();
    let cwd = temp_home();
    let get = env_map(&[("OPENCODE_CTL_PAIR", PID)]);
    let prep = prepare_serve(Host::Claude, h.path(), cwd.path().to_path_buf(), &get);
    assert_eq!(prep.env.session.as_deref(), Some(SID));
    assert_eq!(prep.settings.server_url.as_deref(), Some("http://rec"));
    assert!(prep.identity_warnings.is_empty());
}

#[test]
fn pair_is_dead_keys_on_the_file_name_id() {
    use opencode_ctl::sweep::pair_is_dead;
    let h = temp_home();
    let probe = FakeProbe::new("boot");
    // no record: dead
    assert!(pair_is_dead(h.path(), PID, T0, &probe));
    // a record under PID whose CONTENTS name another pair: liveness is read for PID's files
    let mut r = rec(None);
    r.pair_id = "ffffffffffff".to_string();
    fs::write(record_path(h.path(), PID), serde_json::to_vec(&r).unwrap()).unwrap();
    probe.set_alive(55, "5");
    fs::write(h.path().join(format!("pair.{PID}.claude-pid")), "55:boot:5").unwrap();
    fs::write(h.path().join("pair.ffffffffffff.claude-exited"), "1").unwrap();
    assert!(!pair_is_dead(h.path(), PID, T0, &probe));
}

fn userinfo_prep(h: &Path, via_env: bool) -> opencode_ctl::cli::ServePrep {
    let cwd = temp_home();
    if via_env {
        // env.server is the identity's URL: the record supplies the session
        write_record_new(h, &rec(None)).unwrap();
        let get = env_map(&[
            ("OPENCODE_CTL_PAIR", PID),
            ("OPENCODE_CTL_SERVER", "http://user:s3cretpw@host:1"),
        ]);
        prepare_serve(Host::Claude, h, cwd.path().to_path_buf(), &get)
    } else {
        write_record_new(h, &rec(Some("http://user:s3cretpw@host:1"))).unwrap();
        let get = env_map(&[("OPENCODE_CTL_PAIR", PID)]);
        prepare_serve(Host::Claude, h, cwd.path().to_path_buf(), &get)
    }
}

#[test]
fn userinfo_warning_reaches_the_log_not_the_void() {
    for (via_env, origin) in [(false, "pair record"), (true, "OPENCODE_CTL_SERVER")] {
        let h = temp_home();
        let prep = userinfo_prep(h.path(), via_env);
        assert_eq!(
            prep.identity_warnings.len(),
            1,
            "{:?}",
            prep.identity_warnings
        );
        let probe = FakeProbe::new("boot");
        let report = run_startup(h.path(), &prep.env, &probe, &prep.identity_warnings);
        assert!(report.identity_stderr.is_empty());
        drop(report);
        let text = serve_log(h.path());
        assert!(text.contains("carries userinfo and is ignored"), "{text}");
        assert!(text.contains(origin), "{origin}: {text}");
        assert!(!text.contains("s3cretpw"), "{text}");
    }
}

#[test]
fn userinfo_warning_reaches_stderr_when_log_unwritable() {
    let h = temp_home();
    fs::write(h.path().join("logs"), b"a regular file").unwrap();
    let prep = userinfo_prep(h.path(), false);
    let probe = FakeProbe::new("boot");
    let report = run_startup(h.path(), &prep.env, &probe, &prep.identity_warnings);
    assert!(report.log.is_none());
    assert_eq!(report.identity_stderr.len(), 1);
    let line = &report.identity_stderr[0];
    assert!(line.contains("carries userinfo and is ignored"), "{line}");
    assert!(!line.contains("s3cretpw"), "{line}");
}
