//! `rtk-cc-hook` — the plugin's `PreToolUse` hook.
//!
//! Claude Code runs this binary before every `Bash` / `PowerShell` tool call,
//! hands it the event as JSON on stdin, and reads a JSON verdict from stdout.
//! The verdict is produced by `rtk hook claude`, which already speaks this exact
//! schema; this file only moves bytes between the two.
//!
//! Two rules govern the whole file:
//!
//! 1. **The hook always exits 0.** Exit 2 would feed stderr back to Claude as a
//!    blocking error — an absurd outcome for "I could not optimize your
//!    command". Every failure degrades to silence instead.
//! 2. **Empty stdout means "no opinion".** Claude Code then runs the command the
//!    model actually wrote. That is also rtk's own signal for "nothing to
//!    rewrite", so the fail-open path and the no-op path are the same path.

use rtk_mcp_cc::{delegate, Config};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rtk-cc-hook {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    // stderr only: stdout carries the hook's JSON verdict.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        tracing::warn!("could not read hook input from stdin; passing through");
        return;
    }

    let project_dir = std::env::var("CLAUDE_PROJECT_DIR")
        .map(PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .unwrap_or_else(|_| PathBuf::from("."));

    let config = Config::load(&project_dir);
    let verdict = delegate(&input, &config, |args, payload| {
        run_rtk(&config.rtk_bin, args, payload)
    });

    print!("{verdict}");
}

/// Spawn rtk, write `payload` to its stdin, and return its stdout on a clean
/// exit.
///
/// `None` on any failure — binary not found, non-zero exit, non-UTF-8 output.
/// rtk's stderr is left attached to ours rather than captured: its advisory
/// notices belong in the session log, and Claude Code ignores hook stderr on a
/// zero exit.
fn run_rtk(rtk_bin: &str, args: &[String], payload: &str) -> Option<String> {
    let mut child = match Command::new(rtk_bin)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!(rtk_bin, "could not spawn rtk ({e}); passing through");
            return None;
        }
    };

    // A failed write is not fatal on its own: rtk may have exited early and
    // closed the pipe. Let the exit status below decide.
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(payload.as_bytes()) {
            tracing::warn!("could not write to rtk stdin ({e})");
        }
    }

    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!("rtk did not complete ({e}); passing through");
            return None;
        }
    };

    if !output.status.success() {
        tracing::warn!(status = ?output.status, "rtk exited non-zero; passing through");
        return None;
    }

    match String::from_utf8(output.stdout) {
        Ok(stdout) => Some(stdout),
        Err(_) => {
            tracing::warn!("rtk produced non-UTF-8 stdout; passing through");
            None
        }
    }
}

fn print_help() {
    println!(
        "rtk-cc-hook {} — PreToolUse hook that routes shell commands through rtk",
        env!("CARGO_PKG_VERSION")
    );
    println!();
    println!("Usage: rtk-cc-hook");
    println!("  Reads a Claude Code PreToolUse event as JSON on stdin, delegates to");
    println!("  `rtk hook claude`, and writes the resulting verdict to stdout. Writes");
    println!("  nothing — and always exits 0 — when rtk is unavailable or declines to");
    println!("  rewrite, so the original command runs unchanged.");
    println!();
    println!("Environment:");
    println!("  RTK_BIN               Path to the rtk executable (default: `rtk` on PATH)");
    println!("  RTK_CC_DISABLE=1      Pass every command through untouched");
    println!("  RTK_CC_ULTRA_COMPACT  Pass --ultra-compact to rtk");
    println!("  RTK_CC_SKIP_ENV       Pass --skip-env to rtk");
    println!();
    println!("Settings file: .claude/rtk-mcp-cc.local.md (environment overrides it)");
}
