//! A controllable MCP server, used only to test the prober.
//!
//! The real plugin servers cannot be asked to paginate, hang, die mid-handshake
//! or return an oversized payload on demand, and those are exactly the paths
//! `probe` exists to handle. This speaks the real line-delimited JSON-RPC over
//! real pipes — it is a stand-in for a server, not a mock of the code under
//! test, so the prober is exercised through its actual transport.
//!
//! Behaviour is chosen by argv:
//!   --pages N      answer `tools/list` in N pages, chaining `nextCursor`
//!   --no-prompts   answer `prompts/list` with method-not-found (-32601)
//!   --hang         never answer `tools/list`
//!   --die          exit(3) on the first request
//!   --huge BYTES   pad each tool description to BYTES, to trip the size cap
//!   --loop-cursor  always return the same cursor, to trip the page cap
//!   --wrong-key    return the tools under a key nobody asked for

use std::io::{BufRead, Write};

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);

    let pages: usize = arg_value(&args, "--pages")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let pad: usize = arg_value(&args, "--huge")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { return };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if has("--die") {
            std::process::exit(3);
        }

        // Notifications carry no id and get no reply.
        let Some(id) = msg.get("id").cloned() else {
            continue;
        };
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let reply = match method {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "probe-target", "version": "0" }
                }
            }),
            "tools/list" => {
                if has("--hang") {
                    // Answer nothing at all, but stay alive holding the pipe open.
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(60));
                    }
                }
                let cursor: usize = msg
                    .get("params")
                    .and_then(|p| p.get("cursor"))
                    .and_then(|c| c.as_str())
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(0);
                let key = if has("--wrong-key") {
                    "toolList"
                } else {
                    "tools"
                };
                let mut result = serde_json::json!({
                    key: [{
                        "name": format!("tool_page_{cursor}"),
                        "description": "x".repeat(pad.max(1)),
                        "inputSchema": { "type": "object", "properties": {} }
                    }]
                });
                let more = if has("--loop-cursor") {
                    true
                } else {
                    cursor + 1 < pages
                };
                if more {
                    let next = if has("--loop-cursor") { 0 } else { cursor + 1 };
                    result["nextCursor"] = serde_json::json!(next.to_string());
                }
                serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result })
            }
            "prompts/list" if has("--no-prompts") => serde_json::json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "Method not found" }
            }),
            "prompts/list" => serde_json::json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "prompts": [{ "name": "a_prompt", "description": "p" }] }
            }),
            _ => serde_json::json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "Method not found" }
            }),
        };

        if writeln!(stdout, "{reply}").is_err() || stdout.flush().is_err() {
            return;
        }
    }
}
