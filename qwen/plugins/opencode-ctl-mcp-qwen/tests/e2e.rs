//! End-to-end tests that run the plugin's real `serve` through rmcp's client against the
//! mock `opencode` (`mock-opencode-ctl-qwen`). Nothing here touches the real home or a real
//! `opencode`: every spawned process gets explicit HOME, USERPROFILE and OPENCODE_CTL_HOME.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use opencode_ctl::testing::{temp_home, TempHome};
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{json, Value};

const SERVE_BIN: &str = env!("CARGO_BIN_EXE_opencode-ctl-qwen-mcp");
const MOCK: &str = env!("CARGO_BIN_EXE_mock-opencode-ctl-qwen");

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
        Some(concat!("opencode-ctl-qwen-mcp ", env!("CARGO_PKG_VERSION")))
    );
}
