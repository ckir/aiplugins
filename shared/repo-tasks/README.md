# repo-tasks

The engine behind the `repo-tasks` plugins: one task list per repository, shared by every agent
that works in it (Claude Code, Qwen, Antigravity, OpenCode), stored as files and served over MCP.
Design: `docs/superpowers/specs/2026-11-18-repo-tasks-design.md` (the binding authority).

## The four tools

- `task_create`: create one to 50 tasks in one call; later tasks may name earlier ones as blockers.
- `task_get`: read one task by integer id; a task that has left the list is answered from the archive.
- `task_update`: change status, subject, description, owner or blockers, or delete a task.
- `task_list`: list every task, one line each, under a first line naming the list.

## Environment variables

- `REPO_TASKS_DIR`: the starting directory (absolute, existing). Blank counts as unset.
- `REPO_TASKS_LIST`: the list name, `[A-Za-z0-9_-]{1,64}`; default `default`.

## Where the list lives

- Inside git: the git common directory's `repo-tasks/` folder, so worktrees of one repository
  share a list. The engine reads `.git` itself and runs no `git` process.
- Outside git: the per-user data directory (`$XDG_DATA_HOME` or `~/.local/share` on Linux,
  `~/Library/Application Support` on macOS, `%LOCALAPPDATA%` on Windows), under
  `repo-tasks/<key>/`. The key is the first 16 hex characters of the SHA-256 of the canonical
  starting directory D. A subdirectory of a non-git directory therefore opens a different list.

Files per list `N`: `N.json`, `N.lock`, `N.json.tmp`, `N.archive.jsonl`.

## How the starting directory is found (spec 7.1)

For the server: `REPO_TASKS_DIR`, then the client's first MCP root, then the process working
directory. Antigravity has no working-directory fallback: without a root it reports an error.
The reminder uses the variable, then the host event's workspace path, then the process directory
(again, not for Antigravity). Subcommands use `--dir`, then the variable, then the process directory.

## Archive

Completed tasks leave the list for `N.archive.jsonl`; `task_get` still answers for them, and
`repo-tasks list --archived` prints them.

## Subcommands

```
serve [--host h]
reminder --event <claude-code|qwen|antigravity|opencode> [--workspace <path>]
list [--archived] [--dir <path>]
archive [--dir <path>]
--version
```

## Known risks

- `rmcp`'s `Peer::list_roots` is marked deprecated in rmcp 3.10.0
  (`src/service/server.rs:931`) because the MCP specification is deprecating roots. The engine
  uses it with `#[allow(deprecated)]`. A later rmcp that removes it needs rework of spec 7.1.
- Windows and macOS behaviour is `not measured` until CI has run the branch. That covers the
  canonical form of differently cased paths (`tests/windows_canonical.rs`) and the error kind of a
  refused rename or read while another process holds the file (`windows_reader_and_writer_never_fail`
  in `tests/store.rs`). When `REPO_TASKS_CI_NOTES` names a file, both tests append their findings to
  it so CI can upload them. Status: Task 11 measurements pending; the tests compile for
  `x86_64-pc-windows-gnu` but have not run.
- The three length limits (subject 200, description 8000, owner 64) are proposed and unmeasured.
- Locking rests on OS advisory locks, which not every filesystem honours.
