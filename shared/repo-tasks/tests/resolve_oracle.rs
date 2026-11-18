// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The 7.2 walk against the oracle `git rev-parse --path-format=absolute --git-common-dir`.

use repo_tasks::resolve::{locate, Env};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct Tmp(PathBuf);

impl Tmp {
    fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("repo-tasks-oracle-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Tmp(fs::canonicalize(p).unwrap())
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    let out = Command::new("git")
        .current_dir(dir)
        .env_remove("GIT_DIR")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_CEILING_DIRECTORIES")
        .env_remove("GIT_WORK_TREE")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "protocol.file.allow=always",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("run git");
    out
}

fn git_ok(dir: &Path, args: &[&str]) {
    let out = git(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn oracle(dir: &Path) -> PathBuf {
    let out = git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    PathBuf::from(String::from_utf8(out.stdout).unwrap().trim_end())
}

fn env_for(base: &Path) -> Env {
    Env {
        home: Some(base.to_path_buf()),
        xdg_data_home: Some(base.join("data")),
        local_app_data: Some(base.join("data")),
        ..Env::default()
    }
}

fn engine(dir: &Path, base: &Path) -> PathBuf {
    let s = locate(dir, &env_for(base)).expect("locate");
    s.common_dir.expect("inside a repository")
}

fn init_repo(top: &Path) {
    fs::create_dir_all(top).unwrap();
    git_ok(top, &["init", "-q"]);
    git_ok(top, &["commit", "-q", "--allow-empty", "-m", "first"]);
}

#[test]
fn oracle_matches_git_for_each_case() {
    if !git_available() {
        eprintln!("SKIP oracle_matches_git_for_each_case: git is not on PATH");
        return;
    }
    let t = Tmp::new();
    let base = t.0.clone();

    // 1. the top of a repository; 2. a subdirectory of one.
    let main = base.join("main");
    init_repo(&main);
    let sub = main.join("a").join("b");
    fs::create_dir_all(&sub).unwrap();
    assert_eq!(engine(&main, &base), oracle(&main));
    assert_eq!(engine(&sub, &base), oracle(&sub));
    assert_eq!(engine(&sub, &base), main.join(".git"));

    // 3. a linked worktree: its top and a subdirectory.
    let wt = base.join("wt");
    git_ok(
        &main,
        &["worktree", "add", "-q", wt.to_str().unwrap(), "-b", "other"],
    );
    let wt_sub = wt.join("x");
    fs::create_dir_all(&wt_sub).unwrap();
    assert_eq!(engine(&wt, &base), oracle(&wt));
    assert_eq!(engine(&wt_sub, &base), oracle(&wt_sub));
    assert_eq!(engine(&wt, &base), engine(&main, &base));

    // `.git` file with `gitdir:` and a trailing carriage return.
    let gitfile = fs::read_to_string(wt.join(".git")).unwrap();
    fs::write(wt.join(".git"), format!("{}\r\n", gitfile.trim_end())).unwrap();
    assert_eq!(engine(&wt, &base), oracle(&wt));
    assert_eq!(engine(&wt, &base), main.join(".git"));

    // `commondir` relative (git's own form) and absolute.
    let wt_git = PathBuf::from(gitfile.trim_end().strip_prefix("gitdir: ").unwrap());
    let rel = fs::read_to_string(wt_git.join("commondir")).unwrap();
    assert!(Path::new(rel.trim_end()).is_relative(), "git wrote {rel:?}");
    assert_eq!(engine(&wt, &base), oracle(&wt));
    fs::write(
        wt_git.join("commondir"),
        format!("{}\r\n", main.join(".git").display()),
    )
    .unwrap();
    assert_eq!(engine(&wt, &base), oracle(&wt));
    assert_eq!(engine(&wt_sub, &base), oracle(&wt_sub));

    // 4. inside a submodule: a path under the superproject's `.git/modules/`.
    let subrepo = base.join("subsrc");
    init_repo(&subrepo);
    let sup = base.join("super");
    init_repo(&sup);
    git_ok(
        &sup,
        &["submodule", "add", "-q", subrepo.to_str().unwrap(), "mod"],
    );
    let in_mod = sup.join("mod");
    assert_eq!(engine(&in_mod, &base), oracle(&in_mod));
    assert!(engine(&in_mod, &base).starts_with(sup.join(".git").join("modules")));
    assert_ne!(engine(&in_mod, &base), engine(&sup, &base));

    // A bare repository has no `.git` entry: not a repository.
    let bare = base.join("bare.git");
    fs::create_dir_all(&bare).unwrap();
    git_ok(&bare, &["init", "-q", "--bare"]);
    let s = locate(&bare, &env_for(&base)).unwrap();
    assert_eq!(s.common_dir, None);
    assert!(s.dir.starts_with(base.join("data").join("repo-tasks")));

    // Outside any repository.
    let plain = base.join("plain");
    fs::create_dir_all(&plain).unwrap();
    assert_eq!(locate(&plain, &env_for(&base)).unwrap().common_dir, None);
}

#[test]
fn git_entry_that_leads_nowhere_is_an_error() {
    let t = Tmp::new();
    let base = t.0.clone();
    // A `.git` file pointing at a missing directory (a worktree whose main repository is gone).
    let wt = base.join("wt");
    fs::create_dir_all(&wt).unwrap();
    fs::write(wt.join(".git"), "gitdir: /no/such/main/.git/worktrees/wt\n").unwrap();
    let sub = wt.join("deep");
    fs::create_dir_all(&sub).unwrap();
    for d in [&wt, &sub] {
        let err = locate(d, &env_for(&base)).expect_err("must not fall to non-git");
        let text = err.to_string();
        assert!(text.contains(".git"), "{text}");
        assert!(text.contains(wt.join(".git").to_str().unwrap()), "{text}");
        assert!(!text.contains('\n'));
    }
    // A `.git` file without the `gitdir: ` prefix.
    let bad = base.join("bad");
    fs::create_dir_all(&bad).unwrap();
    fs::write(bad.join(".git"), "nonsense").unwrap();
    assert!(locate(&bad, &env_for(&base)).is_err());
    // A `.git` directory with no HEAD.
    let nohead = base.join("nohead");
    fs::create_dir_all(nohead.join(".git")).unwrap();
    assert!(locate(&nohead, &env_for(&base)).is_err());
}
