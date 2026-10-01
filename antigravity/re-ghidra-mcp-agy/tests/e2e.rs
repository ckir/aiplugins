//! End-to-end tests for the re-ghidra-mcp-agy plugin binaries and configurations.

use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const INHERITED: &[&str] = &[
    "GHIDRA_INSTALL_DIR",
    "GHIDRA_MCP_PROJECT_DIR",
    "GHIDRA_MCP_PROJECT_NAME",
    "GHIDRA_MCP_BOOTSTRAP_PROGRAM",
    "GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH",
    "GHIDRA_MCP_MAX_HEAP",
];

struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "re-ghidra-agy-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(dir.join(".agents")).expect("create workspace");
        Self { dir }
    }

    fn with_settings(self, frontmatter: &str) -> Self {
        std::fs::write(
            self.dir.join(".agents").join("re-ghidra-mcp-agy.local.md"),
            format!("---\n{frontmatter}\n---\n\n# settings\n"),
        )
        .expect("write settings");
        self
    }

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

fn run_hook(workspace_dir: &Path, payload: &Value, env: &[(&str, &str)]) -> (Option<i32>, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_re-ghidra-agy-hook"));
    for key in INHERITED {
        cmd.env_remove(key);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd
        .current_dir(workspace_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn hook");

    let raw = serde_json::to_string(payload).expect("serialize payload");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(raw.as_bytes())
        .expect("write to hook");

    let out = child.wait_with_output().expect("wait for hook");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

#[test]
fn hook_reports_unconfigured_environment() {
    let ws = Workspace::new("unconfigured");
    let payload = serde_json::json!({
        "invocationNum": 1,
        "workspacePaths": [ws.dir.to_string_lossy()],
    });

    let (code, stdout) = run_hook(&ws.dir, &payload, &[]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("re-ghidra-mcp-agy is not ready"));
    assert!(stdout.contains("GHIDRA_INSTALL_DIR is not set"));
}

#[test]
fn a_configured_workspace_draws_no_attention_to_its_config() {
    let ws = Workspace::new("configured").with_settings(
        "project_dir: /tmp/projects\nproject_name: crackme\nbootstrap_program: crackme.exe",
    );
    let install = ws.fake_ghidra_install();
    let payload = serde_json::json!({
        "invocationNum": 1,
        "workspacePaths": [ws.dir.to_string_lossy()],
    });

    let (code, stdout) = run_hook(
        &ws.dir,
        &payload,
        &[("GHIDRA_INSTALL_DIR", &install.to_string_lossy())],
    );

    assert_eq!(code, Some(0));
    assert!(
        !stdout.contains("GHIDRA_INSTALL_DIR"),
        "a valid install dir must not be reported: {stdout}"
    );
    assert!(
        !stdout.contains("Unset:"),
        "settings file supplied every project value: {stdout}"
    );
}

#[test]
fn environment_overrides_settings_file() {
    let ws = Workspace::new("override").with_settings("project_dir: /tmp/from-file");
    let payload = serde_json::json!({
        "invocationNum": 1,
        "workspacePaths": [ws.dir.to_string_lossy()],
    });

    let (code, stdout) = run_hook(
        &ws.dir,
        &payload,
        &[
            ("GHIDRA_MCP_PROJECT_NAME", "from-env"),
            ("GHIDRA_MCP_BOOTSTRAP_PROGRAM", "prog.exe"),
        ],
    );

    assert_eq!(code, Some(0));
    assert!(
        !stdout.contains("Unset:"),
        "file and environment together cover every value: {stdout}"
    );
}

#[test]
fn hook_is_silent_on_subsequent_turns() {
    let ws = Workspace::new("turn2");
    let payload = serde_json::json!({
        "invocationNum": 2,
        "workspacePaths": [ws.dir.to_string_lossy()],
    });

    let (code, stdout) = run_hook(&ws.dir, &payload, &[]);
    assert_eq!(code, Some(0));
    assert_eq!(stdout.trim(), "{}");
}

#[test]
fn mcp_config_declares_serve_argument() {
    let config: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/mcp_config.json"))
            .expect("read mcp_config.json"),
    )
    .expect("valid JSON");

    let server = &config["mcpServers"]["re-ghidra-mcp"];
    assert_eq!(server["command"].as_str(), Some("re-ghidra-agy-mcp"));
    let args = server["args"].as_array().expect("args must be an array");
    assert_eq!(
        args.iter().map(|a| a.as_str().unwrap()).collect::<Vec<_>>(),
        vec!["serve"]
    );
}

#[test]
fn hooks_json_conforms_to_antigravity_schema() {
    let hooks: Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/hooks.json"))
            .expect("read hooks.json"),
    )
    .expect("valid JSON");

    let hooks_map = hooks
        .as_object()
        .expect("hooks.json must be a JSON object mapping hook names to specs");
    assert!(
        !hooks_map.is_empty(),
        "hooks.json must define at least one hook"
    );

    let mut found_preinvocation = false;
    for (hook_name, hook_spec) in hooks_map {
        assert!(
            hook_spec.is_object(),
            "hook {hook_name:?} must be a JSON object"
        );
        if let Some(pre_inv) = hook_spec.get("PreInvocation") {
            let handlers = pre_inv
                .as_array()
                .expect("PreInvocation must be an array of handlers");
            for handler in handlers {
                assert_eq!(handler["command"].as_str(), Some("re-ghidra-agy-hook"));
            }
            found_preinvocation = true;
        }
    }
    assert!(
        found_preinvocation,
        "hooks.json must declare a PreInvocation hook"
    );
}
