//! End-to-end tests that run the plugin's real `serve` through rmcp's client against the
//! mock `opencode` (`mock-opencode-ctl-agy`). Nothing here touches the real home or a real
//! `opencode`: every spawned process gets explicit HOME, USERPROFILE and OPENCODE_CTL_HOME.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use opencode_ctl::testing::{temp_home, TempHome};
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{json, Value};

const SERVE_BIN: &str = env!("CARGO_BIN_EXE_opencode-ctl-agy-mcp");
const MOCK: &str = env!("CARGO_BIN_EXE_mock-opencode-ctl-agy");

fn fixtures_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../shared/opencode-ctl/tests/fixtures")
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
    ctl_home: TempHome,
    user_home: TempHome,
    state: TempHome,
    fixtures: TempHome,
}

impl Rig {
    fn new() -> Rig {
        let r = Rig {
            ctl_home: temp_home(),
            user_home: temp_home(),
            state: temp_home(),
            fixtures: temp_home(),
        };
        copy_dir(&fixtures_src(), r.fixtures.path());
        r
    }

    fn command(&self) -> tokio::process::Command {
        let mut c = tokio::process::Command::new(SERVE_BIN);
        c.arg("serve")
            .env("HOME", self.user_home.path())
            .env("USERPROFILE", self.user_home.path())
            .env("OPENCODE_CTL_HOME", self.ctl_home.path())
            .env("OPENCODE_CTL_BIN", MOCK)
            .env("MOCK_OPENCODE_FIXTURES", self.fixtures.path())
            .env("MOCK_OPENCODE_STATE_DIR", self.state.path());
        for k in [
            "OPENCODE_CTL_SESSION",
            "OPENCODE_CTL_DIR",
            "OPENCODE_CTL_SERVER",
            "OPENCODE_CTL_PAIR",
            "OPENCODE_CTL_HOST",
            "OPENCODE_CTL_LOG",
        ] {
            c.env_remove(k);
        }
        c
    }

    async fn connect(&self) -> RunningService<RoleClient, ()> {
        connect(self.command()).await
    }

    fn argv_log(&self) -> Vec<Value> {
        fs::read_to_string(self.state.path().join("argv.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

async fn connect(cmd: tokio::process::Command) -> RunningService<RoleClient, ()> {
    let transport = TokioChildProcess::new(cmd).expect("spawn serve");
    ().serve(transport).await.expect("initialize")
}

async fn call(client: &RunningService<RoleClient, ()>, name: &str, args: Value) -> Value {
    let params = CallToolRequestParams::new(name.to_string())
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    let result = client.call_tool(params).await.expect("call_tool");
    let text = result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default();
    serde_json::from_str(&text)
        .unwrap_or_else(|_| json!({"error": {"code": "RMCP", "message": text}}))
}

fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

#[tokio::test]
async fn handshake_lists_twenty_tools() {
    let rig = Rig::new();
    let client = rig.connect().await;
    let tools = client.list_all_tools().await.expect("list_tools");
    assert_eq!(
        tools.len(),
        20,
        "{:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );
    let _ = client.cancel().await;
}

#[tokio::test]
async fn service_status_through_mock() {
    let rig = Rig::new();
    let client = rig.connect().await;
    let out = call(&client, "service_status", json!({})).await;
    assert!(out.get("error").is_none(), "{out}");
    assert_eq!(out["running"], true, "{out}");
    let argv: Vec<Value> = rig.argv_log().iter().map(|l| l["argv"].clone()).collect();
    assert!(argv.contains(&json!(["service", "status"])), "{argv:?}");
    let _ = client.cancel().await;
}

#[tokio::test]
async fn service_stop_without_confirm_refused() {
    let rig = Rig::new();
    let client = rig.connect().await;
    let out = call(&client, "service_stop", json!({})).await;
    assert_eq!(code(&out), "CONFIRM_REQUIRED", "{out}");
    let argv: Vec<Value> = rig.argv_log().iter().map(|l| l["argv"].clone()).collect();
    assert!(!argv.contains(&json!(["service", "stop"])), "{argv:?}");
    let _ = client.cancel().await;
}

#[test]
fn version_line() {
    let out = Command::new(SERVE_BIN)
        .arg("--version")
        .env_clear()
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.lines().next(),
        Some(concat!("opencode-ctl-agy-mcp ", env!("CARGO_PKG_VERSION")))
    );
}

#[cfg(unix)]
#[test]
fn dispatch_empty_environment_process() {
    let dir = temp_home();
    let run = |args: &[&str]| {
        Command::new(SERVE_BIN)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .current_dir(dir.path())
            .output()
            .expect("spawn")
    };
    let v = run(&["--version"]);
    assert!(v.status.success());
    assert_eq!(
        String::from_utf8_lossy(&v.stdout).lines().next(),
        Some(concat!("opencode-ctl-agy-mcp ", env!("CARGO_PKG_VERSION")))
    );
    for sub in ["gc", "serve", "doctor-json"] {
        let o = run(&[sub]);
        assert!(!o.status.success(), "{sub} must fail with no home");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains("no home directory"), "{sub}: {err}");
    }
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        0,
        "the working directory must stay empty"
    );
}

#[tokio::test]
async fn serve_empty_server_env_overrides_user_file() {
    // Control: the user file's server_url reaches the mock as `--server`.
    let control = Rig::new();
    fs::write(
        control.ctl_home.path().join("settings.local.md"),
        "---\nserver_url: http://127.0.0.1:1\n---\n",
    )
    .unwrap();
    let client = control.connect().await;
    let _ = call(&client, "service_status", json!({})).await;
    let _ = client.cancel().await;
    let with_flag = control
        .argv_log()
        .iter()
        .filter_map(|l| l["argv"].as_array().cloned())
        .any(|a| {
            a.windows(2)
                .any(|w| w[0] == "--server" && w[1] == "http://127.0.0.1:1")
        });
    assert!(with_flag, "control run: {:?}", control.argv_log());

    // Same child and files, plus OPENCODE_CTL_SERVER="": the empty variable wins.
    let rig = Rig::new();
    fs::write(
        rig.ctl_home.path().join("settings.local.md"),
        "---\nserver_url: http://127.0.0.1:1\n---\n",
    )
    .unwrap();
    let mut cmd = rig.command();
    cmd.env("OPENCODE_CTL_SERVER", "");
    let client = connect(cmd).await;
    let out = call(&client, "service_status", json!({})).await;
    let _ = client.cancel().await;
    let log = rig.argv_log();
    assert!(!log.is_empty(), "the mock must have been called: {out}");
    for l in &log {
        let argv = l["argv"].as_array().unwrap();
        assert!(!argv.iter().any(|a| a == "--server"), "{l}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn serve_finds_opencode_on_path() {
    let rig = Rig::new();
    let bin_dir = temp_home();
    std::os::unix::fs::symlink(MOCK, bin_dir.path().join("opencode")).unwrap();
    let mut cmd = rig.command();
    cmd.env_remove("OPENCODE_CTL_BIN")
        .env("PATH", bin_dir.path());
    let client = connect(cmd).await;
    let out = call(&client, "service_status", json!({})).await;
    let _ = client.cancel().await;
    assert!(out.get("error").is_none(), "{out}");
    let argv: Vec<Value> = rig.argv_log().iter().map(|l| l["argv"].clone()).collect();
    assert!(argv.contains(&json!(["service", "status"])), "{argv:?}");
}

#[tokio::test]
async fn serve_handshakes_without_opencode() {
    let rig = Rig::new();
    let empty_path = temp_home();
    let mut cmd = rig.command();
    cmd.env_remove("OPENCODE_CTL_BIN")
        .env("PATH", empty_path.path());
    let client = connect(cmd).await;
    let tools = client.list_all_tools().await.expect("list_tools");
    assert_eq!(tools.len(), 20);
    let out = call(&client, "service_status", json!({})).await;
    let _ = client.cancel().await;
    assert_eq!(code(&out), "OPENCODE_NOT_FOUND", "{out}");
}

#[test]
fn logs_never_on_stdout() {
    let rig = Rig::new();
    fs::write(rig.ctl_home.path().join("logs"), "a regular file").unwrap();
    let mut child = Command::new(SERVE_BIN)
        .arg("serve")
        .env_clear()
        .env("HOME", rig.user_home.path())
        .env("USERPROFILE", rig.user_home.path())
        .env("OPENCODE_CTL_HOME", rig.ctl_home.path())
        .env("OPENCODE_CTL_BIN", MOCK)
        .env("OPENCODE_CTL_LOG", "debug")
        .env("OPENCODE_CTL_SESSION", "bad")
        .env("MOCK_OPENCODE_FIXTURES", rig.fixtures.path())
        .env("MOCK_OPENCODE_STATE_DIR", rig.state.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().unwrap();
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{},
        "clientInfo":{"name":"e2e","version":"0"}}});
    writeln!(stdin, "{init}").unwrap();
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    stdin.flush().unwrap();

    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let mut lines = Vec::new();
    loop {
        let l = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("the initialize response must arrive on stdout");
        let v: Value =
            serde_json::from_str(&l).unwrap_or_else(|_| panic!("non-JSON on stdout: {l:?}"));
        assert_eq!(v["jsonrpc"], "2.0", "not a JSON-RPC message: {l}");
        let done = v["id"] == 1 && v.get("result").is_some();
        lines.push(l);
        if done {
            break;
        }
    }
    drop(stdin);
    // Closing stdin ends the server; whatever else it printed before exiting is still JSON-RPC.
    let _ = child.kill();
    while let Ok(l) = rx.recv_timeout(Duration::from_millis(200)) {
        let v: Value =
            serde_json::from_str(&l).unwrap_or_else(|_| panic!("non-JSON on stdout: {l:?}"));
        assert_eq!(v["jsonrpc"], "2.0", "{l}");
    }
    let mut err = String::new();
    std::io::Read::read_to_string(&mut child.stderr.take().unwrap(), &mut err).unwrap();
    let _ = child.wait();
    assert!(err.contains("INVALID_ARGUMENT"), "stderr: {err}");
}
