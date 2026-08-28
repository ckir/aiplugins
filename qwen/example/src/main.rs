use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        let request: Value = serde_json::from_str(&line)?;

        if let Some(method) = request.get("method").and_then(|v| v.as_str()) {
            let result = match method {
                "initialize" => {
                    json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "qwen-example-mcp",
                            "version": "0.1.0"
                        }
                    })
                }
                "tools/list" => {
                    json!({
                        "tools": [{
                            "name": "count_words",
                            "description": "Count the words and characters in a passage of text.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "text": {
                                        "type": "string",
                                        "description": "The text to measure."
                                    }
                                },
                                "required": ["text"]
                            }
                        }]
                    })
                }
                "tools/call" => {
                    if let Some(name) = request
                        .get("params")
                        .and_then(|p| p.get("name"))
                        .and_then(|v| v.as_str())
                    {
                        if name == "count_words" {
                            if let Some(text) = request
                                .get("params")
                                .and_then(|p| p.get("arguments"))
                                .and_then(|a| a.get("text"))
                                .and_then(|v| v.as_str())
                            {
                                let words = if text.trim().is_empty() {
                                    0
                                } else {
                                    text.split_whitespace().count()
                                };
                                let characters = text.len();
                                let characters_no_spaces =
                                    text.replace(char::is_whitespace, "").len();
                                json!({
                                    "content": [{
                                        "type": "text",
                                        "text": json!({
                                            "words": words,
                                            "characters": characters,
                                            "charactersNoSpaces": characters_no_spaces
                                        }).to_string()
                                    }]
                                })
                            } else {
                                json!({"error": "Missing 'text' argument"})
                            }
                        } else {
                            json!({"error": format!("Unknown tool: {}", name)})
                        }
                    } else {
                        json!({"error": "Missing tool name"})
                    }
                }
                "ping" => json!({}),
                _ => {
                    // Send error response for unknown methods
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "error": {
                            "code": -32601,
                            "message": format!("Method not found: {}", method)
                        }
                    });
                    writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                    stdout.flush()?;
                    continue;
                }
            };

            let id = request.get("id").cloned();
            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            });
            writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
            stdout.flush()?;
        }
    }

    Ok(())
}
