//! `re-ghidra-agy-hook` — the plugin's `PreInvocation` preflight.
//!
//! The four ways this plugin fails to start are all environmental: no
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

use re_ghidra_mcp_agy::{load_settings, parse_java_major, preflight, Probe};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let mut raw = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut raw);

    // Parse the PreInvocation event payload
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };

    // Only run the preflight check on the first invocation to avoid spamming
    // the user on every turn of the agent loop.
    if let Some(inv_num) = v.get("invocationNum").and_then(|n| n.as_u64()) {
        if inv_num > 1 {
            println!("{}", serde_json::json!({}));
            return;
        }
    }

    let project_dir =
        extract_project_dir(&v).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

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
        println!("{}", serde_json::json!({}));
        return;
    }

    let body = format!(
        "re-ghidra-mcp-agy is not ready to attach to a Ghidra project:\n{}",
        findings
            .iter()
            .map(|f| format!("  - {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let payload = serde_json::json!({
        "injectSteps": [
            {
                "ephemeralMessage": body
            }
        ]
    });

    println!("{payload}");
}

fn extract_project_dir(v: &serde_json::Value) -> Option<PathBuf> {
    // Try to get the first workspace path from the payload
    if let Some(workspaces) = v.get("workspacePaths").and_then(|w| w.as_array()) {
        if let Some(first) = workspaces.first().and_then(|f| f.as_str()) {
            if !first.is_empty() {
                return Some(PathBuf::from(first));
            }
        }
    }
    None
}

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

fn launcher_exists(install_dir: &Path) -> bool {
    let support = install_dir.join("support");
    support.join("analyzeHeadless.bat").exists() || support.join("analyzeHeadless").exists()
}

fn detect_java_major() -> Option<u32> {
    let out = Command::new("java").arg("-version").output().ok()?;
    parse_java_major(&String::from_utf8_lossy(&out.stderr))
        .or_else(|| parse_java_major(&String::from_utf8_lossy(&out.stdout)))
}
