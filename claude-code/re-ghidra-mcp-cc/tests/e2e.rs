//! End-to-end tests for the two real binaries.
//!
//! The unit tests in `src/lib.rs` cover the decisions; these cover the wiring —
//! that the hook actually reads the settings file, actually honours the
//! environment over it, and actually stays silent when there is nothing to say.
//! None of that is exercised by a pure function test, and all of it is what
//! breaks when a variable name or a path is mistyped.
//!
//! No Ghidra, no JVM: every path here stops at the cheap preflight.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Every `GHIDRA_MCP_*` name the binaries read, plus the Ghidra-ecosystem one.
///
/// The child inherits this test process's environment, and a developer running
/// the suite on a machine that is actually set up for Ghidra would otherwise get
/// different results from CI. Clearing these by name — rather than `env_clear`,
/// which on Windows also strips `SystemRoot` and breaks process creation —
/// makes the tests say the same thing everywhere.
const INHERITED: &[&str] = &[
    "GHIDRA_INSTALL_DIR",
    "GHIDRA_MCP_PROJECT_DIR",
    "GHIDRA_MCP_PROJECT_NAME",
    "GHIDRA_MCP_BOOTSTRAP_PROGRAM",
    "GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH",
    "GHIDRA_MCP_MAX_HEAP",
    "CLAUDE_PROJECT_DIR",
];

struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "re-ghidra-cc-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(dir.join(".claude")).expect("create workspace");
        Self { dir }
    }

    fn with_settings(self, frontmatter: &str) -> Self {
        std::fs::write(
            self.dir.join(".claude").join("re-ghidra-mcp-cc.local.md"),
            format!("---\n{frontmatter}\n---\n\n# settings\n"),
        )
        .expect("write settings");
        self
    }

    /// A directory that passes the "is this a Ghidra install root?" check: the
    /// preflight looks for the headless launcher and nothing more, which is
    /// exactly what makes it cheap enough to run every session.
    fn fake_ghidra_install(&self) -> PathBuf {
        let root = self.dir.join("ghidra");
        let support = root.join("support");
        std::fs::create_dir_all(&support).expect("create fake install");
        std::fs::write(support.join("analyzeHeadless"), "#!/bin/sh\n").expect("write launcher");
        std::fs::write(support.join("analyzeHeadless.bat"), "@echo off\n").expect("write launcher");
        root
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// Run the hook with a `SessionStart` event naming `cwd`, and return
/// `(exit code, stdout)`.
fn run_hook(cwd: &Path, env: &[(&str, &str)]) -> (Option<i32>, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_re-ghidra-cc-hook"));
    for key in INHERITED {
        cmd.env_remove(key);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hook");
    let event = serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": "test",
        "cwd": cwd.to_string_lossy(),
    });
    child
        .stdin
        .take()
        .expect("hook stdin")
        .write_all(event.to_string().as_bytes())
        .expect("write event");
    let out = child.wait_with_output().expect("hook exits");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn unconfigured_workspace_reports_what_is_missing() {
    let ws = Workspace::new("bare");
    let (code, stdout) = run_hook(&ws.dir, &[]);

    assert_eq!(code, Some(0), "the preflight must never block a session");
    assert!(
        stdout.contains("GHIDRA_INSTALL_DIR"),
        "expected the install-dir finding, got: {stdout}"
    );
    assert!(
        stdout.contains("project_dir") && stdout.contains("project_name"),
        "expected the unset-project finding, got: {stdout}"
    );

    // The report has to be machine-readable for Claude Code to ingest it at all.
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).expect("hook emits JSON");
    assert_eq!(
        v["hookSpecificOutput"]["hookEventName"], "SessionStart",
        "the SessionStart envelope is what carries additionalContext"
    );
    assert!(v["systemMessage"].is_string());
}

#[test]
fn a_configured_workspace_draws_no_attention_to_its_config() {
    let ws = Workspace::new("configured").with_settings(
        "project_dir: /tmp/projects\nproject_name: crackme\nbootstrap_program: crackme.exe",
    );
    let install = ws.fake_ghidra_install();
    let (code, stdout) = run_hook(
        &ws.dir,
        &[("GHIDRA_INSTALL_DIR", &install.to_string_lossy())],
    );

    assert_eq!(code, Some(0));
    // Deliberately NOT `stdout.is_empty()`: whether a JDK 21 is on PATH is a
    // property of the machine, and CI runners differ. What this pins is that
    // reading the settings file silenced the config findings — the part the
    // plugin controls.
    assert!(
        !stdout.contains("GHIDRA_INSTALL_DIR"),
        "a valid install dir must not be reported: {stdout}"
    );
    assert!(
        !stdout.contains("Unset:"),
        "settings file supplied every project value, yet: {stdout}"
    );
}

#[test]
fn environment_overrides_the_settings_file() {
    let ws = Workspace::new("override").with_settings("project_dir: /tmp/from-file");
    let (code, stdout) = run_hook(
        &ws.dir,
        &[
            ("GHIDRA_MCP_PROJECT_NAME", "from-env"),
            ("GHIDRA_MCP_BOOTSTRAP_PROGRAM", "prog.exe"),
        ],
    );

    assert_eq!(code, Some(0));
    // project_dir came from the file, name and program from the environment:
    // between them every project value resolved, so no "Unset:" finding.
    assert!(
        !stdout.contains("Unset:"),
        "file and environment together cover every value: {stdout}"
    );
}

/// The hook is spawned by Claude Code, not by a person. It must not turn a
/// malformed or absent event into a crash — a non-zero exit here is reported to
/// the user as a broken plugin.
#[test]
fn malformed_event_does_not_crash_the_hook() {
    let ws = Workspace::new("garbage");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_re-ghidra-cc-hook"));
    for key in INHERITED {
        cmd.env_remove(key);
    }
    let mut child = cmd
        .current_dir(&ws.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hook");
    child
        .stdin
        .take()
        .expect("hook stdin")
        .write_all(b"not json at all")
        .expect("write garbage");
    let out = child.wait_with_output().expect("hook exits");
    assert_eq!(out.status.code(), Some(0));
}

/// With no recognized subcommand the server binary prints usage and exits 0 —
/// and prints it to **stderr**, because stdout is the MCP JSON-RPC channel and
/// a usage banner there reads as a protocol violation.
#[test]
fn mcp_binary_keeps_usage_off_stdout() {
    let out = Command::new(env!("CARGO_BIN_EXE_re-ghidra-cc-mcp"))
        .arg("--help")
        .output()
        .expect("run the server binary");

    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "stdout belongs to the protocol, got: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("serve"),
        "usage should list `serve`: {stderr}"
    );
    assert!(
        stderr.contains("emit-skill"),
        "usage should list `emit-skill`: {stderr}"
    );
}

/// A `serve` with nothing configured must still speak MCP.
///
/// It starts, completes the handshake and returns its tool schemas, so a host can
/// read what this plugin offers — and what it costs in context — on a machine with
/// no Ghidra install. The configuration failure is reported on the first tool CALL
/// instead (`shared/ghidra-mcp/tests/config_deferral.rs` pins that half).
///
/// This is a deliberate replacement for an earlier test that asserted exit 2, the
/// old configuration-error contract. That contract was the bug: the process died
/// before `serve` was reached, so a host asking only for tool schemas got nothing.
/// What the old test was really protecting — not booting a JVM and timing out — is
/// still protected, by the `boot_count` assertions in config_deferral.rs.
#[test]
fn serve_without_configuration_still_answers_tools_list() {
    let ws = Workspace::new("noconfig");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_re-ghidra-cc-mcp"));
    for key in INHERITED {
        cmd.env_remove(key);
    }
    // `ws.dir` carries an empty `.claude/`, so no settings file supplies the config
    // that the environment no longer does.
    let mut child = cmd
        .arg("serve")
        .current_dir(&ws.dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn serve");

    {
        let mut stdin = child.stdin.take().expect("serve stdin");
        for request in [
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "e2e", "version": "0"}
            }}),
            serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        ] {
            writeln!(stdin, "{request}").expect("write request");
        }
        // Dropping stdin closes it; the server shuts down on EOF once it has answered.
    }

    let out = child.wait_with_output().expect("serve exits");
    assert_eq!(
        out.status.code(),
        Some(0),
        "an unconfigured serve must exit cleanly; stderr was: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // stdout IS the MCP channel, so "empty" is no longer the hygiene property — "nothing but
    // protocol" is. Parsing leniently and picking out the response we want would silently tolerate a
    // stray `println!` alongside it, which is exactly the corruption the replaced test caught by
    // asserting stdout was empty.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let messages: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("non-protocol output on the MCP channel ({e}): {line}"))
        })
        .collect();

    let tools = messages
        .iter()
        .find(|msg| msg["id"] == 2)
        .and_then(|msg| msg["result"]["tools"].as_array().cloned())
        .expect("a tools/list response on stdout");

    assert!(
        !tools.is_empty(),
        "an unconfigured server must still advertise its tool schemas"
    );
    assert!(
        tools.iter().any(|t| t["name"] == "inspect_function"),
        "inspect_function missing from an unconfigured server's tools/list"
    );
}
