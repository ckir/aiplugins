//! End-to-end tests that run the plugin's real binaries.
//!
//! The unit tests in `src/` pin the decisions; these pin the *contracts* — the
//! JSON the hook writes to stdout, and the MCP protocol the server speaks. Those
//! are exactly the parts that compile fine and still break in production, so
//! they get exercised through a real process rather than a function call.
//!
//! The MCP side uses rmcp's own client against our server binary, which means
//! the handshake, schema and tool dispatch are all tested for real.

use rmcp::{model::CallToolRequestParams, transport::TokioChildProcess, ServiceExt};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── Helpers ────────────────────────────────────────────────────────────

/// Run the hook binary with `input` on stdin, returning `(stdout, exit_ok)`.
fn run_hook(input: &str, project_dir: Option<&Path>) -> (String, bool) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_claude-example-hook"));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = project_dir {
        cmd.env("CLAUDE_PROJECT_DIR", dir);
    } else {
        // Keep the hook away from this repo's own settings file so the test is
        // independent of the checkout it runs in.
        cmd.env("CLAUDE_PROJECT_DIR", unique_dir("hook-no-settings"));
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

/// A unique, not-yet-created path under the system temp directory.
fn unique_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "claude-example-{tag}-{nanos}-{}",
        std::process::id()
    ))
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

/// Pull the first text block out of a tool result, as JSON.
fn tool_text(result: &rmcp::model::CallToolResult) -> serde_json::Value {
    let content = serde_json::to_value(&result.content).expect("serialize content");
    let text = content[0]["text"].as_str().expect("text content block");
    serde_json::from_str(text).expect("tool returned valid JSON")
}

/// Connect an MCP client to the plugin's server binary.
async fn connect_mcp(cwd: &Path) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_claude-example-mcp"));
    cmd.current_dir(cwd).env("CLAUDE_PROJECT_DIR", cwd);
    let transport = TokioChildProcess::new(cmd).expect("spawn mcp server");
    ().serve(transport).await.expect("mcp handshake")
}

// ── Hook: the stdout contract ──────────────────────────────────────────

#[test]
fn hook_reports_an_unowned_marker() {
    let input = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": { "file_path": "src/main.rs", "content": "// TODO: wire this up\n" }
    })
    .to_string();

    let (stdout, ok) = run_hook(&input, None);
    assert!(ok, "hook must exit 0");

    let json: serde_json::Value = serde_json::from_str(&stdout).expect("hook emits valid JSON");
    let msg = json["systemMessage"]
        .as_str()
        .expect("systemMessage present");
    assert!(msg.contains("1 unowned marker"), "got: {msg}");
    assert!(msg.contains("src/main.rs"), "got: {msg}");
}

#[test]
fn hook_is_silent_for_an_owned_marker() {
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": { "file_path": "src/main.rs", "content": "// TODO(alice): mine\n" }
    })
    .to_string();

    let (stdout, ok) = run_hook(&input, None);
    assert!(ok);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.get("systemMessage").is_none(), "got: {stdout}");
    assert_eq!(json["suppressOutput"], true);
}

#[test]
fn hook_ignores_unwatched_tools() {
    let input = serde_json::json!({
        "tool_name": "Read",
        "tool_input": { "file_path": "src/main.rs", "content": "// TODO: ignored" }
    })
    .to_string();

    let (stdout, ok) = run_hook(&input, None);
    assert!(ok);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json.get("systemMessage").is_none());
}

#[test]
fn hook_survives_garbage_input() {
    // A hook that crashes on an unexpected payload breaks the user's session.
    let (stdout, ok) = run_hook("}{ not json", None);
    assert!(ok, "hook must still exit 0 on malformed input");
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("still emits valid JSON");
    assert!(json.get("systemMessage").is_none());
}

#[test]
fn hook_survives_empty_input() {
    let (stdout, ok) = run_hook("", None);
    assert!(ok);
    serde_json::from_str::<serde_json::Value>(&stdout).expect("valid JSON on empty input");
}

#[test]
fn hook_falls_back_to_reading_the_file_from_disk() {
    let root = fixture("hook-disk", &[("src/lib.rs", "// FIXME: only on disk\n")]);
    // No inline content in the payload, so the hook must read the file.
    let input = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": { "file_path": "src/lib.rs" }
    })
    .to_string();

    let (stdout, ok) = run_hook(&input, Some(&root));
    assert!(ok);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let msg = json["systemMessage"].as_str().expect("systemMessage");
    assert!(msg.contains("1 unowned marker"), "got: {msg}");

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn hook_honours_the_settings_file() {
    let root = fixture(
        "hook-settings",
        &[(
            ".claude/claude-example.local.md",
            "---\nrequire_owner: false\n---\n\nOwner checks are off for this project.\n",
        )],
    );
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": { "file_path": "src/main.rs", "content": "// TODO: unowned\n" }
    })
    .to_string();

    let (stdout, ok) = run_hook(&input, Some(&root));
    assert!(ok);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("systemMessage").is_none(),
        "require_owner: false must silence the hook, got: {stdout}"
    );

    std::fs::remove_dir_all(&root).ok();
}

// ── MCP: the protocol contract ─────────────────────────────────────────

#[tokio::test]
async fn mcp_server_advertises_both_tools() {
    let root = fixture("mcp-list", &[("README.md", "nothing here\n")]);
    let client = connect_mcp(&root).await;

    // The server must introduce itself by name; rmcp's default is "rmcp",
    // which would make every plugin's server indistinguishable in /mcp.
    let info = client.peer_info().expect("server info from handshake");
    let server = info.server_info.as_ref().expect("server_info present");
    assert_eq!(
        server.name, "claude-example-mcp",
        "server must self-identify"
    );

    let tools = client.list_all_tools().await.expect("list tools");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"scan_todos"), "got: {names:?}");
    assert!(names.contains(&"check_text"), "got: {names:?}");

    // The input schema is what the model reads; make sure it survived derive.
    let scan = tools.iter().find(|t| t.name == "scan_todos").unwrap();
    let schema = serde_json::to_value(&scan.input_schema).unwrap();
    assert!(
        schema["properties"].get("unowned_only").is_some(),
        "schema missing unowned_only: {schema}"
    );

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_check_text_finds_markers() {
    let root = fixture("mcp-check", &[("README.md", "x\n")]);
    let client = connect_mcp(&root).await;

    let args = serde_json::json!({ "text": "// TODO(alice): a\n// FIXME: b\n" })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("check_text").with_arguments(args))
        .await
        .expect("call check_text");

    let json = tool_text(&result);
    assert_eq!(json["total"], 2);
    assert_eq!(json["unowned"], 1);
    assert_eq!(json["markers"][0]["kind"], "TODO");
    assert_eq!(json["markers"][0]["owner"], "alice");
    assert_eq!(json["markers"][1]["kind"], "FIXME");
    assert_eq!(json["markers"][1]["line"], 2);

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_scan_todos_walks_a_directory() {
    let root = fixture(
        "mcp-scan",
        &[
            ("src/a.rs", "fn a() {}\n// TODO(bob): tidy\n"),
            ("src/b.rs", "// FIXME: broken\n"),
            ("notes.md", "// HACK: workaround\n"),
            // `src/bin/` is ordinary Rust source and must be scanned; an
            // over-eager skip list would silently drop it.
            ("src/bin/tool.rs", "// TODO: in a bin target\n"),
            ("target/ignored.rs", "// TODO: must not be scanned\n"),
        ],
    );
    let client = connect_mcp(&root).await;

    let args = serde_json::json!({ "path": root.to_string_lossy() })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("scan_todos").with_arguments(args))
        .await
        .expect("call scan_todos");

    let json = tool_text(&result);
    assert_eq!(json["total"], 4, "target/ should be skipped: {json}");
    assert_eq!(json["unowned"], 3);

    let files: Vec<&str> = json["markers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["file"].as_str().unwrap())
        .collect();
    assert!(
        !files.iter().any(|f| f.contains("target")),
        "target/ leaked into results: {files:?}"
    );
    assert!(
        files.contains(&"src/bin/tool.rs"),
        "src/bin/ was skipped: {files:?}"
    );

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_scan_todos_filters_to_unowned() {
    let root = fixture(
        "mcp-unowned",
        &[("src/a.rs", "// TODO(bob): mine\n// TODO: nobody's\n")],
    );
    let client = connect_mcp(&root).await;

    let args = serde_json::json!({ "path": root.to_string_lossy(), "unowned_only": true })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(CallToolRequestParams::new("scan_todos").with_arguments(args))
        .await
        .expect("call scan_todos");

    let json = tool_text(&result);
    assert_eq!(json["total"], 1);
    assert_eq!(json["markers"][0]["line"], 2);
    assert!(json["markers"][0]["owner"].is_null());

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn mcp_scan_todos_rejects_a_bad_path() {
    let root = fixture("mcp-badpath", &[("README.md", "x\n")]);
    let client = connect_mcp(&root).await;

    let args = serde_json::json!({ "path": "/definitely/not/a/real/directory/xyzzy" })
        .as_object()
        .unwrap()
        .clone();
    let outcome = client
        .call_tool(CallToolRequestParams::new("scan_todos").with_arguments(args))
        .await;
    assert!(
        outcome.is_err(),
        "expected an error for a missing directory"
    );

    client.cancel().await.ok();
    std::fs::remove_dir_all(&root).ok();
}
