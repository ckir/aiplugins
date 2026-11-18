// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! End-to-end tests that run the plugin's real binary: `serve` through rmcp's client, and
//! `reminder` as the PreInvocation hook runs it, in throwaway directories.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{json, Value};

const BIN: &str = env!("CARGO_BIN_EXE_repo-tasks-agy-mcp");
const AT: &str = "2026-11-18T10:00:00Z";
const LINE_3_1: &str = "repo-tasks: 3 open in this repository's task list, 1 in progress. Call the task_list tool to see them.";

struct Rig {
    root: PathBuf,
    repo: PathBuf,
    home: PathBuf,
    /// A directory that is not inside any repository.
    elsewhere: PathBuf,
}

impl Rig {
    fn new() -> Rig {
        static N: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "repo-tasks-agy-e2e-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        for d in ["repo", "home", "elsewhere"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        let root = fs::canonicalize(&root).unwrap();
        let (repo, home, elsewhere) =
            (root.join("repo"), root.join("home"), root.join("elsewhere"));
        let out = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .output()
            .expect("git init");
        assert!(out.status.success(), "git init failed");
        Rig {
            root,
            repo,
            home,
            elsewhere,
        }
    }

    fn scrub(&self, c: &mut Command, cwd: &Path) {
        c.current_dir(cwd)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env_remove("REPO_TASKS_DIR")
            .env_remove("REPO_TASKS_LIST")
            .env_remove("XDG_DATA_HOME");
    }

    /// Antigravity starts the server in the plugin's own folder, outside the workspace; the
    /// workspace reaches the server through `REPO_TASKS_DIR`.
    async fn connect(&self) -> RunningService<RoleClient, ()> {
        let mut c = Command::new(BIN);
        c.arg("serve");
        self.scrub(&mut c, &self.elsewhere);
        c.env("REPO_TASKS_DIR", &self.repo);
        let transport = TokioChildProcess::new(tokio::process::Command::from(c)).expect("spawn");
        ().serve(transport).await.expect("initialize")
    }

    fn reminder(&self, event: &Value) -> (bool, String, String) {
        let mut c = Command::new(BIN);
        c.args(["reminder", "--event", "antigravity"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // The hook runs from elsewhere; the event names the workspace.
        self.scrub(&mut c, &self.root);
        let mut child = c.spawn().expect("spawn reminder");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(event.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        (
            out.status.success(),
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn task(id: u64, subject: &str, status: &str) -> Value {
    json!({
        "id": id, "subject": subject, "status": status, "blocked_by": [],
        "created_at": AT, "updated_at": AT, "updated_by": "antigravity"
    })
}

fn write_three_one(repo: &Path) {
    let dir = repo.join(".git/repo-tasks");
    fs::create_dir_all(&dir).unwrap();
    let tasks = vec![
        task(1, "SECRET-done", "completed"),
        task(2, "SECRET-doing", "in_progress"),
        task(3, "SECRET-todo", "pending"),
        task(4, "SECRET-todo-too", "pending"),
    ];
    let list = json!({"format": 1, "next_id": 5, "tasks": tasks});
    fs::write(dir.join("default.json"), serde_json::to_vec(&list).unwrap()).unwrap();
}

async fn call_raw(
    client: &RunningService<RoleClient, ()>,
    name: &str,
    args: Value,
) -> (bool, String) {
    let params = CallToolRequestParams::new(name.to_string())
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    let r = client.call_tool(params).await.expect("call_tool");
    let text = r
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default();
    (r.is_error == Some(true), text)
}

async fn call(client: &RunningService<RoleClient, ()>, name: &str, args: Value) -> String {
    let (is_error, text) = call_raw(client, name, args).await;
    assert!(!is_error, "{name} failed: {text}");
    text
}

#[tokio::test]
async fn lists_four_tools() {
    let rig = Rig::new();
    let client = rig.connect().await;
    let tools = client.list_all_tools().await.expect("list_tools");
    let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        ["task_create", "task_get", "task_list", "task_update"]
    );
    let _ = client.cancel().await;
}

#[tokio::test]
async fn create_then_list_round_trip() {
    let rig = Rig::new();
    let client = rig.connect().await;
    let created = call(
        &client,
        "task_create",
        json!({"tasks": [{"subject": "write it"}]}),
    )
    .await;
    assert_eq!(created.trim_end(), "created #1 write it");
    let list = call(&client, "task_list", json!({})).await;
    assert!(list.contains("#1 [ ] write it"), "{list}");
    let _ = client.cancel().await;
}

#[tokio::test]
async fn agy_host_without_root_or_variable_names_the_variable() {
    let rig = Rig::new();
    let mut c = Command::new(BIN);
    c.arg("serve");
    rig.scrub(&mut c, &rig.elsewhere);
    // The client declares no `roots` capability, and no variable is set.
    let transport = TokioChildProcess::new(tokio::process::Command::from(c)).expect("spawn");
    let client = ().serve(transport).await.expect("initialize");
    let (is_error, text) = call_raw(&client, "task_list", json!({})).await;
    assert!(is_error, "{text}");
    assert!(text.contains("REPO_TASKS_DIR"), "{text}");
    let _ = client.cancel().await;
}

#[test]
fn reminder_prints_hook_json() {
    let rig = Rig::new();
    write_three_one(&rig.repo);
    let (ok, stdout, stderr) =
        rig.reminder(&json!({"workspacePaths": [rig.repo], "invocationNum": 1}));
    assert!(ok, "{stderr}");
    let v: Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(v, json!({"injectSteps": [{"ephemeralMessage": LINE_3_1}]}));
    assert!(!stdout.contains("SECRET"));

    // Only the first invocation of a session is reminded.
    let (ok, stdout, stderr) =
        rig.reminder(&json!({"workspacePaths": [rig.repo], "invocationNum": 2}));
    assert!(ok, "{stderr}");
    assert_eq!(stdout, "");
}

#[test]
fn hook_manifest_declares_preinvocation() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks.json");
    let v: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let hooks = v["repo-tasks-reminder"]["PreInvocation"]
        .as_array()
        .unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0]["type"], "command");
    assert_eq!(
        hooks[0]["command"],
        "repo-tasks-agy-mcp reminder --event antigravity"
    );
    assert_eq!(v.as_object().unwrap().len(), 1);
}
