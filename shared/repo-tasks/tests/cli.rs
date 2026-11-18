// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The reminder and the command line, run in-process against throwaway repositories.

use repo_tasks::cli::{run, version_line, Io};
use repo_tasks::host::Host;
use repo_tasks::ops::{self, ListHeader};
use repo_tasks::reminder::{wrap, HookEnv, HostEvent, NOTICE};
use repo_tasks::resolve::{locate, Env};
use repo_tasks::store::Tuning;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const AT: &str = "2026-11-18T10:00:00Z";
const LINE_3_1: &str = "repo-tasks: 3 open in this repository's task list, 1 in progress. Call the task_list tool to see them.";

struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "repo-tasks-cli-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&p).unwrap();
        Tmp(fs::canonicalize(&p).unwrap())
    }

    /// A directory that looks like a git repository to the engine (a `.git` with a `HEAD`).
    fn repo(&self, name: &str) -> PathBuf {
        let r = self.0.join(name);
        fs::create_dir_all(r.join(".git")).unwrap();
        fs::write(r.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        r
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn env_in(tmp: &Tmp, cwd: Option<&Path>) -> Env {
    Env {
        home: Some(tmp.0.join("home")),
        cwd: cwd.map(Path::to_path_buf),
        ..Env::default()
    }
}

fn task(id: u64, subject: &str, status: &str) -> Value {
    json!({
        "id": id, "subject": subject, "status": status, "blocked_by": [],
        "created_at": AT, "updated_at": AT, "updated_by": "claude-code"
    })
}

fn write_list(repo: &Path, tasks: Vec<Value>) {
    let next = tasks.len() as u64 + 1;
    write_list_next(repo, tasks, next);
}

fn write_list_next(repo: &Path, tasks: Vec<Value>, next: u64) {
    let dir = repo.join(".git/repo-tasks");
    fs::create_dir_all(&dir).unwrap();
    let list = json!({"format": 1, "next_id": next, "tasks": tasks});
    fs::write(dir.join("default.json"), serde_json::to_vec(&list).unwrap()).unwrap();
}

/// Three open, one in progress, every subject a marker that must never reach the reminder.
fn write_three_one(repo: &Path) {
    write_list(
        repo,
        vec![
            task(1, "SECRET-done", "completed"),
            task(2, "SECRET-doing", "in_progress"),
            task(3, "SECRET-todo", "pending"),
            task(4, "SECRET-todo-too", "pending"),
        ],
    );
}

struct Ran {
    code: i32,
    out: String,
    err: String,
}

async fn go_with(env: Env, hook_env: HookEnv, stdin: &str, args: &[&str]) -> Ran {
    let mut input = stdin.as_bytes();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut io = Io {
        env,
        hook_env,
        stdin: &mut input,
        out: &mut out,
        err: &mut err,
    };
    let args = args.iter().map(|s| s.to_string()).collect();
    let code = run(Host::Claude, "repo-tasks-test", "9.9.9", args, &mut io).await;
    Ran {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

async fn go(env: Env, stdin: &str, args: &[&str]) -> Ran {
    go_with(env, HookEnv::default(), stdin, args).await
}

fn reminder_opencode(env: Env, ws: &Path) -> impl std::future::Future<Output = Ran> {
    let ws = ws.display().to_string();
    async move {
        go(
            env,
            "",
            &["reminder", "--event", "opencode", "--workspace", &ws],
        )
        .await
    }
}

#[tokio::test]
async fn reminder_line_exact() {
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    write_three_one(&repo);
    let r = reminder_opencode(env_in(&tmp, None), &repo).await;
    assert_eq!(r.code, 0);
    assert_eq!(r.out, format!("{LINE_3_1}\n"));
    assert!(!r.out.contains("SECRET"));
    assert!(r.err.is_empty());
}

#[tokio::test]
async fn reminder_wrapped_forms_per_host() {
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    write_three_one(&repo);
    let cwd = json!({"cwd": repo}).to_string();

    let r = go(
        env_in(&tmp, None),
        &cwd,
        &["reminder", "--event", "claude-code"],
    )
    .await;
    let v: Value = serde_json::from_str(r.out.trim_end()).unwrap();
    assert_eq!(
        v,
        json!({"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": LINE_3_1}})
    );
    assert_eq!(
        r.out,
        format!("{}\n", wrap(HostEvent::ClaudeCode, LINE_3_1, None))
    );

    let r = go(env_in(&tmp, None), &cwd, &["reminder", "--event", "qwen"]).await;
    let v: Value = serde_json::from_str(r.out.trim_end()).unwrap();
    assert_eq!(
        v,
        json!({"systemMessage": LINE_3_1, "hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": LINE_3_1}})
    );

    let ev = json!({"workspacePaths": [repo], "cwd": "/nonexistent-elsewhere", "invocationNum": 1})
        .to_string();
    let r = go(
        env_in(&tmp, None),
        &ev,
        &["reminder", "--event", "antigravity"],
    )
    .await;
    let v: Value = serde_json::from_str(r.out.trim_end()).unwrap();
    assert_eq!(v, json!({"injectSteps": [{"ephemeralMessage": LINE_3_1}]}));
}

#[tokio::test]
async fn reminder_hook_env_and_process_dir_fallbacks() {
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    write_three_one(&repo);
    // Claude Code: no cwd in the event, CLAUDE_PROJECT_DIR names the repository.
    let hook = HookEnv {
        claude_project_dir: Some(repo.display().to_string()),
        qwen_project_dir: None,
    };
    let r = go_with(
        env_in(&tmp, None),
        hook,
        "{}",
        &["reminder", "--event", "claude-code"],
    )
    .await;
    assert!(r.out.contains("3 open"), "{}", r.out);
    // Qwen: the process working directory is the last resort.
    let r = go(
        env_in(&tmp, Some(&repo)),
        "{}",
        &["reminder", "--event", "qwen"],
    )
    .await;
    assert!(r.out.contains("3 open"), "{}", r.out);
}

#[tokio::test]
async fn reminder_silent_cases() {
    let tmp = Tmp::new();
    // n is 0: every task completed.
    let done = tmp.repo("done");
    write_list(&done, vec![task(1, "x", "completed")]);
    let r = reminder_opencode(env_in(&tmp, None), &done).await;
    assert_eq!((r.code, r.out.as_str(), r.err.as_str()), (0, "", ""));
    // No list at all, and nothing is created.
    let bare = tmp.repo("bare");
    let r = reminder_opencode(env_in(&tmp, None), &bare).await;
    assert_eq!((r.code, r.out.as_str()), (0, ""));
    assert!(!bare.join(".git/repo-tasks").exists());
    // An unresolvable directory.
    let r = reminder_opencode(env_in(&tmp, None), &tmp.0.join("nonexistent")).await;
    assert_eq!((r.code, r.out.as_str()), (0, ""));
    // A bad REPO_TASKS_DIR prints nothing instead of failing.
    let mut env = env_in(&tmp, None);
    env.repo_tasks_dir = Some("relative/path".to_string());
    let r = reminder_opencode(env, &done).await;
    assert_eq!((r.code, r.out.as_str()), (0, ""));
    // No workspace at all and no working directory.
    let r = go(env_in(&tmp, None), "", &["reminder", "--event", "opencode"]).await;
    assert_eq!((r.code, r.out.as_str()), (0, ""));
}

#[tokio::test]
async fn reminder_notice_for_unusable_list() {
    let tmp = Tmp::new();
    let notice = format!("{NOTICE}\n");

    let bad = tmp.repo("malformed");
    write_list(&bad, vec![]);
    fs::write(bad.join(".git/repo-tasks/default.json"), "{not json").unwrap();
    let r = reminder_opencode(env_in(&tmp, None), &bad).await;
    assert_eq!((r.code, r.out.clone()), (0, notice.clone()));

    let newer = tmp.repo("newer");
    write_list(&newer, vec![]);
    fs::write(
        newer.join(".git/repo-tasks/default.json"),
        r#"{"format": 2, "next_id": 1, "tasks": []}"#,
    )
    .unwrap();
    let r = reminder_opencode(env_in(&tmp, None), &newer).await;
    assert_eq!((r.code, r.out.clone()), (0, notice.clone()));

    // A list that breaks a rule of 8.1.
    let broken = tmp.repo("broken");
    write_list(
        &broken,
        vec![task(1, "x", "completed"), task(1, "dup", "pending")],
    );
    let r = reminder_opencode(env_in(&tmp, None), &broken).await;
    assert_eq!((r.code, r.out.clone()), (0, notice.clone()));

    // An unfollowable `.git`.
    let gone = tmp.0.join("worktree");
    fs::create_dir_all(&gone).unwrap();
    fs::write(
        gone.join(".git"),
        "gitdir: /nonexistent/main/.git/worktrees/w\n",
    )
    .unwrap();
    let r = reminder_opencode(env_in(&tmp, None), &gone).await;
    assert_eq!((r.code, r.out.clone()), (0, notice.clone()));

    // The notice is wrapped like the line.
    let ev = json!({"cwd": gone}).to_string();
    let r = go(
        env_in(&tmp, None),
        &ev,
        &["reminder", "--event", "claude-code"],
    )
    .await;
    assert!(
        r.out.contains(NOTICE) && r.out.contains("additionalContext"),
        "{}",
        r.out
    );
}

#[cfg(unix)]
#[tokio::test]
async fn reminder_notice_for_unreadable_list() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    write_three_one(&repo);
    let file = repo.join(".git/repo-tasks/default.json");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&file).is_ok() {
        // Running as a user the mode does not bind (root): the case cannot be built.
        eprintln!("skipped: mode 000 does not stop this user");
        return;
    }
    let r = reminder_opencode(env_in(&tmp, None), &repo).await;
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!((r.code, r.out), (0, format!("{NOTICE}\n")));
}

#[tokio::test]
async fn reminder_antigravity_never_uses_process_dir() {
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    write_three_one(&repo);
    // The process directory has a list; Antigravity still prints nothing without a path.
    for ev in [
        "{}",
        r#"{"invocationNum": 1}"#,
        r#"{"workspacePaths": [], "cwd": ""}"#,
        "",
    ] {
        let r = go(
            env_in(&tmp, Some(&repo)),
            ev,
            &["reminder", "--event", "antigravity"],
        )
        .await;
        assert_eq!((r.code, r.out.as_str()), (0, ""), "event {ev:?}");
    }
    // The control: Claude Code falls back to the process directory.
    let r = go(
        env_in(&tmp, Some(&repo)),
        "{}",
        &["reminder", "--event", "claude-code"],
    )
    .await;
    assert!(r.out.contains("3 open"), "{}", r.out);
    // Antigravity acts only on its first invocation.
    let ev = json!({"workspacePaths": [repo], "invocationNum": 2}).to_string();
    let r = go(
        env_in(&tmp, None),
        &ev,
        &["reminder", "--event", "antigravity"],
    )
    .await;
    assert_eq!(r.out, "");
    // The first workspace path wins over cwd.
    let ev = json!({"workspacePaths": [repo], "cwd": tmp.0}).to_string();
    let r = go(
        env_in(&tmp, None),
        &ev,
        &["reminder", "--event", "antigravity"],
    )
    .await;
    assert!(r.out.contains("3 open"), "{}", r.out);
    // With the variable set, Antigravity needs no path in the event.
    let mut env = env_in(&tmp, None);
    env.repo_tasks_dir = Some(repo.display().to_string());
    let r = go(env, "{}", &["reminder", "--event", "antigravity"]).await;
    assert!(r.out.contains("3 open"), "{}", r.out);
}

#[tokio::test]
async fn reminder_variable_outranks_event_path() {
    let tmp = Tmp::new();
    let a = tmp.repo("a");
    write_list(&a, vec![task(1, "x", "pending")]);
    let b = tmp.repo("b");
    write_three_one(&b);
    let mut env = env_in(&tmp, None);
    env.repo_tasks_dir = Some(a.display().to_string());
    let r = reminder_opencode(env.clone(), &b).await;
    assert!(r.out.contains("1 open"), "{}", r.out);
    let ev = json!({"cwd": b}).to_string();
    let r = go(env, &ev, &["reminder", "--event", "claude-code"]).await;
    assert!(r.out.contains("1 open"), "{}", r.out);
}

#[tokio::test]
async fn reminder_has_no_dir_argument() {
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    write_three_one(&repo);
    let dir = repo.display().to_string();
    let r = go(
        env_in(&tmp, None),
        "",
        &["reminder", "--event", "opencode", "--dir", &dir],
    )
    .await;
    assert_ne!(r.code, 0);
    assert_eq!(r.out, "");
    assert!(r.err.contains("--dir"), "{}", r.err);
    // Also: a missing or unknown event is an error.
    let r = go(env_in(&tmp, None), "", &["reminder"]).await;
    assert_eq!((r.code, r.out.as_str()), (2, ""));
    let r = go(env_in(&tmp, None), "", &["reminder", "--event", "emacs"]).await;
    assert_eq!((r.code, r.out.as_str()), (2, ""));
}

#[tokio::test]
async fn list_matches_task_list_reply() {
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    write_three_one(&repo);
    let env = env_in(&tmp, None);
    let store = locate(&repo, &env).unwrap();
    let want = ops::list(&store, &Tuning::default(), &ListHeader::for_store(&store)).unwrap();
    let dir = repo.display().to_string();
    let r = go(env.clone(), "", &["list", "--dir", &dir]).await;
    assert_eq!((r.code, r.out.clone()), (0, format!("{want}\n")));
    // Without --dir the variable, then the process directory.
    let r = go(env_in(&tmp, Some(&repo)), "", &["list"]).await;
    assert_eq!(r.out, format!("{want}\n"));
    let mut by_var = env_in(&tmp, None);
    by_var.repo_tasks_dir = Some(dir);
    let r = go(by_var, "", &["list"]).await;
    assert_eq!(r.out, format!("{want}\n"));
}

#[tokio::test]
async fn list_archived_forms() {
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    let dir = repo.display().to_string();
    let env = env_in(&tmp, None);
    let header = {
        let store = locate(&repo, &env).unwrap();
        ListHeader::for_store(&store).0
    };
    // No archive file: line 1 and the sentence.
    write_list_next(&repo, vec![task(5, "live", "pending")], 10);
    let r = go(env.clone(), "", &["list", "--archived", "--dir", &dir]).await;
    assert_eq!(r.out, format!("{header}\nno archived tasks\n"));

    // Archive lines: completed (archived), an in-progress task with an owner and a blocker that
    // was deleted, a line for an id still in the list, a damaged line, and a later line for id 2.
    let line = |mut t: Value, key: &str, at: &str| {
        t[key] = json!(at);
        t.to_string()
    };
    let mut doing = task(2, "second", "in_progress");
    doing["owner"] = json!("alice");
    doing["blocked_by"] = json!([1]);
    let mut doing_later = task(2, "second, later", "pending");
    doing_later["owner"] = json!("alice");
    let lines = [
        line(
            task(1, "first", "completed"),
            "archived_at",
            "2026-11-19T01:00:00Z",
        ),
        line(doing, "deleted_at", "2026-11-19T02:00:00Z"),
        line(doing_later, "deleted_at", "2026-11-19T03:00:00Z"),
        line(
            task(5, "still live", "completed"),
            "archived_at",
            "2026-11-19T04:00:00Z",
        ),
        "{\"id\": 9, \"subj".to_string(),
    ];
    fs::write(
        repo.join(".git/repo-tasks/default.archive.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .unwrap();
    let r = go(env.clone(), "", &["list", "--archived", "--dir", &dir]).await;
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out,
        format!(
            "{header}\n#1 [x] first [archived 2026-11-19T01:00:00Z]\n#2 [ ] second, later [owner alice] [deleted 2026-11-19T03:00:00Z]\n"
        )
    );
    // The plain list points at the archive.
    let r = go(env, "", &["list", "--dir", &dir]).await;
    assert!(r.out.contains("archived tasks exist"), "{}", r.out);
}

#[tokio::test]
async fn archive_subcommand_messages() {
    let tmp = Tmp::new();
    let repo = tmp.repo("r");
    let dir = repo.display().to_string();
    let env = env_in(&tmp, None);
    let r = go(env.clone(), "", &["archive", "--dir", &dir]).await;
    assert_eq!((r.code, r.out.as_str()), (0, "no completed tasks\n"));
    assert!(!repo.join(".git/repo-tasks").exists());

    write_list(
        &repo,
        vec![task(1, "a", "completed"), task(2, "b", "pending")],
    );
    let r = go(env.clone(), "", &["archive", "--dir", &dir]).await;
    assert_eq!((r.code, r.out.as_str()), (0, "archived 1 task\n"));
    let r = go(env.clone(), "", &["archive", "--dir", &dir]).await;
    assert_eq!(r.out, "no completed tasks\n");

    write_list(
        &repo,
        vec![
            task(1, "a", "completed"),
            task(2, "b", "completed"),
            task(3, "c", "pending"),
        ],
    );
    let r = go(env.clone(), "", &["archive", "--dir", &dir]).await;
    assert_eq!(r.out, "archived 2 tasks\n");
    // Whatever their age: the tasks above are all from the past, and the new ones are listed.
    let r = go(env, "", &["list", "--archived", "--dir", &dir]).await;
    assert!(r.out.contains("#1 [x] a [archived "), "{}", r.out);
    assert!(r.out.contains("#2 [x] b [archived "), "{}", r.out);
}

#[tokio::test]
async fn version_line_and_usage() {
    assert_eq!(
        version_line("repo-tasks-test", "9.9.9"),
        "repo-tasks-test 9.9.9"
    );
    let tmp = Tmp::new();
    let r = go(env_in(&tmp, None), "", &["--version"]).await;
    assert_eq!((r.code, r.out.as_str()), (0, "repo-tasks-test 9.9.9\n"));
    let r = go(env_in(&tmp, None), "", &[]).await;
    assert_eq!((r.code, r.out.as_str()), (2, ""));
    assert!(r.err.starts_with("usage: serve"), "{}", r.err);
    let r = go(env_in(&tmp, None), "", &["list", "--bogus"]).await;
    assert_eq!(r.code, 2);
    let r = go(env_in(&tmp, None), "", &["serve", "--host", "qwen"]).await;
    assert_eq!(r.code, 1);
    assert!(r.err.contains("qwen"), "{}", r.err);
}
