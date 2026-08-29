//! `rtk-mcp` — the plugin's MCP server, spoken over stdio.
//!
//! Antigravity launches this binary and talks line-delimited JSON-RPC to it on
//! stdin/stdout. All the judgement lives in [`rtk_mcp_agy`]; this file only
//! moves bytes and spawns the two external processes — `rtk` to rewrite a
//! command, and a shell to run it.
//!
//! stdout belongs to the protocol: one JSON response per line, nothing else.

use rtk_mcp_agy::{handle_line, parse_rewrite, resolve_rtk_bin, resolve_shell, Execution};
use std::env;
use std::io::{self, BufRead, Write};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rtk-mcp {}", rtk_mcp_agy::version());
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "rtk-mcp {} — MCP server for Antigravity rtk shell command optimization",
            rtk_mcp_agy::version()
        );
        println!();
        println!("Usage: rtk-mcp");
        println!("  JSON-RPC MCP server over stdin/stdout. Exposes a single tool:");
        println!(
            "    {}  — Rewrites the given CommandLine via `rtk rewrite` and executes it.",
            rtk_mcp_agy::TOOL_NAME
        );
        return;
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        let Some(response) = handle_line(&line, &rtk_rewrite, &run_command) else {
            continue;
        };
        if let Ok(response_str) = serde_json::to_string(&response) {
            println!("{response_str}");
            let _ = stdout.flush();
        }
    }
}

/// Ask `rtk` for a token-optimized form of `command`.
///
/// Any failure — rtk absent, non-zero exit, no output — is reported as "no
/// rewrite available", and the caller runs the original command. rtk only
/// covers specific known commands, so this is the ordinary path, not an error.
fn rtk_rewrite(command: &str) -> Option<String> {
    let output = Command::new(resolve_rtk_bin(env::var("RTK_BIN").ok()))
        .arg("rewrite")
        .arg(command)
        .output()
        .ok()?;
    parse_rewrite(&String::from_utf8_lossy(&output.stdout))
}

/// Run `command` in `cwd` through a shell.
///
/// `pwsh` on Windows, `sh` elsewhere, overridable with `RTK_AGY_SHELL`. Both
/// take the command after `-c`. A shell that cannot be spawned is reported to
/// the model as a tool error rather than taking the server down.
fn run_command(command: &str, cwd: &str) -> Execution {
    let shell = resolve_shell(env::var("RTK_AGY_SHELL").ok(), cfg!(windows));
    match Command::new(&shell)
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .output()
    {
        Ok(out) => Execution::Ran {
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            success: out.status.success(),
        },
        Err(e) => Execution::NotRun(format!("could not run {shell}: {e}")),
    }
}
