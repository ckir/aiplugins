//! End-to-end tests that drive the real `rtk-mcp` and `rtk-hook-preinvocation`
//! binaries.
//!
//! These pin the *contracts*: the exact JSON-RPC an MCP client receives, and the
//! `injectSteps` payload Antigravity reads. The unit tests in `src/lib.rs` pin
//! the decisions behind them. Both layers exist because the wire format is the
//! product here — a change that compiles and still speaks different JSON is the
//! failure mode worth guarding.
//!
//! One defect already got through: the `initialize` handshake carried a
//! hardcoded `"version": "0.1.0"` while the crate was at 0.1.5, so `--version`
//! and `plugin.json` said one thing and every MCP client was told another.
//! Nothing caught it, because nothing ran the server.

use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

// ── Helpers ────────────────────────────────────────────────────────────

/// Send `requests` to the server, one JSON line each, and return every response
/// it writes back.
///
/// stdin is closed after the last request, which ends the server's read loop, so
/// the process exits on its own rather than needing to be killed.
fn converse(requests: &[Value]) -> Vec<Value> {
    converse_env(requests, &[])
}

/// Like [`converse`], with environment overrides applied to the server process.
///
/// The inherited values are cleared first: a developer who has `RTK_BIN` set
/// for their own use must not change what these tests measure.
fn converse_env(requests: &[Value], envs: &[(&str, &str)]) -> Vec<Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtk-mcp"));
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("RTK_BIN")
        .env_remove("RTK_AGY_SHELL");
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn rtk-mcp");

    {
        let mut stdin = child.stdin.take().expect("stdin");
        for req in requests {
            writeln!(stdin, "{req}").expect("write request");
        }
    } // dropping stdin closes the pipe, ending the server loop

    let out = child.wait_with_output().expect("wait for rtk-mcp");
    assert!(out.status.success(), "server must exit cleanly");
    parse_lines(&String::from_utf8(out.stdout).expect("utf-8 stdout"))
}

/// Like [`converse`], but writes raw text — for lines that are not valid JSON.
fn converse_raw(raw: &str) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rtk-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rtk-mcp");

    {
        let mut stdin = child.stdin.take().expect("stdin");
        stdin.write_all(raw.as_bytes()).expect("write raw");
    }

    let out = child.wait_with_output().expect("wait for rtk-mcp");
    assert!(out.status.success(), "server must exit cleanly");
    parse_lines(&String::from_utf8(out.stdout).expect("utf-8 stdout"))
}

fn parse_lines(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad response line {l:?}: {e}")))
        .collect()
}

/// Send one request and return the single response.
fn request(payload: Value) -> Value {
    let mut responses = converse(&[payload]);
    assert_eq!(responses.len(), 1, "expected exactly one response");
    responses.remove(0)
}

fn initialize() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} })
}

/// Run a binary with a flag and return its stdout, asserting a clean exit.
fn run_flag(bin: &str, flag: &str) -> String {
    let out = Command::new(bin).arg(flag).output().expect("run binary");
    assert!(out.status.success(), "{flag} must exit 0");
    String::from_utf8(out.stdout).expect("utf-8 stdout")
}

/// The version reported by `--version`, without the binary name.
fn version_flag(bin: &str) -> String {
    run_flag(bin, "--version")
        .trim()
        .rsplit(' ')
        .next()
        .expect("a version after the binary name")
        .to_string()
}

// ── initialize ─────────────────────────────────────────────────────────

#[test]
fn handshake_reports_the_crate_version() {
    let response = request(initialize());
    let info = &response["result"]["serverInfo"];

    // The registration key in mcp_config.json. Renaming it here without
    // renaming it there silently breaks the server's discovery.
    assert_eq!(info["name"], "rtkmcp", "got: {response}");

    // The defect this file exists for: a literal here drifts from Cargo.toml
    // on every release, and the handshake is the only version an MCP client
    // ever sees.
    assert_eq!(
        info["version"],
        env!("CARGO_PKG_VERSION"),
        "handshake version must track Cargo.toml, got: {response}"
    );
}

#[test]
fn version_flag_agrees_with_the_handshake() {
    let response = request(initialize());

    // Asserting both against CARGO_PKG_VERSION separately would still pass if
    // the two code paths diverged from each other in the same release. Compare
    // them directly: these are the two versions a user can actually observe.
    assert_eq!(
        response["result"]["serverInfo"]["version"]
            .as_str()
            .expect("handshake version is a string"),
        version_flag(env!("CARGO_BIN_EXE_rtk-mcp")),
        "`--version` and the MCP handshake report different versions"
    );
}

#[test]
fn handshake_declares_protocol_and_tool_capability() {
    let response = request(initialize());
    let result = &response["result"];

    assert_eq!(result["protocolVersion"], "2024-11-05", "got: {response}");
    // A client that sees no `tools` capability will never call tools/list.
    assert!(
        result["capabilities"]["tools"].is_object(),
        "tools capability must be advertised, got: {response}"
    );
}

// ── tools/list ─────────────────────────────────────────────────────────

#[test]
fn tools_list_advertises_rtk_run_with_its_schema() {
    let response = request(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}
    }));

    let tools = response["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 1, "exactly one tool, got: {response}");

    let tool = &tools[0];
    assert_eq!(tool["name"], "rtk_run");

    let schema = &tool["inputSchema"];
    assert_eq!(schema["type"], "object");

    let props = schema["properties"].as_object().expect("properties");
    for field in [
        "CommandLine",
        "Cwd",
        "WaitMsBeforeAsync",
        "toolAction",
        "toolSummary",
    ] {
        assert!(
            props.contains_key(field),
            "schema missing {field}: {schema}"
        );
    }

    let required: Vec<&str> = schema["required"]
        .as_array()
        .expect("required array")
        .iter()
        .map(|v| v.as_str().expect("string"))
        .collect();
    assert_eq!(
        required,
        vec!["CommandLine", "Cwd", "toolAction", "toolSummary"],
        "required set is part of the contract"
    );
    // WaitMsBeforeAsync is offered but not required; asserting it explicitly
    // keeps a careless edit from quietly making it mandatory.
    assert!(!required.contains(&"WaitMsBeforeAsync"));
}

// ── tools/call ─────────────────────────────────────────────────────────

#[test]
fn tools_call_returns_a_text_content_block() {
    // Deliberately shape-only. This path spawns `pwsh`, which does not exist on
    // a Linux CI runner, so the *outcome* is platform-dependent — but the
    // response envelope is not, and that is what a client parses. The execution
    // semantics are covered by unit tests with an injected executor instead.
    let response = request(json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "rtk_run",
            "arguments": { "CommandLine": "echo hello", "Cwd": "." }
        }
    }));

    let result = &response["result"];
    let content = result["content"].as_array().expect("content array");
    assert_eq!(content.len(), 1, "got: {response}");
    assert_eq!(content[0]["type"], "text");
    assert!(content[0]["text"].is_string(), "got: {response}");
    assert!(
        result["isError"].is_boolean(),
        "isError must always be present, got: {response}"
    );
}

#[test]
fn tools_call_still_answers_when_rtk_is_missing() {
    // rtk is optional at runtime: if it cannot be spawned there is simply no
    // rewrite, and the original command runs. The server must not fail the
    // call over it.
    let responses = converse_env(
        &[json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "arguments": { "CommandLine": "echo hello", "Cwd": "." } }
        })],
        &[("RTK_BIN", "definitely-not-an-installed-binary")],
    );

    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["content"][0]["type"], "text");
}

#[test]
fn tools_call_reports_a_shell_that_cannot_be_spawned() {
    // Portable on every platform, because the shell is guaranteed absent. This
    // is the path that used to be unreachable: `pwsh` was hardcoded, so on
    // Linux and macOS every call landed here with no way to configure a way out.
    let responses = converse_env(
        &[json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "arguments": { "CommandLine": "echo hello", "Cwd": "." } }
        })],
        &[("RTK_AGY_SHELL", "definitely-not-a-shell")],
    );

    let result = &responses[0]["result"];
    assert_eq!(result["isError"], true, "got: {result}");
    let text = result["content"][0]["text"].as_str().expect("text");
    assert!(
        text.contains("definitely-not-a-shell"),
        "the error must name the shell it tried: {text}"
    );
}

#[test]
fn tools_call_runs_through_an_overridden_shell() {
    // Proves the override is actually used for execution, not just accepted.
    // Both defaults can run this: `sh -c` everywhere, `pwsh -c` on Windows.
    let shell = if cfg!(windows) { "pwsh" } else { "sh" };
    let responses = converse_env(
        &[json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "arguments": { "CommandLine": "echo rtk-agy-marker", "Cwd": "." } }
        })],
        &[
            ("RTK_AGY_SHELL", shell),
            // Keep rtk out of it: a real rtk on PATH would rewrite `echo` and
            // the marker might never be printed.
            ("RTK_BIN", "definitely-not-an-installed-binary"),
        ],
    );

    let result = &responses[0]["result"];
    assert_eq!(result["isError"], false, "got: {result}");
    assert!(
        result["content"][0]["text"]
            .as_str()
            .expect("text")
            .contains("rtk-agy-marker"),
        "got: {result}"
    );
}

// ── Error handling and framing ─────────────────────────────────────────

#[test]
fn unknown_method_returns_method_not_found() {
    let response = request(json!({
        "jsonrpc": "2.0", "id": 4, "method": "resources/list", "params": {}
    }));

    assert_eq!(response["error"]["code"], -32601, "got: {response}");
    assert_eq!(response["error"]["message"], "Method not found");
    assert!(
        response.get("result").is_none(),
        "an error response must not also carry a result: {response}"
    );
}

#[test]
fn responses_echo_the_request_id_verbatim() {
    // JSON-RPC ids may be numbers or strings, and a client correlates on them.
    let numeric = request(json!({ "jsonrpc": "2.0", "id": 77, "method": "initialize" }));
    assert_eq!(numeric["id"], 77, "got: {numeric}");

    let string_id = request(json!({ "jsonrpc": "2.0", "id": "abc", "method": "initialize" }));
    assert_eq!(string_id["id"], "abc", "got: {string_id}");
}

#[test]
fn every_response_carries_the_jsonrpc_version() {
    for response in converse(&[
        initialize(),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" }),
    ]) {
        assert_eq!(response["jsonrpc"], "2.0", "got: {response}");
    }
}

#[test]
fn notifications_without_an_id_get_no_response() {
    // `notifications/initialized` is sent by every MCP client immediately after
    // the handshake. Answering it would corrupt the stream.
    let responses = converse(&[
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        initialize(),
    ]);
    assert_eq!(responses.len(), 1, "only the request may be answered");
    assert_eq!(responses[0]["id"], 1);
}

#[test]
fn requests_without_a_method_are_ignored() {
    let responses = converse(&[json!({ "jsonrpc": "2.0", "id": 9 }), initialize()]);
    assert_eq!(responses.len(), 1, "got: {responses:?}");
    assert_eq!(responses[0]["id"], 1);
}

#[test]
fn malformed_and_blank_lines_are_skipped_without_killing_the_server() {
    // The server must survive junk on the wire: one bad line cannot be allowed
    // to end the session, or a single client bug takes the whole server down.
    let raw = format!(
        "\n   \n}}{{ not json\n{}\nalso not json\n{}\n",
        initialize(),
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/list" })
    );
    let responses = converse_raw(&raw);

    assert_eq!(responses.len(), 2, "got: {responses:?}");
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[1]["id"], 5);
}

#[test]
fn requests_are_answered_in_order() {
    let responses = converse(&[
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "initialize" }),
    ]);

    let ids: Vec<i64> = responses
        .iter()
        .map(|r| r["id"].as_i64().expect("numeric id"))
        .collect();
    assert_eq!(ids, vec![1, 2, 3]);
}

#[test]
fn no_input_produces_no_output() {
    assert!(converse(&[]).is_empty());
}

// ── CLI surface ────────────────────────────────────────────────────────

#[test]
fn mcp_binary_reports_version_and_help() {
    let bin = env!("CARGO_BIN_EXE_rtk-mcp");
    assert!(run_flag(bin, "--version").contains("rtk-mcp"));

    let help = run_flag(bin, "--help");
    assert!(help.contains("rtk_run"), "help must name the tool: {help}");
}

#[test]
fn preinvocation_binary_reports_version_and_help() {
    let bin = env!("CARGO_BIN_EXE_rtk-hook-preinvocation");
    assert!(run_flag(bin, "--version").contains("rtk-hook-preinvocation"));
    assert!(run_flag(bin, "--help").contains("injectSteps"));

    // Both binaries ship from one crate, so their versions cannot legitimately
    // differ; if they ever do, something is being built from a stale artifact.
    assert_eq!(
        version_flag(bin),
        version_flag(env!("CARGO_BIN_EXE_rtk-mcp")),
        "the two binaries in this crate must report the same version"
    );
}

// ── PreInvocation hook ─────────────────────────────────────────────────

#[test]
fn preinvocation_emits_a_single_ephemeral_message() {
    let out = Command::new(env!("CARGO_BIN_EXE_rtk-hook-preinvocation"))
        .output()
        .expect("run hook");
    assert!(out.status.success());

    let payload: Value =
        serde_json::from_str(&String::from_utf8(out.stdout).expect("utf-8")).expect("valid JSON");

    let steps = payload["injectSteps"]
        .as_array()
        .expect("injectSteps array");
    assert_eq!(steps.len(), 1, "got: {payload}");
    assert!(steps[0]["ephemeralMessage"].is_string(), "got: {payload}");
}

#[test]
fn preinvocation_names_the_registered_server_and_tool() {
    // This is a cross-file contract with no compiler behind it: the message
    // tells the model to call ServerName='rtkmcp' / ToolName='rtk_run', and
    // those names must match mcp_config.json and tools/list respectively.
    // Rename either side alone and the hook silently points at nothing.
    let out = Command::new(env!("CARGO_BIN_EXE_rtk-hook-preinvocation"))
        .output()
        .expect("run hook");
    let payload: Value =
        serde_json::from_str(&String::from_utf8(out.stdout).expect("utf-8")).expect("valid JSON");
    let message = payload["injectSteps"][0]["ephemeralMessage"]
        .as_str()
        .expect("message");

    let config: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/mcp_config.json"))
            .expect("read mcp_config.json"),
    )
    .expect("valid mcp_config.json");
    let registered = config["mcpServers"]
        .as_object()
        .expect("mcpServers")
        .keys()
        .next()
        .expect("one registered server")
        .clone();

    assert!(
        message.contains(&format!("ServerName='{registered}'")),
        "hook must name the registered server {registered:?}: {message}"
    );

    let tool = request(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))["result"]
        ["tools"][0]["name"]
        .as_str()
        .expect("tool name")
        .to_string();
    assert!(
        message.contains(&format!("ToolName='{tool}'")),
        "hook must name the advertised tool {tool:?}: {message}"
    );
}
