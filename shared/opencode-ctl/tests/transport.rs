//! Task 13: `CliTransport` driven against the mock `opencode` binary (`mock-opencode-ctl`).
//! Nothing here touches the process environment, the real home or a real `opencode`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use opencode_ctl::clock::SystemClock;
use opencode_ctl::engine::{Engine, EnvSnapshot};
use opencode_ctl::error::ErrorCode;
use opencode_ctl::host::Host;
use opencode_ctl::liveness::sweep_stdout_dirs;
use opencode_ctl::openapi::{Operation, OperationTable};
use opencode_ctl::settings::Settings;
use opencode_ctl::testing::mock::UNREACHABLE_STDERR;
use opencode_ctl::testing::{temp_home, FakeProbe, TempHome};
use opencode_ctl::tools::{SessionWaitArgs, WorkspaceStatusArgs};
use opencode_ctl::transport::cli::{
    map_cli_failure, npm_shim_exe, resolve_binary, CliTransport, Mapped, STDOUT_READ_CEILING,
};
use opencode_ctl::transport::{
    parse_cli_version, service_running, Lifecycle, Transport, TransportError, WaitOutcome,
};
use serde_json::{json, Value};

const MOCK: &str = env!("CARGO_BIN_EXE_mock-opencode-ctl");
const SES: &str = "ses_aaaaaaaaaaaaaaaaaaaaaaaaaa";

fn starter_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn op(id: &str) -> Operation {
    OperationTable::snapshot()
        .get(id)
        .unwrap_or_else(|| panic!("no operation {id}"))
        .clone()
}

fn p(k: &str, v: &str) -> (String, String) {
    (k.to_string(), v.to_string())
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

struct Rig {
    home: TempHome,
    user_home: TempHome,
    state: TempHome,
    fixtures: PathBuf,
    _fx_copy: Option<TempHome>,
    env: Vec<(String, String)>,
    settings: Settings,
}

impl Rig {
    fn new() -> Self {
        let settings = Settings {
            opencode_bin: Some(PathBuf::from(MOCK)),
            call_timeout_s: 5,
            ..Settings::default()
        };
        Self {
            home: temp_home(),
            user_home: temp_home(),
            state: temp_home(),
            fixtures: starter_fixtures(),
            _fx_copy: None,
            env: Vec::new(),
            settings,
        }
    }

    /// A per-test copy of the starter fixture set that the test may extend.
    fn with_fixture_copy(mut self) -> Self {
        let copy = temp_home();
        copy_dir(&starter_fixtures(), copy.path());
        self.fixtures = copy.path().to_path_buf();
        self._fx_copy = Some(copy);
        self
    }

    fn fixture(&self, name: &str, body: &str) {
        fs::write(self.fixtures.join("mock").join(name), body).unwrap();
    }

    fn mode(self, spec: &str) -> Self {
        self.env("MOCK_OPENCODE_MODE", spec)
    }

    fn env(mut self, k: &str, v: &str) -> Self {
        self.env.push((k.into(), v.into()));
        self
    }

    fn server(mut self, url: &str) -> Self {
        self.settings.server_url = Some(url.into());
        self
    }

    fn transport(&self) -> CliTransport {
        let mut env = vec![
            p(
                "MOCK_OPENCODE_FIXTURES",
                self.fixtures.to_str().expect("utf8 path"),
            ),
            p(
                "MOCK_OPENCODE_STATE_DIR",
                self.state.path().to_str().expect("utf8 path"),
            ),
        ];
        env.extend(self.env.clone());
        CliTransport::new(&self.settings, self.home.path(), self.user_home.path())
            .with_extra_env(env)
    }

    /// The mock's argv log, one JSON value per spawned call.
    fn log(&self) -> Vec<Value> {
        fs::read_to_string(self.state.path().join("argv.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).expect("argv log line"))
            .collect()
    }

    fn argvs(&self) -> Vec<Vec<String>> {
        self.log()
            .iter()
            .map(|l| {
                l["argv"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect()
            })
            .collect()
    }
}

fn leftover_files(t: &CliTransport) -> usize {
    fs::read_dir(t.stdout_dir()).map(|d| d.count()).unwrap_or(0)
}

fn code_of<T: std::fmt::Debug>(r: Result<T, TransportError>) -> TransportError {
    r.expect_err("expected a TransportError")
}

fn session_params() -> Vec<(String, String)> {
    vec![p("sessionID", SES)]
}

fn tag(e: &TransportError) -> Option<&str> {
    e.details.as_ref()?.get("tag")?.as_str()
}

// ---------------------------------------------------------------- mapping

#[test]
fn transport_error_mapping_table() {
    // Pure: one case per `_tag` named in the table, `LocationNotFoundError` included.
    let cases: &[(&str, ErrorCode)] = &[
        ("SessionNotFoundError", ErrorCode::SessionNotFound),
        ("UnauthorizedError", ErrorCode::AuthFailed),
        ("InvalidRequestError", ErrorCode::InvalidRequest),
        ("InvalidCursorError", ErrorCode::InvalidRequest),
        ("FormInvalidAnswerError", ErrorCode::InvalidRequest),
        ("MessageNotFoundError", ErrorCode::InvalidRequest),
        ("LocationNotFoundError", ErrorCode::InvalidRequest),
        ("ServiceUnavailableError", ErrorCode::ServiceDown),
        ("ConflictError", ErrorCode::Conflict),
        ("SessionBusyError", ErrorCode::Conflict),
        ("FormAlreadySettledError", ErrorCode::Conflict),
        ("PermissionNotFoundError", ErrorCode::Conflict),
        ("FormNotFoundError", ErrorCode::Conflict),
        ("UnknownError", ErrorCode::CliError),
        ("SomethingNew", ErrorCode::CliError),
    ];
    for (t, code) in cases {
        let stdout = json!({"_tag": t, "message": "m"}).to_string();
        match map_cli_failure(Some(1), &stdout, "HTTP 4xx") {
            Mapped::Error(e) => {
                assert_eq!(e.code, *code, "tag {t}");
                assert_eq!(tag(&e), Some(*t), "details.tag for {t}");
            }
            Mapped::ProbeNeeded(_) => panic!("a tagged failure is never probed ({t})"),
        }
    }
    // stderr rules.
    for s in [
        "Error: Server at http://x rejected the password",
        "this requires a password",
    ] {
        let Mapped::Error(e) = map_cli_failure(Some(1), "", s) else {
            panic!("{s}")
        };
        assert_eq!(e.code, ErrorCode::AuthFailed);
        assert!(e.hint.unwrap().contains("opencode service get password"));
    }
    let Mapped::Error(e) =
        map_cli_failure(Some(1), "", "Error: Could not reach server at http://x")
    else {
        panic!()
    };
    assert_eq!(e.code, ErrorCode::ServiceDown);
    // Unmapped: probe needed, carrying the first non-blank stderr line, or the exit code text.
    let Mapped::ProbeNeeded(e) = map_cli_failure(Some(7), "", "\n  \nboom\nsecond") else {
        panic!()
    };
    assert_eq!((e.code, e.message.as_str()), (ErrorCode::CliError, "boom"));
    let Mapped::ProbeNeeded(e) = map_cli_failure(Some(7), "", "") else {
        panic!()
    };
    assert_eq!(e.message, "opencode exited 7 without output");
    // Non-JSON stdout falls through to the stderr rules.
    assert!(matches!(
        map_cli_failure(Some(1), "not json", "x"),
        Mapped::ProbeNeeded(_)
    ));
}

#[tokio::test]
async fn transport_error_mapping_via_mock_modes() {
    let cases: &[(&str, &str, &str, ErrorCode, Option<&str>)] = &[
        (
            "session.get",
            "not-found",
            "",
            ErrorCode::SessionNotFound,
            Some("SessionNotFoundError"),
        ),
        (
            "session.get",
            "invalid-request",
            "",
            ErrorCode::InvalidRequest,
            Some("InvalidRequestError"),
        ),
        (
            "session.get",
            "unauthorized",
            "",
            ErrorCode::AuthFailed,
            None,
        ),
        (
            "session.get",
            "503",
            "",
            ErrorCode::ServiceDown,
            Some("ServiceUnavailableError"),
        ),
        (
            "session.get",
            "unknown-op",
            "",
            ErrorCode::UnknownOperation,
            None,
        ),
        (
            "session.get",
            "unknown-error",
            "",
            ErrorCode::CliError,
            None,
        ),
        (
            "session.get",
            "unreachable",
            "",
            ErrorCode::ServiceDown,
            None,
        ),
        (
            "session.get",
            "fail",
            "service.status:stopped",
            ErrorCode::CliError,
            Some("UnknownError"),
        ),
    ];
    for (opid, mode, extra, code, t) in cases {
        let spec = if extra.is_empty() {
            format!("{opid}:{mode}")
        } else {
            format!("{opid}:{mode},{extra}")
        };
        let rig = Rig::new().mode(&spec);
        let e = code_of(
            rig.transport()
                .call(&op(opid), &session_params(), None, Duration::from_secs(5))
                .await,
        );
        assert_eq!(e.code, *code, "{spec}: {e:?}");
        assert_eq!(tag(&e), *t, "{spec}");
    }
    // BODY_TOO_LARGE and INVALID_ARGUMENT never reach a child.
    let mut rig = Rig::new();
    rig.settings.body_max_bytes = 1024;
    let body = json!({"text": "x".repeat(2000)});
    let e = code_of(
        rig.transport()
            .call(
                &op("session.prompt"),
                &session_params(),
                Some(&body),
                Duration::from_secs(5),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::BodyTooLarge);
    assert!(e.hint.is_some());
    let e = code_of(
        rig.transport()
            .call(
                &op("session.get"),
                &vec![p("nope", "1")],
                None,
                Duration::from_secs(5),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::InvalidArgument);
    assert!(rig.log().is_empty(), "no spawn for a rejected call");
}

#[tokio::test]
async fn transport_204_no_retry_of_mutation() {
    let rig = Rig::new();
    let params = vec![p("sessionID", SES), p("requestID", "per_1")];
    let v = rig
        .transport()
        .call(
            &op("session.permission.reply"),
            &params,
            Some(&json!({"decision": "once"})),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(v, json!({"ok": true}));
    assert_eq!(rig.log().len(), 1);
    // A silent 200 mutation is not retried either.
    let rig = Rig::new().mode("session.prompt:silent");
    let _ = rig
        .transport()
        .call(
            &op("session.prompt"),
            &session_params(),
            Some(&json!({"text": "hi"})),
            Duration::from_secs(5),
        )
        .await;
    assert_eq!(rig.log().len(), 1, "a mutation is never retried");
}

#[tokio::test]
async fn transport_get_empty_or_garbage_stdout_retried_once() {
    for mode in ["silent", "garbage"] {
        let rig = Rig::new().mode(&format!("session.get:{mode}"));
        let e = code_of(
            rig.transport()
                .call(
                    &op("session.get"),
                    &session_params(),
                    None,
                    Duration::from_secs(5),
                )
                .await,
        );
        assert_eq!(e.code, ErrorCode::CliError, "{mode}");
        assert_eq!(e.details.as_ref().unwrap()["reason"], "unparseable stdout");
        assert!(
            e.details.as_ref().unwrap().get("outcome").is_none(),
            "a GET has no unknown outcome"
        );
        assert_eq!(rig.log().len(), 2, "{mode}: one retry, no more");
    }
}

#[tokio::test]
async fn transport_mutation_unknown_outcome() {
    for mode in ["silent", "garbage"] {
        let rig = Rig::new().mode(&format!("session.prompt:{mode}"));
        let e = code_of(
            rig.transport()
                .call(
                    &op("session.prompt"),
                    &session_params(),
                    Some(&json!({"text": "hi"})),
                    Duration::from_secs(5),
                )
                .await,
        );
        assert_eq!(e.code, ErrorCode::CliError);
        let d = e.details.as_ref().unwrap();
        assert_eq!(d["reason"], "unparseable stdout");
        assert_eq!(d["outcome"], "unknown");
        assert!(e.hint.is_some(), "the hint says to check before repeating");
        assert_eq!(rig.log().len(), 1);
    }
}

#[tokio::test]
async fn transport_409_is_conflict() {
    let rig = Rig::new().mode("session.prompt:409");
    let e = code_of(
        rig.transport()
            .call(
                &op("session.prompt"),
                &session_params(),
                Some(&json!({"text": "hi"})),
                Duration::from_secs(5),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::Conflict);
    assert_eq!(tag(&e), Some("ConflictError"));
    // The synthetic-id message is kept verbatim for the launcher.
    let rig = Rig::new().mode("session.prompt:synthetic-conflict");
    let e = code_of(
        rig.transport()
            .call(
                &op("session.prompt"),
                &session_params(),
                Some(&json!({"text": "hi"})),
                Duration::from_secs(5),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::Conflict);
    assert_eq!(
        e.message,
        "Synthetic input ID conflicts with an existing durable record: msg_x"
    );
}

#[tokio::test]
async fn transport_conflict_tags_and_vanished_404() {
    let rig = Rig::new().mode("session.permission.reply:404-vanished");
    let params = vec![p("sessionID", SES), p("requestID", "per_1")];
    let e = code_of(
        rig.transport()
            .call(
                &op("session.permission.reply"),
                &params,
                Some(&json!({"decision": "once"})),
                Duration::from_secs(5),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::Conflict);
    assert_eq!(tag(&e), Some("PermissionNotFoundError"));
    for t in [
        "ConflictError",
        "SessionBusyError",
        "FormAlreadySettledError",
        "PermissionNotFoundError",
        "FormNotFoundError",
    ] {
        let out = json!({"_tag": t, "message": "m"}).to_string();
        let Mapped::Error(e) = map_cli_failure(Some(1), &out, "HTTP 409") else {
            panic!()
        };
        assert_eq!(e.code, ErrorCode::Conflict, "{t}");
    }
}

#[tokio::test]
async fn transport_503_is_service_down() {
    let rig = Rig::new().mode("session.get:503");
    let e = code_of(
        rig.transport()
            .call(
                &op("session.get"),
                &session_params(),
                None,
                Duration::from_secs(5),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::ServiceDown);
    assert_eq!(tag(&e), Some("ServiceUnavailableError"));
    assert_eq!(
        rig.argvs().iter().filter(|a| a[0] == "service").count(),
        0,
        "no probe for a tagged failure"
    );
}

// --------------------------------------------------------------- deadlines

#[tokio::test]
async fn transport_call_timeout_is_deadline() {
    let rig = Rig::new().mode("session.get:slow,session.prompt:slow");
    let t = rig.transport();
    let started = Instant::now();
    let e = code_of(
        t.call(
            &op("session.get"),
            &session_params(),
            None,
            Duration::from_secs(1),
        )
        .await,
    );
    assert_eq!(e.code, ErrorCode::Deadline);
    assert!(
        e.details
            .as_ref()
            .is_none_or(|d| d.get("outcome").is_none()),
        "a GET carries no outcome"
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
    // A mutation's deadline leaves the outcome unknown.
    let e = code_of(
        t.call(
            &op("session.prompt"),
            &session_params(),
            Some(&json!({"text": "hi"})),
            Duration::from_secs(1),
        )
        .await,
    );
    assert_eq!(e.code, ErrorCode::Deadline);
    assert_eq!(e.details.as_ref().unwrap()["outcome"], "unknown");
    assert_eq!(leftover_files(&t), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn transport_kills_grandchild() {
    use nix::errno::Errno;
    use nix::sys::signal;
    use nix::unistd::Pid;
    let rig = Rig::new().mode("session.get:slow");
    let e = code_of(
        rig.transport()
            .call(
                &op("session.get"),
                &session_params(),
                None,
                Duration::from_secs(1),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::Deadline);
    let pid: i32 = fs::read_to_string(rig.state.path().join("grandchild.pid"))
        .expect("the mock wrote the grandchild pid")
        .trim()
        .parse()
        .unwrap();
    let is_zombie = |pid: i32| {
        #[cfg(target_os = "linux")]
        {
            fs::read_to_string(format!("/proc/{pid}/stat"))
                .map(|s| s.rsplit(") ").next().is_some_and(|r| r.starts_with('Z')))
                .unwrap_or(false)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = pid;
            false
        }
    };
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let gone =
            matches!(signal::kill(Pid::from_raw(pid), None), Err(Errno::ESRCH)) || is_zombie(pid);
        if gone {
            break;
        }
        assert!(
            Instant::now() < until,
            "grandchild {pid} must die with the group"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    let out = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
}

#[cfg(windows)]
fn grandchild_pid(rig: &Rig) -> u32 {
    fs::read_to_string(rig.state.path().join("grandchild.pid"))
        .expect("the mock wrote the grandchild pid")
        .trim()
        .parse()
        .unwrap()
}

/// Control for the test below: a call that hits its deadline still kills the whole tree on Windows.
#[cfg(windows)]
#[tokio::test]
async fn transport_kills_grandchild_on_deadline_windows() {
    let rig = Rig::new().mode("session.get:slow");
    let e = code_of(
        rig.transport()
            .call(
                &op("session.get"),
                &session_params(),
                None,
                Duration::from_secs(1),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::Deadline);
    let pid = grandchild_pid(&rig);
    let until = Instant::now() + Duration::from_secs(3);
    while pid_alive(pid) {
        assert!(
            Instant::now() < until,
            "grandchild {pid} must die with the job"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// MEASURED on Windows 11 (Task 25): a managed-mode call that auto-starts the service lost it when the
/// call's job closed, and every later call failed with "Unable to connect". A call that EXITS normally
/// must leave what it started running; only a deadline or the stdout ceiling kills the tree.
#[cfg(windows)]
#[tokio::test]
async fn transport_leaves_the_service_running_after_a_clean_exit_windows() {
    let rig = Rig::new().mode("session.get:leave");
    let v = rig
        .transport()
        .call(
            &op("session.get"),
            &session_params(),
            None,
            Duration::from_secs(10),
        )
        .await
        .expect("the call succeeds");
    assert_eq!(v, json!({}));
    let pid = grandchild_pid(&rig);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let alive = pid_alive(pid);
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output();
    assert!(alive, "the child the call left behind ({pid}) must survive");
}

#[tokio::test]
async fn wait_idle_outcomes() {
    let rig = Rig::new();
    assert_eq!(
        rig.transport()
            .wait_idle(SES, Duration::from_secs(5))
            .await
            .unwrap(),
        WaitOutcome::Idle
    );
    assert_eq!(
        rig.argvs()[0],
        [
            "api",
            "experimental.session.wait",
            "--param",
            &format!("sessionID={SES}")
        ]
    );
    let rig = Rig::new().mode("experimental.session.wait:slow");
    let t = rig.transport();
    assert_eq!(
        t.wait_idle(SES, Duration::from_secs(1)).await.unwrap(),
        WaitOutcome::DeadlineHit
    );
    assert_eq!(leftover_files(&t), 0);
    let rig = Rig::new().mode("experimental.session.wait:503");
    assert_eq!(
        code_of(rig.transport().wait_idle(SES, Duration::from_secs(5)).await).code,
        ErrorCode::ServiceDown
    );
}

// ------------------------------------------------------------ stdout file

#[tokio::test]
async fn transport_reads_large_stdout_whole() {
    let rig = Rig::new().mode("session.list:big");
    let v = rig
        .transport()
        .call(&op("session.list"), &vec![], None, Duration::from_secs(10))
        .await
        .unwrap();
    assert!(v.is_array());
    assert!(
        v.to_string().len() > 262_144,
        "the whole 300000 bytes arrive, not a pipe-cut prefix"
    );
    assert_eq!(rig.log()[0]["stdout_kind"], "file");
}

#[tokio::test]
async fn transport_temp_file_removed() {
    let rig = Rig::new();
    let t = rig.transport();
    t.call(
        &op("session.get"),
        &session_params(),
        None,
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert!(t.stdout_dir().is_dir(), "created on first use");
    assert_eq!(leftover_files(&t), 0);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(t.stdout_dir()).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    let name = t
        .stdout_dir()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        name.starts_with(&format!("{}-", std::process::id())),
        "{name}"
    );
    assert!(t.stdout_dir().parent().unwrap().ends_with("stdout"));
}

#[tokio::test]
async fn stdout_file_removed_on_every_exit_path() {
    let rig = Rig::new().mode(
        "session.list:garbage,session.message.list:fail,session.active:slow,vcs.get:silent,vcs.status:503",
    );
    let t = rig.transport();
    let d = Duration::from_secs(1);
    let _ = t.call(&op("session.get"), &session_params(), None, d).await; // success
    let _ = t.call(&op("session.list"), &vec![], None, d).await; // parse failure
    let _ = t
        .call(&op("session.message.list"), &session_params(), None, d)
        .await; // exit 1
    let _ = t.call(&op("session.active"), &vec![], None, d).await; // deadline kill
    let _ = t
        .call(
            &op("vcs.get"),
            &vec![p("location[directory]", "/tmp/w")],
            None,
            d,
        )
        .await; // silent
    let _ = t
        .call(
            &op("vcs.status"),
            &vec![p("location[directory]", "/tmp/w")],
            None,
            d,
        )
        .await; // 503
    let _ = t
        .call(&op("session.get"), &vec![p("bad", "1")], None, d)
        .await; // rejected before spawn
    let _ = t.fetch_openapi().await;
    let _ = t.lifecycle(Lifecycle::Status).await;
    let _ = t.cli_version().await;
    assert_eq!(leftover_files(&t), 0);

    // A failed spawn (missing binary) leaves nothing behind either.
    let mut rig = Rig::new();
    rig.settings.opencode_bin = Some(rig.user_home.path().join("missing"));
    let t = rig.transport();
    let _ = t.call(&op("session.get"), &session_params(), None, d).await;
    assert_eq!(leftover_files(&t), 0);
}

#[tokio::test]
async fn stdout_ceiling_kills_child() {
    let rig = Rig::new()
        .mode("session.get:big")
        .env("MOCK_OPENCODE_BIG_BYTES", "40000000");
    let t = rig.transport();
    assert_eq!(STDOUT_READ_CEILING, 32 * 1024 * 1024);
    let e = code_of(
        t.call(
            &op("session.get"),
            &session_params(),
            None,
            Duration::from_secs(20),
        )
        .await,
    );
    assert_eq!(e.code, ErrorCode::CliError);
    assert_eq!(e.details.as_ref().unwrap()["reason"], "stdout_ceiling");
    assert_eq!(leftover_files(&t), 0);
}

#[tokio::test]
async fn transport_handles_trailing_newlines() {
    // The real CLI appends EOL to stdout and stderr (api.ts:46,48); the mock does not.
    let rig = Rig::new().with_fixture_copy();
    rig.fixture(
        "session.get.json",
        &json!({"exit": 0, "stdout": "{\"data\":{\"id\":\"x\"}}\n", "stderr": "\n"}).to_string(),
    );
    let v = rig
        .transport()
        .call(
            &op("session.get"),
            &session_params(),
            None,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(v, json!({"data": {"id": "x"}}));
    rig.fixture(
        "session.list.json",
        &json!({"exit": 1, "stdout": "{\"_tag\":\"SessionNotFoundError\",\"message\":\"gone\"}\n", "stderr": "HTTP 404\n"})
            .to_string(),
    );
    let e = code_of(
        rig.transport()
            .call(&op("session.list"), &vec![], None, Duration::from_secs(5))
            .await,
    );
    assert_eq!(
        (e.code, e.message.as_str()),
        (ErrorCode::SessionNotFound, "gone")
    );
    rig.fixture(
        "session.active.json",
        &json!({"exit": 1, "stdout": "", "stderr": "boom\nsecond\n"}).to_string(),
    );
    let rig2 = rig.mode("vcs.get:silent");
    let e = code_of(
        rig2.transport()
            .call(&op("session.active"), &vec![], None, Duration::from_secs(5))
            .await,
    );
    assert_eq!(
        e.message, "boom",
        "no trailing newline in the first stderr line"
    );
    let out = rig2.transport().cli_version().await.unwrap();
    assert_eq!(
        out, "opencode v2.0.32",
        "trimmed (the mock's --version ends in a newline)"
    );
}

// -------------------------------------------------------------- env, argv

#[tokio::test]
async fn transport_password_passthrough_env() {
    let rig = Rig::new()
        .env("OPENCODE_SERVER_PASSWORD", "sekret-pw")
        .env("OPENCODE_PASSWORD", "other-pw");
    rig.transport()
        .call(
            &op("session.get"),
            &session_params(),
            None,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    let log = rig.log();
    assert_eq!(log[0]["env"]["OPENCODE_SERVER_PASSWORD"]["present"], true);
    assert_eq!(log[0]["env"]["OPENCODE_SERVER_PASSWORD"]["len"], 9);
    assert_eq!(log[0]["env"]["OPENCODE_PASSWORD"]["present"], true);
    // with_env_removed wins over the inherited and the extra environment.
    let rig = Rig::new().env("OPENCODE_PASSWORD", "other-pw");
    let t = rig
        .transport()
        .with_env_removed(vec!["OPENCODE_PASSWORD".into()]);
    t.call(
        &op("session.get"),
        &session_params(),
        None,
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(rig.log()[0]["env"]["OPENCODE_PASSWORD"]["present"], false);
}

#[tokio::test]
async fn transport_no_password_in_argv() {
    let rig = Rig::new()
        .env("OPENCODE_SERVER_PASSWORD", "sekret-pw")
        .server("http://127.0.0.1:49374");
    let t = rig.transport();
    t.call(
        &op("session.get"),
        &session_params(),
        None,
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    t.wait_idle(SES, Duration::from_secs(5)).await.unwrap();
    t.fetch_openapi().await.unwrap();
    let raw = fs::read_to_string(rig.state.path().join("argv.jsonl")).unwrap();
    assert!(
        !raw.contains("sekret-pw"),
        "the password never reaches argv or the log"
    );
}

#[test]
fn transport_binary_resolution_order() {
    let root = temp_home();
    let bin = if cfg!(windows) {
        "opencode.exe"
    } else {
        "opencode"
    };
    let mk = |dir: &str| -> PathBuf {
        let d = root.path().join(dir);
        fs::create_dir_all(&d).unwrap();
        d
    };
    let (a, b, home) = (mk("a"), mk("b"), mk("home"));
    let hb = home.join(".opencode/bin");
    fs::create_dir_all(&hb).unwrap();
    let path = std::env::join_paths([&a, &b]).unwrap();
    let path = path.to_str().unwrap();
    // Nothing anywhere.
    let e = resolve_binary(None, Some(path), &home).unwrap_err();
    assert_eq!(e.code, ErrorCode::OpencodeNotFound);
    assert!(e.hint.is_some());
    // Home fallback.
    fs::write(hb.join(bin), "").unwrap();
    assert_eq!(
        resolve_binary(None, Some(path), &home).unwrap(),
        hb.join(bin)
    );
    assert_eq!(resolve_binary(None, None, &home).unwrap(), hb.join(bin));
    // PATH beats home; first entry wins.
    fs::write(b.join(bin), "").unwrap();
    assert_eq!(
        resolve_binary(None, Some(path), &home).unwrap(),
        b.join(bin)
    );
    fs::write(a.join(bin), "").unwrap();
    assert_eq!(
        resolve_binary(None, Some(path), &home).unwrap(),
        a.join(bin)
    );
    // None = no PATH search.
    assert_eq!(resolve_binary(None, None, &home).unwrap(), hb.join(bin));
    // Explicit beats everything, even when it does not exist.
    let explicit = root.path().join("missing");
    assert_eq!(
        resolve_binary(Some(&explicit), Some(path), &home).unwrap(),
        explicit
    );
}

/// An npm install puts `opencode.cmd` on PATH and the real executable under
/// `node_modules\@opencode\cli\bin` (MEASURED on Windows with opencode 2.0.32). The shim is never run:
/// that would pass every argument through `cmd.exe`, which expands `%VAR%` inside prompt text.
fn npm_install(dir: &Path, with_exe: bool) -> PathBuf {
    fs::write(dir.join("opencode.cmd"), "@echo off\r\n").unwrap();
    let exe_dir = dir.join("node_modules/@opencode/cli/bin");
    fs::create_dir_all(&exe_dir).unwrap();
    let exe = exe_dir.join("opencode.exe");
    if with_exe {
        fs::write(&exe, "").unwrap();
    }
    exe
}

#[test]
fn npm_shim_exe_finds_the_real_executable_beside_the_cmd_shim() {
    let root = temp_home();
    let dir = root.path().join("npm");
    fs::create_dir_all(&dir).unwrap();
    assert_eq!(npm_shim_exe(&dir), None, "no shim");
    let exe = npm_install(&dir, false);
    assert_eq!(npm_shim_exe(&dir), None, "shim without the executable");
    fs::write(&exe, "").unwrap();
    assert_eq!(npm_shim_exe(&dir), Some(exe));
    fs::remove_file(dir.join("opencode.cmd")).unwrap();
    assert_eq!(npm_shim_exe(&dir), None, "executable without the shim");
}

#[cfg(windows)]
#[test]
fn resolve_binary_follows_an_npm_cmd_shim_on_windows() {
    let root = temp_home();
    let (npm, other, home) = (
        root.path().join("npm"),
        root.path().join("other"),
        root.path().join("home"),
    );
    for d in [&npm, &other, &home] {
        fs::create_dir_all(d).unwrap();
    }
    let exe = npm_install(&npm, true);
    let path = std::env::join_paths([&other, &npm]).unwrap();
    let path = path.to_str().unwrap();
    assert_eq!(resolve_binary(None, Some(path), &home).unwrap(), exe);
    // A real opencode.exe earlier on PATH still wins over a later npm shim.
    fs::write(other.join("opencode.exe"), "").unwrap();
    assert_eq!(
        resolve_binary(None, Some(path), &home).unwrap(),
        other.join("opencode.exe")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn transport_path_search_via_with_path() {
    let rig = Rig::new();
    let dir = temp_home();
    std::os::unix::fs::symlink(MOCK, dir.path().join("opencode")).unwrap();
    let mut settings = rig.settings.clone();
    settings.opencode_bin = None;
    let empty_home = temp_home();
    let mut env = vec![
        p(
            "MOCK_OPENCODE_FIXTURES",
            starter_fixtures().to_str().unwrap(),
        ),
        p(
            "MOCK_OPENCODE_STATE_DIR",
            rig.state.path().to_str().unwrap(),
        ),
    ];
    env.push(p("X_UNUSED", "1"));
    let t = CliTransport::new(&settings, rig.home.path(), empty_home.path())
        .with_extra_env(env)
        .with_path(Some(dir.path().to_str().unwrap().to_string()));
    t.call(
        &op("session.get"),
        &session_params(),
        None,
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    assert_eq!(
        rig.log().len(),
        1,
        "the mock behind the PATH hit ran exactly once"
    );
}

#[tokio::test]
async fn transport_spawn_failure_is_opencode_not_found() {
    let mut rig = Rig::new();
    let missing = rig.user_home.path().join("missing");
    rig.settings.opencode_bin = Some(missing);
    let t = rig.transport();
    let d = Duration::from_secs(2);
    assert_eq!(
        code_of(t.call(&op("session.get"), &session_params(), None, d).await).code,
        ErrorCode::OpencodeNotFound
    );
    assert_eq!(
        code_of(t.lifecycle(Lifecycle::Status).await).code,
        ErrorCode::OpencodeNotFound
    );
    assert_eq!(
        code_of(t.wait_idle(SES, d).await).code,
        ErrorCode::OpencodeNotFound
    );
    assert_eq!(
        code_of(t.fetch_openapi().await).code,
        ErrorCode::OpencodeNotFound
    );
    assert_eq!(
        code_of(t.cli_version().await).code,
        ErrorCode::OpencodeNotFound
    );
    assert_eq!(leftover_files(&t), 0);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let f = rig.user_home.path().join("not-exec");
        fs::write(&f, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&f, fs::Permissions::from_mode(0o644)).unwrap();
        rig.settings.opencode_bin = Some(f);
        let t = rig.transport();
        assert_eq!(
            code_of(t.call(&op("session.get"), &session_params(), None, d).await).code,
            ErrorCode::OpencodeNotFound
        );
        assert_eq!(leftover_files(&t), 0);
    }
}

#[tokio::test]
async fn transport_session_id_validated_before_spawn() {
    let rig = Rig::new();
    let t = rig.transport();
    let e = code_of(
        t.call(
            &op("session.get"),
            &vec![p("sessionID", "nope")],
            None,
            Duration::from_secs(2),
        )
        .await,
    );
    assert_eq!(e.code, ErrorCode::SessionNotFound);
    assert!(e.hint.is_some());
    assert_eq!(
        code_of(t.wait_idle("nope", Duration::from_secs(2)).await).code,
        ErrorCode::SessionNotFound
    );
    assert!(rig.log().is_empty(), "no spawn");
}

#[tokio::test]
async fn transport_empty_param_rejected_before_spawn() {
    let rig = Rig::new();
    let t = rig.transport();
    for params in [vec![p("sessionID", "")], vec![p("", SES)]] {
        let e = code_of(
            t.call(&op("session.get"), &params, None, Duration::from_secs(2))
                .await,
        );
        assert_eq!(e.code, ErrorCode::InvalidArgument);
    }
    assert!(rig.log().is_empty(), "no spawn");
}

#[tokio::test]
async fn transport_server_url_forwarded() {
    let rig = Rig::new().server("http://127.0.0.1:49374");
    let t = rig.transport();
    t.call(
        &op("session.get"),
        &session_params(),
        None,
        Duration::from_secs(5),
    )
    .await
    .unwrap();
    for a in rig.argvs() {
        let i = a
            .iter()
            .position(|x| x == "--server")
            .expect("--server forwarded");
        assert_eq!(a[i + 1], "http://127.0.0.1:49374");
    }
    let rig = Rig::new();
    rig.transport()
        .call(
            &op("session.get"),
            &session_params(),
            None,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(!rig.argvs()[0].contains(&"--server".to_string()));
}

#[tokio::test]
async fn call_argv_shape() {
    let rig = Rig::new();
    rig.transport()
        .call(
            &op("session.prompt"),
            &session_params(),
            Some(&json!({"text": "hi"})),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert_eq!(
        rig.argvs()[0],
        [
            "api",
            "session.prompt",
            "--param",
            &format!("sessionID={SES}"),
            "-d",
            r#"{"text":"hi"}"#
        ]
    );
}

#[tokio::test]
async fn auth_failed_wrong_password_hint() {
    let rig = Rig::new()
        .mode("session.get:unauthorized")
        .server("http://127.0.0.1:49374");
    let e = code_of(
        rig.transport()
            .call(
                &op("session.get"),
                &session_params(),
                None,
                Duration::from_secs(5),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::AuthFailed);
    let h = e.hint.unwrap();
    for needle in [
        "opencode service get password",
        "OPENCODE_PASSWORD",
        "OPENCODE_SERVER_PASSWORD",
        "http://127.0.0.1:49374",
    ] {
        assert!(h.contains(needle), "{needle} in {h}");
    }
}

// --------------------------------------------------------------- the probe

#[tokio::test]
async fn transport_dead_service_is_service_down() {
    let rig = Rig::new().mode("server.info:unreachable");
    let e = code_of(
        rig.transport()
            .call(&op("server.info"), &vec![], None, Duration::from_secs(5))
            .await,
    );
    assert_eq!(e.code, ErrorCode::ServiceDown);
    assert_eq!(
        rig.argvs().iter().filter(|a| a[0] == "service").count(),
        0,
        "decided by stderr, no probe"
    );
}

#[tokio::test]
async fn transport_probe_decides_service_down() {
    let call = |rig: &Rig| {
        let t = rig.transport();
        async move {
            code_of(
                t.call(&op("server.info"), &vec![], None, Duration::from_secs(5))
                    .await,
            )
        }
    };
    let status_calls = |rig: &Rig| {
        rig.argvs()
            .iter()
            .filter(|a| a[..2] == ["service", "status"])
            .count()
    };

    let rig = Rig::new().mode("server.info:unmapped,service.status:stopped");
    assert_eq!(call(&rig).await.code, ErrorCode::ServiceDown);
    assert_eq!(status_calls(&rig), 1);

    let rig = Rig::new().mode("server.info:unmapped,service.status:fail");
    assert_eq!(call(&rig).await.code, ErrorCode::ServiceDown);

    let rig = Rig::new().mode("server.info:unmapped");
    let e = call(&rig).await;
    assert_eq!(e.code, ErrorCode::CliError);
    assert_eq!(
        e.message,
        "Error: something the mapping table does not know"
    );
    assert_eq!(status_calls(&rig), 1);

    let rig = Rig::new().mode("server.info:fail,service.status:stopped");
    let e = call(&rig).await;
    assert_eq!(
        (e.code, tag(&e)),
        (ErrorCode::CliError, Some("UnknownError"))
    );
    assert_eq!(status_calls(&rig), 0, "a tagged failure is never probed");

    let rig = Rig::new()
        .mode("server.info:unmapped,service.status:stopped")
        .server("http://127.0.0.1:49374");
    assert_eq!(call(&rig).await.code, ErrorCode::CliError);
    assert_eq!(
        status_calls(&rig),
        0,
        "with server_url set there is no probe"
    );
}

#[tokio::test]
async fn transport_unknown_error_cause_line_is_message() {
    let rig = Rig::new().mode("session.get:unknown-error");
    let e = code_of(
        rig.transport()
            .call(
                &op("session.get"),
                &session_params(),
                None,
                Duration::from_secs(5),
            )
            .await,
    );
    assert_eq!(e.code, ErrorCode::CliError);
    assert_eq!(e.message, "Missing path parameter: sessionID");
}

#[test]
fn transport_unreachable_wins_over_cause_line() {
    let Mapped::Error(e) = map_cli_failure(Some(1), "", UNREACHABLE_STDERR) else {
        panic!("the unreachable stderr is decided without a probe")
    };
    assert_eq!(e.code, ErrorCode::ServiceDown);
}

// ----------------------------------------------------------------- others

#[tokio::test]
async fn service_status_running_from_url_and_stopped() {
    let rig = Rig::new();
    let out = rig.transport().lifecycle(Lifecycle::Status).await.unwrap();
    assert!(service_running(&out), "{out}");
    assert_eq!(rig.argvs()[0], ["service", "status"]);
    let rig = Rig::new().mode("service.status:stopped");
    let out = rig.transport().lifecycle(Lifecycle::Status).await.unwrap();
    assert!(!service_running(&out));
    // A failing verb: the first stderr line, no probe, no tag parsing.
    let rig = Rig::new().mode("service.start:unmapped");
    let e = code_of(rig.transport().lifecycle(Lifecycle::Start).await);
    assert_eq!(
        (e.code, e.message.as_str()),
        (
            ErrorCode::CliError,
            "Error: something the mapping table does not know"
        )
    );
    assert_eq!(rig.argvs().len(), 1);
    let rig = Rig::new().mode("service.stop:silent,service.restart:fail");
    assert_eq!(
        rig.transport().lifecycle(Lifecycle::Stop).await.unwrap(),
        ""
    );
    let e = code_of(rig.transport().lifecycle(Lifecycle::Restart).await);
    assert_eq!(e.message, "HTTP 500 Internal Server Error");
}

#[tokio::test]
async fn cli_version_parsed_from_real_form() {
    assert_eq!(
        parse_cli_version("opencode v2.0.32").as_deref(),
        Some("2.0.32")
    );
    assert_eq!(parse_cli_version("2.0.32").as_deref(), Some("2.0.32"));
    assert_eq!(parse_cli_version("v2.0.32").as_deref(), Some("2.0.32"));
    let rig = Rig::new();
    let raw = rig.transport().cli_version().await.unwrap();
    assert_eq!(parse_cli_version(&raw).as_deref(), Some("2.0.32"));
}

#[cfg(unix)]
fn script_bin(body: &str) -> (TempHome, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_home();
    let f = dir.path().join("opencode");
    fs::write(&f, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&f, fs::Permissions::from_mode(0o755)).unwrap();
    (dir, f)
}

#[cfg(unix)]
#[tokio::test]
async fn fetch_openapi_and_version_enforce_call_timeout() {
    let (_d, bin) = script_bin("exec sleep 30");
    let mut rig = Rig::new();
    rig.settings.opencode_bin = Some(bin);
    rig.settings.call_timeout_s = 1;
    let t = rig.transport();
    for _ in 0..5 {
        // A freshly written script can hit ETXTBSY while sibling tests fork; retry that only.
        let started = Instant::now();
        let e = code_of(t.fetch_openapi().await);
        if e.code == ErrorCode::IoError {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }
        assert_eq!(e.code, ErrorCode::Deadline, "{e:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
        let e = code_of(t.cli_version().await);
        assert_eq!(e.code, ErrorCode::Deadline);
        let e = code_of(t.lifecycle(Lifecycle::Status).await);
        assert_eq!(e.code, ErrorCode::Deadline);
        assert_eq!(leftover_files(&t), 0);
        return;
    }
    panic!("the script never became executable");
}

#[test]
fn sweep_stdout_dirs_removes_dead_and_mismatched() {
    let home = temp_home();
    let stdout = home.path().join("stdout");
    for n in ["10-aaa", "20-bbb", "30-ccc", "40-0", "junk"] {
        fs::create_dir_all(stdout.join(n)).unwrap();
        fs::write(stdout.join(n).join("x.json"), "{}").unwrap();
    }
    let probe = FakeProbe::new("1000");
    probe.set_alive(20, "bbb"); // same starttime: kept
    probe.set_alive(30, "other"); // pid reused: swept
    probe.set_alive(40, "0"); // "0" is the name used when the start time was unreadable
                              // 10 is dead: swept.
    let mut removed = sweep_stdout_dirs(home.path(), &probe);
    removed.sort();
    assert_eq!(removed, vec![stdout.join("10-aaa"), stdout.join("30-ccc")]);
    assert!(stdout.join("20-bbb").is_dir() && stdout.join("40-0").is_dir());
    assert!(
        stdout.join("junk").is_dir(),
        "a name that is not <pid>-<starttime> is not ours"
    );
    assert!(sweep_stdout_dirs(&home.path().join("none"), &probe).is_empty());
}

// ------------------------------------------------- through the engine

fn engine(rig: &Rig) -> Engine<CliTransport, SystemClock> {
    let settings = Settings {
        wait_timeout_s: 2,
        poll_s: 1,
        ..rig.settings.clone()
    };
    let cwd = rig.home.path().to_path_buf();
    Engine::new(
        settings,
        Vec::new(),
        Host::Claude,
        EnvSnapshot::for_tests(cwd.clone(), cwd),
        rig.transport(),
        SystemClock,
        Arc::new(FakeProbe::new("1000")),
    )
}

fn param_pairs(argv: &[String]) -> Vec<String> {
    argv.windows(2)
        .filter(|w| w[0] == "--param")
        .map(|w| w[1].clone())
        .collect()
}

fn wait_args() -> SessionWaitArgs {
    SessionWaitArgs {
        session_id: Some(SES.into()),
        after: None,
        timeout_s: Some(2),
        poll_s: Some(1),
        hard: None,
    }
}

#[tokio::test]
async fn wait_argv_order_desc_only_cursorless() {
    // The two-page pair lives in the starter fixtures (`...__limit=200&order=desc&...` -> next `c2`).
    let rig = Rig::new();
    let rig = rig.env("MOCK_OPENCODE_NOW_MS", &chrono_free_now().to_string());
    let e = engine(&rig);
    let waited = e.session_wait(wait_args()).await;
    assert!(waited.is_ok(), "session_wait must succeed: {waited:?}");
    let reads: Vec<Vec<String>> = rig
        .argvs()
        .into_iter()
        .filter(|a| a[1] == "session.message.list")
        .collect();
    assert!(!reads.is_empty(), "{:?}", rig.argvs());
    // reads[0] is the anchor read (`type=user limit=1`); the slice's first read is `limit=200`.
    let first = reads
        .iter()
        .map(|a| param_pairs(a))
        .find(|p| p.contains(&"limit=200".to_string()))
        .expect("a first wait read");
    assert!(
        first.contains(&"order=desc".to_string()) && first.contains(&"limit=200".to_string()),
        "{first:?}"
    );
    let cursor_reads: Vec<_> = reads
        .iter()
        .map(|a| param_pairs(a))
        .filter(|p| p.iter().any(|x| x.starts_with("cursor=")))
        .collect();
    assert!(
        !cursor_reads.is_empty(),
        "the second page was fetched: {reads:?}"
    );
    for pr in cursor_reads {
        assert!(
            !pr.iter()
                .any(|x| x.starts_with("order=") || x.starts_with("type=")),
            "{pr:?}"
        );
    }
}

fn chrono_free_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tokio::test]
async fn workspace_argv_deep_object_location() {
    let rig = Rig::new();
    let e = engine(&rig);
    let out = e
        .workspace_status(WorkspaceStatusArgs {
            directory: Some("/tmp/w".into()),
        })
        .await
        .unwrap();
    assert_eq!(out["directory"], "/tmp/w");
    let argvs = rig.argvs();
    for opid in ["vcs.get", "vcs.status"] {
        let a = argvs
            .iter()
            .find(|a| a.get(1).map(String::as_str) == Some(opid))
            .unwrap_or_else(|| panic!("{opid} in {argvs:?}"));
        assert!(
            param_pairs(a).contains(&"location[directory]=/tmp/w".to_string()),
            "{a:?}"
        );
    }
    // `session.diff` declares no `location` (measured in the openapi document).
    let doc: Value =
        serde_json::from_str(&fs::read_to_string(starter_fixtures().join("openapi.json")).unwrap())
            .unwrap();
    let diff = doc["paths"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|m| m.as_object().unwrap().values())
        .find(|o| o["operationId"] == "session.diff")
        .expect("session.diff in the document");
    let names: Vec<&str> = diff["parameters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["sessionID", "from", "to", "context"]);
}

#[tokio::test]
async fn anchor_argv_type_user_limit_1() {
    let rig = Rig::new().env("MOCK_OPENCODE_NOW_MS", &chrono_free_now().to_string());
    let e = engine(&rig);
    let waited = e.session_wait(wait_args()).await;
    assert!(waited.is_ok(), "session_wait must succeed: {waited:?}");
    let anchor = rig
        .argvs()
        .into_iter()
        .map(|a| param_pairs(&a))
        .find(|p| p.contains(&"type=user".to_string()))
        .expect("an anchor read with type=user");
    let mut sorted = anchor.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        [
            "limit=1".to_string(),
            "order=desc".to_string(),
            format!("sessionID={SES}"),
            "type=user".to_string()
        ]
    );
}
