//! Task 12: the mock `opencode` binary (`mock-opencode-ctl`) and its library helpers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use opencode_ctl::testing::mock::{
    env_presence_line, fixture_key, parse_mode, percent_encode, select_fixture, UNREACHABLE_STDERR,
};
use opencode_ctl::testing::{temp_home, TempHome};
use serde_json::{json, Value};

const MOCK: &str = env!("CARGO_BIN_EXE_mock-opencode-ctl");
const SES: &str = "ses_aaaaaaaaaaaaaaaaaaaaaaaaaa";

fn starter_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct Rig {
    state: TempHome,
    fixtures: PathBuf,
    _copy: Option<TempHome>,
    env: Vec<(String, String)>,
}

impl Rig {
    fn new() -> Self {
        Self {
            state: temp_home(),
            fixtures: starter_fixtures(),
            _copy: None,
            env: Vec::new(),
        }
    }

    /// A rig over an empty fixtures dir (with an empty `mock/`) that the test fills itself.
    fn empty() -> Self {
        let copy = temp_home();
        fs::create_dir_all(copy.path().join("mock")).unwrap();
        Self {
            state: temp_home(),
            fixtures: copy.path().to_path_buf(),
            _copy: Some(copy),
            env: Vec::new(),
        }
    }

    fn fixture(&self, name: &str, body: &str) {
        fs::write(self.fixtures.join("mock").join(name), body).unwrap();
    }

    fn env(mut self, k: &str, v: &str) -> Self {
        self.env.push((k.into(), v.into()));
        self
    }

    fn mode(self, spec: &str) -> Self {
        self.env("MOCK_OPENCODE_MODE", spec)
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut c = Command::new(MOCK);
        c.env_clear()
            .env("MOCK_OPENCODE_FIXTURES", &self.fixtures)
            .env("MOCK_OPENCODE_STATE_DIR", self.state.path())
            .args(args);
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("spawn mock")
    }

    fn argv_log(&self) -> Vec<Value> {
        fs::read_to_string(self.state.path().join("argv.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).expect("argv line is JSON"))
            .collect()
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}
fn code(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

#[test]
fn mock_fixture_key_percent_encoding() {
    assert_eq!(percent_encode("AZaz09_.-"), "AZaz09_.-");
    assert_eq!(percent_encode("/tmp/a b&c=d"), "%2Ftmp%2Fa%20b%26c%3Dd");
    assert_eq!(percent_encode("é"), "%C3%A9");
    assert_eq!(percent_encode("~"), "%7E");
    // Values are encoded, names are not (the literal brackets stay), pairs sort by name.
    let key = fixture_key(
        "vcs.get",
        &[
            ("location[directory]".to_string(), "/tmp/w x".to_string()),
            ("a".to_string(), "1".to_string()),
        ],
    );
    assert_eq!(key, "vcs.get__a=1&location[directory]=%2Ftmp%2Fw%20x");
    assert_eq!(fixture_key("server.info", &[]), "server.info");
    assert_eq!(
        parse_mode("server.info:unreachable,service.status:stopped"),
        vec![
            ("server.info".to_string(), "unreachable".to_string()),
            ("service.status".to_string(), "stopped".to_string())
        ]
    );
}

#[test]
fn mock_lookup_most_specific_first() {
    let dir = temp_home();
    let mock = dir.path().join("mock");
    fs::create_dir_all(&mock).unwrap();
    let p = vec![("a".to_string(), "1".to_string())];
    assert!(select_fixture(dir.path(), "x.op", &p, 1).is_none());
    for name in [
        "x.op.json",
        "x.op__1.json",
        "x.op__a=1.json",
        "x.op__a=1__1.json",
    ] {
        fs::write(mock.join(name), "{}").unwrap();
    }
    let name = |n| {
        select_fixture(dir.path(), "x.op", &p, n)
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    };
    assert_eq!(name(1), "x.op__a=1__1.json");
    // Past the last numbered file the last one repeats.
    assert_eq!(name(5), "x.op__a=1__1.json");
    fs::remove_file(mock.join("x.op__a=1__1.json")).unwrap();
    assert_eq!(name(1), "x.op__a=1.json");
    fs::remove_file(mock.join("x.op__a=1.json")).unwrap();
    assert_eq!(name(1), "x.op__1.json");
    assert_eq!(name(9), "x.op__1.json");
    fs::remove_file(mock.join("x.op__1.json")).unwrap();
    assert_eq!(name(1), "x.op.json");
    // Params that no file names fall through to the bare operation.
    let other = vec![("b".to_string(), "2".to_string())];
    assert!(select_fixture(dir.path(), "x.op", &other, 1)
        .unwrap()
        .ends_with("x.op.json"));
}

#[test]
fn mock_sequence_counter_atomic() {
    let rig = Rig::empty();
    for i in 1..=8 {
        rig.fixture(&format!("x.seq__{i}.json"), &format!("{{\"n\":{i}}}"));
    }
    let children: Vec<_> = (0..8)
        .map(|_| {
            rig.command(&["api", "x.seq"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let mut seen: Vec<i64> = children
        .into_iter()
        .map(|c| {
            let o = c.wait_with_output().unwrap();
            assert_eq!(code(&o), 0, "{}", err(&o));
            serde_json::from_str::<Value>(&out(&o)).unwrap()["n"]
                .as_i64()
                .unwrap()
        })
        .collect();
    seen.sort();
    assert_eq!(seen, (1..=8).collect::<Vec<_>>());
    assert_eq!(rig.argv_log().len(), 8, "no interleaved or lost log lines");
}

#[test]
fn mock_now_expansion() {
    let rig = Rig::empty().env("MOCK_OPENCODE_NOW_MS", "1000000");
    rig.fixture(
        "x.now.json",
        r#"{"a":"{{now-60000}}","b":"{{now+5}}","c":"{{now}}","d":["{{now-1}}"],"e":"keep {{now}}"}"#,
    );
    let o = rig.run(&["api", "x.now"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let v: Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["a"], json!(940000));
    assert_eq!(v["b"], json!(1000005));
    assert_eq!(v["c"], json!(1000000));
    assert_eq!(v["d"], json!([999999]));
    assert_eq!(
        v["e"],
        json!("keep {{now}}"),
        "only whole-string placeholders expand"
    );
}

#[test]
fn mock_type_idle_is_400() {
    let rig = Rig::new();
    let o = rig.run(&[
        "api",
        "session.message.list",
        "--param",
        &format!("sessionID={SES}"),
        "--param",
        "type=idle",
    ]);
    assert_eq!(code(&o), 1);
    assert_eq!(
        out(&o),
        r#"{"_tag":"InvalidRequestError","message":"invalid type"}"#
    );
    assert_eq!(err(&o), "HTTP 400");
}

#[test]
fn mock_cursor_with_order_is_400() {
    let rig = Rig::new();
    let o = rig.run(&[
        "api",
        "session.message.list",
        "--param",
        &format!("sessionID={SES}"),
        "--param",
        "cursor=abc",
        "--param",
        "order=asc",
    ]);
    assert_eq!(code(&o), 1);
    assert_eq!(
        out(&o),
        r#"{"_tag":"InvalidCursorError","message":"order not allowed with cursor"}"#
    );
    assert_eq!(err(&o), "HTTP 400");
}

#[test]
fn mock_body_required_validated() {
    let rig = Rig::new();
    let args = |body: &'static str| {
        [
            "api",
            "session.permission.reply",
            "--param",
            "sessionID=ses_aaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--param",
            "requestID=per_1",
            "-d",
            body,
        ]
    };
    let bad = rig.run(&args(r#"{"reply":"once"}"#));
    assert_eq!(code(&bad), 1);
    assert_eq!(
        out(&bad),
        r#"{"_tag":"InvalidRequestError","message":"missing decision"}"#
    );
    assert_eq!(err(&bad), "HTTP 400");
    let ok = rig.run(&args(r#"{"decision":"once"}"#));
    assert_eq!(code(&ok), 0, "{}", err(&ok));
    assert_eq!(out(&ok), "");
}

#[cfg(unix)]
fn big_len(o: &Output) -> usize {
    o.stdout.len()
}

#[cfg(unix)]
#[test]
fn mock_big_truncates_on_pipe_only() {
    let rig = Rig::new().mode("x.big:big");
    let piped = rig.run(&["api", "x.big"]);
    assert_eq!(code(&piped), 0);
    assert_eq!(big_len(&piped), 262_144);

    let file_path = rig.state.path().join("stdout.bin");
    let status = rig
        .command(&["api", "x.big"])
        .stdout(fs::File::create(&file_path).unwrap())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    let body = fs::read(&file_path).unwrap();
    assert_eq!(body.len(), 300_000);
    let v: Value = serde_json::from_slice(&body).expect("file stdout is a whole JSON array");
    assert!(v.is_array());

    let rig = Rig::new()
        .mode("x.big:big")
        .env("MOCK_OPENCODE_BIG_BYTES", "40000000");
    let file_path = rig.state.path().join("stdout.bin");
    let status = rig
        .command(&["api", "x.big"])
        .stdout(fs::File::create(&file_path).unwrap())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(0));
    assert_eq!(fs::metadata(&file_path).unwrap().len(), 40_000_000);

    let log = rig.argv_log();
    assert_eq!(log[0]["stdout_kind"], "file");
    let piped_rig_log = Rig::new().mode("x.big:big");
    piped_rig_log.run(&["api", "x.big"]);
    assert_eq!(piped_rig_log.argv_log()[0]["stdout_kind"], "pipe");
}

#[cfg(unix)]
#[test]
fn mock_slow_writes_grandchild_pid() {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let rig = Rig::new().mode("x.slow:slow");
    let mut child = rig
        .command(&["api", "x.slow"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid_file = rig.state.path().join("grandchild.pid");
    let mut pid = None;
    for _ in 0..200 {
        if let Ok(s) = fs::read_to_string(&pid_file) {
            if let Ok(p) = s.trim().parse::<i32>() {
                pid = Some(p);
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let pid = pid.expect("grandchild.pid written");
    assert!(
        kill(Pid::from_raw(pid), None).is_ok(),
        "grandchild is alive"
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "the mock never returns"
    );
    child.kill().unwrap();
    child.wait().unwrap();
    // The grandchild outlives its parent (that is the point); clean it up.
    assert!(
        kill(Pid::from_raw(pid), None).is_ok(),
        "grandchild outlives the mock"
    );
    let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
}

#[test]
fn mock_argv_log_env_presence_not_value() {
    let rig = Rig::new()
        .env("OPENCODE_PASSWORD", "hunter2")
        .env("OPENCODE_CTL_SESSION", SES)
        .env("UNRELATED_SECRET", "zzz");
    let o = rig.run(&["api", "server.info", "--server", "http://127.0.0.1:49374"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let raw = fs::read_to_string(rig.state.path().join("argv.jsonl")).unwrap();
    assert!(!raw.contains("hunter2"), "the value is never logged");
    assert!(!raw.contains("UNRELATED_SECRET"));
    let log = rig.argv_log();
    assert_eq!(log.len(), 1);
    let line = &log[0];
    assert_eq!(
        line["argv"],
        json!(["api", "server.info", "--server", "http://127.0.0.1:49374"])
    );
    let pw = &line["env"]["OPENCODE_PASSWORD"];
    assert_eq!(pw["present"], true);
    assert_eq!(pw["len"], 7);
    // sha256("hunter2") begins f52fbd32.
    assert_eq!(pw["sha8"], "f52fbd32");
    assert_eq!(line["env"]["OPENCODE_SERVER_PASSWORD"]["present"], false);
    assert_eq!(line["env"]["OPENCODE_CTL_SESSION"]["len"], SES.len());
    assert!(line["stdout_kind"] == "pipe" || line["stdout_kind"] == "file");

    // The pure helper produces the same shape.
    let l = env_presence_line(
        &["x".to_string()],
        &[("OPENCODE_SERVER_PASSWORD".to_string(), "p".to_string())],
        "file",
    );
    let v: Value = serde_json::from_str(&l).unwrap();
    assert_eq!(v["env"]["OPENCODE_SERVER_PASSWORD"]["present"], true);
    assert_eq!(v["env"]["OPENCODE_PASSWORD"]["present"], false);
    assert_eq!(v["stdout_kind"], "file");
    assert!(!l.contains('\n'));
}

#[test]
fn mock_openapi_route() {
    let rig = Rig::new();
    let o = rig.run(&["api", "GET", "/openapi.json"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(
        o.stdout,
        fs::read(starter_fixtures().join("openapi.json")).unwrap()
    );
}

#[test]
fn mock_missing_fixture_tag() {
    let rig = Rig::new();
    let o = rig.run(&["api", "no.such.op"]);
    assert_eq!(code(&o), 1);
    assert_eq!(
        out(&o),
        r#"{"_tag":"MockMissingFixture","key":"no.such.op"}"#
    );
    let o = rig.run(&["api", "no.such.op", "--param", "k=a b"]);
    assert_eq!(
        out(&o),
        r#"{"_tag":"MockMissingFixture","key":"no.such.op__k=a%20b"}"#
    );
}

#[test]
fn mock_requires_state_dir() {
    let o = Command::new(MOCK)
        .env_clear()
        .env("MOCK_OPENCODE_FIXTURES", starter_fixtures())
        .args(["api", "server.info"])
        .output()
        .unwrap();
    assert_eq!(code(&o), 2);
    assert!(err(&o).contains("MOCK_OPENCODE_STATE_DIR"), "{}", err(&o));
}

// ---- Tests below derive their assertions from the brief's rule text. ----

#[test]
fn mock_version_and_service_defaults() {
    let rig = Rig::empty();
    let o = rig.run(&["--version"]);
    assert_eq!(
        (code(&o), out(&o).trim().to_string()),
        (0, "opencode v2.0.32".into())
    );
    // Missing service fixtures: status prints the URL, the rest print nothing.
    let o = rig.run(&["service", "status"]);
    assert_eq!(
        (code(&o), out(&o).trim().to_string()),
        (0, "http://127.0.0.1:49374".into())
    );
    let o = rig.run(&["service", "stop"]);
    assert_eq!((code(&o), out(&o)), (0, String::new()));
    let o = rig.run(&["service", "bogus"]);
    assert_ne!(code(&o), 0);
    let rig = Rig::new().mode("service.status:stopped");
    let o = rig.run(&["service", "status"]);
    assert_eq!(
        (code(&o), out(&o), err(&o)),
        (0, "stopped".into(), String::new())
    );
}

#[test]
fn mock_starter_fixtures_serve_the_engine_reads() {
    let rig = Rig::new();
    let info: Value =
        serde_json::from_str(&out(&rig.run(&["api", "server.info"]))).expect("server.info JSON");
    assert_eq!(info["version"], "2.0.32");
    assert_eq!(info["pid"], 4242);
    let o = rig.run(&["service", "start"]);
    assert_eq!(out(&o).trim(), "http://127.0.0.1:49374");
    let list: Value = serde_json::from_str(&out(&rig.run(&["api", "session.list"]))).unwrap();
    assert_eq!(
        list,
        json!({"data":[],"cursor":{"previous":null,"next":null}})
    );
    let get: Value = serde_json::from_str(&out(&rig.run(&[
        "api",
        "session.get",
        "--param",
        &format!("sessionID={SES}"),
    ])))
    .unwrap();
    assert_eq!(get["data"]["id"], SES);
    assert_eq!(get["data"]["location"]["directory"], "/tmp/w");
    let vcs = rig.run(&["api", "vcs.get", "--param", "location[directory]=/tmp/w"]);
    let vcs: Value = serde_json::from_str(&out(&vcs)).unwrap();
    assert_eq!(vcs["data"]["provider"], "git");
    let st = rig.run(&["api", "vcs.status", "--param", "location[directory]=/tmp/w"]);
    assert_eq!(
        serde_json::from_str::<Value>(&out(&st)).unwrap(),
        json!({"location":{"directory":"/tmp/w"},"data":[]})
    );
    // The user-only message variant is selected by its exact params.
    let m = rig.run(&[
        "api",
        "session.message.list",
        "--param",
        "limit=1",
        "--param",
        "order=desc",
        "--param",
        &format!("sessionID={SES}"),
        "--param",
        "type=user",
    ]);
    let m: Value = serde_json::from_str(&out(&m)).unwrap();
    assert_eq!(m["data"].as_array().unwrap().len(), 1);
    assert_eq!(m["data"][0]["type"], "user");
    assert!(
        m["data"][0]["time"]["created"].is_number(),
        "now placeholders expand"
    );
    let p = rig.run(&[
        "api",
        "session.prompt",
        "--param",
        &format!("sessionID={SES}"),
        "-d",
        r#"{"text":"hi"}"#,
    ]);
    assert_eq!(code(&p), 0, "{}", err(&p));
    assert_eq!(
        serde_json::from_str::<Value>(&out(&p)).unwrap()["id"],
        "msg_u2"
    );
    for op in [
        "session.active",
        "session.inbox.list",
        "session.permission.list",
        "session.form.list",
    ] {
        let o = rig.run(&["api", op, "--param", &format!("sessionID={SES}")]);
        assert_eq!(code(&o), 0, "{op}: {}", err(&o));
        serde_json::from_str::<Value>(&out(&o)).expect(op);
    }
    let w = rig.run(&["api", "experimental.session.wait", "-d", "{}"]);
    assert_eq!((code(&w), out(&w)), (0, String::new()));
}

#[test]
fn mock_failure_mode_table() {
    let cases: &[(&str, i32, &str, &str)] = &[
        (
            "fail",
            1,
            r#"{"_tag":"UnknownError","message":"mock fail"}"#,
            "HTTP 500 Internal Server Error",
        ),
        (
            "unmapped",
            1,
            "",
            "Error: something the mapping table does not know",
        ),
        ("garbage", 0, "not json", ""),
        ("silent", 0, "", ""),
        (
            "not-found",
            1,
            r#"{"_tag":"SessionNotFoundError","message":"session not found"}"#,
            "HTTP 404 Not Found",
        ),
        (
            "invalid-request",
            1,
            r#"{"_tag":"InvalidRequestError","message":"bad request"}"#,
            "HTTP 400 Bad Request",
        ),
        (
            "409",
            1,
            r#"{"_tag":"ConflictError","message":"busy"}"#,
            "HTTP 409 Conflict",
        ),
        (
            "synthetic-conflict",
            1,
            r#"{"_tag":"ConflictError","message":"Synthetic input ID conflicts with an existing durable record: msg_x"}"#,
            "HTTP 409 Conflict",
        ),
        (
            "404-vanished",
            1,
            r#"{"_tag":"PermissionNotFoundError","message":"gone"}"#,
            "HTTP 404 Not Found",
        ),
        (
            "503",
            1,
            r#"{"_tag":"ServiceUnavailableError","message":"unavailable"}"#,
            "HTTP 503 Service Unavailable",
        ),
        (
            "unauthorized",
            1,
            "",
            "Error: Server at http://x rejected the password",
        ),
    ];
    for (mode, exit, stdout, stderr) in cases {
        let rig = Rig::new().mode(&format!("server.info:{mode}"));
        let o = rig.run(&["api", "server.info"]);
        assert_eq!(
            (code(&o), out(&o), err(&o)),
            (*exit, stdout.to_string(), stderr.to_string()),
            "mode {mode}"
        );
        // A mode keyed to another operation does not apply.
        let o = rig.run(&["api", "session.list"]);
        assert_eq!(code(&o), 0, "mode {mode} leaked");
    }
    for (mode, cause) in [
        ("unknown-op", "[cause]: Error: Operation not found: nope"),
        (
            "unknown-error",
            "[cause]: Error: Missing path parameter: sessionID",
        ),
    ] {
        let rig = Rig::new().mode(&format!("server.info:{mode}"));
        let o = rig.run(&["api", "server.info"]);
        assert_eq!((code(&o), out(&o)), (1, String::new()), "{mode}");
        let e = err(&o);
        assert!(e.contains("UnknownError"), "{e}");
        let causes: Vec<_> = e.lines().filter(|l| l.contains("[cause]:")).collect();
        assert_eq!(causes.len(), 1, "{e}");
        assert_eq!(causes[0].trim(), cause);
    }
    let rig = Rig::new().mode("server.info:unreachable");
    let o = rig.run(&["api", "server.info"]);
    assert_eq!(
        (code(&o), out(&o), err(&o)),
        (1, String::new(), UNREACHABLE_STDERR.to_string())
    );
    assert!(UNREACHABLE_STDERR.starts_with("Error: Could not reach server at http://127.0.0.1:1\n"));
    assert!(UNREACHABLE_STDERR.contains(
        "[cause]: ClientError: Transport: Unable to connect. Is the computer able to access the url?"
    ));
    assert!(UNREACHABLE_STDERR.contains("[cause]: TypeError:"));
}

#[test]
fn mock_poll_modes_turn_non_empty_after_n_calls() {
    for (mode, op) in [
        (
            "session.permission.list:permission",
            "session.permission.list",
        ),
        ("session.form.list:form", "session.form.list"),
    ] {
        let rig = Rig::new().mode(mode).env("MOCK_OPENCODE_AFTER_POLLS", "2");
        let sizes: Vec<usize> = (0..4)
            .map(|_| {
                let o = rig.run(&["api", op, "--param", &format!("sessionID={SES}")]);
                assert_eq!(code(&o), 0, "{}", err(&o));
                let v: Value = serde_json::from_str(&out(&o)).unwrap();
                v["data"].as_array().unwrap().len()
            })
            .collect();
        assert_eq!(sizes[..2], [0, 0], "{op}");
        assert!(sizes[2] > 0 && sizes[3] > 0, "{op}: {sizes:?}");
    }
}

#[test]
fn mock_fixture_file_formats() {
    let rig = Rig::empty();
    rig.fixture(
        "x.full.json",
        r#"{"exit":3,"stdout":{"a":1},"stderr":"boom"}"#,
    );
    rig.fixture("x.bare.json", r#"{"a":[1,2]}"#);
    let o = rig.run(&["api", "x.full"]);
    assert_eq!(
        (code(&o), out(&o), err(&o)),
        (3, r#"{"a":1}"#.into(), "boom".into())
    );
    let o = rig.run(&["api", "x.bare"]);
    assert_eq!((code(&o), out(&o)), (0, r#"{"a":[1,2]}"#.into()));
}
