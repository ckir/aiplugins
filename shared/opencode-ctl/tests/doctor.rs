//! `doctor-json` (Task 16, part C): the eighteen checks, their messages, the compute and emit
//! order, `launcher_link_status` and `parse_doctor_args`. Part A's startup/identity/serve tests
//! live in tests/startup.rs.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::*;
use opencode_ctl::cli::{parse_doctor_args, DoctorArgs};
use opencode_ctl::clock::TestClock;
#[cfg(unix)]
use opencode_ctl::doctor::launcher_link_status;
use opencode_ctl::doctor::{
    doctor_json, parse_source_file, windows_link_status, windows_source_old_check, DoctorCtx,
    LinkStatus, SourceFile, Status,
};
use opencode_ctl::engine::{Engine, EnvSnapshot};
use opencode_ctl::error::ErrorCode;
use opencode_ctl::host::Host;
use opencode_ctl::liveness::Sentinel;
use opencode_ctl::pairing::{record_path, write_record_new, write_sentinel, Record};
use opencode_ctl::settings::{self, Note, NoteReason, Settings, Source};
use opencode_ctl::testing::fake::FakeTransport;
use opencode_ctl::testing::{env_map, temp_home, FakeProbe, TempHome};
use opencode_ctl::transport::{Lifecycle, TransportError};
use serde_json::{json, Value};

const PAIR: &str = "0123456789ab";
const OTHER_PAIR: &str = "fe9876543210";
const URL: &str = "http://127.0.0.1:49374";
/// The registration file's `password` (Service.Info, packages/client/src/effect/service.ts:146).
const SECRET: &str = "hunter2-fixture-secret";

const IDS: [&str; 18] = [
    "ctl_home_override",
    "binary",
    "version_range",
    "registration",
    "server_info",
    "wait_route",
    "pairing",
    "lock_source",
    "env_vs_user_file",
    "windows_source_old",
    "launcher_link",
    "server_url_password",
    "opencode_password_set",
    "log",
    "orphaned_controllers",
    "orphaned_workers",
    "settings_notes",
    "settings_file_hash",
];

struct Rig {
    fake: FakeTransport,
    engine: Arc<TestEngine>,
    probe: Arc<FakeProbe>,
    home: TempHome,
}

struct Opts {
    settings: Settings,
    notes: Vec<Note>,
    host: Host,
    pair: Option<String>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts {
            settings: Settings::default(),
            notes: Vec::new(),
            host: Host::Claude,
            pair: None,
        }
    }
}

fn terr(code: ErrorCode, message: &str) -> TransportError {
    TransportError {
        code,
        message: message.to_string(),
        hint: None,
        details: None,
    }
}

fn info(version: &str, pid: u64) -> Value {
    json!({"data": {"version": version, "pid": pid, "urls": [URL]}})
}

impl Rig {
    fn ctl(&self) -> PathBuf {
        self.home.path().join("ctl")
    }
    fn user_home(&self) -> PathBuf {
        self.home.path().join("user")
    }
    fn bin_dir(&self) -> PathBuf {
        self.home.path().join("bin")
    }
    fn reg(&self) -> PathBuf {
        self.home.path().join("state").join("service.json")
    }
    fn cfg(&self) -> PathBuf {
        self.home.path().join("config").join("service.json")
    }
    fn opencode(&self) -> PathBuf {
        self.home.path().join("opencode")
    }
    fn exe() -> PathBuf {
        PathBuf::from("/opt/plugin/bin/opencode-ctl-cc-mcp")
    }

    fn new() -> Rig {
        Rig::with(Opts::default())
    }

    fn with(mut o: Opts) -> Rig {
        let home = temp_home();
        let ctl = home.path().join("ctl");
        fs::create_dir_all(&ctl).unwrap();
        fs::create_dir_all(home.path().join("bin")).unwrap();
        fs::create_dir_all(home.path().join("state")).unwrap();
        fs::create_dir_all(home.path().join("config")).unwrap();
        fs::create_dir_all(home.path().join("user")).unwrap();
        let bin = home.path().join("opencode");
        fs::write(&bin, "#!/bin/sh\n").unwrap();
        if o.settings.opencode_bin.is_none() {
            o.settings.opencode_bin = Some(bin);
        }
        let fake = FakeTransport::new();
        fake.on("server.info", vec![Ok(info("2.0.32", 7))]);
        fake.on("session.active", vec![Ok(json!({"data": {}}))]);
        let probe = Arc::new(FakeProbe::new("boot"));
        probe.set_alive(4242, "s");
        let mut env = EnvSnapshot::for_tests(home.path().join("cwd"), ctl.clone());
        env.pair = o.pair.clone();
        let engine = Arc::new(Engine::new(
            o.settings,
            o.notes,
            o.host,
            env,
            fake.clone(),
            TestClock::new(1_800_000_000_000),
            probe.clone(),
        ));
        let rig = Rig {
            fake,
            engine,
            probe,
            home,
        };
        fs::write(
            rig.reg(),
            json!({"url": URL, "pid": 4242, "version": "2.0.32", "password": SECRET}).to_string(),
        )
        .unwrap();
        rig
    }

    async fn run_with(&self, env: &[(&str, &str)], reg: Option<&Path>, verbose: bool) -> Value {
        let env = env_map(env);
        let (bin, exe, user, cfg) = (self.bin_dir(), Rig::exe(), self.user_home(), self.cfg());
        let ctx = DoctorCtx {
            bin_dir: &bin,
            running_exe: &exe,
            version: "0.1.0",
            user_home: &user,
            path: None,
            registration_file: reg,
            config_file: Some(&cfg),
            env: &env,
        };
        doctor_json(&*self.engine, &ctx, verbose).await
    }

    async fn run(&self) -> Value {
        let reg = self.reg();
        self.run_with(&[], Some(&reg), false).await
    }

    async fn run_env(&self, env: &[(&str, &str)]) -> Value {
        let reg = self.reg();
        self.run_with(env, Some(&reg), false).await
    }

    fn server_info_calls(&self) -> usize {
        self.fake
            .calls()
            .iter()
            .filter(|c| c.op == "server.info")
            .count()
    }
}

/// The distinctive password VALUE appears nowhere in the whole report.
#[track_caller]
fn no_secret(v: &Value) {
    assert!(
        !v.to_string().contains(SECRET),
        "registration password leaked: {v}"
    );
}

fn check<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no check {id}"))
}

#[track_caller]
fn expect(v: &Value, id: &str, status: &str, message: &str) {
    let c = check(v, id);
    assert_eq!(c["status"], status, "{id}: {c}");
    assert_eq!(c["message"], message, "{id}: {c}");
}

fn write_pair(rig: &Rig, id: &str, tab: bool, controller: Option<&str>) -> Record {
    let r = Record {
        pair_id: id.to_string(),
        session_id: SID.to_string(),
        directory: PathBuf::from("/work"),
        server_url: None,
        created: 1_800_000_000_000 - 120_000,
        controller: controller.map(str::to_string),
        generation: 1,
        tab,
    };
    write_record_new(&rig.ctl(), &r).unwrap();
    r
}

fn sentinel(rig: &Rig, file: &str, pid: u32) {
    rig.probe.set_alive(pid, "s");
    let s = Sentinel {
        pid,
        boot_id: "boot".to_string(),
        starttime: "s".to_string(),
    };
    write_sentinel(&rig.ctl().join(file), &s).unwrap();
}

fn paired_rig() -> Rig {
    let rig = Rig::with(Opts {
        pair: Some(PAIR.to_string()),
        ..Opts::default()
    });
    write_pair(&rig, PAIR, true, Some("claude"));
    sentinel(&rig, &format!("pair.{PAIR}.g1.tui-pid"), 501);
    sentinel(&rig, &format!("pair.{PAIR}.claude-pid"), 502);
    rig.fake
        .on("session.get", vec![Ok(json!({"data": {"id": SID}}))]);
    rig
}

fn doc_without_wait_route() -> Value {
    let mut doc: Value =
        serde_json::from_str(include_str!("fixtures/openapi.json")).expect("fixture");
    let paths = doc["paths"].as_object_mut().unwrap();
    let mut emptied = Vec::new();
    for (p, item) in paths.iter_mut() {
        let item = item.as_object_mut().unwrap();
        item.retain(|_, op| op["operationId"] != "experimental.session.wait");
        if item.is_empty() {
            emptied.push(p.clone());
        }
    }
    for p in emptied {
        paths.remove(&p);
    }
    doc
}

// ------------------------------------------------------------------ order and shape

#[tokio::test]
async fn doctor_json_check_ids_in_order() {
    let rig = Rig::new();
    let v = rig.run().await;
    let ids: Vec<&str> = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, IDS);
    assert_eq!(v["notes"], json!([]));
    assert!(v.get("settings").is_none(), "settings only with verbose");
    for c in v["checks"].as_array().unwrap() {
        assert!(["ok", "warn", "fail", "skip"].contains(&c["status"].as_str().unwrap()));
        assert!(c["message"].is_string());
    }
}

#[tokio::test]
async fn doctor_json_baseline_all_checks() {
    let rig = Rig::new();
    let v = rig.run().await;
    let ctl = rig.ctl().display().to_string();
    expect(
        &v,
        "ctl_home_override",
        "ok",
        "OPENCODE_CTL_HOME is not set",
    );
    expect(
        &v,
        "binary",
        "ok",
        &format!("opencode found at {}", rig.opencode().display()),
    );
    expect(
        &v,
        "version_range",
        "ok",
        "CLI 2.0.32 and server 2.0.32 are within the tested 2.0.x range",
    );
    expect(&v, "registration", "ok", "registered pid 4242 is alive");
    no_secret(&v);
    expect(
        &v,
        "server_info",
        "ok",
        "server.info answered: version 2.0.32, pid 7",
    );
    expect(
        &v,
        "wait_route",
        "ok",
        "experimental.session.wait is in the operation table",
    );
    expect(
        &v,
        "pairing",
        "skip",
        "not paired: OPENCODE_CTL_PAIR is not set",
    );
    expect(
        &v,
        "orphaned_controllers",
        "ok",
        "no pair has a dead controller beside a live TUI",
    );
    expect(&v, "log", "skip", &format!("no log file in {ctl}/logs"));
    assert_eq!(rig.fake.openapi_fetches(), 1, "one fetch, by server.info");
}

#[tokio::test]
async fn doctor_json_verbose_prints_resolved_settings() {
    let rig = Rig::new();
    let reg = rig.reg();
    let v = rig.run_with(&[], Some(&reg), true).await;
    let s = &v["settings"];
    assert_eq!(s["wait_timeout_s"], 120);
    assert_eq!(s["permission_policy"], "inherit");
    assert_eq!(s["permissions_locked"], false);
    assert_eq!(s["sources"]["permission_policy"], "default");
    assert_eq!(s["server_url"], Value::Null);
}

// ------------------------------------------------------------------ one test per id

#[tokio::test]
async fn doctor_json_reports_ctl_home_override() {
    let rig = Rig::new();
    expect(
        &rig.run_env(&[("OPENCODE_CTL_HOME", "/tmp/elsewhere")]).await,
        "ctl_home_override",
        "warn",
        "OPENCODE_CTL_HOME is set to /tmp/elsewhere: the settings file and the pair files are read from there",
    );
    // A blank value is unset.
    expect(
        &rig.run_env(&[("OPENCODE_CTL_HOME", "  ")]).await,
        "ctl_home_override",
        "ok",
        "OPENCODE_CTL_HOME is not set",
    );
}

#[tokio::test]
async fn doctor_json_reports_binary() {
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "binary",
        "ok",
        &format!("opencode found at {}", rig.opencode().display()),
    );

    let rig = Rig::with(Opts {
        settings: Settings {
            opencode_bin: Some(PathBuf::from("/nonexistent/opencode")),
            ..Settings::default()
        },
        ..Opts::default()
    });
    expect(
        &rig.run().await,
        "binary",
        "fail",
        "the configured opencode_bin /nonexistent/opencode is not a file",
    );

    // No explicit path, no PATH, nothing under the user home.
    let rig = Rig::with(Opts::default());
    let engine = Engine::new(
        Settings::default(),
        Vec::new(),
        Host::Claude,
        EnvSnapshot::for_tests(rig.home.path().join("cwd"), rig.ctl()),
        rig.fake.clone(),
        TestClock::new(0),
        rig.probe.clone(),
    );
    let env = env_map(&[]);
    let (bin, exe, user) = (rig.bin_dir(), Rig::exe(), rig.user_home());
    let ctx = DoctorCtx {
        bin_dir: &bin,
        running_exe: &exe,
        version: "0.1.0",
        user_home: &user,
        path: None,
        registration_file: None,
        config_file: None,
        env: &env,
    };
    let v = doctor_json(&engine, &ctx, false).await;
    expect(
        &v,
        "binary",
        "fail",
        "opencode executable not found: install opencode or set OPENCODE_CTL_BIN",
    );
    expect(&v, "version_range", "skip", "binary failed");
}

#[tokio::test]
async fn doctor_json_reports_version_range() {
    // server version outside 2.0.x
    let rig = Rig::new();
    rig.fake.on("server.info", vec![Ok(info("2.1.0", 7))]);
    expect(
        &rig.run().await,
        "version_range",
        "warn",
        "server 2.1.0 is outside the tested 2.0.x range",
    );
    // server_info failed: CLI only
    let rig = Rig::new();
    rig.fake
        .on_lifecycle(Lifecycle::Status, Ok("stopped".to_string()));
    rig.fake
        .on_lifecycle(Lifecycle::Start, Err(terr(ErrorCode::CliError, "boom")));
    expect(
        &rig.run().await,
        "version_range",
        "ok",
        "server version unknown; CLI 2.0.32",
    );
    // below 2.0
    let rig = Rig::new();
    rig.fake.on_cli_version(Ok("opencode v1.9.4".to_string()));
    expect(
        &rig.run().await,
        "version_range",
        "fail",
        "1.9.4 is below 2.0",
    );
    // unparsable
    let rig = Rig::new();
    rig.fake.on_cli_version(Ok("garbage".to_string()));
    expect(
        &rig.run().await,
        "version_range",
        "fail",
        "unparsable version output",
    );
    // the CLI cannot run
    let rig = Rig::new();
    rig.fake.on_cli_version(Err(terr(
        ErrorCode::OpencodeNotFound,
        "opencode executable could not be started: denied",
    )));
    expect(
        &rig.run().await,
        "version_range",
        "fail",
        "cannot run opencode --version: opencode executable could not be started: denied",
    );
}

#[tokio::test]
async fn doctor_json_reports_registration() {
    let rig = Rig::new();
    // absent
    let missing = rig.home.path().join("state").join("absent.json");
    let v = rig.run_with(&[], Some(&missing), false).await;
    expect(
        &v,
        "registration",
        "skip",
        &format!("no registration file at {}", missing.display()),
    );
    // no location known
    let v = rig.run_with(&[], None, false).await;
    expect(
        &v,
        "registration",
        "skip",
        "no registration file location is known",
    );
    // unreadable content
    fs::write(rig.reg(), "not json").unwrap();
    expect(
        &rig.run().await,
        "registration",
        "warn",
        &format!(
            "registration file {} is not a valid service registration",
            rig.reg().display()
        ),
    );
    // disabled in the service config
    fs::write(
        rig.reg(),
        json!({"url": URL, "pid": 4242, "password": SECRET}).to_string(),
    )
    .unwrap();
    fs::write(rig.cfg(), json!({"disabled": true}).to_string()).unwrap();
    expect(
        &rig.run().await,
        "registration",
        "warn",
        &format!(
            "service disabled in {}: every CLI call runs a private standalone server",
            rig.cfg().display()
        ),
    );
    fs::write(rig.cfg(), json!({"disabled": false}).to_string()).unwrap();
    let v = rig.run().await;
    expect(&v, "registration", "ok", "registered pid 4242 is alive");
    no_secret(&v);
    // a dead pid fails and carries the url from the file
    rig.probe.set_dead(4242);
    let v = rig.run().await;
    no_secret(&v);
    expect(&v, "registration", "fail", "registered pid 4242 is dead");
    assert_eq!(check(&v, "registration")["details"]["url"], URL);
    // plain server_info: ok next to the stale registration (another server answers)
    expect(
        &v,
        "server_info",
        "ok",
        "server.info answered: version 2.0.32, pid 7",
    );
}

#[tokio::test]
async fn doctor_json_registration_never_calls_server_info() {
    // Stopped service whose start fails: server_info is the only check allowed to touch the
    // service, and it must not reach server.info; registration (dead pid) adds no call either.
    let rig = Rig::new();
    rig.probe.set_dead(4242);
    rig.fake
        .on_lifecycle(Lifecycle::Status, Ok("stopped".to_string()));
    rig.fake
        .on_lifecycle(Lifecycle::Start, Err(terr(ErrorCode::CliError, "no start")));
    let v = rig.run().await;
    expect(&v, "registration", "fail", "registered pid 4242 is dead");
    assert_eq!(rig.server_info_calls(), 0);
}

#[tokio::test]
async fn doctor_json_reports_server_info() {
    // ok
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "server_info",
        "ok",
        "server.info answered: version 2.0.32, pid 7",
    );
    assert_eq!(
        rig.fake
            .lifecycle_calls()
            .iter()
            .map(|c| c.0)
            .collect::<Vec<_>>(),
        vec![Lifecycle::Status]
    );

    // ok_started
    let rig = Rig::new();
    rig.fake
        .on_lifecycle(Lifecycle::Status, Ok("stopped".to_string()));
    let v = rig.run().await;
    expect(
        &v,
        "server_info",
        "ok",
        "service was stopped; opencode service start brought it up",
    );
    assert_eq!(
        check(&v, "server_info")["details"]["started_by_doctor"],
        true
    );

    // fail_start
    let rig = Rig::new();
    rig.fake
        .on_lifecycle(Lifecycle::Status, Ok("stopped".to_string()));
    rig.fake.on_lifecycle(
        Lifecycle::Start,
        Err(terr(ErrorCode::CliError, "port 49374 in use\nsecond line")),
    );
    expect(
        &rig.run().await,
        "server_info",
        "fail",
        "service stopped and opencode service start failed: port 49374 in use",
    );

    // fail_unreachable (server_url set: server.info directly)
    let rig = Rig::with(Opts {
        settings: Settings {
            server_url: Some("http://remote:1".to_string()),
            ..Settings::default()
        },
        ..Opts::default()
    });
    rig.fake.on(
        "server.info",
        vec![Err(terr(ErrorCode::ServiceDown, "connection refused"))],
    );
    expect(
        &rig.run().await,
        "server_info",
        "fail",
        "cannot reach http://remote:1: connection refused",
    );
    assert!(
        rig.fake.lifecycle_calls().is_empty(),
        "server_url set: no service status"
    );

    // fail_auth
    let rig = Rig::new();
    rig.fake.on(
        "server.info",
        vec![Err(terr(
            ErrorCode::AuthFailed,
            "the server rejected the password",
        ))],
    );
    expect(
        &rig.run().await,
        "server_info",
        "fail",
        &format!("server at {URL} rejected the credentials: compare `opencode service get password` with OPENCODE_SERVER_PASSWORD"),
    );

    // fail_other
    let rig = Rig::new();
    rig.fake.on(
        "server.info",
        vec![Err(terr(ErrorCode::CliError, "unexpected 500"))],
    );
    expect(
        &rig.run().await,
        "server_info",
        "fail",
        "server.info failed (CLI_ERROR): unexpected 500",
    );

    // running per `service status` but server.info says SERVICE_DOWN
    let rig = Rig::new();
    rig.fake.on(
        "server.info",
        vec![Err(terr(ErrorCode::ServiceDown, "refused"))],
    );
    expect(
        &rig.run().await,
        "server_info",
        "fail",
        &format!("cannot reach {URL}: refused"),
    );

    // `service status` itself failing
    let rig = Rig::new();
    rig.fake.on_lifecycle(
        Lifecycle::Status,
        Err(terr(
            ErrorCode::OpencodeNotFound,
            "opencode executable not found",
        )),
    );
    expect(
        &rig.run().await,
        "server_info",
        "fail",
        "server.info failed (OPENCODE_NOT_FOUND): opencode executable not found",
    );
}

#[tokio::test]
async fn doctor_json_server_info_starts_stopped_service() {
    let rig = Rig::new();
    rig.fake
        .on_lifecycle(Lifecycle::Status, Ok("stopped".to_string()));
    let v = rig.run().await;
    expect(
        &v,
        "server_info",
        "ok",
        "service was stopped; opencode service start brought it up",
    );
    assert_eq!(
        check(&v, "server_info")["details"]["started_by_doctor"],
        true
    );
    let verbs: Vec<Lifecycle> = rig.fake.lifecycle_calls().iter().map(|c| c.0).collect();
    assert_eq!(verbs, vec![Lifecycle::Status, Lifecycle::Start]);
    assert_eq!(
        rig.server_info_calls(),
        1,
        "server.info recorded after Start"
    );

    let rig = Rig::new();
    rig.fake
        .on_lifecycle(Lifecycle::Status, Ok("stopped".to_string()));
    rig.fake.on_lifecycle(
        Lifecycle::Start,
        Err(terr(ErrorCode::CliError, "cannot bind 127.0.0.1")),
    );
    let v = rig.run().await;
    let m = check(&v, "server_info")["message"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(check(&v, "server_info")["status"], "fail");
    assert!(m.ends_with("cannot bind 127.0.0.1"), "{m}");
    assert_eq!(rig.server_info_calls(), 0);
}

#[tokio::test]
async fn doctor_json_reports_wait_route() {
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "wait_route",
        "ok",
        "experimental.session.wait is in the operation table",
    );
    // the live document lacks the route
    let rig = Rig::new();
    rig.fake.set_openapi(Some(doc_without_wait_route()));
    expect(
        &rig.run().await,
        "wait_route",
        "warn",
        "experimental.session.wait is not in the operation table: session_wait falls back to polling",
    );
    // the live document is unavailable: the bundled snapshot cannot confirm the route
    let rig = Rig::new();
    rig.fake.set_openapi(None);
    expect(
        &rig.run().await,
        "wait_route",
        "warn",
        "the live operation table is unavailable, so experimental.session.wait could not be confirmed",
    );
    // server_info failed
    let rig = Rig::new();
    rig.fake.on(
        "server.info",
        vec![Err(terr(ErrorCode::ServiceDown, "refused"))],
    );
    expect(&rig.run().await, "wait_route", "skip", "server_info failed");
}

#[tokio::test]
async fn doctor_json_api_checks_skip_when_service_down() {
    let rig = paired_rig();
    rig.fake.set_openapi(None);
    rig.fake
        .on_lifecycle(Lifecycle::Status, Ok("stopped".to_string()));
    rig.fake
        .on_lifecycle(Lifecycle::Start, Err(terr(ErrorCode::CliError, "no start")));
    let v = rig.run().await;
    expect(
        &v,
        "server_info",
        "fail",
        "service stopped and opencode service start failed: no start",
    );
    expect(&v, "wait_route", "skip", "server_info failed");
    expect(&v, "orphaned_workers", "skip", "server_info failed");
    expect(
        &v,
        "pairing",
        "ok",
        &format!(
            "pair {PAIR}: tui alive (the tab is running); controller alive (the controller is running); generation 1 sentinel present; session {SID} skipped"
        ),
    );
    assert_eq!(check(&v, "pairing")["details"]["session"], "skipped");
    assert!(
        rig.fake.calls().is_empty(),
        "no API call at all: {:?}",
        rig.fake.calls()
    );
    assert_eq!(rig.fake.openapi_fetches(), 0);
}

#[tokio::test]
async fn doctor_json_reports_pairing() {
    // fully alive
    let rig = paired_rig();
    let v = rig.run().await;
    expect(
        &v,
        "pairing",
        "ok",
        &format!(
            "pair {PAIR}: tui alive (the tab is running); controller alive (the controller is running); generation 1 sentinel present; session {SID} found"
        ),
    );
    assert_eq!(check(&v, "pairing")["details"]["session"], "found");
    let sess: Vec<_> = rig
        .fake
        .calls()
        .into_iter()
        .filter(|c| c.op == "session.get")
        .collect();
    assert_eq!(sess.len(), 1);
    assert_eq!(
        sess[0].params,
        vec![("sessionID".to_string(), SID.to_string())]
    );

    // starting and none are not faults
    let rig = Rig::with(Opts {
        pair: Some(PAIR.to_string()),
        ..Opts::default()
    });
    write_pair(&rig, PAIR, false, None);
    rig.fake
        .on("session.get", vec![Ok(json!({"data": {"id": SID}}))]);
    expect(
        &rig.run().await,
        "pairing",
        "ok",
        &format!(
            "pair {PAIR}: tui none (no tab was requested); controller none (no controller was recorded); generation 1 sentinel not applicable (no tab); session {SID} found"
        ),
    );

    // a dead controller warns
    let rig = paired_rig();
    fs::write(rig.ctl().join(format!("pair.{PAIR}.claude-exited")), "").unwrap();
    expect(
        &rig.run().await,
        "pairing",
        "warn",
        &format!(
            "pair {PAIR}: tui alive (the tab is running); controller dead (the process is gone); generation 1 sentinel present; session {SID} found"
        ),
    );

    // an unreadable sentinel is `unknown`
    let rig = paired_rig();
    fs::write(rig.ctl().join(format!("pair.{PAIR}.claude-pid")), "junk").unwrap();
    expect(
        &rig.run().await,
        "pairing",
        "warn",
        &format!(
            "pair {PAIR}: tui alive (the tab is running); controller unknown (the sentinel cannot be verified); generation 1 sentinel present; session {SID} found"
        ),
    );

    // the record's generation has no tui sentinel
    let rig = paired_rig();
    fs::remove_file(rig.ctl().join(format!("pair.{PAIR}.g1.tui-pid"))).unwrap();
    fs::write(rig.ctl().join(format!("pair.{PAIR}.g1.tui-exited")), "").unwrap();
    expect(
        &rig.run().await,
        "pairing",
        "warn",
        &format!(
            "pair {PAIR}: tui dead (the process is gone); controller alive (the controller is running); generation 1 sentinel missing; session {SID} found"
        ),
    );

    // session not found on the server
    let rig = paired_rig();
    rig.fake.on(
        "session.get",
        vec![Err(terr(ErrorCode::SessionNotFound, "no such session"))],
    );
    let v = rig.run().await;
    expect(
        &v,
        "pairing",
        "fail",
        &format!(
            "pair {PAIR}: tui alive (the tab is running); controller alive (the controller is running); generation 1 sentinel present; session {SID} not found"
        ),
    );
    assert_eq!(check(&v, "pairing")["details"]["session"], "not_found");

    // another API error on the session half
    let rig = paired_rig();
    rig.fake
        .on("session.get", vec![Err(terr(ErrorCode::CliError, "weird"))]);
    expect(
        &rig.run().await,
        "pairing",
        "warn",
        &format!(
            "pair {PAIR}: tui alive (the tab is running); controller alive (the controller is running); generation 1 sentinel present; session {SID} lookup failed (CLI_ERROR)"
        ),
    );

    // no record
    let rig = Rig::with(Opts {
        pair: Some(PAIR.to_string()),
        ..Opts::default()
    });
    expect(
        &rig.run().await,
        "pairing",
        "fail",
        &format!("no usable pair record for {PAIR}"),
    );
    fs::write(record_path(&rig.ctl(), PAIR), "{not json").unwrap();
    expect(
        &rig.run().await,
        "pairing",
        "fail",
        &format!("no usable pair record for {PAIR}"),
    );
}

#[cfg(unix)]
#[tokio::test]
async fn doctor_json_pairing_record_unreadable() {
    let rig = Rig::with(Opts {
        pair: Some(PAIR.to_string()),
        ..Opts::default()
    });
    fs::create_dir(record_path(&rig.ctl(), PAIR)).unwrap();
    expect(
        &rig.run().await,
        "pairing",
        "fail",
        &format!("pair record for {PAIR} is unreadable: IO_ERROR: pair record unreadable: Is a directory (os error 21)"),
    );
}

#[tokio::test]
async fn doctor_json_pairing_keys_on_file_name_id() {
    // pair.<PAIR>.json holds a record whose inner id is OTHER_PAIR. OTHER_PAIR's sentinels say
    // alive/alive; PAIR's own say dead. Every decision keys on the FILE NAME's id.
    let rig = Rig::with(Opts {
        pair: Some(PAIR.to_string()),
        ..Opts::default()
    });
    let mut r = write_pair(&rig, OTHER_PAIR, true, Some("claude"));
    fs::remove_file(record_path(&rig.ctl(), OTHER_PAIR)).unwrap();
    r.pair_id = OTHER_PAIR.to_string();
    fs::write(
        record_path(&rig.ctl(), PAIR),
        serde_json::to_vec(&r).unwrap(),
    )
    .unwrap();
    sentinel(&rig, &format!("pair.{OTHER_PAIR}.g1.tui-pid"), 601);
    sentinel(&rig, &format!("pair.{OTHER_PAIR}.claude-pid"), 602);
    rig.fake
        .on("session.get", vec![Ok(json!({"data": {"id": SID}}))]);
    fs::write(rig.ctl().join(format!("pair.{PAIR}.g1.tui-exited")), "").unwrap();
    fs::write(rig.ctl().join(format!("pair.{PAIR}.claude-exited")), "").unwrap();
    let v = rig.run().await;
    expect(
        &v,
        "pairing",
        "warn",
        &format!(
            "pair {PAIR}: tui dead (the process is gone); controller dead (the process is gone); generation 1 sentinel missing; session {SID} found"
        ),
    );
    // orphaned_controllers: PAIR (by name) is dead/dead, not dead/alive
    expect(
        &v,
        "orphaned_controllers",
        "ok",
        "no pair has a dead controller beside a live TUI",
    );
    // and the reverse: by name PAIR is tui alive + controller dead; the inner id's files say the opposite
    let rig2 = Rig::new();
    let mut r = write_pair(&rig2, OTHER_PAIR, true, Some("claude"));
    fs::remove_file(record_path(&rig2.ctl(), OTHER_PAIR)).unwrap();
    r.pair_id = OTHER_PAIR.to_string();
    fs::write(
        record_path(&rig2.ctl(), PAIR),
        serde_json::to_vec(&r).unwrap(),
    )
    .unwrap();
    sentinel(&rig2, &format!("pair.{PAIR}.g1.tui-pid"), 701);
    fs::write(rig2.ctl().join(format!("pair.{PAIR}.claude-exited")), "").unwrap();
    sentinel(&rig2, &format!("pair.{OTHER_PAIR}.claude-pid"), 702);
    sentinel(&rig2, &format!("pair.{OTHER_PAIR}.g1.tui-pid"), 703);
    let v = rig2.run().await;
    expect(
        &v,
        "orphaned_controllers",
        "warn",
        &format!("controller dead while the TUI is alive for pair(s): {PAIR}"),
    );
}

#[tokio::test]
async fn doctor_json_reports_lock_source() {
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "lock_source",
        "ok",
        "permissions_locked=false (default); permission_policy=inherit (default)",
    );
    let env = env_map(&[
        ("OPENCODE_CTL_PERMISSIONS_LOCKED", "true"),
        ("OPENCODE_CTL_PERMISSION_POLICY", "deny"),
    ]);
    let (s, _) = settings::load(settings::Input {
        host: Host::Claude,
        env: &env,
        user_file: None,
        workspace_file: None,
    });
    let rig = Rig::with(Opts {
        settings: s,
        ..Opts::default()
    });
    expect(
        &rig.run().await,
        "lock_source",
        "ok",
        "permissions_locked=true (env); permission_policy=deny (env)",
    );
    let (s, _) = settings::load(settings::Input {
        host: Host::Claude,
        env: &env_map(&[]),
        user_file: Some("---\npermissions_locked: true\npermission_policy: ask\n---\n"),
        workspace_file: None,
    });
    let rig = Rig::with(Opts {
        settings: s,
        ..Opts::default()
    });
    expect(
        &rig.run().await,
        "lock_source",
        "ok",
        "permissions_locked=true (user file); permission_policy=ask (user file)",
    );
}

#[tokio::test]
async fn doctor_json_reports_env_vs_user_file() {
    let rig = Rig::new();
    fs::write(
        rig.ctl().join("settings.local.md"),
        "---\ndefault_model: a/b\nserver_url: http://u\nwait_max_s: 200\n---\n",
    )
    .unwrap();
    // same values
    expect(
        &rig.run_env(&[("OPENCODE_CTL_DEFAULT_MODEL", "a/b")]).await,
        "env_vs_user_file",
        "ok",
        "no protected key's environment value differs from the user-level file",
    );
    // differing value, and an EMPTY OPENCODE_CTL_SERVER is read raw (the background-service override)
    expect(
        &rig.run_env(&[
            ("OPENCODE_CTL_DEFAULT_MODEL", "c/d"),
            ("OPENCODE_CTL_SERVER", ""),
            ("OPENCODE_CTL_WAIT_MAX_S", "  "),
        ])
        .await,
        "env_vs_user_file",
        "warn",
        "environment overrides the user-level file for: server_url (env \"\", file \"http://u\"), default_model (env \"c/d\", file \"a/b\")",
    );
    // a key the file does not set is not an override of the file
    let rig = Rig::new();
    fs::write(rig.ctl().join("settings.local.md"), "---\npoll_s: 5\n---\n").unwrap();
    expect(
        &rig.run_env(&[("OPENCODE_CTL_DEFAULT_AGENT", "build")])
            .await,
        "env_vs_user_file",
        "ok",
        "no protected key's environment value differs from the user-level file",
    );
}

#[test]
fn doctor_json_reports_windows_source_old_pure() {
    let dir = temp_home();
    let d = dir.path();
    // no copy
    let c = windows_source_old_check(d, true);
    assert_eq!(c.status, Status::Skip);
    assert_eq!(
        c.message,
        format!(
            "no launcher copy at {}",
            d.join("opencode-pair.exe").display()
        )
    );
    // not Windows
    let c = windows_source_old_check(d, false);
    assert_eq!(c.status, Status::Skip);
    assert_eq!(c.message, "not applicable off Windows");
    // a copy with a good .source and no .old
    fs::write(d.join("opencode-pair.exe"), "x").unwrap();
    fs::write(
        d.join("opencode-pair.exe.source"),
        "path=C:\\p\\opencode-ctl-cc-mcp.exe\nversion=0.1.0\n",
    )
    .unwrap();
    let c = windows_source_old_check(d, true);
    assert_eq!(c.status, Status::Ok);
    assert_eq!(
        c.message,
        "opencode-pair.exe.source is valid and no .old leftover exists"
    );
    // a leftover
    fs::write(d.join("opencode-pair.exe.old"), "x").unwrap();
    let c = windows_source_old_check(d, true);
    assert_eq!(c.status, Status::Warn);
    assert_eq!(
        c.message,
        format!(
            "launcher copy problems: {} is left over from an in-use replacement (gc deletes it)",
            d.join("opencode-pair.exe.old").display()
        )
    );
    // an unparsable .source as well
    fs::write(d.join("opencode-pair.exe.source"), "path=\n").unwrap();
    let c = windows_source_old_check(d, true);
    assert_eq!(c.status, Status::Warn);
    assert_eq!(
        c.message,
        format!(
            "launcher copy problems: {} is missing or unparsable (re-run install-launcher); {} is left over from an in-use replacement (gc deletes it)",
            d.join("opencode-pair.exe.source").display(),
            d.join("opencode-pair.exe.old").display()
        )
    );
}

#[cfg(windows)]
#[tokio::test]
async fn doctor_json_reports_windows_source_old() {
    let rig = Rig::new();
    let d = rig.bin_dir();
    expect(
        &rig.run().await,
        "windows_source_old",
        "skip",
        &format!(
            "no launcher copy at {}",
            d.join("opencode-pair.exe").display()
        ),
    );
    fs::write(d.join("opencode-pair.exe"), "x").unwrap();
    fs::write(
        d.join("opencode-pair.exe.source"),
        "path=C:\\p\\opencode-ctl-cc-mcp.exe\nversion=0.1.0\n",
    )
    .unwrap();
    expect(
        &rig.run().await,
        "windows_source_old",
        "ok",
        "opencode-pair.exe.source is valid and no .old leftover exists",
    );
    fs::write(d.join("opencode-pair.exe.old"), "x").unwrap();
    expect(
        &rig.run().await,
        "windows_source_old",
        "warn",
        &format!(
            "launcher copy problems: {} is left over from an in-use replacement (gc deletes it)",
            d.join("opencode-pair.exe.old").display()
        ),
    );
}

#[cfg(not(windows))]
#[tokio::test]
async fn doctor_json_windows_source_old_skips_off_windows() {
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "windows_source_old",
        "skip",
        "not applicable off Windows",
    );
}

#[cfg(unix)]
#[tokio::test]
async fn doctor_json_reports_launcher_link() {
    use std::os::unix::fs::symlink;
    let rig = Rig::new();
    let link = rig.bin_dir().join("opencode-pair");
    let l = link.display().to_string();
    // missing
    let v = rig.run().await;
    expect(
        &v,
        "launcher_link",
        "warn",
        &format!("missing: {l} does not exist; run install-launcher"),
    );
    assert_eq!(check(&v, "launcher_link")["details"]["link"], "missing");
    // fresh
    symlink(Rig::exe(), &link).unwrap();
    let v = rig.run().await;
    expect(
        &v,
        "launcher_link",
        "ok",
        &format!("{l} resolves to the running binary"),
    );
    assert_eq!(check(&v, "launcher_link")["details"]["link"], "fresh");
    // stale: this host's binary name, another location
    fs::remove_file(&link).unwrap();
    symlink("/old/plugin/bin/opencode-ctl-cc-mcp", &link).unwrap();
    let v = rig.run().await;
    expect(
        &v,
        "launcher_link",
        "warn",
        &format!(
            "stale: {l} does not match the running binary (version 0.1.0); re-run install-launcher"
        ),
    );
    // another host's binary
    fs::remove_file(&link).unwrap();
    symlink("/p/bin/opencode-ctl-qwen-mcp", &link).unwrap();
    let v = rig.run().await;
    expect(
        &v,
        "launcher_link",
        "warn",
        &format!("other_host: {l} belongs to another host's binary; run install-launcher --force to take it over"),
    );
    // a regular file
    fs::remove_file(&link).unwrap();
    fs::write(&link, "x").unwrap();
    let v = rig.run().await;
    expect(
        &v,
        "launcher_link",
        "warn",
        &format!("not_a_link: {l} is not a link install-launcher made; it is left alone and install-launcher reports LAUNCHER_CONFLICT"),
    );
    // not offered on the OpenCode host
    let rig = Rig::with(Opts {
        host: Host::Opencode,
        ..Opts::default()
    });
    expect(
        &rig.run().await,
        "launcher_link",
        "skip",
        "not offered on this host",
    );
}

#[tokio::test]
async fn doctor_json_reports_server_url_password() {
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "server_url_password",
        "skip",
        "server_url is not set",
    );
    let rig = Rig::with(Opts {
        settings: Settings {
            server_url: Some("http://remote:1".to_string()),
            ..Settings::default()
        },
        ..Opts::default()
    });
    expect(
        &rig.run().await,
        "server_url_password",
        "warn",
        "server_url is set (http://remote:1) but OPENCODE_SERVER_PASSWORD is not: the CLI cannot authenticate",
    );
    expect(
        &rig.run_env(&[("OPENCODE_SERVER_PASSWORD", " ")]).await,
        "server_url_password",
        "warn",
        "server_url is set (http://remote:1) but OPENCODE_SERVER_PASSWORD is not: the CLI cannot authenticate",
    );
    expect(
        &rig.run_env(&[("OPENCODE_SERVER_PASSWORD", "pw")]).await,
        "server_url_password",
        "ok",
        "server_url is set and OPENCODE_SERVER_PASSWORD is present",
    );
}

#[tokio::test]
async fn doctor_json_reports_opencode_password_set() {
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "opencode_password_set",
        "ok",
        "OPENCODE_PASSWORD is not set",
    );
    expect(
        &rig.run_env(&[("OPENCODE_PASSWORD", "  ")]).await,
        "opencode_password_set",
        "ok",
        "OPENCODE_PASSWORD is not set",
    );
    let v = rig.run_env(&[("OPENCODE_PASSWORD", "s3cret")]).await;
    expect(
        &v,
        "opencode_password_set",
        "warn",
        "OPENCODE_PASSWORD is set: the CLI reads it before OPENCODE_SERVER_PASSWORD",
    );
    assert!(!v.to_string().contains("s3cret"), "the value never appears");
}

#[tokio::test]
async fn doctor_json_reports_log() {
    let rig = Rig::new();
    let logs = rig.ctl().join("logs");
    expect(
        &rig.run().await,
        "log",
        "skip",
        &format!("no log file in {}", logs.display()),
    );
    fs::create_dir_all(&logs).unwrap();
    let old = logs.join("serve-100.log");
    let new = logs.join("serve-200.log");
    fs::write(&old, "ERROR old failure\n").unwrap();
    fs::write(&new, "INFO fine\n").unwrap();
    let f = fs::OpenOptions::new().write(true).open(&old).unwrap();
    f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_000))
        .unwrap();
    expect(
        &rig.run().await,
        "log",
        "ok",
        &format!("newest log {}: no error line", new.display()),
    );
    fs::write(
        &new,
        "2026-10-20T10:00:00Z  INFO fine\n2026-10-20T10:00:01Z ERROR first http://u:pw@h/x\n2026-10-20T10:00:02Z ERROR last bad thing\n2026-10-20T10:00:03Z  INFO after\n",
    )
    .unwrap();
    expect(
        &rig.run().await,
        "log",
        "warn",
        &format!(
            "newest log {}: last error: 2026-10-20T10:00:02Z ERROR last bad thing",
            new.display()
        ),
    );
}

#[tokio::test]
async fn doctor_json_reports_orphaned_controllers() {
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "orphaned_controllers",
        "ok",
        "no pair has a dead controller beside a live TUI",
    );
    // dead controller, live tui: orphan
    write_pair(&rig, PAIR, true, Some("claude"));
    sentinel(&rig, &format!("pair.{PAIR}.g1.tui-pid"), 501);
    fs::write(rig.ctl().join(format!("pair.{PAIR}.claude-exited")), "").unwrap();
    // controller `none` (no controller recorded) is not dead
    write_pair(&rig, OTHER_PAIR, true, None);
    sentinel(&rig, &format!("pair.{OTHER_PAIR}.g1.tui-pid"), 503);
    expect(
        &rig.run().await,
        "orphaned_controllers",
        "warn",
        &format!("controller dead while the TUI is alive for pair(s): {PAIR}"),
    );
    // a dead tui as well: no longer an orphan
    rig.probe.set_dead(501);
    expect(
        &rig.run().await,
        "orphaned_controllers",
        "ok",
        "no pair has a dead controller beside a live TUI",
    );
}

#[tokio::test]
async fn doctor_json_reports_orphaned_workers() {
    let s2 = SID2;
    let rig = Rig::new();
    rig.fake.on(
        "session.active",
        vec![Ok(
            json!({"data": {SID: {}, s2: {}, "ses_zzzzzzzzzzzzzzzzzzzzzzzzzz": {}}}),
        )],
    );
    // SID's pair has a living side; SID2's pair has none; the third has no record
    write_pair(&rig, PAIR, true, Some("claude"));
    sentinel(&rig, &format!("pair.{PAIR}.claude-pid"), 502);
    let r2 = Record {
        pair_id: OTHER_PAIR.to_string(),
        session_id: s2.to_string(),
        directory: PathBuf::from("/w"),
        server_url: None,
        created: 1,
        controller: Some("claude".to_string()),
        generation: 1,
        tab: true,
    };
    write_record_new(&rig.ctl(), &r2).unwrap();
    fs::write(
        rig.ctl().join(format!("pair.{OTHER_PAIR}.claude-exited")),
        "",
    )
    .unwrap();
    fs::write(
        rig.ctl().join(format!("pair.{OTHER_PAIR}.g1.tui-exited")),
        "",
    )
    .unwrap();
    expect(
        &rig.run().await,
        "orphaned_workers",
        "warn",
        &format!("active session(s) whose pair has no living side: {s2}"),
    );
    // all fine
    fs::remove_file(rig.ctl().join(format!("pair.{OTHER_PAIR}.claude-exited"))).unwrap();
    sentinel(&rig, &format!("pair.{OTHER_PAIR}.claude-pid"), 504);
    expect(
        &rig.run().await,
        "orphaned_workers",
        "ok",
        "no active session belongs to a pair without a living side",
    );
    // session.active fails
    let rig = Rig::new();
    rig.fake.on(
        "session.active",
        vec![Err(terr(ErrorCode::CliError, "broken"))],
    );
    expect(
        &rig.run().await,
        "orphaned_workers",
        "warn",
        "cannot list active sessions: broken",
    );
}

#[tokio::test]
async fn doctor_json_reports_settings_notes() {
    let rig = Rig::new();
    expect(
        &rig.run().await,
        "settings_notes",
        "ok",
        "no settings entry was ignored or clamped",
    );
    let notes = vec![
        Note {
            source: Source::UserFile,
            key: "poll_s".to_string(),
            value: "1".to_string(),
            reason: NoteReason::Clamped { min: 2, max: 60 },
        },
        Note {
            source: Source::WorkspaceFile,
            key: "opencode_bin".to_string(),
            value: "/x".to_string(),
            reason: NoteReason::ProtectedInWorkspace,
        },
    ];
    let rig = Rig::with(Opts {
        notes,
        ..Opts::default()
    });
    let v = rig.run().await;
    expect(
        &v,
        "settings_notes",
        "warn",
        "2 settings entries ignored or clamped: user file poll_s=1 (clamped to 2..60); workspace file opencode_bin=/x (protected key in a workspace file)",
    );
    assert_eq!(v["notes"].as_array().unwrap().len(), 2);
    assert_eq!(v["notes"][0]["key"], "poll_s");
    assert_eq!(v["notes"][0]["source"], "user file");
}

#[tokio::test]
async fn doctor_json_reports_settings_file_hash() {
    use sha2::{Digest, Sha256};
    let rig = Rig::new();
    let f = rig.ctl().join("settings.local.md");
    expect(
        &rig.run().await,
        "settings_file_hash",
        "skip",
        &format!("no user-level settings file at {}", f.display()),
    );
    fs::write(&f, "---\npoll_s: 5\n---\n").unwrap();
    let t = std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_790_000_000_123);
    fs::OpenOptions::new()
        .write(true)
        .open(&f)
        .unwrap()
        .set_modified(t)
        .unwrap();
    let hash: String = Sha256::digest(b"---\npoll_s: 5\n---\n")
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let v = rig.run().await;
    expect(
        &v,
        "settings_file_hash",
        "ok",
        &format!(
            "{}: sha256 {hash}, modified 1790000000123 ms since the epoch",
            f.display()
        ),
    );
    assert_eq!(
        check(&v, "settings_file_hash")["details"]["sha256"],
        hash.as_str()
    );
}

// ------------------------------------------------------------------ host, args, link table

#[tokio::test]
async fn doctor_json_host_flag_narrows() {
    let rig = Rig::with(Opts {
        host: Host::Opencode,
        ..Opts::default()
    });
    expect(
        &rig.run().await,
        "launcher_link",
        "skip",
        "not offered on this host",
    );
    let a = parse_doctor_args(&["verbose".into(), "--host".into(), "opencode".into()]);
    let b = parse_doctor_args(&["--host".into(), "opencode".into(), "verbose".into()]);
    assert_eq!(
        a.as_ref().unwrap(),
        &DoctorArgs {
            verbose: true,
            host_arg: Some("opencode".to_string())
        }
    );
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(
        parse_doctor_args(&[]).unwrap(),
        DoctorArgs {
            verbose: false,
            host_arg: None
        }
    );
    let e = parse_doctor_args(&["bogus".into()]).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument);
    let e = parse_doctor_args(&["--host".into()]).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument);
}

#[test]
fn launcher_link_status_table_windows_rules() {
    let exe = Path::new("C:\\p\\opencode-ctl-cc-mcp.exe");
    let src = |p: &str, v: &str| SourceFile {
        path: p.to_string(),
        version: v.to_string(),
    };
    let same = src("C:\\p\\opencode-ctl-cc-mcp.exe", "0.1.0");
    assert_eq!(
        windows_link_status(true, Some(&same), exe, Host::Claude, "0.1.0"),
        LinkStatus::Fresh
    );
    // version differs
    assert_eq!(
        windows_link_status(true, Some(&same), exe, Host::Claude, "0.2.0"),
        LinkStatus::Stale
    );
    // path differs, this host's binary name
    let moved = src("C:\\old\\opencode-ctl-cc-mcp.exe", "0.1.0");
    assert_eq!(
        windows_link_status(true, Some(&moved), exe, Host::Claude, "0.1.0"),
        LinkStatus::Stale
    );
    // another host's staged binary
    let other = src("C:\\q\\opencode-ctl-qwen-mcp.exe", "0.1.0");
    assert_eq!(
        windows_link_status(true, Some(&other), exe, Host::Claude, "0.1.0"),
        LinkStatus::OtherHost
    );
    // a staged name nobody knows
    let alien = src("C:\\q\\tool.exe", "0.1.0");
    assert_eq!(
        windows_link_status(true, Some(&alien), exe, Host::Claude, "0.1.0"),
        LinkStatus::NotALink
    );
    // no copy; a copy without a parsable .source
    assert_eq!(
        windows_link_status(false, Some(&same), exe, Host::Claude, "0.1.0"),
        LinkStatus::Missing
    );
    assert_eq!(
        windows_link_status(true, None, exe, Host::Claude, "0.1.0"),
        LinkStatus::NotALink
    );
}

#[test]
fn launcher_link_status_table_source_parse() {
    assert_eq!(
        parse_source_file("path=C:\\a=b\\x.exe\nversion=0.1.0\n"),
        Some(SourceFile {
            path: "C:\\a=b\\x.exe".to_string(),
            version: "0.1.0".to_string()
        })
    );
    assert_eq!(
        parse_source_file("version=1\r\npath=/p\r\n"),
        Some(SourceFile {
            path: "/p".to_string(),
            version: "1".to_string()
        })
    );
    assert_eq!(parse_source_file("path=/p\n"), None);
    assert_eq!(parse_source_file("path=\nversion=1\n"), None);
    assert_eq!(parse_source_file(""), None);
}

#[cfg(unix)]
#[test]
fn launcher_link_status_table() {
    use std::os::unix::fs::symlink;
    let dir = temp_home();
    let d = dir.path();
    let exe = Path::new("/plug/bin/opencode-ctl-cc-mcp");
    let st = |host| launcher_link_status(d, exe, host, "0.1.0");
    let link = d.join("opencode-pair");
    assert_eq!(st(Host::Claude), LinkStatus::Missing);
    symlink(exe, &link).unwrap();
    assert_eq!(st(Host::Claude), LinkStatus::Fresh);
    fs::remove_file(&link).unwrap();
    symlink("/old/bin/opencode-ctl-cc-mcp", &link).unwrap();
    assert_eq!(st(Host::Claude), LinkStatus::Stale);
    assert_eq!(
        st(Host::Qwen),
        LinkStatus::OtherHost,
        "seen from Qwen the cc link is another host's"
    );
    fs::remove_file(&link).unwrap();
    symlink("/q/bin/opencode-ctl-qwen-mcp", &link).unwrap();
    assert_eq!(st(Host::Claude), LinkStatus::OtherHost);
    assert_eq!(st(Host::Qwen), LinkStatus::Stale);
    fs::remove_file(&link).unwrap();
    symlink("/elsewhere/some-tool", &link).unwrap();
    assert_eq!(st(Host::Claude), LinkStatus::NotALink);
    fs::remove_file(&link).unwrap();
    fs::write(&link, "x").unwrap();
    assert_eq!(st(Host::Claude), LinkStatus::NotALink);
    // a dangling link to the running binary's path is still Fresh by path
    fs::remove_file(&link).unwrap();
    symlink(exe, &link).unwrap();
    assert_eq!(st(Host::Claude), LinkStatus::Fresh);
}

fn disabled_msg(rig: &Rig) -> String {
    format!(
        "service disabled in {}: every CLI call runs a private standalone server",
        rig.cfg().display()
    )
}

#[tokio::test]
async fn registration_disabled_wins_over_absent_file() {
    let rig = Rig::new();
    fs::write(rig.cfg(), json!({"disabled": true}).to_string()).unwrap();
    let missing = rig.home.path().join("state").join("absent.json");
    let v = rig.run_with(&[], Some(&missing), false).await;
    expect(&v, "registration", "warn", &disabled_msg(&rig));
    no_secret(&v);
}

#[tokio::test]
async fn registration_disabled_wins_over_dead_pid() {
    let rig = Rig::new();
    fs::write(rig.cfg(), json!({"disabled": true}).to_string()).unwrap();
    rig.probe.set_dead(4242);
    let v = rig.run().await;
    expect(&v, "registration", "warn", &disabled_msg(&rig));
    no_secret(&v);
}

#[tokio::test]
async fn registration_disabled_with_live_pid() {
    let rig = Rig::new();
    fs::write(rig.cfg(), json!({"disabled": true}).to_string()).unwrap();
    let v = rig.run().await;
    expect(&v, "registration", "warn", &disabled_msg(&rig));
    no_secret(&v);
    assert!(check(&v, "registration").get("details").is_none());
}
