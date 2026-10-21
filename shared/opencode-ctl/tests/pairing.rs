//! Pairing identity, records, sentinels, liveness, re-bind on spawn and `pair_status` (Task 15).

mod common;

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Barrier};
use std::time::{Duration, UNIX_EPOCH};

use common::*;
use opencode_ctl::clock::TestClock;
use opencode_ctl::engine::EnvSnapshot;
use opencode_ctl::error::ErrorCode;
use opencode_ctl::host::Host;
use opencode_ctl::liveness::{
    boot_matches_windows, parse_boot_output, rebind_on_spawn, sentinel_alive, side_liveness,
    Liveness, RebindOutcome, Sentinel, Side,
};
use opencode_ctl::pairing::{
    apply_identity, apply_record_server, claim_file, delete_pair_files, list_records, read_record,
    read_sentinel, record_path, resolve_identity, rewrite_record, write_record_new, write_sentinel,
    IdentitySource, Record,
};
use opencode_ctl::settings::{Settings, Source};
use opencode_ctl::testing::fake::FakeTransport;
use opencode_ctl::testing::{env_map, temp_home, FakeProbe};
use opencode_ctl::transport::WaitOutcome;
use serde_json::{json, Value};

const PID: &str = "0123456789ab";
const T0: i64 = 1_800_000_000_000;

fn rec(gen: u32, tab: bool, controller: Option<&str>) -> Record {
    Record {
        pair_id: PID.to_string(),
        session_id: SID.to_string(),
        directory: PathBuf::from("/work/dir"),
        server_url: None,
        created: T0,
        controller: controller.map(str::to_string),
        generation: gen,
        tab,
    }
}

fn put(home: &Path, r: &Record) {
    write_record_new(home, r).expect("write record");
}

fn pf(home: &Path, suffix: &str) -> PathBuf {
    home.join(format!("pair.{PID}.{suffix}"))
}

fn set_mtime(path: &Path, ms: i64) {
    let f = OpenOptions::new().write(true).open(path).expect("open");
    f.set_modified(UNIX_EPOCH + Duration::from_millis(ms as u64))
        .expect("set mtime");
}

fn touch(path: &Path, content: &str) {
    fs::write(path, content).expect("write");
}

fn sent(pid: u32, boot: &str, start: &str) -> Sentinel {
    Sentinel {
        pid,
        boot_id: boot.to_string(),
        starttime: start.to_string(),
    }
}

fn write_sent(path: &Path, s: &Sentinel) {
    write_sentinel(path, s).expect("write sentinel");
}

// ---------------------------------------------------------------- sentinels

#[test]
fn sentinel_parse_trims_line_endings() {
    let want = sent(4242, "7f3a-boot", "99887");
    for raw in [
        "4242:7f3a-boot:99887",
        "4242:7f3a-boot:99887\n",
        "4242:7f3a-boot:99887\r\n",
        "\u{feff}4242:7f3a-boot:99887",
        "\u{feff}4242:7f3a-boot:99887\r\n",
        "  4242:7f3a-boot:99887  \r\n",
    ] {
        assert_eq!(Sentinel::parse(raw), Some(want.clone()), "{raw:?}");
    }
    assert_eq!(want.to_string(), "4242:7f3a-boot:99887");
    for bad in ["", "  ", "abc:b:s", "1:b", "1::2", ":b:2"] {
        assert_eq!(Sentinel::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn sentinel_round_trip() {
    let home = temp_home();
    let p = home.path().join("sub").join("x.tui-pid");
    let s = sent(77, "boot-a", "12345");
    write_sent(&p, &s);
    assert_eq!(read_sentinel(&p), Some(s));
    touch(&p, "");
    assert_eq!(read_sentinel(&p), None, "empty reads as None");
    touch(&p, "garbage");
    assert_eq!(read_sentinel(&p), None, "unparsable reads as None");
    assert_eq!(read_sentinel(&home.path().join("absent")), None);
}

#[cfg(unix)]
#[test]
fn modes_records_sentinels_wrappers() {
    use std::os::unix::fs::PermissionsExt;
    let home = temp_home();
    let root = home.path().join("ctl");
    put(&root, &rec(1, true, Some("claude")));
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&root), 0o700);
    assert_eq!(mode(&record_path(&root, PID)), 0o600);
    write_sent(&root.join("s"), &sent(1, "b", "2"));
    assert_eq!(mode(&root.join("s")), 0o600);
    assert!(claim_file(&root.join("w.sh"), b"#!/bin/sh\n", 0o700).unwrap());
    assert_eq!(mode(&root.join("w.sh")), 0o700);
}

// ---------------------------------------------------------------- liveness probes

#[test]
fn liveness_boot_id_and_starttime() {
    let probe = FakeProbe::new("boot-1");
    probe.set_alive(100, "555");
    assert!(sentinel_alive(&sent(100, "boot-1", "555"), &probe));
    assert!(
        !sentinel_alive(&sent(100, "boot-1", "556"), &probe),
        "start time differs"
    );
    assert!(
        !sentinel_alive(&sent(100, "boot-0", "555"), &probe),
        "another boot"
    );
    assert!(
        !sentinel_alive(&sent(101, "boot-1", "555"), &probe),
        "no such pid"
    );
    probe.set_dead(100);
    assert!(!sentinel_alive(&sent(100, "boot-1", "555"), &probe));
}

#[test]
fn liveness_windows_boot_tolerance() {
    assert!(boot_matches_windows("1700000000", "1700000000"));
    assert!(boot_matches_windows("1700000000", "1700000060"));
    assert!(boot_matches_windows("1700000060", "1700000000"));
    assert!(boot_matches_windows("1700000000", "1700000120"));
    assert!(!boot_matches_windows("1700000000", "1700000121"));
    assert!(!boot_matches_windows("1700000121", "1700000000"));
    assert!(!boot_matches_windows("", "1700000000"));
    assert!(!boot_matches_windows("abc", "1700000000"));
    // the POSIX rule is exact
    #[cfg(not(windows))]
    {
        assert!(opencode_ctl::liveness::boot_matches("a-b", "a-b"));
        assert!(!opencode_ctl::liveness::boot_matches(
            "1700000000",
            "1700000060"
        ));
    }
}

#[test]
fn boot_output_failed_or_empty_is_not_a_value() {
    // Task 3 carry-over: an empty or failed PowerShell result must never be cached.
    assert_eq!(parse_boot_output(""), None);
    assert_eq!(parse_boot_output("  \r\n"), None);
    assert_eq!(parse_boot_output("Get-CimInstance : error"), None);
    assert_eq!(
        parse_boot_output("1700000040\r\n"),
        Some("1700000040".to_string())
    );
}

// ---------------------------------------------------------------- side liveness

const NOW: i64 = T0 + 1_000_000;

fn alive_pid(probe: &FakeProbe, home: &Path, suffix: &str, pid: u32) {
    probe.set_alive(pid, "st");
    write_sent(&pf(home, suffix), &sent(pid, "boot", "st"));
}

#[test]
fn liveness_values_per_side() {
    let probe = FakeProbe::new("boot");
    let tui = |home: &Path, r: &Record| side_liveness(Side::Tui, r, home, &probe, NOW);
    let ctl = |home: &Path, r: &Record| side_liveness(Side::Controller, r, home, &probe, NOW);

    // TUI side
    let h = temp_home();
    let r = rec(1, false, Some("claude"));
    put(h.path(), &r);
    assert_eq!(tui(h.path(), &r), Liveness::None, "tab false");

    let h = temp_home();
    let r = rec(1, true, Some("claude"));
    put(h.path(), &r);
    touch(&pf(h.path(), "g1.wrapper.sh"), "#!/bin/sh");
    set_mtime(&pf(h.path(), "g1.wrapper.sh"), NOW - 5_000);
    assert_eq!(tui(h.path(), &r), Liveness::Starting);
    set_mtime(&pf(h.path(), "g1.wrapper.sh"), NOW - 31_000);
    assert_eq!(
        tui(h.path(), &r),
        Liveness::Unknown,
        "no sentinel after 30 s"
    );
    touch(&pf(h.path(), "g1.tui-pid"), "not a sentinel");
    assert_eq!(tui(h.path(), &r), Liveness::Unknown, "unparsable");
    alive_pid(&probe, h.path(), "g1.tui-pid", 300);
    assert_eq!(tui(h.path(), &r), Liveness::Alive);
    probe.set_dead(300);
    assert_eq!(tui(h.path(), &r), Liveness::Dead, "pid check fails");
    alive_pid(&probe, h.path(), "g1.tui-pid", 300);
    touch(&pf(h.path(), "g1.tui-exited"), "123");
    assert_eq!(tui(h.path(), &r), Liveness::Dead, "exited sentinel");

    // controller side
    let h = temp_home();
    let r = rec(1, true, None);
    put(h.path(), &r);
    assert_eq!(ctl(h.path(), &r), Liveness::None, "attach record");

    let h = temp_home();
    let r = rec(1, true, Some("claude"));
    put(h.path(), &r);
    set_mtime(&record_path(h.path(), PID), NOW - 5_000);
    assert_eq!(ctl(h.path(), &r), Liveness::Starting, "record mtime");
    set_mtime(&record_path(h.path(), PID), NOW - 31_000);
    assert_eq!(ctl(h.path(), &r), Liveness::Unknown);
    touch(&pf(h.path(), "claude-pid"), "???");
    assert_eq!(ctl(h.path(), &r), Liveness::Unknown, "unparsable");
    alive_pid(&probe, h.path(), "claude-pid", 400);
    assert_eq!(ctl(h.path(), &r), Liveness::Alive);
    probe.set_dead(400);
    assert_eq!(ctl(h.path(), &r), Liveness::Dead);
    alive_pid(&probe, h.path(), "claude-pid", 400);
    touch(&pf(h.path(), "claude-exited"), "123");
    assert_eq!(ctl(h.path(), &r), Liveness::Dead, "exited sentinel");
}

#[test]
fn liveness_reads_record_generation_then_fallback() {
    let probe = FakeProbe::new("boot");
    let h = temp_home();
    let r = rec(3, true, Some("claude"));
    put(h.path(), &r);
    // g1 live, g2 live, g3 (the record's) has no tui-pid yet and no wrapper clock either
    alive_pid(&probe, h.path(), "g1.tui-pid", 11);
    alive_pid(&probe, h.path(), "g2.tui-pid", 12);
    probe.set_dead(11);
    // fallback: the highest generation that HAS a sentinel is g2 (alive)
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Alive
    );
    probe.set_dead(12);
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Dead,
        "g2 is the one read, not g1"
    );
    // once the record's generation has its sentinel, it is the one read
    alive_pid(&probe, h.path(), "g3.tui-pid", 13);
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Alive
    );
    // a record at generation 2 ignores a g3 left over
    let r2 = Record { generation: 2, ..r };
    assert_eq!(
        side_liveness(Side::Tui, &r2, h.path(), &probe, NOW),
        Liveness::Dead
    );
}

#[test]
fn liveness_reopened_tab_is_starting_over_a_dead_older_generation() {
    let probe = FakeProbe::new("boot");
    let h = temp_home();
    let r = rec(2, true, Some("claude"));
    put(h.path(), &r);
    // g1's tab ran and exited; `attach` claimed g2 and wrote its wrapper, whose tab has not started yet
    alive_pid(&probe, h.path(), "g1.tui-pid", 11);
    touch(&pf(h.path(), "g1.tui-exited"), "1");
    let w = pf(h.path(), "g2.wrapper.sh");
    touch(&w, "x");
    set_mtime(&w, NOW - 1_000);
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Starting,
        "a dead older tab does not hide the re-opened one"
    );
    // the same with g1 dead by its pid check rather than by its exited file
    std::fs::remove_file(pf(h.path(), "g1.tui-exited")).unwrap();
    probe.set_dead(11);
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Starting
    );
    // a live older tab still wins over the fresh wrapper
    probe.set_alive(11, "st");
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Alive
    );
    // once the wrapper is 30 s old with no sentinel, the older generation's reading is back
    probe.set_dead(11);
    set_mtime(&w, NOW - 30_000);
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Dead
    );
}

#[test]
fn liveness_starting_from_wrapper_mtime() {
    let probe = FakeProbe::new("boot");
    let h = temp_home();
    let r = rec(2, true, Some("claude"));
    put(h.path(), &r);
    // an old g1 sentinel must not hide that g2's fresh wrapper is starting? it does by the spec
    // fallback; so test the wrapper clock on a pair without any sentinel
    let w = pf(h.path(), "g2.wrapper.ps1");
    touch(&w, "x");
    set_mtime(&w, NOW - 29_999);
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Starting,
        ".ps1 wrapper counts"
    );
    set_mtime(&w, NOW - 30_000);
    assert_eq!(
        side_liveness(Side::Tui, &r, h.path(), &probe, NOW),
        Liveness::Unknown,
        "30 s is no longer young"
    );
    // controller: the resume-lock mtime is the clock when present
    set_mtime(&record_path(h.path(), PID), NOW - 500_000);
    let lock = pf(h.path(), "resume-lock");
    touch(&lock, "1:b:s");
    set_mtime(&lock, NOW - 2_000);
    assert_eq!(
        side_liveness(Side::Controller, &r, h.path(), &probe, NOW),
        Liveness::Starting
    );
}

// ---------------------------------------------------------------- identity

fn env_with(home: &Path, pairs: &[(&str, &str)]) -> EnvSnapshot {
    EnvSnapshot::from_lookup(&env_map(pairs), PathBuf::from("/cwd"), home.to_path_buf())
}

#[test]
fn pairing_env_then_file_then_none() {
    let h = temp_home();
    put(h.path(), &rec(1, true, Some("claude")));

    // env first: the record is not consulted for a session
    let env = env_with(
        h.path(),
        &[
            ("OPENCODE_CTL_SESSION", SID2),
            ("OPENCODE_CTL_DIR", "/env/dir"),
            ("OPENCODE_CTL_SERVER", "http://127.0.0.1:1"),
            ("OPENCODE_CTL_PAIR", PID),
        ],
    );
    let (id, warns) = resolve_identity(&env, h.path());
    assert!(warns.is_empty());
    assert_eq!(id.source, IdentitySource::Env);
    assert_eq!(id.session_id.as_deref(), Some(SID2));
    assert_eq!(id.directory, Some(PathBuf::from("/env/dir")));
    assert_eq!(id.server_url.as_deref(), Some("http://127.0.0.1:1"));
    assert_eq!(id.pair_id.as_deref(), Some(PID));

    // only the pair id: the record supplies the rest
    let env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, warns) = resolve_identity(&env, h.path());
    assert!(warns.is_empty());
    assert_eq!(id.source, IdentitySource::File);
    assert_eq!(id.session_id.as_deref(), Some(SID));
    assert_eq!(id.directory, Some(PathBuf::from("/work/dir")));
    assert_eq!(id.server_url, None);
    assert_eq!(id.pair_id.as_deref(), Some(PID));

    // nothing set
    let env = env_with(h.path(), &[]);
    let (id, warns) = resolve_identity(&env, h.path());
    assert!(warns.is_empty());
    assert_eq!(id.source, IdentitySource::None);
    assert_eq!(id.session_id, None);
    assert_eq!(id.pair_id, None);
}

#[test]
fn pairing_ids_validated() {
    let h = temp_home();
    for (k, v) in [
        ("OPENCODE_CTL_PAIR", "XYZ"),
        ("OPENCODE_CTL_PAIR", "0123456789AB"),
        ("OPENCODE_CTL_PAIR", "0123456789abc"),
        ("OPENCODE_CTL_SESSION", "ses_short"),
        ("OPENCODE_CTL_SESSION", "garbage"),
    ] {
        let env = env_with(h.path(), &[(k, v), ("OPENCODE_CTL_DIR", "/d")]);
        let (id, warns) = resolve_identity(&env, h.path());
        assert_eq!(warns.len(), 1, "{k}={v}");
        assert_eq!(warns[0].code, ErrorCode::InvalidArgument, "{k}={v}");
        assert_eq!(id.source, IdentitySource::None, "{k}={v}");
        assert_eq!(id.pair_id, None);
        assert_eq!(id.session_id, None);
        assert_eq!(id.directory, None, "unpaired means no identity at all");
    }
    // a bad session with a good pair id is unpaired as well
    let env = env_with(
        h.path(),
        &[("OPENCODE_CTL_SESSION", "nope"), ("OPENCODE_CTL_PAIR", PID)],
    );
    let (id, warns) = resolve_identity(&env, h.path());
    assert_eq!(warns.len(), 1);
    assert_eq!(id.pair_id, None);
    // applying it leaves the server unpaired
    let mut env = env;
    let mut settings = Settings::default();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(env.session, None);
    assert_eq!(env.pair, None);
}

#[test]
fn pairing_file_fallback_from_pair_id() {
    let h = temp_home();
    let user = Settings {
        server_url: Some("http://user-file:9".to_string()),
        ..Settings::default()
    };

    // missing record: the pair id survives, nothing else, one warning, settings untouched
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, warns) = resolve_identity(&env, h.path());
    assert_eq!(warns.len(), 1);
    assert_eq!(id.pair_id.as_deref(), Some(PID));
    assert_eq!(id.session_id, None);
    assert_eq!(id.directory, None);
    assert_eq!(id.server_url, None);
    assert_eq!(id.source, IdentitySource::None);
    let mut settings = user.clone();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(env.pair.as_deref(), Some(PID));
    assert_eq!(env.session, None);
    assert_eq!(settings.server_url.as_deref(), Some("http://user-file:9"));

    // unparsable record: the same
    touch(&record_path(h.path(), PID), "{not json");
    let (id, warns) = resolve_identity(&env, h.path());
    assert_eq!(warns.len(), 1);
    assert_eq!(id.pair_id.as_deref(), Some(PID));
    assert_eq!(id.session_id, None);
    assert_eq!(id.source, IdentitySource::None);

    // a readable record with a server_url fills env and settings
    fs::remove_file(record_path(h.path(), PID)).unwrap();
    let mut r = rec(1, true, Some("claude"));
    r.server_url = Some("http://remote:4096".to_string());
    put(h.path(), &r);
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, warns) = resolve_identity(&env, h.path());
    assert!(warns.is_empty());
    let mut settings = user.clone();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(env.session.as_deref(), Some(SID));
    assert_eq!(env.directory, Some(PathBuf::from("/work/dir")));
    assert_eq!(env.server.as_deref(), Some("http://remote:4096"));
    assert_eq!(settings.server_url.as_deref(), Some("http://remote:4096"));

    // a File identity whose record has a null server_url IS the background service: it overrides the user file
    fs::remove_file(record_path(h.path(), PID)).unwrap();
    put(h.path(), &rec(1, true, Some("claude")));
    let (id, _) = resolve_identity(&env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]), h.path());
    assert_eq!(id.source, IdentitySource::File);
    let mut settings = user_u();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(settings.server_url, None);
    assert_eq!(settings.sources["server_url"], Source::Env);
}

fn user_u() -> Settings {
    let mut s = Settings {
        server_url: Some("http://u".to_string()),
        ..Settings::default()
    };
    s.sources.insert("server_url", Source::UserFile);
    s
}

#[test]
fn apply_identity_missing_or_unparsable_record_keeps_settings() {
    let h = temp_home();
    for unparsable in [false, true] {
        if unparsable {
            touch(&record_path(h.path(), PID), "{nope");
        }
        let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
        let (id, _) = resolve_identity(&env, h.path());
        assert_eq!(id.source, IdentitySource::None);
        let mut settings = user_u();
        let _ = apply_identity(&mut env, &mut settings, &id);
        assert_eq!(settings.server_url.as_deref(), Some("http://u"));
        assert_eq!(settings.sources["server_url"], Source::UserFile);
        assert_eq!(env.pair.as_deref(), Some(PID));
    }
}

#[test]
fn apply_identity_env_identity_keeps_user_file_server_url() {
    let h = temp_home();
    let mut r = rec(1, true, Some("claude"));
    r.server_url = Some("http://record".to_string());
    put(h.path(), &r);
    // SESSION set, no PAIR
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_SESSION", SID)]);
    let (id, _) = resolve_identity(&env, h.path());
    assert_eq!(id.source, IdentitySource::Env);
    let mut settings = user_u();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(settings.server_url.as_deref(), Some("http://u"));
    assert_eq!(settings.sources["server_url"], Source::UserFile);
    // SESSION and PAIR both set: the record is not consulted, settings untouched
    let mut env = env_with(
        h.path(),
        &[("OPENCODE_CTL_SESSION", SID), ("OPENCODE_CTL_PAIR", PID)],
    );
    let (id, _) = resolve_identity(&env, h.path());
    let mut settings = user_u();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(settings.server_url.as_deref(), Some("http://u"));
}

#[test]
fn apply_identity_pair_only_fills_session_and_env_dir_wins() {
    let h = temp_home();
    put(h.path(), &rec(1, true, Some("claude")));
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, _) = resolve_identity(&env, h.path());
    let _ = apply_identity(&mut env, &mut Settings::default(), &id);
    assert_eq!(env.session.as_deref(), Some(SID));
    assert_eq!(env.directory, Some(PathBuf::from("/work/dir")));
    // an explicit OPENCODE_CTL_DIR wins over the record's directory (resolve_identity: env first)
    let mut env = env_with(
        h.path(),
        &[("OPENCODE_CTL_PAIR", PID), ("OPENCODE_CTL_DIR", "/env/dir")],
    );
    let (id, _) = resolve_identity(&env, h.path());
    assert_eq!(id.source, IdentitySource::File);
    let _ = apply_identity(&mut env, &mut Settings::default(), &id);
    assert_eq!(env.session.as_deref(), Some(SID));
    assert_eq!(env.directory, Some(PathBuf::from("/env/dir")));
}

#[test]
fn apply_identity_userinfo_url_is_ignored() {
    // A record URL with userinfo is dropped: settings keep the user-file value and its source,
    // and env.server is cleared so the credentials are not carried anywhere.
    let h = temp_home();
    let mut r = rec(1, true, Some("claude"));
    r.server_url = Some("http://user:pw@host:1".to_string());
    put(h.path(), &r);
    let mut env = env_with(h.path(), &[("OPENCODE_CTL_PAIR", PID)]);
    let (id, _) = resolve_identity(&env, h.path());
    let mut settings = user_u();
    let _ = apply_identity(&mut env, &mut settings, &id);
    assert_eq!(settings.server_url.as_deref(), Some("http://u"));
    assert_eq!(settings.sources["server_url"], Source::UserFile);
    assert_eq!(env.server, None);
    assert_eq!(env.session.as_deref(), Some(SID));
}

// ---------------------------------------------------------------- re-bind

fn rebind_rig(parent: u32) -> (opencode_ctl::testing::TempHome, FakeProbe) {
    let h = temp_home();
    put(h.path(), &rec(1, true, Some("claude")));
    let probe = FakeProbe::new("boot");
    probe.set_alive(parent, "77");
    (h, probe)
}

#[test]
fn rebind_on_spawn_clears_claude_exited() {
    let (h, probe) = rebind_rig(900);
    touch(&pf(h.path(), "claude-exited"), "123");
    touch(&pf(h.path(), "dead-seen"), "456");
    write_sent(&pf(h.path(), "claude-pid"), &sent(5, "boot", "x")); // dead pid
    let out = rebind_on_spawn(h.path(), PID, 900, &probe).unwrap();
    assert_eq!(out, RebindOutcome::Rebound);
    assert!(!pf(h.path(), "claude-exited").exists());
    assert!(!pf(h.path(), "dead-seen").exists());
    assert_eq!(
        read_sentinel(&pf(h.path(), "claude-pid")),
        Some(sent(900, "boot", "77"))
    );
}

#[test]
fn rebind_on_dead_claude_pid() {
    // no claude-exited (a SIGKILLed launcher): a dead, absent or unparsable claude-pid re-binds
    for setup in ["dead", "absent", "garbage"] {
        let (h, probe) = rebind_rig(900);
        touch(&pf(h.path(), "dead-seen"), "456");
        match setup {
            "dead" => write_sent(&pf(h.path(), "claude-pid"), &sent(5, "boot", "x")),
            "garbage" => touch(&pf(h.path(), "claude-pid"), "zzz"),
            _ => {}
        }
        let out = rebind_on_spawn(h.path(), PID, 900, &probe).unwrap();
        assert_eq!(out, RebindOutcome::Rebound, "{setup}");
        assert!(!pf(h.path(), "dead-seen").exists(), "{setup}");
        assert_eq!(
            read_sentinel(&pf(h.path(), "claude-pid")),
            Some(sent(900, "boot", "77")),
            "{setup}"
        );
    }
    // no record at all
    let h = temp_home();
    let probe = FakeProbe::new("boot");
    assert_eq!(
        rebind_on_spawn(h.path(), PID, 900, &probe).unwrap(),
        RebindOutcome::NoRecord
    );
    assert!(!pf(h.path(), "claude-pid").exists());
}

#[test]
fn rebind_never_over_alive_controller() {
    let (h, probe) = rebind_rig(900);
    probe.set_alive(50, "st");
    write_sent(&pf(h.path(), "claude-pid"), &sent(50, "boot", "st"));
    touch(&pf(h.path(), "claude-exited"), "123");
    touch(&pf(h.path(), "dead-seen"), "456");
    let before = fs::read_to_string(pf(h.path(), "claude-pid")).unwrap();
    let out = rebind_on_spawn(h.path(), PID, 900, &probe).unwrap();
    assert_eq!(out, RebindOutcome::ControllerAlive);
    assert_eq!(
        fs::read_to_string(pf(h.path(), "claude-pid")).unwrap(),
        before
    );
    assert_eq!(
        fs::read_to_string(pf(h.path(), "claude-exited")).unwrap(),
        "123"
    );
    assert_eq!(
        fs::read_to_string(pf(h.path(), "dead-seen")).unwrap(),
        "456"
    );
}

// ---------------------------------------------------------------- tools

fn info_ok() -> Result<Value, opencode_ctl::transport::TransportError> {
    Ok(json!({"pid": 4242, "version": "2.0.32", "urls": ["http://127.0.0.1:49374"]}))
}

#[tokio::test]
async fn pair_status_missing_record() {
    let h = temp_home();
    let e = engine(
        FakeTransport::new(),
        TestClock::new(NOW),
        Host::Claude,
        Settings::default(),
        env_paired(&h, SID, Some(PID)),
    );
    let client = client_for(e).await;
    let out = call(&client, "pair_status", json!({})).await;
    assert_eq!(out["error"]["code"], json!("PAIR_NOT_FOUND"), "{out}");
}

#[tokio::test]
async fn pair_status_unpaired_returns_null() {
    for env in [
        |h: &opencode_ctl::testing::TempHome| env_unpaired(h),
        |h: &opencode_ctl::testing::TempHome| env_paired(h, SID, None),
    ] {
        let h = temp_home();
        // even a record lying around is not reported without a pair id
        put(h.path(), &rec(1, true, Some("claude")));
        let e = engine(
            FakeTransport::new(),
            TestClock::new(NOW),
            Host::Claude,
            Settings::default(),
            env(&h),
        );
        let client = client_for(e).await;
        let out = call(&client, "pair_status", json!({})).await;
        assert_eq!(out, json!({"paired": null}));
    }
}

fn paired_probe_rig(h: &opencode_ctl::testing::TempHome) -> FakeProbe {
    let probe = FakeProbe::new("boot");
    let mut r = rec(2, true, Some("claude"));
    r.server_url = Some("http://127.0.0.1:4096".to_string());
    put(h.path(), &r);
    alive_pid(&probe, h.path(), "claude-pid", 400);
    alive_pid(&probe, h.path(), "g2.tui-pid", 300);
    touch(&pf(h.path(), "g2.tui-exited"), "1");
    probe
}

#[tokio::test]
async fn pair_status_paired_returns_record_and_liveness() {
    let h = temp_home();
    let probe = paired_probe_rig(&h);
    let e = engine_with_probe(
        FakeTransport::new(),
        TestClock::new(NOW),
        Host::Claude,
        Settings::default(),
        env_paired(&h, SID, Some(PID)),
        probe,
    );
    let client = client_for(e).await;
    let out = call(&client, "pair_status", json!({})).await;
    assert_eq!(
        out,
        json!({
            "pair_id": PID, "session_id": SID, "directory": "/work/dir",
            "server_url": "http://127.0.0.1:4096", "created": T0,
            "controller": "claude", "generation": 2, "tab": true,
            "tui": "dead", "claude": "alive"
        })
    );
}

#[tokio::test]
async fn service_status_paired_block() {
    let h = temp_home();
    let probe = paired_probe_rig(&h);
    fs::remove_file(pf(h.path(), "g2.tui-exited")).unwrap();
    let fake = FakeTransport::new();
    fake.on("server.info", vec![info_ok()]);
    let mut env = env_paired(&h, SID, Some(PID));
    env.directory = Some(PathBuf::from("/work/dir"));
    let e = engine_with_probe(
        fake,
        TestClock::new(NOW),
        Host::Claude,
        Settings::default(),
        env,
        probe,
    );
    let client = client_for(e).await;
    let out = call(&client, "service_status", json!({})).await;
    let p = &out["paired"];
    assert_eq!(p["pair_id"], json!(PID));
    assert_eq!(p["tui"], json!("alive"), "{out}");
    assert_eq!(p["claude"], json!("alive"), "{out}");
    // re-read on every call
    fs::write(pf(h.path(), "claude-exited"), "9").unwrap();
    let out = call(&client, "service_status", json!({})).await;
    assert_eq!(out["paired"]["claude"], json!("dead"), "{out}");
}

#[tokio::test]
async fn wait_result_carries_pair_liveness() {
    let h = temp_home();
    let probe = paired_probe_rig(&h);
    fs::remove_file(pf(h.path(), "g2.tui-exited")).unwrap();
    fs::write(pf(h.path(), "claude-exited"), "9").unwrap();
    let clock = TestClock::new(NOW);
    let fake = FakeTransport::new().with_clock(clock.clone());
    let msgs = json!([{"id": "msg_a", "type": "user", "text": "do it", "time": {"created": NOW - 60_000}}]);
    fake.on(
        "session.message.list",
        vec![Ok(
            json!({"data": msgs, "cursor": {"next": null, "previous": "prev0"}}),
        )],
    );
    fake.on("session.inbox.list", vec![Ok(json!({"data": []}))]);
    fake.on("session.active", vec![Ok(json!({"data": {}}))]);
    fake.on("session.permission.list", vec![Ok(json!({"data": []}))]);
    fake.on("session.form.list", vec![Ok(json!({"data": []}))]);
    fake.on(
        "session.get",
        vec![Ok(json!({"data": {"id": SID, "tokens": {"input": 1, "output": 2, "reasoning": 3, "cache_read": 4}}}))],
    );
    fake.on_wait(vec![(Ok(WaitOutcome::Idle), 0)]);
    let e = engine_with_probe(
        fake,
        clock.clone(),
        Host::Claude,
        Settings::default(),
        env_paired(&h, SID, Some(PID)),
        probe,
    );
    let client = client_for(e).await;
    let args = json!({"session_id": SID, "timeout_s": 1, "after": "msg_a"});
    let out = tokio::select! {
        out = call(&client, "session_wait", args) => out,
        () = clock.run_until_idle() => unreachable!("run_until_idle never returns"),
    };
    assert_eq!(
        out["pair"],
        json!({"tui": "alive", "claude": "dead"}),
        "{out}"
    );
}

// ---------------------------------------------------------------- records on disk

#[test]
fn record_rewrite_never_lowers_generation() {
    let h = temp_home();
    put(h.path(), &rec(3, true, Some("claude")));
    rewrite_record(h.path(), PID, 2, false, None).unwrap();
    let r = read_record(h.path(), PID).unwrap().unwrap();
    assert_eq!(
        (r.generation, r.tab),
        (3, true),
        "a lower generation is a no-op"
    );
    rewrite_record(h.path(), PID, 4, false, None).unwrap();
    let r = read_record(h.path(), PID).unwrap().unwrap();
    assert_eq!((r.generation, r.tab), (4, false));
    assert_eq!(r.session_id, SID);
    assert_eq!(r.created, T0);
    // a missing record
    let h2 = temp_home();
    let e = rewrite_record(h2.path(), PID, 1, true, None).unwrap_err();
    assert_eq!(e.code, ErrorCode::PairNotFound);
}

#[test]
fn record_rewrite_concurrent_keeps_highest() {
    let h = temp_home();
    let n = 5;
    put(h.path(), &rec(n, true, Some("claude")));
    let home = h.path().to_path_buf();

    let (lock_held_tx, lock_held_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let ha = home.clone();
    let a = std::thread::spawn(move || {
        let hold = || {
            lock_held_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        };
        rewrite_record(&ha, PID, n + 2, true, Some(&hold))
    });
    lock_held_rx.recv().unwrap();

    // the lock is really held
    let probe = OpenOptions::new()
        .write(true)
        .open(home.join(format!("pair.{PID}.lock")))
        .expect("lock file exists");
    assert!(
        matches!(probe.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
        "rewrite_record must hold the lock while after_lock runs"
    );
    drop(probe);

    let (seen_tx, seen_rx) = mpsc::channel::<u32>();
    let hb = home.clone();
    let b = std::thread::spawn(move || {
        let seen = || {
            let g = read_record(&hb, PID).unwrap().unwrap().generation;
            seen_tx.send(g).unwrap();
        };
        rewrite_record(&hb, PID, n + 1, true, Some(&seen))
    });
    release_tx.send(()).unwrap();
    a.join().unwrap().unwrap();
    b.join().unwrap().unwrap();
    assert_eq!(seen_rx.recv().unwrap(), n + 2, "B waited for A's lock");
    assert_eq!(read_record(&home, PID).unwrap().unwrap().generation, n + 2);
}

#[test]
fn claim_file_exclusive_and_complete() {
    let h = temp_home();
    let path = h.path().join("pair.x.resume-lock");
    let content = vec![b'x'; 4096];
    let barrier = Arc::new(Barrier::new(8));
    let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut threads = Vec::new();
    for _ in 0..8 {
        let (p, c, b, d) = (path.clone(), content.clone(), barrier.clone(), done.clone());
        threads.push(std::thread::spawn(move || {
            b.wait();
            let r = claim_file(&p, &c, 0o600).unwrap();
            d.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            r
        }));
    }
    while done.load(std::sync::atomic::Ordering::SeqCst) < 8 {
        if let Ok(m) = fs::metadata(&path) {
            assert_eq!(m.len(), 4096, "a reader saw a partial or empty file");
        }
    }
    let wins = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .filter(|w| *w)
        .count();
    assert_eq!(wins, 1);
    assert_eq!(fs::read(&path).unwrap(), content);
    // no temp files left behind
    let stray: Vec<_> = fs::read_dir(h.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(stray.is_empty(), "{stray:?}");
}

#[test]
fn records_list_read_delete() {
    let h = temp_home();
    assert!(list_records(&h.path().join("absent")).is_empty());
    put(h.path(), &rec(1, true, Some("claude")));
    // exists -> INVALID_ARGUMENT
    let e = write_record_new(h.path(), &rec(1, true, Some("claude"))).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument);
    // a second, a corrupt one and unrelated files
    let other = "fedcba987654";
    let mut r2 = rec(1, false, None);
    r2.pair_id = other.to_string();
    put(h.path(), &r2);
    touch(&h.path().join("pair.aaaaaaaaaaaa.json"), "{broken");
    touch(&pf(h.path(), "claude-pid"), "1:b:s");
    touch(&h.path().join("settings.local.md"), "x");
    let mut ids: Vec<String> = list_records(h.path())
        .into_iter()
        .map(|r| r.pair_id)
        .collect();
    ids.sort();
    assert_eq!(ids, vec![PID.to_string(), other.to_string()]);
    assert_eq!(
        read_record(h.path(), "aaaaaaaaaaaa").unwrap(),
        None,
        "unparsable"
    );
    assert_eq!(
        read_record(h.path(), "bbbbbbbbbbbb").unwrap(),
        None,
        "missing"
    );
    // the JSON shape: null for the absent controller / server_url
    let raw: Value =
        serde_json::from_str(&fs::read_to_string(record_path(h.path(), other)).unwrap()).unwrap();
    assert_eq!(raw["controller"], Value::Null);
    assert_eq!(raw["server_url"], Value::Null);
    assert_eq!(raw["directory"], json!("/work/dir"));
    // delete removes every file of one pair and nothing else
    touch(&pf(h.path(), "g1.wrapper.sh"), "x");
    delete_pair_files(h.path(), PID).unwrap();
    assert!(!record_path(h.path(), PID).exists());
    assert!(!pf(h.path(), "claude-pid").exists());
    assert!(!pf(h.path(), "g1.wrapper.sh").exists());
    assert!(record_path(h.path(), other).exists());
    assert!(h.path().join("settings.local.md").exists());
    let _ = File::open(h.path());
}

#[test]
fn resume_applies_the_record_server_url_like_a_file_identity() {
    let h = temp_home();
    let mut r = rec(1, true, Some("claude"));
    r.server_url = Some("http://remote:4096".to_string());
    put(h.path(), &r);
    // the launcher's own settings name a different server: the record wins
    let mut env = env_with(h.path(), &[]);
    let mut settings = user_u();
    let warns = apply_record_server(h.path(), PID, &mut env, &mut settings);
    assert!(warns.is_empty());
    assert_eq!(settings.server_url.as_deref(), Some("http://remote:4096"));
    assert_eq!(settings.sources["server_url"], Source::Env);

    // Ruling 13: a record with a null server_url IS the background service
    fs::remove_file(record_path(h.path(), PID)).unwrap();
    put(h.path(), &rec(1, true, Some("claude")));
    let mut settings = user_u();
    let _ = apply_record_server(h.path(), PID, &mut env_with(h.path(), &[]), &mut settings);
    assert_eq!(settings.server_url, None);

    // a record URL with userinfo is ignored with a warning; no record leaves the settings alone
    fs::remove_file(record_path(h.path(), PID)).unwrap();
    let mut r = rec(1, true, Some("claude"));
    r.server_url = Some("http://u:pw@remote:4096".to_string());
    put(h.path(), &r);
    let mut settings = user_u();
    let warns = apply_record_server(h.path(), PID, &mut env_with(h.path(), &[]), &mut settings);
    assert_eq!(warns.len(), 1);
    assert!(!warns[0].message.contains("pw"), "{}", warns[0].message);
    assert_eq!(settings.server_url.as_deref(), Some("http://u"));
    fs::remove_file(record_path(h.path(), PID)).unwrap();
    let mut settings = user_u();
    let warns = apply_record_server(h.path(), PID, &mut env_with(h.path(), &[]), &mut settings);
    assert!(warns.is_empty());
    assert_eq!(settings.server_url.as_deref(), Some("http://u"));
}

#[test]
fn attach_record_controller_side_follows_a_resumed_controller() {
    let h = temp_home();
    let probe = FakeProbe::new("boot");
    let r = rec(1, false, None);
    // a genuine attach record with no controller files reads none
    assert_eq!(
        side_liveness(Side::Controller, &r, h.path(), &probe, NOW),
        Liveness::None
    );
    // resume wrote a claude-pid: alive while the process lives, dead after, dead once claude-exited exists
    probe.set_alive(7100, "st");
    write_sentinel(
        &pf(h.path(), "claude-pid"),
        &Sentinel {
            pid: 7100,
            boot_id: "boot".into(),
            starttime: "st".into(),
        },
    )
    .unwrap();
    assert_eq!(
        side_liveness(Side::Controller, &r, h.path(), &probe, NOW),
        Liveness::Alive
    );
    touch(&pf(h.path(), "claude-exited"), "");
    assert_eq!(
        side_liveness(Side::Controller, &r, h.path(), &probe, NOW),
        Liveness::Dead
    );
}

#[test]
fn rebind_on_spawn_refuses_an_empty_boot_id() {
    let (h, _) = rebind_rig(900);
    let probe = FakeProbe::new(""); // the boot read failed
    probe.set_alive(900, "77");
    touch(&pf(h.path(), "claude-exited"), "123");
    let err =
        rebind_on_spawn(h.path(), PID, 900, &probe).expect_err("an empty boot id is no value");
    assert_eq!(err.code, opencode_ctl::error::ErrorCode::IoError);
    assert!(pf(h.path(), "claude-exited").exists(), "nothing changed");
    assert!(!pf(h.path(), "claude-pid").exists(), "no sentinel written");
}

#[test]
fn records_are_keyed_on_the_file_name_id() {
    let h = temp_home();
    let other = "ffffffffffff";
    // pair.<PID>.json whose contents claim another pair id
    let mut r = rec(1, true, Some("claude"));
    r.pair_id = other.to_string();
    let body = serde_json::to_string(&r).unwrap();
    touch(&record_path(h.path(), PID), &body);
    let got = read_record(h.path(), PID).unwrap().expect("record");
    assert_eq!(got.pair_id, PID);
    let all = list_records(h.path());
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].pair_id, PID);
}
