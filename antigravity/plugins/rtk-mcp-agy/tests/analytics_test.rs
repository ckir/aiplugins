//! Integration and contract tests for the analytics tools in `rtk-mcp-agy`.

use rtk_mcp_agy::{
    handle_line, handle_request, Execution, TOOL_CHECK, TOOL_DISCOVER, TOOL_GAIN, TOOL_NAME,
};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn test_rtk_gain_tool_call() {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": TOOL_GAIN,
            "arguments": { "history": true }
        }
    });
    let res = handle_request(&req);
    assert!(res.is_some());
    let res = res.unwrap();
    assert_eq!(res["id"], 1);
    assert_eq!(res["jsonrpc"], "2.0");
    assert!(res["result"]["content"].is_array());
    assert_eq!(res["result"]["isError"], false);
}

#[test]
fn test_rtk_check_tool_call() {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": TOOL_CHECK,
            "arguments": { "command": "grep -rn foo src/" }
        }
    });
    let res = handle_request(&req);
    assert!(res.is_some());
    let res = res.unwrap();
    assert_eq!(res["id"], 2);
    assert_eq!(res["jsonrpc"], "2.0");
    assert_eq!(res["result"]["content"][0]["type"], "text");
    assert_eq!(res["result"]["isError"], false);
}

#[test]
fn test_rtk_discover_tool_call() {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": TOOL_DISCOVER,
            "arguments": {
                "project": "myproject",
                "limit": 10,
                "since": 14,
                "all_projects": true,
                "format": "text"
            }
        }
    });
    let res = handle_request(&req);
    assert!(res.is_some());
    let res = res.unwrap();
    assert_eq!(res["id"], 3);
    assert_eq!(res["jsonrpc"], "2.0");
    assert_eq!(res["result"]["content"][0]["type"], "text");
    assert_eq!(res["result"]["isError"], false);
}

#[test]
fn test_analytics_dispatch_with_mock_executor() {
    let log = RefCell::new(Vec::new());
    let recording_exec = |cmd: &str, cwd: &str| {
        log.borrow_mut().push((cmd.to_string(), cwd.to_string()));
        Execution::Ran {
            stdout: format!("output of: {cmd}"),
            stderr: String::new(),
            success: true,
        }
    };
    let no_rewrite = |_: &str| None;

    // rtk_gain dispatch
    let gain_req = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": TOOL_GAIN,
            "arguments": {
                "history": true,
                "project_only": true,
                "format": "json",
                "Cwd": "/work"
            }
        }
    })
    .to_string();
    let resp = handle_line(&gain_req, &no_rewrite, &recording_exec).expect("response");
    assert_eq!(
        resp["result"]["content"][0]["text"],
        "output of: rtk gain --history --project --format json"
    );
    assert_eq!(resp["result"]["isError"], false);

    // rtk_check dispatch
    let check_req = json!({
        "jsonrpc": "2.0",
        "id": 11,
        "method": "tools/call",
        "params": {
            "name": TOOL_CHECK,
            "arguments": { "command": "grep -rn foo src/", "Cwd": "/tmp" }
        }
    })
    .to_string();
    let resp = handle_line(&check_req, &no_rewrite, &recording_exec).expect("response");
    assert_eq!(
        resp["result"]["content"][0]["text"],
        "output of: rtk hook check -- 'grep -rn foo src/'"
    );
    assert_eq!(resp["result"]["isError"], false);

    // rtk_discover dispatch
    let disc_req = json!({
        "jsonrpc": "2.0",
        "id": 12,
        "method": "tools/call",
        "params": {
            "name": TOOL_DISCOVER,
            "arguments": {
                "project": "proj",
                "limit": 5,
                "since": 7,
                "all_projects": true,
                "format": "text"
            }
        }
    })
    .to_string();
    let resp = handle_line(&disc_req, &no_rewrite, &recording_exec).expect("response");
    assert_eq!(
        resp["result"]["content"][0]["text"],
        "output of: rtk discover --project 'proj' --limit 5 --since 7 --all --format text"
    );
    assert_eq!(resp["result"]["isError"], false);

    assert_eq!(
        *log.borrow(),
        vec![
            (
                "rtk gain --history --project --format json".to_string(),
                "/work".to_string()
            ),
            (
                "rtk hook check -- 'grep -rn foo src/'".to_string(),
                "/tmp".to_string()
            ),
            (
                "rtk discover --project 'proj' --limit 5 --since 7 --all --format text".to_string(),
                ".".to_string()
            ),
        ]
    );
}

#[test]
fn test_unknown_tool_returns_error() {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "tools/call",
        "params": {
            "name": "non_existent_tool",
            "arguments": {}
        }
    });
    let res = handle_request(&req).expect("response");
    assert_eq!(res["id"], 20);
    assert_eq!(res["result"]["isError"], true);
    assert_eq!(
        res["result"]["content"][0]["text"],
        "Unknown tool: non_existent_tool"
    );
}

#[test]
fn test_validation_errors_return_error_response() {
    // Empty command in rtk_check
    let req_check = json!({
        "jsonrpc": "2.0",
        "id": 21,
        "method": "tools/call",
        "params": {
            "name": TOOL_CHECK,
            "arguments": { "command": "   " }
        }
    });
    let res = handle_request(&req_check).expect("response");
    assert_eq!(res["id"], 21);
    assert_eq!(res["result"]["isError"], true);
    assert_eq!(
        res["result"]["content"][0]["text"],
        "command must not be empty"
    );

    // Invalid format in rtk_gain
    let req_gain = json!({
        "jsonrpc": "2.0",
        "id": 22,
        "method": "tools/call",
        "params": {
            "name": TOOL_GAIN,
            "arguments": { "format": "unsupported" }
        }
    });
    let res = handle_request(&req_gain).expect("response");
    assert_eq!(res["id"], 22);
    assert_eq!(res["result"]["isError"], true);
    assert!(res["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("unsupported format"));
}

#[test]
fn test_analytics_stdio_e2e() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtk-mcp"));
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("RTK_AGY_SHELL", if cfg!(windows) { "pwsh" } else { "sh" });

    let mut child = command.spawn().expect("spawn rtk-mcp");

    {
        let mut stdin = child.stdin.take().expect("stdin");
        let requests = [
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/list",
                "params": {}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": TOOL_GAIN,
                    "arguments": { "history": true }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": TOOL_CHECK,
                    "arguments": { "command": "grep -rn foo src/" }
                }
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": {
                    "name": TOOL_DISCOVER,
                    "arguments": {}
                }
            }),
        ];
        for req in requests {
            writeln!(stdin, "{req}").expect("write request");
        }
    }

    let out = child.wait_with_output().expect("wait for rtk-mcp");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).expect("utf-8 stdout");
    let responses: Vec<Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("valid JSON line"))
        .collect();

    assert_eq!(responses.len(), 4);
    assert_eq!(responses[0]["id"], 1);
    let tools = responses[0]["result"]["tools"]
        .as_array()
        .expect("tools array");
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec![TOOL_NAME, TOOL_GAIN, TOOL_CHECK, TOOL_DISCOVER]);

    // rtk_gain response
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[1]["result"]["content"][0]["type"], "text");
    assert!(responses[1]["result"]["isError"].is_boolean());

    // rtk_check response
    assert_eq!(responses[2]["id"], 3);
    assert_eq!(responses[2]["result"]["content"][0]["type"], "text");
    assert!(responses[2]["result"]["isError"].is_boolean());

    // rtk_discover response
    assert_eq!(responses[3]["id"], 4);
    assert_eq!(responses[3]["result"]["content"][0]["type"], "text");
    assert!(responses[3]["result"]["isError"].is_boolean());

    // If rtk binary is available on PATH, rtk hook check should succeed cleanly:
    if Command::new("rtk").arg("--version").output().is_ok() {
        assert_eq!(responses[2]["result"]["isError"], false);
    }
}
