//! End-to-end tests that run the plugin's real binaries.
//!
//! The unit tests in `src/lib.rs` pin the decisions; these pin the *contracts* —
//! the JSON the hook writes to stdout, and the MCP protocol the server speaks.
//! Those are exactly the parts that compile fine and still break in production,
//! so they get exercised through a real process rather than a function call.
//!
//! `RTK_BIN` points at the `mock-rtk` fixture rather than the real rtk, so the
//! suite runs identically on a machine that has never heard of rtk — and so the
//! failure modes (missing binary, non-zero exit, prose on stdout) can be forced
//! on demand instead of waited for.

use rmcp::{model::CallToolRequestParams, transport::TokioChildProcess, ServiceExt};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── Helpers ────────────────────────────────────────────────────────────

/// A PreToolUse event carrying `command`, as Claude Code would send it.
fn event(command: &str) -> String {
    serde_json::json!({
        "session_id": "test",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": { "command": command, "description": "a command" }
    })
    .to_string()
}

/// Run the hook binary with `input` on stdin, returning `(stdout, exit_ok)`.
///
/// `envs` are applied on top of the defaults, which already point `RTK_BIN` at
/// the mock and isolate the hook from this repo's own settings file.
fn run_hook(input: &str, envs: &[(&str, &str)]) -> (String, bool) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rtk-cc-hook"));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("RTK_BIN", env!("CARGO_BIN_EXE_mock-rtk-cc"))
        .env("CLAUDE_PROJECT_DIR", unique_dir("hook-no-settings"))
        // Inherited values would leak the developer's own preferences into the
        // assertions; a test that passes only on an unconfigured machine is not
        // a test.
        .env_remove("RTK_CC_DISABLE")
        .env_remove("RTK_CC_ULTRA_COMPACT")
        .env_remove("RTK_CC_SKIP_ENV")
        .env_remove("MOCK_RTK_MODE");
    for (key, value) in envs {
        cmd.env(key, value);
    }

    let mut child = cmd.spawn().expect("spawn hook binary");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait for hook");
    (
        String::from_utf8(out.stdout).expect("utf-8 stdout"),
        out.status.success(),
    )
}

/// The rewritten command the hook reported, or `None` if it stayed silent.
fn rewritten(stdout: &str) -> Option<String> {
    if stdout.trim().is_empty() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_str(stdout).expect("hook emits valid JSON");
    Some(
        json["hookSpecificOutput"]["updatedInput"]["command"]
            .as_str()
            .expect("updatedInput.command")
            .to_string(),
    )
}

/// A unique, not-yet-created path under the system temp directory.
fn unique_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rtk-mcp-cc-{tag}-{nanos}-{}", std::process::id()))
}

/// Create a temp directory containing `files`, returning its path.
fn fixture(tag: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = unique_dir(tag);
    for (name, body) in files {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture dir");
        }
        std::fs::write(&path, body).expect("write fixture file");
    }
    root
}

/// Pull the first text block out of a tool result.
fn tool_text(result: &rmcp::model::CallToolResult) -> String {
    let content = serde_json::to_value(&result.content).expect("serialize content");
    content[0]["text"]
        .as_str()
        .expect("text content block")
        .to_string()
}

/// Connect an MCP client to the plugin's server binary.
async fn connect_mcp(cwd: &Path) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_rtk-cc-mcp"));
    cmd.current_dir(cwd)
        .env("CLAUDE_PROJECT_DIR", cwd)
        .env("RTK_BIN", env!("CARGO_BIN_EXE_mock-rtk-cc"))
        .env_remove("MOCK_RTK_MODE");
    let transport = TokioChildProcess::new(cmd).expect("spawn mcp server");
    ().serve(transport).await.expect("mcp handshake")
}

// ── Hook: the stdout contract ──────────────────────────────────────────

#[test]
fn hook_forwards_a_rewrite() {
    let (stdout, ok) = run_hook(&event("cat README.md"), &[]);
    assert!(ok, "hook must exit 0");
    assert_eq!(rewritten(&stdout).as_deref(), Some("rtk cat README.md"));
}

#[test]
fn hook_is_silent_when_there_is_nothing_to_rewrite() {
    // An already-rewritten command. Verified against real rtk 0.45.0: feeding
    // its own output back produces empty stdout and exit 0. This is what makes
    // the plugin safe to install alongside a hand-wired `rtk hook claude` in
    // settings.json — the second pass is a no-op rather than a double rewrite.
    let (stdout, ok) = run_hook(&event("rtk read README.md"), &[]);
    assert!(ok);
    assert_eq!(rewritten(&stdout), None, "got: {stdout}");
}

#[test]
fn hook_passes_through_when_rtk_is_missing() {
    let (stdout, ok) = run_hook(
        &event("cat README.md"),
        &[("RTK_BIN", "definitely-not-an-installed-binary")],
    );
    assert!(ok, "a missing rtk must never fail the tool call");
    assert!(stdout.trim().is_empty(), "got: {stdout}");
}

#[test]
fn hook_passes_through_when_rtk_fails() {
    let (stdout, ok) = run_hook(&event("cat README.md"), &[("MOCK_RTK_MODE", "fail")]);
    assert!(ok, "a non-zero rtk exit must never fail the tool call");
    assert!(stdout.trim().is_empty(), "got: {stdout}");
}

#[test]
fn hook_suppresses_non_json_from_rtk() {
    // Claude Code treats unparsable hook output as an error, so prose on
    // stdout must be swallowed rather than forwarded.
    let (stdout, ok) = run_hook(&event("cat README.md"), &[("MOCK_RTK_MODE", "garbage")]);
    assert!(ok);
    assert!(stdout.trim().is_empty(), "got: {stdout}");
}

#[test]
fn hook_survives_garbage_input() {
    let (stdout, ok) = run_hook("}{ not json", &[]);
    assert!(ok, "hook must still exit 0 on malformed input");
    // The mock finds no command and says nothing; the point is that we did not
    // crash and did not emit anything Claude Code would choke on.
    assert!(stdout.trim().is_empty(), "got: {stdout}");
}

#[test]
fn hook_survives_empty_input() {
    let (stdout, ok) = run_hook("", &[]);
    assert!(ok);
    assert!(stdout.trim().is_empty());
}

#[test]
fn hook_can_be_disabled_by_environment() {
    let (stdout, ok) = run_hook(&event("cat README.md"), &[("RTK_CC_DISABLE", "1")]);
    assert!(ok);
    assert!(stdout.trim().is_empty(), "got: {stdout}");
}

#[test]
fn hook_disable_flag_ignores_a_falsy_value() {
    let (stdout, ok) = run_hook(&event("cat README.md"), &[("RTK_CC_DISABLE", "0")]);
    assert!(ok);
    assert_eq!(
        rewritten(&stdout).as_deref(),
        Some("rtk cat README.md"),
        "RTK_CC_DISABLE=0 must leave the hook active"
    );
}

#[test]
fn hook_honours_the_settings_file() {
    let root = fixture(
        "hook-settings",
        &[(
            ".claude/rtk-mcp-cc.local.md",
            "---\nenabled: false\n---\n\nrtk is off for this project.\n",
        )],
    );

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rtk-cc-hook"));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("RTK_BIN", env!("CARGO_BIN_EXE_mock-rtk-cc"))
        .env("CLAUDE_PROJECT_DIR", &root)
        .env_remove("RTK_CC_DISABLE");
    let mut child = cmd.spawn().expect("spawn hook");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event("cat README.md").as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8(out.stdout).unwrap();

    assert!(out.status.success());
    assert!(
        stdout.trim().is_empty(),
        "enabled: false must silence the hook, got: {stdout}"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn hook_reports_its_version_and_help() {
    for flag in ["--version", "--help"] {
        let out = Command::new(env!("CARGO_BIN_EXE_rtk-cc-hook"))
            .arg(flag)
            .output()
            .expect("run hook");
        assert!(out.status.success(), "{flag} must exit 0");
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("rtk-cc-hook"), "{flag} gave: {text}");
    }
}

// ── MCP: the protocol contract ─────────────────────────────────────────

#[tokio::test]
async fn mcp_server_advertises_every_tool() {
    let root = fixture("mcp-list", &[("README.md", "nothing here\n")]);
    let client = connect_mcp(&root).await;

    // The server must introduce itself by name; rmcp's default is "rmcp",
    // which would make every plugin's server indistinguishable in /mcp.
    let info = client.peer_info().expect("server info from handshake");
    let server = info.server_info.as_ref().expect("server_info present");
    assert_eq!(server.name, "rtk-cc-mcp", "server must self-identify");

    let tools = client.list_all_tools().await.expect("list tools");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    for expected in ["rtk_gain", "rtk_discover", "rtk_check", "rtk_proxy"] {
        assert!(
            names.contains(&expected),
            "missing {expected}, got: {names:?}"
        );
    }

    // The input schema is what the model reads; make sure it survived derive.
    let gain = tools.iter().find(|t| t.name == "rtk_gain").unwrap();
    let schema = serde_json::to_value(&gain.input_schema).unwrap();
    assert!(
        schema["properties"].get("history").is_some(),
        "schema missing history: {schema}"
    );

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_check_reports_a_rewrite_without_running_it() {
    let root = fixture("mcp-check", &[("README.md", "x\n")]);
    let client = connect_mcp(&root).await;

    let args = serde_json::json!({ "command": "grep -rn foo src/" })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("rtk_check").with_arguments(args))
        .await
        .expect("call rtk_check");

    assert!(
        tool_text(&result).contains("rtk grep -rn foo src/"),
        "got: {}",
        tool_text(&result)
    );

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_gain_forwards_its_options() {
    let root = fixture("mcp-gain", &[("README.md", "x\n")]);
    let client = connect_mcp(&root).await;

    let args = serde_json::json!({ "history": true, "format": "json" })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("rtk_gain").with_arguments(args))
        .await
        .expect("call rtk_gain");

    let text = tool_text(&result);
    assert!(text.contains("MOCK gain"), "got: {text}");
    assert!(text.contains("--history"), "got: {text}");
    assert!(text.contains("json"), "got: {text}");

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_rejects_an_unsupported_format() {
    let root = fixture("mcp-badfmt", &[("README.md", "x\n")]);
    let client = connect_mcp(&root).await;

    // csv is valid for `rtk gain` but not for `rtk discover`; the server must
    // catch that before rtk does, and report it as invalid params.
    let args = serde_json::json!({ "format": "csv" })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("rtk_discover").with_arguments(args))
        .await;

    assert!(result.is_err(), "csv must be rejected for rtk_discover");

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_proxy_passes_argv_through_without_a_shell() {
    let root = fixture("mcp-proxy", &[("README.md", "x\n")]);
    let client = connect_mcp(&root).await;

    let args = serde_json::json!({ "argv": ["git", "status"] })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("rtk_proxy").with_arguments(args))
        .await
        .expect("call rtk_proxy");

    let text = tool_text(&result);
    assert!(text.contains("MOCK proxy"), "got: {text}");
    assert!(text.contains("git status"), "got: {text}");

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_proxy_rejects_an_empty_argv() {
    let root = fixture("mcp-proxy-empty", &[("README.md", "x\n")]);
    let client = connect_mcp(&root).await;

    let args = serde_json::json!({ "argv": [] })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("rtk_proxy").with_arguments(args))
        .await;

    assert!(result.is_err(), "an empty argv must be rejected");

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}
