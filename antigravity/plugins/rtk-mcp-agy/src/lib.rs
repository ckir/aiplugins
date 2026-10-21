//! Shared logic for the `rtk-mcp-agy` plugin.
//!
//! Everything here is pure: JSON-RPC dispatch, response shaping, and the
//! `PreInvocation` payload. The two binaries in `src/bin/` only move bytes and
//! spawn processes.
//!
//! The split exists because the interesting behaviour is otherwise unreachable
//! by a test. [`handle_line`] takes the rewriter and the executor as closures,
//! so the `tools/call` path — which in production shells out to `rtk` and then
//! to a shell — can be exercised on a machine that has neither installed, and
//! its failure modes can be forced rather than waited for.
//!
//! The seam earned itself immediately: it is what made the hardcoded `pwsh`
//! visible as a portability bug (see [`resolve_shell`]) rather than something
//! only a Linux user would ever discover.

use serde::Serialize;
use serde_json::{json, Value};

/// The server name an MCP client sees, and the key `mcp_config.json` registers.
pub const SERVER_NAME: &str = "rtkmcp";
/// Run an rtk-optimized command.
pub const TOOL_NAME: &str = "rtk_run";
/// Report rtk token savings.
pub const TOOL_GAIN: &str = "rtk_gain";
/// Show how rtk would rewrite a shell command without running it.
pub const TOOL_CHECK: &str = "rtk_check";
/// Analyze past session history for missed token savings.
pub const TOOL_DISCOVER: &str = "rtk_discover";
/// The MCP protocol revision spoken by this server.
pub const PROTOCOL_VERSION: &str = "2024-11-05";
/// JSON-RPC's "method not found".
pub const METHOD_NOT_FOUND: i64 = -32601;

/// The crate version, and the single source for every place one is reported.
///
/// Both binaries' `--version` and the `initialize` handshake read this. They
/// used to compute it separately, and the handshake's copy was a literal that
/// drifted to `0.1.0` while the crate was at `0.1.5`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ── External programs ──────────────────────────────────────────────────

/// The rtk executable, when `RTK_BIN` says nothing.
pub const DEFAULT_RTK_BIN: &str = "rtk";
/// The shell used to run commands on Windows.
pub const DEFAULT_WINDOWS_SHELL: &str = "pwsh";
/// The shell used to run commands everywhere else.
///
/// `sh` rather than `bash`: POSIX guarantees it exists, and the commands rtk
/// produces do not need bash extensions.
pub const DEFAULT_UNIX_SHELL: &str = "sh";

/// Decide which rtk executable to invoke.
///
/// `RTK_BIN` is spelled without a plugin prefix on purpose: it is the same
/// variable the sibling `rtk-mcp-qwen` and `rtk-mcp-cc` plugins honour, so a
/// user who relocates rtk sets one variable for all three.
pub fn resolve_rtk_bin(override_value: Option<String>) -> String {
    non_blank(override_value).unwrap_or_else(|| DEFAULT_RTK_BIN.to_string())
}

/// Decide which shell runs the rewritten command.
///
/// This used to be a hardcoded `pwsh`, which meant the `tools/call` path could
/// not execute anything at all on Linux or macOS — PowerShell 7 is rarely
/// installed there. Windows keeps `pwsh` so existing installs are unaffected;
/// everywhere else falls back to `sh`. `RTK_AGY_SHELL` overrides both.
pub fn resolve_shell(override_value: Option<String>, is_windows: bool) -> String {
    non_blank(override_value).unwrap_or_else(|| {
        if is_windows {
            DEFAULT_WINDOWS_SHELL.to_string()
        } else {
            DEFAULT_UNIX_SHELL.to_string()
        }
    })
}

/// Trim a value, discarding it if nothing is left.
///
/// An environment variable set to the empty string is how a shell spells
/// "unset" often enough that treating it as a real value would be a trap.
fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

// ── Execution ──────────────────────────────────────────────────────────

/// The outcome of trying to run a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Execution {
    /// The process ran. It may still have failed, hence `success`.
    Ran {
        stdout: String,
        stderr: String,
        success: bool,
    },
    /// The process could not be spawned at all; carries the OS error text.
    NotRun(String),
}

/// Render an execution as the `tools/call` result body.
///
/// stdout and stderr are joined with a single newline, and the separator is
/// omitted when either side is empty — so a command that writes only to stderr
/// does not come back with a leading blank line.
pub fn format_execution(execution: &Execution) -> Value {
    let (text, is_error) = match execution {
        Execution::Ran {
            stdout,
            stderr,
            success,
        } => {
            let mut text = stdout.clone();
            if !stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(stderr);
            }
            (text, !success)
        }
        Execution::NotRun(message) => (message.clone(), true),
    };

    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error
    })
}

/// Interpret the stdout of `rtk rewrite`.
///
/// Empty output means rtk had no rewrite to offer, which is not an error: the
/// caller falls back to the original command. Only known commands are covered,
/// so most invocations legitimately land here.
pub fn parse_rewrite(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Pull `CommandLine` and `Cwd` out of a `tools/call` request.
///
/// Missing or non-string values fall back to `""` and `"."`. A malformed call
/// therefore runs an empty command in the current directory rather than
/// panicking the server.
pub fn call_arguments(request: &Value) -> (String, String) {
    let empty = json!({});
    let arguments = request
        .get("params")
        .and_then(|p| p.get("arguments"))
        .unwrap_or(&empty);

    let command = arguments
        .get("CommandLine")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let cwd = arguments
        .get("Cwd")
        .and_then(|c| c.as_str())
        .unwrap_or(".")
        .to_string();

    (command, cwd)
}

/// Quote a string safely for inclusion in a shell command line.
///
/// Wraps the string in single quotes, replacing any internal single quote `'`
/// with `'\''`. Single-quoted strings are interpreted literally by POSIX shells
/// and PowerShell, preventing command injection and shell metacharacter expansion.
pub fn shell_quote(s: &str) -> String {
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn validate_format(value: &str, allowed: &[&str]) -> Result<String, String> {
    let lowered = value.trim().to_ascii_lowercase();
    if allowed.contains(&lowered.as_str()) {
        Ok(lowered)
    } else {
        Err(format!(
            "unsupported format {value:?}; expected one of {}",
            allowed.join(", ")
        ))
    }
}

/// Extract the `Cwd` argument or fall back to `"."`.
pub fn extract_cwd(arguments: &Value) -> String {
    arguments
        .get("Cwd")
        .and_then(|c| c.as_str())
        .unwrap_or(".")
        .to_string()
}

/// Build the command line for `rtk gain`.
pub fn build_gain_command(arguments: &Value) -> Result<String, String> {
    let mut cmd = "rtk gain".to_string();
    if arguments
        .get("history")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        cmd.push_str(" --history");
    }
    if arguments
        .get("project_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        cmd.push_str(" --project");
    }
    if let Some(format) = arguments.get("format").and_then(|v| v.as_str()) {
        let trimmed = format.trim();
        if !trimmed.is_empty() {
            let fmt = validate_format(trimmed, &["text", "json", "csv"])?;
            cmd.push_str(&format!(" --format {fmt}"));
        }
    }
    Ok(cmd)
}

/// Build the command line for `rtk hook check`.
///
/// The dry-run inspection subcommand in `rtk` is `hook check` (not `check`).
/// The command argument is safely single-quoted via [`shell_quote`] to prevent
/// arbitrary command injection when executed via a shell.
pub fn build_check_command(arguments: &Value) -> Result<String, String> {
    let command = arguments
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if command.is_empty() {
        return Err("command must not be empty".to_string());
    }
    Ok(format!("rtk hook check -- {}", shell_quote(command)))
}

/// Build the command line for `rtk discover`.
pub fn build_discover_command(arguments: &Value) -> Result<String, String> {
    let mut cmd = "rtk discover".to_string();
    if let Some(project) = arguments.get("project").and_then(|v| v.as_str()) {
        let p = project.trim();
        if !p.is_empty() {
            cmd.push_str(&format!(" --project {}", shell_quote(p)));
        }
    }
    if let Some(limit) = arguments.get("limit").and_then(|v| v.as_u64()) {
        cmd.push_str(&format!(" --limit {limit}"));
    }
    if let Some(since) = arguments.get("since").and_then(|v| v.as_u64()) {
        cmd.push_str(&format!(" --since {since}"));
    }
    if arguments
        .get("all_projects")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        cmd.push_str(" --all");
    }
    if let Some(format) = arguments.get("format").and_then(|v| v.as_str()) {
        let trimmed = format.trim();
        if !trimmed.is_empty() {
            let fmt = validate_format(trimmed, &["text", "json"])?;
            cmd.push_str(&format!(" --format {fmt}"));
        }
    }
    Ok(cmd)
}

// ── Method results ─────────────────────────────────────────────────────

/// The `initialize` result.
pub fn server_info() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": version() }
    })
}

/// The `tools/list` result advertising all 4 tools.
pub fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": TOOL_NAME,
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
            },
            {
                "name": TOOL_GAIN,
                "description": "Report rtk's token-savings statistics: how many tokens rtk has saved, and optionally the recent per-command history. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "history": {
                            "type": "boolean",
                            "description": "Include the recent per-command history rather than the summary alone."
                        },
                        "project_only": {
                            "type": "boolean",
                            "description": "Restrict the statistics to the current working project."
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: `text` (default), `json`, or `csv`."
                        }
                    }
                }
            },
            {
                "name": TOOL_CHECK,
                "description": "Show how rtk would rewrite a given shell command, without running it. Use this to explain why a command was or was not optimized. Read-only: the command is analyzed, never executed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to analyze, e.g. `grep -rn foo src/`. It is never executed — this reports only how rtk would rewrite it."
                        }
                    },
                    "required": ["command"]
                }
            },
            {
                "name": TOOL_DISCOVER,
                "description": "Analyze past session history for commands that rtk could have optimized but did not, i.e. missed token savings. Read-only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": {
                            "type": "string",
                            "description": "Filter to projects whose path contains this substring."
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum commands reported per section. rtk's default is 15."
                        },
                        "since": {
                            "type": "integer",
                            "description": "Only consider sessions from the last N days. rtk's default is 30."
                        },
                        "all_projects": {
                            "type": "boolean",
                            "description": "Scan every project rather than only the current one."
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: `text` (default) or `json`."
                        }
                    }
                }
            }
        ]
    })
}

/// The body used for an unrecognised method.
pub fn method_not_found() -> Value {
    json!({
        "error": {
            "code": METHOD_NOT_FOUND,
            "message": "Method not found"
        }
    })
}

// ── Dispatch ───────────────────────────────────────────────────────────

/// Dispatch a parsed JSON-RPC request to the appropriate method handler.
pub fn dispatch_request<R, E>(request: &Value, rewrite: &R, exec: &E) -> Option<Value>
where
    R: Fn(&str) -> Option<String>,
    E: Fn(&str, &str) -> Execution,
{
    // A notification has no id and must not be answered.
    let id = request.get("id")?;
    let method = request.get("method").and_then(|m| m.as_str())?;

    let result = match method {
        "initialize" => server_info(),
        "tools/list" => tools_list(),
        "tools/call" => {
            let empty = json!({});
            let params = request.get("params").unwrap_or(&empty);
            let tool_name = params
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(TOOL_NAME);
            let arguments = params.get("arguments").unwrap_or(&empty);
            let cwd = extract_cwd(arguments);

            match tool_name {
                TOOL_GAIN => match build_gain_command(arguments) {
                    Ok(cmd) => format_execution(&exec(&cmd, &cwd)),
                    Err(err) => json!({
                        "content": [{ "type": "text", "text": err }],
                        "isError": true
                    }),
                },
                TOOL_CHECK => match build_check_command(arguments) {
                    Ok(cmd) => format_execution(&exec(&cmd, &cwd)),
                    Err(err) => json!({
                        "content": [{ "type": "text", "text": err }],
                        "isError": true
                    }),
                },
                TOOL_DISCOVER => match build_discover_command(arguments) {
                    Ok(cmd) => format_execution(&exec(&cmd, &cwd)),
                    Err(err) => json!({
                        "content": [{ "type": "text", "text": err }],
                        "isError": true
                    }),
                },
                TOOL_NAME => {
                    let (command, cwd) = call_arguments(request);
                    let to_run = rewrite(&command).unwrap_or_else(|| command.clone());
                    format_execution(&exec(&to_run, &cwd))
                }
                _ => json!({
                    "content": [{ "type": "text", "text": format!("Unknown tool: {tool_name}") }],
                    "isError": true
                }),
            }
        }
        _ => method_not_found(),
    };

    Some(envelope(id, result))
}

/// Handle one line of input, returning the response to write — or `None` when
/// the server must stay silent.
///
/// Silence is the correct answer more often than it looks: blank lines,
/// unparsable JSON, and notifications (which carry no `id`) all arrive on a
/// healthy connection. `notifications/initialized` in particular is sent by
/// every MCP client right after the handshake, and answering it would corrupt
/// the stream.
///
/// `rewrite` maps a command to its rtk-optimized form, and `exec` runs the
/// resulting command in a directory. Both are injected so the whole dispatch
/// can be tested without spawning anything.
pub fn handle_line<R, E>(line: &str, rewrite: &R, exec: &E) -> Option<Value>
where
    R: Fn(&str) -> Option<String>,
    E: Fn(&str, &str) -> Execution,
{
    if line.trim().is_empty() {
        return None;
    }

    let request: Value = serde_json::from_str(line).ok()?;
    dispatch_request(&request, rewrite, exec)
}

/// Handle a parsed JSON-RPC request using default fallback closures.
pub fn handle_request(request: &Value) -> Option<Value> {
    dispatch_request(request, &|_| None, &|_, _| Execution::Ran {
        stdout: String::new(),
        stderr: String::new(),
        success: true,
    })
}

/// Wrap a result body in its JSON-RPC envelope.
///
/// An error body is hoisted to the top-level `error` member; anything else
/// becomes `result`. A response carries one or the other, never both.
fn envelope(id: &Value, result: Value) -> Value {
    match result.get("error") {
        Some(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        None => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    }
}

// ── PreInvocation hook ─────────────────────────────────────────────────

/// The payload the `PreInvocation` hook writes to stdout.
#[derive(Debug, Serialize)]
pub struct PreInvocationOutput {
    #[serde(rename = "injectSteps")]
    pub inject_steps: Vec<Value>,
}

/// Build the injected instruction that steers the model onto the MCP tool.
///
/// Antigravity's `PreToolUse` hook cannot rewrite tool arguments, so the model
/// has to be told to call the MCP tool instead of its native `run_command`.
/// The server and tool names are interpolated from the constants above rather
/// than spelled out, so this message cannot drift from what the server actually
/// advertises.
pub fn preinvocation_payload() -> PreInvocationOutput {
    PreInvocationOutput {
        inject_steps: vec![json!({
            "ephemeralMessage": format!(
                "CRITICAL: Do NOT use the native `run_command` tool. You MUST use the \
                 `call_mcp_tool` with ServerName='{SERVER_NAME}' and ToolName='{TOOL_NAME}' \
                 for all shell commands to ensure token-optimized execution."
            )
        })],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    // Closures for the dispatch tests. `never` panics so a test proves a path
    // was not taken, rather than merely not asserting on it.
    fn no_rewrite(_: &str) -> Option<String> {
        None
    }
    fn never_exec(_: &str, _: &str) -> Execution {
        panic!("the executor must not run for this method")
    }
    fn never_rewrite(_: &str) -> Option<String> {
        panic!("the rewriter must not run for this method")
    }
    fn ok_exec(_: &str, _: &str) -> Execution {
        Execution::Ran {
            stdout: "ok".to_string(),
            stderr: String::new(),
            success: true,
        }
    }

    // ── version ────────────────────────────────────────────────────

    #[test]
    fn version_matches_the_crate() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
        assert!(!version().is_empty());
    }

    #[test]
    fn handshake_version_comes_from_the_crate() {
        // The original defect: a literal here drifted to 0.1.0 while the crate
        // was at 0.1.5, and the handshake is the only version a client sees.
        assert_eq!(server_info()["serverInfo"]["version"], version());
    }

    // ── external program resolution ────────────────────────────────

    #[test]
    fn rtk_bin_defaults_to_the_path_lookup() {
        assert_eq!(resolve_rtk_bin(None), DEFAULT_RTK_BIN);
    }

    #[test]
    fn rtk_bin_honours_an_override() {
        assert_eq!(
            resolve_rtk_bin(Some("/opt/rtk/bin/rtk".to_string())),
            "/opt/rtk/bin/rtk"
        );
        assert_eq!(resolve_rtk_bin(Some("  rtk-dev  ".to_string())), "rtk-dev");
    }

    #[test]
    fn a_blank_rtk_bin_does_not_erase_the_default() {
        // `RTK_BIN=` is how a shell commonly spells "unset"; taking it
        // literally would try to spawn the empty string.
        for blank in ["", "   ", "\t"] {
            assert_eq!(resolve_rtk_bin(Some(blank.to_string())), DEFAULT_RTK_BIN);
        }
    }

    #[test]
    fn the_shell_defaults_per_platform() {
        // The bug this fixes: a hardcoded `pwsh` made tools/call unable to run
        // anything on Linux or macOS.
        assert_eq!(resolve_shell(None, true), DEFAULT_WINDOWS_SHELL);
        assert_eq!(resolve_shell(None, false), DEFAULT_UNIX_SHELL);
        assert_ne!(
            DEFAULT_UNIX_SHELL, DEFAULT_WINDOWS_SHELL,
            "the whole point is that these differ"
        );
    }

    #[test]
    fn the_shell_override_wins_on_every_platform() {
        for windows in [true, false] {
            assert_eq!(
                resolve_shell(Some("bash".to_string()), windows),
                "bash",
                "override must apply regardless of platform"
            );
        }
    }

    #[test]
    fn a_blank_shell_override_falls_back_to_the_platform_default() {
        assert_eq!(
            resolve_shell(Some("  ".to_string()), true),
            DEFAULT_WINDOWS_SHELL
        );
        assert_eq!(
            resolve_shell(Some(String::new()), false),
            DEFAULT_UNIX_SHELL
        );
    }

    // ── parse_rewrite ──────────────────────────────────────────────

    #[test]
    fn parse_rewrite_reads_a_rewritten_command() {
        assert_eq!(
            parse_rewrite("rtk grep -rn foo src/\n"),
            Some("rtk grep -rn foo src/".to_string())
        );
    }

    #[test]
    fn parse_rewrite_treats_empty_output_as_no_rewrite() {
        assert_eq!(parse_rewrite(""), None);
        assert_eq!(parse_rewrite("   "), None);
        assert_eq!(parse_rewrite("\n\t \r\n"), None);
    }

    #[test]
    fn parse_rewrite_trims_only_the_edges() {
        assert_eq!(
            parse_rewrite("  rg  foo   bar  "),
            Some("rg  foo   bar".to_string()),
            "internal spacing is part of the command"
        );
    }

    #[test]
    fn parse_rewrite_keeps_multiline_output_whole() {
        assert_eq!(
            parse_rewrite("line one\nline two\n"),
            Some("line one\nline two".to_string())
        );
    }

    // ── call_arguments ─────────────────────────────────────────────

    #[test]
    fn call_arguments_reads_both_fields() {
        let req = json!({ "params": { "arguments": { "CommandLine": "ls -la", "Cwd": "/tmp" } } });
        assert_eq!(
            call_arguments(&req),
            ("ls -la".to_string(), "/tmp".to_string())
        );
    }

    #[test]
    fn call_arguments_defaults_the_working_directory() {
        let req = json!({ "params": { "arguments": { "CommandLine": "ls" } } });
        assert_eq!(call_arguments(&req).1, ".");
    }

    #[test]
    fn call_arguments_defaults_a_missing_command_to_empty() {
        let req = json!({ "params": { "arguments": { "Cwd": "/tmp" } } });
        assert_eq!(call_arguments(&req).0, "");
    }

    #[test]
    fn call_arguments_survives_a_missing_arguments_object() {
        assert_eq!(
            call_arguments(&json!({ "params": {} })),
            (String::new(), ".".to_string())
        );
    }

    #[test]
    fn call_arguments_survives_missing_params() {
        assert_eq!(call_arguments(&json!({})), (String::new(), ".".to_string()));
    }

    #[test]
    fn call_arguments_ignores_non_string_values() {
        // A client sending the wrong type must not take the server down.
        let req = json!({ "params": { "arguments": { "CommandLine": 42, "Cwd": ["/tmp"] } } });
        assert_eq!(call_arguments(&req), (String::new(), ".".to_string()));
    }

    // ── format_execution ───────────────────────────────────────────

    fn text_of(value: &Value) -> String {
        value["content"][0]["text"].as_str().unwrap().to_string()
    }

    #[test]
    fn format_execution_returns_stdout_alone() {
        let out = format_execution(&Execution::Ran {
            stdout: "hello".to_string(),
            stderr: String::new(),
            success: true,
        });
        assert_eq!(text_of(&out), "hello");
        assert_eq!(out["isError"], false);
        assert_eq!(out["content"][0]["type"], "text");
    }

    #[test]
    fn format_execution_returns_stderr_alone_without_a_leading_newline() {
        let out = format_execution(&Execution::Ran {
            stdout: String::new(),
            stderr: "boom".to_string(),
            success: false,
        });
        assert_eq!(text_of(&out), "boom", "no separator when stdout is empty");
        assert_eq!(out["isError"], true);
    }

    #[test]
    fn format_execution_joins_both_streams_with_one_newline() {
        let out = format_execution(&Execution::Ran {
            stdout: "out".to_string(),
            stderr: "err".to_string(),
            success: true,
        });
        assert_eq!(text_of(&out), "out\nerr");
    }

    #[test]
    fn format_execution_handles_silence() {
        let out = format_execution(&Execution::Ran {
            stdout: String::new(),
            stderr: String::new(),
            success: true,
        });
        assert_eq!(text_of(&out), "");
        assert_eq!(out["isError"], false);
    }

    #[test]
    fn format_execution_marks_a_failed_exit() {
        let out = format_execution(&Execution::Ran {
            stdout: "partial".to_string(),
            stderr: String::new(),
            success: false,
        });
        assert_eq!(
            out["isError"], true,
            "a non-zero exit is an error even with output"
        );
    }

    #[test]
    fn format_execution_reports_a_spawn_failure() {
        let out = format_execution(&Execution::NotRun("pwsh not found".to_string()));
        assert_eq!(text_of(&out), "pwsh not found");
        assert_eq!(out["isError"], true);
    }

    // ── method bodies ──────────────────────────────────────────────

    #[test]
    fn server_info_declares_protocol_name_and_tools() {
        let info = server_info();
        assert_eq!(info["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(info["serverInfo"]["name"], SERVER_NAME);
        assert!(
            info["capabilities"]["tools"].is_object(),
            "without this a client never calls tools/list"
        );
    }

    #[test]
    fn tools_list_advertises_four_tools() {
        let tools = tools_list();
        let list = tools["tools"].as_array().unwrap();
        assert_eq!(list.len(), 4);
        let names: Vec<&str> = list.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec![TOOL_NAME, TOOL_GAIN, TOOL_CHECK, TOOL_DISCOVER]);
    }

    #[test]
    fn tools_list_requires_the_four_mandatory_fields() {
        let tools = tools_list();
        let required: Vec<&str> = tools["tools"][0]["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            required,
            vec!["CommandLine", "Cwd", "toolAction", "toolSummary"]
        );
    }

    #[test]
    fn tools_list_offers_wait_ms_without_requiring_it() {
        let tools = tools_list();
        let schema = &tools["tools"][0]["inputSchema"];
        assert!(schema["properties"]["WaitMsBeforeAsync"].is_object());
        assert!(!schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "WaitMsBeforeAsync"));
    }

    #[test]
    fn method_not_found_uses_the_json_rpc_code() {
        let body = method_not_found();
        assert_eq!(body["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(body["error"]["message"], "Method not found");
    }

    // ── handle_line: silence ───────────────────────────────────────

    #[test]
    fn blank_input_is_ignored() {
        for line in ["", "   ", "\t", "\r\n"] {
            assert!(
                handle_line(line, &never_rewrite, &never_exec).is_none(),
                "expected silence for {line:?}"
            );
        }
    }

    #[test]
    fn unparsable_input_is_ignored() {
        for line in ["}{ not json", "{\"unterminated\":", "garbage"] {
            assert!(
                handle_line(line, &never_rewrite, &never_exec).is_none(),
                "expected silence for {line:?}"
            );
        }
    }

    #[test]
    fn non_object_json_is_ignored() {
        for line in ["[1,2,3]", "\"a string\"", "42", "null"] {
            assert!(
                handle_line(line, &never_rewrite, &never_exec).is_none(),
                "expected silence for {line:?}"
            );
        }
    }

    #[test]
    fn a_notification_without_an_id_is_ignored() {
        let line = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string();
        assert!(handle_line(&line, &never_rewrite, &never_exec).is_none());
    }

    #[test]
    fn a_request_without_a_method_is_ignored() {
        let line = json!({ "jsonrpc": "2.0", "id": 1 }).to_string();
        assert!(handle_line(&line, &never_rewrite, &never_exec).is_none());
    }

    #[test]
    fn a_non_string_method_is_ignored() {
        let line = json!({ "jsonrpc": "2.0", "id": 1, "method": 7 }).to_string();
        assert!(handle_line(&line, &never_rewrite, &never_exec).is_none());
    }

    // ── handle_line: answers ───────────────────────────────────────

    fn call(method: &str) -> Value {
        let line = json!({ "jsonrpc": "2.0", "id": 1, "method": method }).to_string();
        handle_line(&line, &no_rewrite, &ok_exec).expect("a response")
    }

    #[test]
    fn initialize_returns_the_handshake() {
        let response = call("initialize");
        assert_eq!(response["result"]["serverInfo"]["name"], SERVER_NAME);
        assert_eq!(response["jsonrpc"], "2.0");
    }

    #[test]
    fn tools_list_is_dispatched() {
        assert_eq!(call("tools/list")["result"]["tools"][0]["name"], TOOL_NAME);
    }

    #[test]
    fn an_unknown_method_produces_an_error_envelope() {
        let response = call("resources/list");
        assert_eq!(response["error"]["code"], METHOD_NOT_FOUND);
        assert!(
            response.get("result").is_none(),
            "a response carries error or result, never both: {response}"
        );
    }

    #[test]
    fn ids_are_echoed_verbatim() {
        // Clients correlate on the id, and JSON-RPC allows numbers or strings.
        for id in [json!(7), json!("abc"), json!(null)] {
            let line = json!({ "jsonrpc": "2.0", "id": id, "method": "initialize" }).to_string();
            let response = handle_line(&line, &no_rewrite, &ok_exec).expect("a response");
            assert_eq!(response["id"], id);
        }
    }

    #[test]
    fn methods_that_need_no_shell_never_touch_the_executor() {
        // `never_exec` and `never_rewrite` panic, so this passing proves the
        // handshake path is free of side effects.
        for method in ["initialize", "tools/list", "unknown/method"] {
            let line = json!({ "jsonrpc": "2.0", "id": 1, "method": method }).to_string();
            handle_line(&line, &never_rewrite, &never_exec).expect("a response");
        }
    }

    // ── handle_line: tools/call ────────────────────────────────────

    /// Record what the executor was handed.
    fn recording_exec(
        log: &RefCell<Vec<(String, String)>>,
    ) -> impl Fn(&str, &str) -> Execution + '_ {
        move |command, cwd| {
            log.borrow_mut()
                .push((command.to_string(), cwd.to_string()));
            Execution::Ran {
                stdout: "done".to_string(),
                stderr: String::new(),
                success: true,
            }
        }
    }

    fn tools_call(arguments: Value) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": TOOL_NAME, "arguments": arguments }
        })
        .to_string()
    }

    #[test]
    fn tools_call_executes_the_rewritten_command() {
        let log = RefCell::new(Vec::new());
        let rewrite = |c: &str| Some(format!("rtk {c}"));

        let response = handle_line(
            &tools_call(json!({ "CommandLine": "cat README.md", "Cwd": "/work" })),
            &rewrite,
            &recording_exec(&log),
        )
        .expect("a response");

        assert_eq!(
            log.borrow().as_slice(),
            &[("rtk cat README.md".to_string(), "/work".to_string())],
            "the rewritten command must be what actually runs"
        );
        assert_eq!(response["result"]["content"][0]["text"], "done");
        assert_eq!(response["result"]["isError"], false);
    }

    #[test]
    fn tools_call_falls_back_to_the_original_command() {
        // rtk only covers known commands, so declining a rewrite is the common
        // case — `git`, `npm` and project scripts must still run.
        let log = RefCell::new(Vec::new());

        handle_line(
            &tools_call(json!({ "CommandLine": "git status", "Cwd": "." })),
            &no_rewrite,
            &recording_exec(&log),
        )
        .expect("a response");

        assert_eq!(
            log.borrow()[0].0,
            "git status",
            "an un-rewritable command must run unchanged"
        );
    }

    #[test]
    fn tools_call_applies_the_default_working_directory() {
        let log = RefCell::new(Vec::new());
        handle_line(
            &tools_call(json!({ "CommandLine": "ls" })),
            &no_rewrite,
            &recording_exec(&log),
        )
        .expect("a response");
        assert_eq!(log.borrow()[0].1, ".");
    }

    #[test]
    fn tools_call_surfaces_a_failed_command() {
        let failing = |_: &str, _: &str| Execution::Ran {
            stdout: String::new(),
            stderr: "no such file".to_string(),
            success: false,
        };
        let response = handle_line(
            &tools_call(json!({ "CommandLine": "cat missing" })),
            &no_rewrite,
            &failing,
        )
        .expect("a response");

        assert_eq!(response["result"]["isError"], true);
        assert_eq!(response["result"]["content"][0]["text"], "no such file");
    }

    #[test]
    fn tools_call_surfaces_a_spawn_failure() {
        let broken = |_: &str, _: &str| Execution::NotRun("program not found".to_string());
        let response = handle_line(
            &tools_call(json!({ "CommandLine": "anything" })),
            &no_rewrite,
            &broken,
        )
        .expect("a response");

        assert_eq!(response["result"]["isError"], true);
        assert_eq!(
            response["result"]["content"][0]["text"],
            "program not found"
        );
    }

    #[test]
    fn tools_call_without_arguments_still_answers() {
        // A malformed call must produce a response, not a dead server.
        let log = RefCell::new(Vec::new());
        let line = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call" }).to_string();

        let response = handle_line(&line, &no_rewrite, &recording_exec(&log)).expect("a response");
        assert_eq!(log.borrow()[0], (String::new(), ".".to_string()));
        assert!(response["result"]["isError"].is_boolean());
    }

    // ── analytics tools tests ─────────────────────────────────────

    #[test]
    fn shell_quote_protects_against_injection() {
        assert_eq!(shell_quote("hello"), "'hello'");
        assert_eq!(shell_quote("hello world"), "'hello world'");
        assert_eq!(shell_quote("echo 'hi'"), "'echo '\\''hi'\\'''");
        assert_eq!(shell_quote("foo; rm -rf /"), "'foo; rm -rf /'");
    }

    #[test]
    fn build_gain_command_handles_options() {
        assert_eq!(build_gain_command(&json!({})).unwrap(), "rtk gain");
        assert_eq!(
            build_gain_command(&json!({
                "history": true,
                "project_only": true,
                "format": "json"
            }))
            .unwrap(),
            "rtk gain --history --project --format json"
        );
        assert_eq!(
            build_gain_command(&json!({
                "history": false,
                "project_only": true,
                "format": "csv"
            }))
            .unwrap(),
            "rtk gain --project --format csv"
        );
        assert!(build_gain_command(&json!({ "format": "xml" })).is_err());
    }

    #[test]
    fn build_check_command_formats_command() {
        assert_eq!(
            build_check_command(&json!({ "command": "grep -rn foo src/" })).unwrap(),
            "rtk hook check -- 'grep -rn foo src/'"
        );
        assert_eq!(
            build_check_command(&json!({ "command": "echo 'hi'; rm -rf /" })).unwrap(),
            "rtk hook check -- 'echo '\\''hi'\\''; rm -rf /'"
        );
        assert!(build_check_command(&json!({})).is_err());
        assert!(build_check_command(&json!({ "command": "   " })).is_err());
    }

    #[test]
    fn build_discover_command_handles_options() {
        assert_eq!(build_discover_command(&json!({})).unwrap(), "rtk discover");
        assert_eq!(
            build_discover_command(&json!({
                "project": "my project",
                "limit": 5,
                "since": 14,
                "all_projects": true,
                "format": "json"
            }))
            .unwrap(),
            "rtk discover --project 'my project' --limit 5 --since 14 --all --format json"
        );
        assert!(build_discover_command(&json!({ "format": "yaml" })).is_err());
    }

    #[test]
    fn tools_call_dispatches_gain() {
        let log = RefCell::new(Vec::new());
        let line = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": TOOL_GAIN,
                "arguments": { "history": true, "Cwd": "/my/repo" }
            }
        })
        .to_string();

        let response = handle_line(&line, &no_rewrite, &recording_exec(&log)).expect("a response");
        assert_eq!(
            log.borrow().as_slice(),
            &[("rtk gain --history".to_string(), "/my/repo".to_string())]
        );
        assert_eq!(response["result"]["content"][0]["text"], "done");
        assert_eq!(response["result"]["isError"], false);
    }

    #[test]
    fn tools_call_dispatches_check() {
        let log = RefCell::new(Vec::new());
        let line = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": TOOL_CHECK,
                "arguments": { "command": "ls -la", "Cwd": "/my/repo" }
            }
        })
        .to_string();

        let response = handle_line(&line, &no_rewrite, &recording_exec(&log)).expect("a response");
        assert_eq!(
            log.borrow().as_slice(),
            &[(
                "rtk hook check -- 'ls -la'".to_string(),
                "/my/repo".to_string()
            )]
        );
        assert_eq!(response["result"]["content"][0]["text"], "done");
        assert_eq!(response["result"]["isError"], false);
    }

    #[test]
    fn tools_call_dispatches_discover() {
        let log = RefCell::new(Vec::new());
        let line = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": TOOL_DISCOVER,
                "arguments": { "all_projects": true }
            }
        })
        .to_string();

        let response = handle_line(&line, &no_rewrite, &recording_exec(&log)).expect("a response");
        assert_eq!(
            log.borrow().as_slice(),
            &[("rtk discover --all".to_string(), ".".to_string())]
        );
        assert_eq!(response["result"]["content"][0]["text"], "done");
        assert_eq!(response["result"]["isError"], false);
    }

    #[test]
    fn tools_call_rejects_unknown_tool() {
        let line = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "not_a_real_tool",
                "arguments": {}
            }
        })
        .to_string();

        let response = handle_line(&line, &no_rewrite, &never_exec).expect("a response");
        assert_eq!(response["result"]["isError"], true);
        assert_eq!(
            response["result"]["content"][0]["text"],
            "Unknown tool: not_a_real_tool"
        );
    }

    #[test]
    fn tools_call_surfaces_validation_errors() {
        let line = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": TOOL_CHECK,
                "arguments": { "command": "" }
            }
        })
        .to_string();

        let response = handle_line(&line, &no_rewrite, &never_exec).expect("a response");
        assert_eq!(response["result"]["isError"], true);
        assert_eq!(
            response["result"]["content"][0]["text"],
            "command must not be empty"
        );
    }

    // ── PreInvocation payload ──────────────────────────────────────

    fn injected_message() -> String {
        let value = serde_json::to_value(preinvocation_payload()).unwrap();
        value["injectSteps"][0]["ephemeralMessage"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn preinvocation_emits_one_step() {
        let value = serde_json::to_value(preinvocation_payload()).unwrap();
        assert_eq!(value["injectSteps"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn preinvocation_names_the_server_and_tool_it_advertises() {
        // The names are interpolated from the same constants the server uses,
        // so this cannot drift — which is the point of asserting it.
        let message = injected_message();
        assert!(
            message.contains(&format!("ServerName='{SERVER_NAME}'")),
            "got: {message}"
        );
        assert!(
            message.contains(&format!("ToolName='{TOOL_NAME}'")),
            "got: {message}"
        );
    }

    #[test]
    fn preinvocation_steers_away_from_the_native_tool() {
        // The whole reason the hook exists: agy's PreToolUse cannot rewrite
        // tool arguments, so the model must be told not to use run_command.
        assert!(injected_message().contains("run_command"));
    }
}
