use serde_json::{json, Value};
use std::env;
use std::io::{self, BufRead, Write};
use std::process::Command;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rtk-mcp {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "rtk-mcp {} — MCP server for Antigravity rtk shell command optimization",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        println!("Usage: rtk-mcp");
        println!("  JSON-RPC MCP server over stdin/stdout. Exposes a single tool:");
        println!(
            "    rtk_run  — Rewrites the given CommandLine via `rtk rewrite` and executes it."
        );
        return;
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }

        let Ok(req): Result<Value, _> = serde_json::from_str(&line) else {
            continue;
        };

        let Some(id) = req.get("id") else {
            // It might be a notification (like initialized), which has no id.
            continue;
        };

        let Some(method) = req.get("method").and_then(|m| m.as_str()) else {
            continue;
        };

        let result = match method {
            "initialize" => {
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "rtkmcp", "version": "0.1.0" }
                })
            }
            "tools/list" => {
                json!({
                    "tools": [{
                        "name": "rtk_run",
                        "description": "Run an rtk optimized shell command",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "CommandLine": { "type": "string" },
                                "Cwd": { "type": "string" },
                                "WaitMsBeforeAsync": { "type": "integer" },
                                "toolAction": {
                                    "type": "string",
                                    "description": "Brief 2-5 word summary of what this tool is doing."
                                },
                                "toolSummary": {
                                    "type": "string",
                                    "description": "Brief 2-5 word noun phrase describing what this tool call is about."
                                }
                            },
                            "required": ["CommandLine", "Cwd", "toolAction", "toolSummary"]
                        }
                    }]
                })
            }
            "tools/call" => {
                let default_args = json!({});
                let args = req
                    .get("params")
                    .and_then(|p| p.get("arguments"))
                    .unwrap_or(&default_args);
                let cmd = args
                    .get("CommandLine")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                let cwd = args.get("Cwd").and_then(|c| c.as_str()).unwrap_or(".");

                let rewritten = rtk_rewrite(cmd).unwrap_or_else(|| cmd.to_string());

                let output = Command::new("pwsh")
                    .arg("-c")
                    .arg(&rewritten)
                    .current_dir(cwd)
                    .output();

                match output {
                    Ok(out) => {
                        let stdout_str = String::from_utf8_lossy(&out.stdout).to_string();
                        let stderr_str = String::from_utf8_lossy(&out.stderr).to_string();
                        let mut text = stdout_str;
                        if !stderr_str.is_empty() {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(&stderr_str);
                        }
                        json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": !out.status.success()
                        })
                    }
                    Err(e) => {
                        json!({
                            "content": [{ "type": "text", "text": e.to_string() }],
                            "isError": true
                        })
                    }
                }
            }
            _ => {
                // Method not found
                json!({
                    "error": {
                        "code": -32601,
                        "message": "Method not found"
                    }
                })
            }
        };

        // If it's an error response, we format differently
        let response = if result.get("error").is_some() {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": result.get("error").unwrap()
            })
        } else {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            })
        };

        if let Ok(response_str) = serde_json::to_string(&response) {
            println!("{}", response_str);
            let _ = stdout.flush();
        }
    }
}

fn rtk_rewrite(command: &str) -> Option<String> {
    let output = Command::new("rtk")
        .arg("rewrite")
        .arg(command)
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}
