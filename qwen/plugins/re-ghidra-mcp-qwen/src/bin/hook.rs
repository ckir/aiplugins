//! `re-ghidra-qwen-hook` — the extension's `SessionStart` preflight.
//!
//! The four ways this extension fails to start are all environmental: no
//! `GHIDRA_INSTALL_DIR`, a directory that is not actually a Ghidra root, a JDK
//! older than 21, and an unconfigured project. Without this hook each of them
//! surfaces only on the first tool call, as an error from inside a JVM that
//! never booted.
//!
//! Two rules govern what it may do:
//!
//! 1. **Silent when healthy.** It runs on every session start, so a clean
//!    environment must produce no output at all. Anything else is noise the
//!    user reads once and then learns to ignore.
//! 2. **Never blocks.** It always exits 0. A preflight that can stop a session
//!    is worse than the problem it reports — the user may not even intend to
//!    touch Ghidra this session.
//!
//! Every check is filesystem-or-`java -version` cheap: no JVM boot, no Ghidra
//! project open.

use re_ghidra_mcp_qwen::{load_settings, parse_java_major, preflight, Probe};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Draining stdin is not optional. Qwen Code writes the event JSON to the
    // hook's stdin, and exiting without reading it can hand the writer a broken
    // pipe. We take `cwd` from it when present, since it is authoritative for
    // the session's project root.
    let mut raw = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut raw);

    let project_dir = event_cwd(&raw)
        .or_else(|| std::env::var("QWEN_PROJECT_DIR").ok().map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();

    // Same three layers the server itself resolves, minus CLI flags: there are
    // none here. Reporting on a different config than the server will use would
    // make this hook worse than useless.
    let mut cfg = load_settings(&project_dir);
    overlay_env(&mut cfg);

    let probe = Probe {
        launcher_present: cfg
            .ghidra_install_dir
            .as_deref()
            .is_some_and(|d| launcher_exists(Path::new(d))),
        java_major: detect_java_major(),
    };

    let findings = preflight(&cfg, &probe);
    if findings.is_empty() {
        return; // The silent path.
    }

    let body = format!(
        "re-ghidra-mcp-qwen is not ready to attach to a Ghidra project:\n{}",
        findings
            .iter()
            .map(|f| format!("  - {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    // `additionalContext` is how SessionStart feeds text to the model;
    // `systemMessage` is the generic channel. Emitting both means the report
    // lands whichever one the running Qwen Code honours.
    let payload = serde_json::json!({
        "systemMessage": body,
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": body,
        }
    });
    println!("{payload}");
}

/// Pull `cwd` out of the hook event without a full deserialization struct —
/// the field is the only one we want and the schema may grow.
fn event_cwd(raw: &str) -> Option<PathBuf> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let cwd = v.get("cwd")?.as_str()?;
    (!cwd.is_empty()).then(|| PathBuf::from(cwd))
}

/// Layer the environment over the settings file, matching the server's own
/// precedence. Empty is treated as unset, as it is there.
fn overlay_env(cfg: &mut ghidra_mcp::config::RawConfig) {
    let get = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty());
    for (slot, key) in [
        (&mut cfg.ghidra_install_dir, "GHIDRA_INSTALL_DIR"),
        (&mut cfg.project_dir, "GHIDRA_MCP_PROJECT_DIR"),
        (&mut cfg.project_name, "GHIDRA_MCP_PROJECT_NAME"),
        (&mut cfg.bootstrap_program, "GHIDRA_MCP_BOOTSTRAP_PROGRAM"),
        (
            &mut cfg.bootstrap_program_path,
            "GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH",
        ),
        (&mut cfg.max_heap, "GHIDRA_MCP_MAX_HEAP"),
    ] {
        if let Some(v) = get(key) {
            *slot = Some(v);
        }
    }
}

/// A Ghidra install root is identified by its headless launcher, the same file
/// the server's own config validation looks for.
fn launcher_exists(install_dir: &Path) -> bool {
    let support = install_dir.join("support");
    support.join("analyzeHeadless.bat").exists() || support.join("analyzeHeadless").exists()
}

/// `java -version` writes to **stderr**, not stdout. Any spawn failure means
/// "no usable java", which is exactly what the preflight wants to hear.
fn detect_java_major() -> Option<u32> {
    let out = Command::new("java").arg("-version").output().ok()?;
    parse_java_major(&String::from_utf8_lossy(&out.stderr))
        .or_else(|| parse_java_major(&String::from_utf8_lossy(&out.stdout)))
}
