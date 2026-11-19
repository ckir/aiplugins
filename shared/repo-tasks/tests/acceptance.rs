// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Section 18 of the design, sentence by sentence: two server processes (a main checkout and a
//! linked worktree of one repository), the `reminder` subcommand, list files aged by writing old
//! timestamps, and `list --archived` at the end.

// Roots are deprecated in the MCP specification; the client below declares none.
#![allow(deprecated)]

use repo_tasks::cli::{run, Io};
use repo_tasks::clock::rfc3339;
use repo_tasks::host::Host;
use repo_tasks::reminder::HookEnv;
use repo_tasks::resolve::Env;
use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientConfig, Implementation};
use rmcp::service::RunningService;
use rmcp::transport::TokioChildProcess;
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SERVE_BIN: &str = env!("CARGO_BIN_EXE_mock-repo-tasks");

struct Tmp(PathBuf);

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct NoRoots;

impl ClientHandler for NoRoots {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::new(
            ClientCapabilities::builder().build(),
            Implementation::new("acceptance-client", "0"),
        )
    }
}

type Client = RunningService<RoleClient, NoRoots>;

struct World {
    home: PathBuf,
    main: PathBuf,
    linked: PathBuf,
}

impl World {
    fn server_command(&self, cwd: &Path) -> tokio::process::Command {
        let mut c = Command::new(SERVE_BIN);
        c.current_dir(cwd)
            .env("HOME", &self.home)
            .env("REPO_TASKS_TEST_HOST", "claude")
            .env("REPO_TASKS_TEST_ROOTS_WINDOW_MS", "200")
            .env_remove("REPO_TASKS_DIR")
            .env_remove("REPO_TASKS_LIST")
            .env_remove("XDG_DATA_HOME");
        tokio::process::Command::from(c)
    }

    async fn start(&self, cwd: &Path) -> Client {
        let t = TokioChildProcess::new(self.server_command(cwd)).expect("spawn server");
        NoRoots.serve(t).await.expect("initialize")
    }

    fn store(&self) -> PathBuf {
        self.main.join(".git").join("repo-tasks")
    }

    fn env(&self) -> Env {
        Env {
            home: Some(self.home.clone()),
            ..Env::default()
        }
    }

    /// The engine's command line, in-process; stdout and the exit code.
    async fn cli(&self, args: &[&str]) -> (i32, String) {
        let mut input: &[u8] = b"";
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut io = Io {
            env: self.env(),
            hook_env: HookEnv::default(),
            stdin: &mut input,
            out: &mut out,
            err: &mut err,
        };
        let args = args.iter().map(|s| s.to_string()).collect();
        let code = run(Host::Claude, "repo-tasks", "test", args, &mut io).await;
        assert!(err.is_empty(), "stderr: {}", String::from_utf8_lossy(&err));
        (code, String::from_utf8(out).unwrap())
    }

    /// Moves every task's `updated_at` to `age_secs` ago, as a clock moved forward would.
    fn age_list(&self, age_secs: i64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let path = self.store().join("default.json");
        let mut v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for t in v["tasks"].as_array_mut().unwrap() {
            t["updated_at"] = json!(rfc3339(now - age_secs));
        }
        fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    }

    fn list_ids(&self) -> Vec<u64> {
        let v: Value =
            serde_json::from_slice(&fs::read(self.store().join("default.json")).unwrap()).unwrap();
        v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_u64().unwrap())
            .collect()
    }

    fn archive_lines(&self) -> usize {
        fs::read_to_string(self.store().join("default.archive.jsonl"))
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }
}

fn git(dir: &Path, home: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env_remove("GIT_DIR")
        .env_remove("GIT_COMMON_DIR")
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

async fn call(client: &Client, name: &str, args: Value) -> String {
    let params = CallToolRequestParams::new(name.to_string())
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    let r = client.call_tool(params).await.expect("call_tool");
    let text = r
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert_ne!(r.is_error, Some(true), "{name} failed: {text}");
    text
}

#[tokio::test]
async fn acceptance_section_18() {
    let base = std::env::temp_dir().join(format!("repo-tasks-accept-{}", std::process::id()));
    fs::create_dir_all(&base).unwrap();
    let base = repo_tasks::resolve::canonical(&base).unwrap();
    let _tmp = Tmp(base.clone());
    let home = base.join("home");
    let main = base.join("main");
    let linked = base.join("linked");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&main).unwrap();
    git(&main, &home, &["init", "-q"]);
    git(
        &main,
        &home,
        &["commit", "-q", "--allow-empty", "-m", "start"],
    );
    git(
        &main,
        &home,
        &["worktree", "add", "-q", linked.to_str().unwrap()],
    );
    let w = World { home, main, linked };

    // S1-S2: in a repository the model calls `task_create` once with five tasks, the later ones
    // naming the earlier; the engine writes repo-tasks/default.json and replies with five ids.
    let first = w.start(&w.main).await;
    let created = call(
        &first,
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
    assert!(w.store().join("default.json").is_file());

    // S3: tasks 1 and 2 completed, task 3 in progress.
    call(
        &first,
        "task_update",
        json!({"id": 1, "status": "completed"}),
    )
    .await;
    call(
        &first,
        "task_update",
        json!({"id": 2, "status": "completed"}),
    )
    .await;
    call(
        &first,
        "task_update",
        json!({"id": 3, "status": "in_progress"}),
    )
    .await;

    // S4: the session is killed (the client drops the child without a goodbye).
    drop(first);

    // S5-S6: a second worktree of the same repository; the reminder says three open, one in
    // progress, and names no task. Both worktrees resolve to one git common directory.
    assert!(
        w.linked.join(".git").is_file(),
        "the second checkout is a linked worktree"
    );
    let (code, out) = w
        .cli(&[
            "reminder",
            "--event",
            "opencode",
            "--workspace",
            w.linked.to_str().unwrap(),
        ])
        .await;
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "repo-tasks: 3 open in this repository's task list, 1 in progress. Call the task_list tool to see them.\n"
    );
    for subject in ["one", "two", "three", "four", "five"] {
        assert!(!out.split_whitespace().any(|w| w == subject), "{out}");
    }

    // S7: `task_list` shows five lines: two completed, task 3 in progress, last changed by
    // claude-code with its time, tasks 4 and 5 blocked.
    let second = w.start(&w.linked).await;
    let listed = call(&second, "task_list", json!({})).await;
    let lines: Vec<&str> = listed.lines().collect();
    assert_eq!(lines.len(), 6, "{listed}");
    assert!(lines[0].starts_with("repo-tasks: "), "{listed}");
    assert_eq!(lines[1], "#1 [x] one");
    assert_eq!(lines[2], "#2 [x] two");
    assert!(
        lines[3].starts_with("#3 [>] three [claude-code 20") && lines[3].ends_with("Z]"),
        "{listed}"
    );
    assert_eq!(lines[4], "#4 [ ] four [blocked by #3]");
    assert_eq!(lines[5], "#5 [ ] five [blocked by #4]");

    // S8: it takes task 3 over with an ordinary update, and deletes task 5, which moves to the
    // archive.
    call(
        &second,
        "task_update",
        json!({"id": 3, "owner": "second-agent", "status": "in_progress"}),
    )
    .await;
    call(&second, "task_update", json!({"id": 5, "delete": true})).await;
    assert_eq!(w.list_ids(), [1, 2, 3, 4]);
    assert_eq!(w.archive_lines(), 1);
    let got = call(&second, "task_get", json!({"id": 5})).await;
    assert!(got.contains("deleted"), "{got}");

    // S9: when the other four are completed they stay in the list, and an agent opened an hour
    // later still sees them.
    call(
        &second,
        "task_update",
        json!({"id": 3, "status": "completed"}),
    )
    .await;
    call(
        &second,
        "task_update",
        json!({"id": 4, "status": "completed"}),
    )
    .await;
    drop(second);
    assert_eq!(w.list_ids(), [1, 2, 3, 4]);
    w.age_list(3600);
    let hour_later = w.start(&w.main).await;
    let listed = call(&hour_later, "task_list", json!({})).await;
    for l in ["#1 [x] one", "#2 [x] two", "#3 [x] three", "#4 [x] four"] {
        assert!(listed.lines().any(|x| x.starts_with(l)), "{listed}");
    }
    drop(hour_later);
    assert_eq!(
        w.list_ids(),
        [1, 2, 3, 4],
        "one hour is not enough to archive"
    );
    assert_eq!(w.archive_lines(), 1);

    // S10: the next day, more than 12 hours after the last change, an agent's MCP server starts,
    // finds every task completed, and moves them to the archive.
    w.age_list(13 * 3600);
    let next_day = w.start(&w.linked).await;
    let listed = call(&next_day, "task_list", json!({})).await;
    assert!(listed.lines().any(|l| l == "no tasks"), "{listed}");
    drop(next_day);
    assert_eq!(w.list_ids(), Vec::<u64>::new());
    assert_eq!(w.archive_lines(), 5, "four archived and one deleted");

    // S11: the owner prints all five with `list --archived`.
    let (code, out) = w
        .cli(&["list", "--archived", "--dir", w.main.to_str().unwrap()])
        .await;
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 6, "{out}");
    assert!(lines[0].starts_with("repo-tasks: "), "{out}");
    for (i, subject) in ["one", "two", "three", "four"].iter().enumerate() {
        let owner = if i == 2 { " [owner second-agent]" } else { "" };
        let want = format!("#{} [x] {subject}{owner} [archived 20", i + 1);
        assert!(
            lines[i + 1].starts_with(&want) && lines[i + 1].ends_with("Z]"),
            "{out}"
        );
    }
    assert!(
        lines[5].starts_with("#5 [ ] five [deleted 20") && lines[5].ends_with("Z]"),
        "{out}"
    );
}
