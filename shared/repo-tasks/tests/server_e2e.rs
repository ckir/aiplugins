// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! End-to-end tests: the real server (`mock-repo-tasks`) as a child process, driven by rmcp's
//! client, in throwaway git repositories under a temporary directory.

// Roots are deprecated in the MCP specification; the tests speak them on purpose.
#![allow(deprecated)]

use repo_tasks::clock::rfc3339;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientCapabilities, ClientConfig, Implementation,
    ListRootsResult, Root,
};
use rmcp::service::{RequestContext, RunningService};
use rmcp::transport::TokioChildProcess;
use rmcp::{ClientHandler, ErrorData, RoleClient, ServiceExt};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SERVE_BIN: &str = env!("CARGO_BIN_EXE_mock-repo-tasks");
const SENTENCE: &str =
    "Use these tools for work that must outlive the session or be seen by another agent.";

struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "repo-tasks-e2e-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let p = fs::canonicalize({
            fs::create_dir_all(&p).unwrap();
            &p
        })
        .unwrap();
        Tmp(p)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A throwaway git repository, a home, and a plain directory that is not a repository.
struct Rig {
    _tmp: Tmp,
    repo: PathBuf,
    home: PathBuf,
    elsewhere: PathBuf,
}

impl Rig {
    fn new() -> Rig {
        let tmp = Tmp::new();
        let repo = tmp.0.join("repo");
        let home = tmp.0.join("home");
        let elsewhere = tmp.0.join("elsewhere");
        for d in [&repo, &home, &elsewhere] {
            fs::create_dir_all(d).unwrap();
        }
        let out = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .output()
            .expect("git init");
        assert!(out.status.success(), "git init failed");
        Rig {
            _tmp: tmp,
            repo,
            home,
            elsewhere,
        }
    }

    fn store(&self) -> PathBuf {
        self.repo.join(".git").join("repo-tasks")
    }

    fn command(&self, cwd: &Path, host: &str, window_ms: u64) -> Command {
        let mut c = Command::new(SERVE_BIN);
        self.scrub(&mut c, cwd, host, window_ms);
        c
    }

    fn scrub(&self, c: &mut Command, cwd: &Path, host: &str, window_ms: u64) {
        c.current_dir(cwd)
            .env("HOME", &self.home)
            .env("REPO_TASKS_TEST_HOST", host)
            .env("REPO_TASKS_TEST_ROOTS_WINDOW_MS", window_ms.to_string())
            .env_remove("REPO_TASKS_DIR")
            .env_remove("REPO_TASKS_LIST")
            .env_remove("XDG_DATA_HOME");
    }

    fn tokio_command(&self, cwd: &Path, host: &str, window_ms: u64) -> tokio::process::Command {
        let mut std_cmd = Command::new(SERVE_BIN);
        self.scrub(&mut std_cmd, cwd, host, window_ms);
        tokio::process::Command::from(std_cmd)
    }
}

/// How the test client answers `roots/list`.
#[derive(Clone)]
enum Roots {
    /// Declares no `roots` capability.
    Undeclared,
    /// Declares it and answers with these URIs.
    Answers(Vec<String>),
    /// Declares it and never answers.
    Silent,
}

#[derive(Clone)]
struct TestClient {
    roots: Roots,
    asked: Arc<AtomicUsize>,
}

impl TestClient {
    fn new(roots: Roots) -> TestClient {
        TestClient {
            roots,
            asked: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl ClientHandler for TestClient {
    fn get_info(&self) -> ClientConfig {
        let caps = match self.roots {
            Roots::Undeclared => ClientCapabilities::builder().build(),
            _ => ClientCapabilities::builder().enable_roots().build(),
        };
        ClientConfig::new(caps, Implementation::new("test-client", "0"))
    }

    async fn list_roots(
        &self,
        _context: RequestContext<RoleClient>,
    ) -> Result<ListRootsResult, ErrorData> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        match &self.roots {
            Roots::Answers(uris) => Ok(ListRootsResult::new(
                uris.iter().map(|u| Root::new(u.clone())).collect(),
            )),
            _ => std::future::pending().await,
        }
    }
}

type Client = RunningService<RoleClient, TestClient>;

async fn connect(cmd: tokio::process::Command, client: TestClient) -> Client {
    let transport = TokioChildProcess::new(cmd).expect("spawn server");
    client.serve(transport).await.expect("initialize")
}

fn file_uri(p: &Path) -> String {
    format!("file://{}", p.display())
}

async fn call_raw(client: &Client, name: &str, args: Value) -> CallToolResult {
    let params = CallToolRequestParams::new(name.to_string())
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    client.call_tool(params).await.expect("call_tool")
}

fn text_of(r: &CallToolResult) -> String {
    r.content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// The reply text of a call that must have succeeded.
async fn call(client: &Client, name: &str, args: Value) -> String {
    let r = call_raw(client, name, args).await;
    assert_ne!(r.is_error, Some(true), "{name} failed: {}", text_of(&r));
    text_of(&r)
}

/// A list file whose tasks are all completed 13 hours ago.
fn write_old_completed_list(rig: &Rig) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let at = rfc3339(now - 13 * 3600);
    let task = |id: u64| {
        json!({
            "id": id, "subject": format!("old {id}"), "status": "completed",
            "blocked_by": [], "created_at": at, "updated_at": at, "updated_by": "claude-code"
        })
    };
    let list = json!({"format": 1, "next_id": 3, "tasks": [task(1), task(2)]});
    let dir = rig.store();
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("default.json"), serde_json::to_vec(&list).unwrap()).unwrap();
}

fn archive_lines(rig: &Rig) -> usize {
    fs::read_to_string(rig.store().join("default.archive.jsonl"))
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

fn list_task_count(rig: &Rig) -> usize {
    let v: Value =
        serde_json::from_slice(&fs::read(rig.store().join("default.json")).unwrap()).unwrap();
    v["tasks"].as_array().unwrap().len()
}

#[tokio::test]
async fn lists_four_tools_with_the_sentence() {
    let rig = Rig::new();
    let client = connect(
        rig.tokio_command(&rig.repo, "claude", 2000),
        TestClient::new(Roots::Undeclared),
    )
    .await;
    let tools = client.list_all_tools().await.expect("list_tools");
    let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    names.sort();
    assert_eq!(
        names,
        ["task_create", "task_get", "task_list", "task_update"]
    );
    for t in &tools {
        let d = t.description.as_deref().unwrap_or_default();
        assert_eq!(d.matches(SENTENCE).count(), 1, "{}: {d}", t.name);
    }
    let _ = client.cancel().await;
}

#[tokio::test]
async fn five_step_scenario() {
    let rig = Rig::new();
    let client = connect(
        rig.tokio_command(&rig.repo, "claude", 2000),
        TestClient::new(Roots::Undeclared),
    )
    .await;
    let created = call(
        &client,
        "task_create",
        json!({"tasks": [
            {"subject": "one"},
            {"subject": "two", "blocked_by_earlier": [1]},
            {"subject": "three", "blocked_by_earlier": [2]},
            {"subject": "four", "blocked_by_earlier": [3]},
            {"subject": "five", "blocked_by_earlier": [4]},
        ]}),
    )
    .await;
    assert_eq!(
        created.lines().collect::<Vec<_>>(),
        [
            "created #1 one",
            "created #2 two",
            "created #3 three",
            "created #4 four",
            "created #5 five"
        ]
    );
    call(
        &client,
        "task_update",
        json!({"id": 1, "status": "completed"}),
    )
    .await;
    call(
        &client,
        "task_update",
        json!({"id": 2, "status": "completed"}),
    )
    .await;
    call(
        &client,
        "task_update",
        json!({"id": 3, "status": "in_progress"}),
    )
    .await;
    let _ = client.cancel().await;

    // A second server process reads what the first wrote.
    let second = connect(
        rig.tokio_command(&rig.repo, "claude", 2000),
        TestClient::new(Roots::Undeclared),
    )
    .await;
    let list = call(&second, "task_list", json!({})).await;
    assert!(list.contains("#1 [x] one"), "{list}");
    assert!(list.contains("#2 [x] two"), "{list}");
    assert!(list.contains("#3 [>] three"), "{list}");
    assert!(list.contains("#4 [ ] four [blocked by #3]"), "{list}");
    assert!(list.contains("#5 [ ] five [blocked by #4]"), "{list}");
    let got = call(&second, "task_get", json!({"id": 3})).await;
    assert!(got.contains("status: in_progress"), "{got}");
    let _ = second.cancel().await;
}

#[tokio::test]
async fn roots_used_when_no_variable() {
    let rig = Rig::new();
    let client = connect(
        rig.tokio_command(&rig.elsewhere, "claude", 2000),
        TestClient::new(Roots::Answers(vec![file_uri(&rig.repo)])),
    )
    .await;
    call(
        &client,
        "task_create",
        json!({"tasks": [{"subject": "via root"}]}),
    )
    .await;
    let list = call(&client, "task_list", json!({})).await;
    assert!(list.contains("#1 [ ] via root"), "{list}");
    assert!(
        list.contains(&format!("from {}", rig.repo.display())),
        "{list}"
    );
    assert!(rig.store().join("default.json").exists());
    let _ = client.cancel().await;
}

#[tokio::test]
async fn antigravity_host_without_root_or_variable_fails_naming_variable() {
    let rig = Rig::new();
    let client = connect(
        rig.tokio_command(&rig.repo, "agy", 2000),
        TestClient::new(Roots::Undeclared),
    )
    .await;
    let r = call_raw(&client, "task_list", json!({})).await;
    assert_eq!(r.is_error, Some(true), "{}", text_of(&r));
    let t = text_of(&r);
    assert!(t.contains("REPO_TASKS_DIR"), "{t}");
    assert_eq!(t.lines().count(), 1, "{t}");
    assert!(!rig.store().exists());
    let _ = client.cancel().await;
}

/// Reads one JSON line from the child's stdout, with a limit.
fn read_json_line(rx: &mpsc::Receiver<String>) -> Value {
    let line = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("a response line");
    serde_json::from_str(&line).expect("json")
}

#[test]
fn call_before_initialized_is_json_rpc_error() {
    let rig = Rig::new();
    let mut child = rig
        .command(&rig.repo, "claude", 2000)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut send = |v: Value| {
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap();
    };
    send(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": {"name": "raw", "version": "0"}}}),
    );
    let init = read_json_line(&rx);
    assert_eq!(init["id"], 1, "{init}");
    assert!(init.get("result").is_some(), "{init}");
    // No `notifications/initialized` yet.
    send(
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
        "name": "task_create", "arguments": {"tasks": [{"subject": "too early"}]}}}),
    );
    let reply = read_json_line(&rx);
    assert_eq!(reply["id"], 2, "{reply}");
    assert_eq!(reply["error"]["code"], -32600, "{reply}");
    assert!(reply.get("result").is_none(), "{reply}");
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("not initialized"),
        "{reply}"
    );
    assert!(!rig.store().exists(), "a list was touched");
    let _ = child.kill();
    let _ = child.wait();
}

#[tokio::test]
async fn client_that_never_answers_roots_does_not_hang() {
    let rig = Rig::new();
    write_old_completed_list(&rig);
    let client = connect(
        rig.tokio_command(&rig.repo, "claude", 400),
        TestClient::new(Roots::Silent),
    )
    .await;
    let began = Instant::now();
    let list = call(&client, "task_list", json!({})).await;
    let took = began.elapsed();
    assert!(took < Duration::from_millis(400 + 1500), "took {took:?}");
    // Timed out: the call falls to the working directory (the repository) and the list is untouched.
    assert!(list.contains("#1 [x] old 1"), "{list}");
    assert_eq!(list_task_count(&rig), 2);
    assert_eq!(archive_lines(&rig), 0, "start-up archiving ran");
    let _ = client.cancel().await;
}

#[tokio::test]
async fn startup_archives_old_completed_list() {
    let rig = Rig::new();
    write_old_completed_list(&rig);
    let client = connect(
        rig.tokio_command(&rig.elsewhere, "claude", 2000),
        TestClient::new(Roots::Answers(vec![file_uri(&rig.repo)])),
    )
    .await;
    let list = call(&client, "task_list", json!({})).await;
    assert!(list.contains("no tasks"), "{list}");
    assert_eq!(archive_lines(&rig), 2);
    let _ = client.cancel().await;
}

#[tokio::test]
async fn startup_skipped_on_timeout_only() {
    let rig = Rig::new();
    write_old_completed_list(&rig);
    // A request settled only by the window running out never triggers it.
    let silent = connect(
        rig.tokio_command(&rig.repo, "claude", 300),
        TestClient::new(Roots::Silent),
    )
    .await;
    let _ = call(&silent, "task_list", json!({})).await;
    assert_eq!(archive_lines(&rig), 0);
    assert_eq!(list_task_count(&rig), 2);
    let _ = silent.cancel().await;

    // A client that declared no roots has nothing to settle: the same list is archived.
    let plain = TestClient::new(Roots::Undeclared);
    let asked = plain.asked.clone();
    let client = connect(rig.tokio_command(&rig.repo, "claude", 300), plain).await;
    let list = call(&client, "task_list", json!({})).await;
    assert!(list.contains("no tasks"), "{list}");
    assert_eq!(archive_lines(&rig), 2);
    assert_eq!(asked.load(Ordering::SeqCst), 0, "a request was sent");
    let _ = client.cancel().await;
}
