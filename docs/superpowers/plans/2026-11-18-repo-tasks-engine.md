# repo-tasks engine and wrappers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `shared/repo-tasks`, a Rust engine that keeps one shared task list per git repository, and its four wrapper plugins (Claude Code, Qwen, Antigravity, OpenCode), as the design document specifies.

**Architecture:** One library crate holds everything (location, file format, locked writes, archive, four MCP tools, roots handling, reminder, command line); each wrapper is a thin binary that names its host and calls `dispatch`, exactly as `claude-code/plugins/opencode-ctl-mcp-cc/src/bin/mcp.rs` does for `opencode-ctl`. The OpenCode wrapper is a TypeScript plugin that runs the Claude Code wrapper's binary with `--host opencode`. Pure logic (resolution, validation, state machines) is separated from I/O so the spec's rules are tested without a host.

**Tech Stack:** Rust (workspace `edition = "2021"`), `rmcp` 3.10.0 (as locked in `Cargo.lock`), `serde`, `serde_json`, `sha2`, `schemars`, `tokio`; standard-library file locking (`File::lock`, `File::try_lock`, as `shared/opencode-ctl/src/pairing.rs:427` uses `lock.lock()`); TypeScript with `bun` for the OpenCode plugin test.

**Spec:** `docs/superpowers/specs/2026-11-18-repo-tasks-design.md` (read it with this plan; section numbers below are its). Measurements the design rests on are in its section 15.7; raw logs of the spike were in `/tmp/repo-tasks-spike/` and are not needed.

## Global Constraints

- The list file is format 1 JSON; `next_id` never decreases; limits: subject 1 to 200 characters after trimming, description at most 8,000, owner at most 64, lengths in Unicode scalar values, all three [PROPOSED, UNMEASURED] (8.1).
- Locking: exclusive advisory lock on `N.lock`, non-blocking attempts repeated until 5 seconds have passed; temp file `N.json.tmp`, flush, rename, folder flush on Unix; Unix modes 0700 folder, 0600 files (8.2, 7.3).
- Windows: a refused rename is retried for up to 5 seconds counted from the first refusal, only for `PermissionDenied` or a sharing violation; a refused read is retried for up to 1 second, same two kinds (8.2, 8.3).
- Store: inside git `C/repo-tasks/`; outside git `B/repo-tasks/K/` with K the first 16 hex characters of the SHA-256 of the UTF-8 bytes of canonical D, `\\?\` removed on Windows, no case folding (7.5). The engine never runs `git` and ignores `GIT_DIR`, `GIT_COMMON_DIR`, `GIT_CEILING_DIRECTORIES` (7.2).
- Roots: the 2 seconds are an engine constant that tests may shorten through a parameter no host configuration reaches (7.1). Start-up archiving: every task completed and every `updated_at` at least 12 hours old [PROPOSED, UNMEASURED] (8.5).
- Four tools only: `task_create`, `task_get`, `task_update`, `task_list`; names and parameters are a contract (9). No skill ships in the first release (13).
- Binaries: `repo-tasks-cc-mcp`, `repo-tasks-qwen-mcp`, `repo-tasks-agy-mcp`; the OpenCode wrapper uses the Claude Code binary with `--host opencode` (14).
- Context stop line: a wrapper whose `tiers.resident.bytes` in `docs/footprints/repo-tasks-mcp-<x>.json` is 3,690 or more is reported to the owner before any budget is recorded (13).
- Gate: `just check` (`Justfile:360`) must stay green; clippy runs with `-D warnings` over `--workspace --all-targets --all-features` (`Justfile:25`); tests run with `cargo nextest run --workspace` (`Justfile:29`) on Ubuntu, Windows and macOS (`.github/workflows/ci.yml`, matrix `os`).
- No new crate dependency without `cargo deny check` passing (`deny.toml` exists). Time (RFC 3339 UTC, whole seconds) and percent-decoding are written in the crate, not added as dependencies.
- Nothing is pushed by this plan. Commits are local; the owner decides when to push.

## Review Focus

- A list file edited by hand or by an older engine (unknown members, a newer `format`, an id above `next_id`): kept unchanged / refused with both numbers (8.4). Test in Task 2.
- Two writers at once, and a writer killed between archive append and list rewrite: no lost task, no duplicate in `task_list` (8.2, 8.5). Tests in Tasks 3 and 4.
- A subject or owner with a control character or an escape sequence: refused, never stored (8.1). Test in Task 2.
- A task named as its own blocker, a cycle, or a blocker that has left the list (9.6). Tests in Task 5.
- A host that sends a tool call before `initialized`, never answers `roots/list`, or answers with a non-file URI (7.1). Tests in Task 6 and Task 7.
- A repository whose `.git` is a worktree pointer to a missing main repository (7.2): an error, never "not a git repository". Test in Task 1.

---

## File structure

`shared/repo-tasks/` (new):
- `Cargo.toml`, `src/lib.rs` (module list), `src/error.rs` (error type and code)
- `src/clock.rs` (RFC 3339 formatting, a `Clock` trait)
- `src/resolve.rs` (7.1 starting directory, 7.2 git directory, 7.3 to 7.5 store folder)
- `src/model.rs` (8.1 list file, validation, 8.4 version handling)
- `src/store.rs` (8.2 writing, 8.3 reading)
- `src/archive.rs` (8.5)
- `src/ops.rs` (9.x operations on the list, reply texts)
- `src/roots.rs` (7.1 roots state machine and URI conversion)
- `src/server.rs` (rmcp server)
- `src/reminder.rs` (10)
- `src/cli.rs` (11: `dispatch`)
- `src/host.rs` (host identity)
- `tests/` (integration tests), `README.md`, `CHANGELOG.md`

Wrappers (new): `claude-code/plugins/repo-tasks-mcp-cc`, `qwen/plugins/repo-tasks-mcp-qwen`, `antigravity/plugins/repo-tasks-mcp-agy`, `opencode/plugins/repo-tasks-mcp-opencode`. Registration edits are listed in Task 10.

Existing patterns the tasks follow (all checked at the commit this plan was written):
- engine crate manifest: `shared/opencode-ctl/Cargo.toml` (`publish = false`, `[package.metadata.dist] dist = false`, `[lib]`, workspace dependencies, `[target.'cfg(unix)'.dependencies] nix`);
- server: `shared/opencode-ctl/src/server.rs` (`#[rmcp::tool_router]` on an `impl`, `#[rmcp::tool(description = ...)]` per method, `Parameters<Args>`, `#[rmcp::tool_handler(router = self.tool_router)]` at line 273, served with `.serve(rmcp::transport::stdio())` at `src/cli.rs:278`);
- wrapper binary: `claude-code/plugins/opencode-ctl-mcp-cc/src/bin/mcp.rs` (calls `dispatch(Host::Claude, env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"), argv0, args)`), manifest `claude-code/plugins/opencode-ctl-mcp-cc/Cargo.toml` (`[package.metadata.release] pre-release-replacements`, `[package.metadata.dist.binaries]`);
- host narrowing: `shared/opencode-ctl/src/host.rs` (`enum Host { Claude, Qwen, Agy, Opencode, Unknown }`, `resolve(constant, host_arg, env_host)`);
- rmcp 3.10.0 hooks the roots work uses: `ServerHandler::on_initialized`, `ServerHandler::on_roots_list_changed` (`rmcp-3.10.0/src/handler/server.rs:642`, `:649`), `Peer::list_roots` (`rmcp-3.10.0/src/service/server.rs:931`, marked `#[deprecated]` there, since the MCP specification deprecates roots; the engine calls it under `#[allow(deprecated)]`, and Task 7 records the risk in the README).

---

### Task 1: Scaffold, errors, host, and location resolution (7.1 table rows, 7.2 to 7.5)

**Files:**
- Create: `shared/repo-tasks/Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/host.rs`, `src/clock.rs`, `src/resolve.rs`, `tests/resolve_oracle.rs`
- Modify: `Cargo.toml` (workspace members already include `shared/*`; check only)

**Interfaces:**
- Produces `error::Error` (`thiserror`) with `Display` giving the one-line texts the spec requires, and `error::Result<T>`.
- Produces `host::Host { Claude, Qwen, Agy, Opencode }` and `host::Host::updated_by(&self) -> &'static str` (`claude-code`, `qwen`, `antigravity`, `opencode`); `host::resolve(constant: Host, host_arg: Option<&str>, env_host: Option<&str>) -> Result<Host>` narrowing to `Opencode` exactly as `shared/opencode-ctl/src/host.rs:resolve` does.
- Produces `clock::Clock` (`fn now_secs(&self) -> i64`), `clock::SystemClock`, `clock::rfc3339(secs: i64) -> String` (UTC, whole seconds, ends in `Z`), `clock::parse_rfc3339(&str) -> Option<i64>`.
- Produces `resolve::Env` (a struct of the values read from the environment: `repo_tasks_dir`, `repo_tasks_list`, `home`, `xdg_data_home`, `local_app_data`, `cwd`) with `Env::from_process()`.
- Produces `resolve::StartDir { path: PathBuf, source: StartSource }`, `enum StartSource { Variable, Argument, Root, Workspace, ProcessDir }`, and `resolve::start_dir(entry: Entry, env: &Env, arg: Option<&Path>, root: Option<&Path>, workspace: Option<&Path>, host: Host) -> Result<StartDir>` implementing the 7.1 table, where `enum Entry { ServerCall, Reminder, Subcommand }`; on `Host::Agy` with `Entry::ServerCall` there is no `ProcessDir` step and the error names `REPO_TASKS_DIR`.
- Produces `resolve::Store { dir: PathBuf, list: String, common_dir: Option<PathBuf>, start: PathBuf }` and `resolve::locate(start: &Path, env: &Env) -> Result<Store>` (7.2 walk, 7.3 folder, 7.4 name check `[A-Za-z0-9_-]{1,64}`, 7.5 key) and `Store::file(&self, kind: FileKind) -> PathBuf`, `enum FileKind { List, Lock, Tmp, Archive }`.

- [ ] **Step 1: Write the failing tests** in `tests/resolve_oracle.rs` and `src/resolve.rs`'s test module. Names and assertions: `oracle_matches_git_for_each_case` builds in a temp directory each case 7.2 lists (main checkout, subdirectory, linked worktree, `.git` file with `gitdir:` and trailing carriage return, `commondir` relative and absolute, bare repository counts as not a repository, `.git` entry pointing at a missing directory is an error) and asserts the engine's `common_dir` equals `git rev-parse --path-format=absolute --git-common-dir` run in the same directory (skip a case with a clear message when `git` is not on `PATH`); `start_dir_table` asserts each row of the 7.1 table including: a blank `REPO_TASKS_DIR` is absent; a relative or non-existent `REPO_TASKS_DIR` is an error naming the variable and its value (the reminder gets "nothing" instead); `Entry::Reminder` ranks variable, then workspace, then process directory except on `Host::Agy`; `Entry::ServerCall` on `Host::Agy` with neither variable nor root is an error naming `REPO_TASKS_DIR`; `store_key_vector` asserts the 16-hex key of a fixed path against a value computed once with `sha256sum` and written into the test; `list_name_rules` asserts 64 characters pass and 65, an empty string and `a b` fail without being altered.
- [ ] **Step 2: Run to see them fail.** Run: `cargo nextest run -p repo-tasks`. Expected: build failure (crate missing).
- [ ] **Step 3: Create the crate** from the manifest pattern in `shared/opencode-ctl/Cargo.toml` (name `repo-tasks`, `publish = false`, `[package.metadata.dist] dist = false`, `[lib] name = "repo_tasks"`), then implement the signatures above. Walk and canonicalisation: `std::fs::canonicalize`, remove a leading `\\?\` on Windows before showing or hashing. No `git` process anywhere in non-test code.
- [ ] **Step 4: Run.** Run: `cargo nextest run -p repo-tasks` and `cargo clippy -p repo-tasks --all-targets -- -D warnings`. Expected: all pass.
- [ ] **Step 5: Commit.** `git add shared/repo-tasks && git commit -m "feat(repo-tasks): scaffold the engine and resolve the list location"`

### Task 2: The list file (8.1, 8.4)

**Files:**
- Create: `shared/repo-tasks/src/model.rs`, `shared/repo-tasks/tests/model.rs`

**Interfaces:**
- Consumes: `clock::{rfc3339, parse_rfc3339}`, `error::Error`.
- Produces `model::{List, Task, Status}` with serde (de)serialisation; `List { format: u32, next_id: u64, tasks: Vec<Task>, extra: serde_json::Map<String, Value> }` and `Task { id, subject, description: Option<String>, status, owner: Option<String>, blocked_by: Vec<u64>, created_at: String, updated_at: String, updated_by: String, extra: Map }`, where `extra` keeps members this engine does not know (8.4) and `Option` fields are omitted, never null.
- Produces `List::empty() -> List`, `List::parse(bytes: &[u8]) -> Result<List>` (fails for invalid JSON, a rule of 8.1 broken, or `format` above 1 with an error naming both numbers and saying to update the plugin), `List::validate(&self) -> Result<()>`, `List::to_bytes(&self) -> Vec<u8>` (compact or indented is the builder's choice; one line break at the end), `model::validate_subject(&str) -> Result<String>` (trims), `model::validate_owner`, `model::validate_description`.

- [ ] **Step 1: Write the failing tests** (`tests/model.rs`): `round_trip_keeps_unknown_members` (a top-level and a per-task unknown member survive parse then to_bytes); `omitted_not_null` (a task without owner serialises with no `owner` key); `rejects_each_rule` table-driven, one row per rule of 8.1 (format not 1 and not newer, ids not ascending, duplicate id, `next_id` not above every id, blank subject, subject 201 characters, 200 accepted, description 8,001 rejected, owner 65 rejected, control character in subject and owner rejected, tab and line feed in description accepted, other control character in description rejected, unsorted or duplicate or self-referencing or unknown `blocked_by`, a timestamp ending in `+00:00`, a number as `created_at`); `newer_format_names_both_numbers` asserts the message contains `2` and `1`.
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p repo-tasks --test model`. Expected: compile error.
- [ ] **Step 3: Implement** the signatures; length checks count `chars()` (Unicode scalar values).
- [ ] **Step 4: Run.** Run: `cargo nextest run -p repo-tasks`. Expected: pass.
- [ ] **Step 5: Commit.** `git commit -am "feat(repo-tasks): the list file and its rules"` (after `git add`).

### Task 3: Writing and reading the list (8.2, 8.3)

**Files:**
- Create: `shared/repo-tasks/src/store.rs`, `shared/repo-tasks/tests/store.rs`, `shared/repo-tasks/tests/bin/hold_lock.rs` (a helper binary that takes the lock on a given path and sleeps, for the cross-process test)

**Interfaces:**
- Consumes: `resolve::{Store, FileKind}`, `model::List`, `clock::Clock`.
- Produces `store::Tuning { lock_deadline: Duration, rename_retry: Duration, read_retry: Duration, poll: Duration }` with `Tuning::default()` (5 s, 5 s, 1 s, 10 ms) and a way for tests to shorten it.
- Produces `store::read_list(store: &Store, tuning: &Tuning) -> Result<List>` (missing file or folder is `List::empty()` and creates nothing; a file that is not valid or breaks a rule is an error naming the file, never repaired; reader opens, reads all, closes; on Windows a refused open or read of kind `PermissionDenied` or sharing violation is retried for `read_retry`).
- Produces `store::mutate<T>(store: &Store, tuning: &Tuning, f: impl FnOnce(&mut List) -> Result<T>) -> Result<T>`: the five steps of 8.2 (creates the folder with mode 0700 and files 0600 on Unix on first write, takes the lock with `File::try_lock` repeated until `lock_deadline`, reads, applies `f`, and only if `f` returned Ok and the list changed writes `N.json.tmp`, flushes, renames, flushes the folder on Unix; removes a leftover temp file under the lock; a failed write leaves the old list and removes the temp file; outside git the first write also creates `directory.txt` in the store folder holding canonical D, 7.5; on Windows retries a refused rename as 8.2 step 4 says).

- [ ] **Step 1: Write the failing tests:** `missing_list_reads_empty_and_creates_nothing` (no folder afterwards); `first_write_creates_folder_with_modes` (Unix only: 0700 and 0600); `f_error_leaves_file_untouched` (bytes identical); `no_change_writes_nothing` (mtime and bytes unchanged, no temp file); `leftover_tmp_is_removed_under_lock`; `malformed_list_is_never_overwritten` (an error naming the file; file bytes identical after a `mutate` attempt); `two_threads_allocate_distinct_ids` (50 threads each create a task through `mutate`; ids are exactly 1..=50 with no duplicate and `next_id` is 51); `second_process_blocks_then_fails_at_deadline` (spawn `hold_lock`, then `mutate` with a 300 ms deadline returns an error mentioning the lock and the list is unchanged); `lock_released_when_holder_dies` (kill the helper, then `mutate` succeeds); `first_write_outside_git_creates_directory_txt` (the file holds canonical D and nothing is created by a read); `store_unwritable_names_path` (7.6: a folder that cannot be created, Unix only, makes `mutate` fail with an error naming the path and the operating-system error while `read_list` still answers); `windows_reader_and_writer_never_fail` (`#[cfg(windows)]`: one writer thread looping `mutate` and four reader threads looping `read_list` for 3 seconds; no call returns an error; this is the CI test 8.3 names, and it prints the error kind it saw on any retry).
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p repo-tasks --test store`. Expected: compile error.
- [ ] **Step 3: Implement** to the interfaces. The retry kinds on Windows are `io::ErrorKind::PermissionDenied` and raw OS error 32 (`ERROR_SHARING_VIOLATION`); every other error fails at once.
- [ ] **Step 4: Run** on Linux: `cargo nextest run -p repo-tasks`; the Windows test runs only in CI and is checked there (Task 11).
- [ ] **Step 5: Commit.** `git commit -m "feat(repo-tasks): locked atomic writes and reads"` (after `git add`).

### Task 4: The archive (8.5)

**Files:**
- Create: `shared/repo-tasks/src/archive.rs`, `shared/repo-tasks/tests/archive.rs`

**Interfaces:**
- Consumes: `store::{mutate, read_list, Tuning}`, `model::{List, Task}`, `clock::Clock`.
- Produces `archive::Entry { task: Task, how: Left }` with `enum Left { Archived(String), Deleted(String) }` (the RFC 3339 time), `archive::move_to_archive(store: &Store, tuning: &Tuning, list: &mut List, ids: &[u64], how: LeftKind, now: &str) -> Result<()>` to be called inside a `mutate` closure: appends the lines (beginning with a line feed when the file is not empty and its last byte is not one), flushes, then removes the ids from `list.tasks` and from every remaining `blocked_by`; `next_id` unchanged.
- Produces `archive::read_archive(store: &Store, list: &List) -> Result<Vec<Entry>>` applying 8.5's reader rules (ignore a line whose id is in `list`; last line wins for any other id; ignore every line that is not a complete JSON object; never an error for a damaged line; no archive file is an empty vec), returned in id order.
- Produces `archive::find(store: &Store, list: &List, id: u64) -> Result<Option<Entry>>`.
- Produces `archive::startup_candidate(list: &List, now_secs: i64) -> bool` (at least one task, all completed, every `updated_at` at least 43,200 seconds before `now_secs`) and `archive::run_startup(store: &Store, tuning: &Tuning, clock: &dyn Clock) -> Result<usize>`: reads without the lock, returns `Ok(0)` without touching disk unless `startup_candidate`; then one non-blocking lock attempt (a `Tuning` with `lock_deadline` zero), re-read under the lock, archives all with `archived_at` if the condition still holds; any failure returns an error the caller prints to standard error.
- Produces `archive::archive_completed(store, tuning, clock) -> Result<usize>` for the `archive` subcommand (every completed task regardless of age).

- [ ] **Step 1: Write the failing tests:** `append_after_torn_line_starts_new_line` (write a half line by hand, move a task, assert the new line parses and the half line is skipped); `reader_rules` (list wins; last line wins; damaged middle line skipped; no error); `crash_between_steps_leaves_task_in_both` (simulate: append, do not rewrite; `read_archive` shows nothing for that id; a later complete move makes one line count); `startup_condition_table` (empty list false; one pending false; all completed but one updated 11 h 59 min ago false; all older than 12 h true; a timestamp in the future false); `startup_reads_before_locking` (a repository with no list: no folder and no lock file created); `startup_one_attempt_when_locked` (hold the lock from another thread, `run_startup` returns without waiting and the list is unchanged); `deleted_ids_leave_other_blockers` (after moving task 2, no remaining task lists 2 in `blocked_by`).
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p repo-tasks --test archive`. Expected: compile error.
- [ ] **Step 3: Implement.** Append mode with `OpenOptions::append`, `sync_all` before returning.
- [ ] **Step 4: Run.** Run: `cargo nextest run -p repo-tasks`. Expected: pass.
- [ ] **Step 5: Commit.** `git commit -m "feat(repo-tasks): the archive and start-up archiving"` (after `git add`).

### Task 5: The operations and their replies (9)

**Files:**
- Create: `shared/repo-tasks/src/ops.rs`, `shared/repo-tasks/tests/ops.rs`

**Interfaces:**
- Consumes: Tasks 2 to 4.
- Produces argument structs deriving `serde::Deserialize` and `schemars::JsonSchema`: `CreateArgs { tasks: Vec<NewTask> }`, `NewTask { subject: String, description: Option<String>, owner: Option<String>, blocked_by: Option<Vec<i64>>, blocked_by_earlier: Option<Vec<i64>> }`, `GetArgs { id: i64 }`, `UpdateArgs { id: i64, status: Option<Status>, subject, description, owner: Option<String>, add_blocked_by: Option<Vec<i64>>, remove_blocked_by: Option<Vec<i64>>, delete: Option<bool> }`. Integer ids are `i64` so that a string or a negative number is a validation error rather than a parse panic; a JSON string id is rejected (9.1).
- Produces `ops::create(store, tuning, host: Host, clock, args) -> Result<String>`, `ops::get(store, tuning, args) -> Result<String>`, `ops::update(store, tuning, host, clock, args) -> Result<String>`, `ops::list(store, tuning, env_note: &ListHeader) -> Result<String>`, each returning the reply text exactly as 9.2 to 9.5 give, and `ListHeader` giving line 1 of 9.5 (inside git: `repo-tasks: <C> (list <N>, from <D>)`; outside: `repo-tasks: <D> (list <N>, not a git repository)`).

- [ ] **Step 1: Write the failing tests** against golden strings copied from 9.2 to 9.5 of the spec: `create_five_with_earlier` (the section 18 scenario: five tasks, later ones using `blocked_by_earlier`; reply is five `created #<id> <subject>` lines); `create_limits` (0 and 51 entries fail, 50 pass; `blocked_by_earlier` value not lower than its own position fails); `create_blocker_left_the_list_prints_satisfied_line` (golden: `#<id>: blocker #<blocker id> has left the list and counts as satisfied`, ordered after all `created` lines, by task id then blocker id); `get_template` (exact block of 9.3 with owner/blocked_by/blocks lines omitted when empty; description separated by a blank line); `get_from_archive` (`archived <t>` first line, no `blocks` line); `get_errors` (never issued; left the list and archive holds no record); `update_no_op_writes_nothing` (file bytes and `updated_at` unchanged, reply `updated #<id>`); `update_sets_updated_by_and_time`; `update_remove_blocked_by_rules` (id below 1 or at or above `next_id` fails; absent id ignored; same id in add and remove fails); `delete_rules` (open task moves to archive with `deleted_at`, other tasks' `blocked_by` lose it; completed task fails; delete combined with another member fails; archive write failure keeps the task); `cycle_and_self_block_rejected`; `list_format` (golden of 9.5 including `[owner x]`, `[blocked by #a, #b]` listing only open blockers, `[<updated_by> <updated_at>]` on in_progress, empty list prints `no tasks`, archive present adds the `archived tasks exist` line); `ids_never_reused` (create 3, delete 3, create again gives 4).
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p repo-tasks --test ops`. Expected: compile error.
- [ ] **Step 3: Implement.** One `mutate` call per create/update; the whole call applies or none (8.2 step 3). Errors are single-line texts per 9.1.
- [ ] **Step 4: Run.** Run: `cargo nextest run -p repo-tasks`. Expected: pass.
- [ ] **Step 5: Commit.** `git commit -m "feat(repo-tasks): the four operations and their replies"` (after `git add`).

### Task 6: The roots state machine and URI reading (7.1)

**Files:**
- Create: `shared/repo-tasks/src/roots.rs`, `shared/repo-tasks/tests/roots.rs`

**Interfaces:**
- Produces `roots::uri_to_path(uri: &str, windows: bool) -> Option<PathBuf>` per 7.1 (RFC 8089: authority empty or `localhost`, percent-decoded as UTF-8, other scheme or invalid UTF-8 is `None`, Windows drive letters of either case).
- Produces `roots::first_usable(roots: &[String], windows: bool, is_dir: &dyn Fn(&Path) -> bool) -> Option<PathBuf>`.
- Produces the state machine `roots::RootState` (pure; time passed in) with: `RootState::new(declared_roots: bool, window: Duration)`; `on_initialized(&mut self, now: Instant) -> Action` (`Action::SendRequest` when roots were declared); `on_reply(&mut self, roots: Option<Vec<String>>, now) -> Action`; `on_list_changed(&mut self, now) -> Action`; `on_tick(&mut self, now) -> Action` (settles by timeout and may return `SendRequest` for a remembered notification); `fn call_started(&mut self, now) -> CallGate` (`CallGate::Refuse` before initialized, `Wait(Duration)` while unsettled, `Go`); `fn current_root(&self) -> Option<&Path>`; `fn first_settlement(&self) -> Option<Settlement>` with `enum Settlement { NoRoots, Answered, TimedOut }` (what 8.5 consults).

- [ ] **Step 1: Write the failing tests**, one per rule of 7.1, each in both directions (as the spec's test list demands): `late_first_reply_accepted_before_any_call_ignored_after`; `refresh_reply_always_used`; `notification_during_request_gives_exactly_one_followup_after_reply_and_after_timeout`; `never_two_requests_in_flight`; `call_before_initialized_refused`; `no_roots_declared_never_sends`; `timeout_alone_is_not_answered_for_archiving` (`first_settlement` is `TimedOut`); `failed_refresh_keeps_held_root`; `uri_table` (empty authority, `localhost`, other authority rejected, `%20`, invalid UTF-8 `%FF`, `http:`, `/c:/x` and `/C:/x` on Windows give `c:\x` and `C:\x`, a non-existent directory is skipped for the next root). All use an injected `Instant`, never sleeping.
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p repo-tasks --test roots`. Expected: compile error.
- [ ] **Step 3: Implement** the pure machine; the 2-second window is `RootState::new`'s parameter, defaulted by `ROOTS_WINDOW: Duration = Duration::from_secs(2)`.
- [ ] **Step 4: Run.** Run: `cargo nextest run -p repo-tasks`. Expected: pass.
- [ ] **Step 5: Commit.** `git commit -m "feat(repo-tasks): the roots state machine"` (after `git add`).

### Task 7: The MCP server (9, 7.1, 8.5 start)

**Files:**
- Create: `shared/repo-tasks/src/server.rs`, `shared/repo-tasks/tests/bin/serve.rs` (a three-line binary that calls `cli::dispatch(Host::Claude, ...)`, declared in `shared/repo-tasks/Cargo.toml` as `[[bin]] name = "mock-repo-tasks"` the way `shared/opencode-ctl/Cargo.toml` declares `mock-opencode-ctl`), `shared/repo-tasks/tests/server_e2e.rs` (the test client is `rmcp`'s client over a child process, as `claude-code/plugins/opencode-ctl-mcp-cc/tests/e2e.rs` does: read that file first and mirror its helper for starting the child).

**Interfaces:**
- Consumes: Tasks 1 to 6.
- Produces `server::RepoTasksServer` with `#[rmcp::tool_router]` methods `task_create`, `task_get`, `task_update`, `task_list` (descriptions each contain the sentence of 12: use these tools for work that must outlive the session or be seen by another agent), `ServerHandler` implementing `get_info` (capabilities: tools), `on_initialized` (starts the roots request when the client declared `roots`, using `context.peer.list_roots()` under `#[allow(deprecated)]`, then runs start-up archiving once per 8.5), `on_roots_list_changed`, and a call guard that refuses a tool call before `initialized` with a JSON-RPC error code -32600 (`rmcp::ErrorData::invalid_request`) and no list touched.
- Produces `server::serve(host: Host, version: &str, env: Env) -> Result<()>` using `.serve(rmcp::transport::stdio())`.
- Produces `server::tool_descriptions() -> Vec<(&'static str, &'static str)>` (as `shared/opencode-ctl/src/server.rs` does) so the footprint and a test can read them.

- [ ] **Step 1: Write the failing tests** (`tests/server_e2e.rs`, with a throwaway git repository and the test child process): `lists_four_tools_with_the_sentence`; `five_step_scenario` (section 18's first paragraph through the client: create five, update three, then a second server process reads them); `roots_used_when_no_variable` (client replies one root; a call resolves to that repository though the child's working directory is elsewhere); `antigravity_host_without_root_or_variable_fails_naming_variable` (child started with host Agy and a client that declares no roots); `call_before_initialized_is_json_rpc_error` (write raw JSON-RPC to the child's stdin before `notifications/initialized`; assert an `error` response with code -32600 and no `repo-tasks` folder created); `client_that_never_answers_roots_does_not_hang` (tool call returns within the shortened window plus a margin; start-up archiving did not run); `startup_archives_old_completed_list` (a list file with all tasks completed 13 hours ago; after `initialize`/`initialized` and the roots reply, `task_list` shows `no tasks` and the archive has the lines); `startup_skipped_on_timeout_only`.
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p repo-tasks --test server_e2e`. Expected: compile error or missing binary.
- [ ] **Step 3: Implement** to the interfaces; the roots window for the child comes from an environment variable read only under `cfg(test)`/an `#[doc(hidden)]` constructor parameter that no host configuration can set, as 7.1 requires.
- [ ] **Step 4: Run.** Run: `cargo nextest run -p repo-tasks && cargo clippy -p repo-tasks --all-targets --all-features -- -D warnings`. Expected: pass.
- [ ] **Step 5: Commit.** `git commit -m "feat(repo-tasks): the MCP server with roots"` (after `git add`).

### Task 8: The reminder and the command line (10, 11)

**Files:**
- Create: `shared/repo-tasks/src/reminder.rs`, `shared/repo-tasks/src/cli.rs`, `shared/repo-tasks/tests/cli.rs`, `shared/repo-tasks/tests/acceptance.rs`

**Interfaces:**
- Produces `reminder::line(store_result: Result<List, ReadFailure>, ...) -> Option<String>` returning exactly the one line of 10.1 (no subject, description or owner), `None` when n is 0, there is no list, or anything else fails, and the notice line `repo-tasks: this repository's task list cannot be read. Call the task_list tool to see why.` when the list file exists and cannot be used, cannot be opened or read, or 7.2 stopped with an error.
- Produces `reminder::wrap(host_event: HostEvent, line: &str, workspace: Option<&str>) -> String` giving each host's output form of 10.2: Claude Code `hookSpecificOutput.additionalContext` as `claude-code/plugins/re-ghidra-mcp-cc/src/bin/hook.rs:78-80`, Qwen `systemMessage` plus `hookSpecificOutput.additionalContext` as `qwen/plugins/re-ghidra-mcp-qwen/src/bin/hook.rs:77-80`, Antigravity `injectSteps` as `antigravity/plugins/re-ghidra-mcp-agy/src/bin/hook.rs:78` (read these three files before writing the forms: the plan states where, not what), OpenCode the bare line. Antigravity reads the workspace as the first entry of `workspacePaths`, else `cwd` (`antigravity/plugins/re-ghidra-mcp-agy/src/bin/hook.rs:89-96`); Claude Code reads `cwd` then `CLAUDE_PROJECT_DIR`.
- Produces `cli::dispatch(host: Host, product: &str, version: &str, argv0: Option<&str>, args: Vec<String>) -> i32` with subcommands `serve` (OpenCode adds `--host opencode`), `reminder --event <host> [--workspace <path>]` (no `--dir`; the event JSON arrives on standard input for the three hooks hosts), `list [--archived] [--dir <path>]`, `archive [--dir <path>]` (`archived <n> tasks`, `archived 1 task`, or `no completed tasks`), `--version`; every subcommand prints per 11 and `updated_by` is `cli` for subcommand writes. `list --archived` follows 11 exactly.

- [ ] **Step 1: Write the failing tests** (`tests/cli.rs`, running `cli::dispatch` in-process against temp repositories, plus one spawn per wrapper binary in Task 9): `reminder_line_exact` (three open one in progress gives the 10.1 sentence; no task text appears); `reminder_silent_cases` (n is 0; no list; unresolvable directory); `reminder_notice_for_unusable_list` (malformed JSON, newer format, unreadable file on Unix via mode 000, unfollowable `.git`); `reminder_antigravity_never_uses_process_dir` (no variable, no workspace in the event: nothing); `reminder_variable_outranks_event_path`; `list_matches_task_list_reply` (identical text to `ops::list`); `list_archived_forms` (line 1, marks as when the task left, `[archived <t>]` / `[deleted <t>]`, `no archived tasks` when none); `archive_subcommand_messages`; `version_line`; `reminder_has_no_dir_argument` (passing `--dir` is an error); `acceptance_section_18` in `tests/acceptance.rs` (the eleven sentences of section 18 with two server processes started in a main checkout and a linked worktree, the `reminder` subcommand, a clock moved 13 hours forward by writing old timestamps, and `list --archived` at the end).
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p repo-tasks --test cli`. Expected: compile error.
- [ ] **Step 3: Implement.** Read the three existing `hook.rs` files first and keep each host's output keys byte-identical to what those files print.
- [ ] **Step 4: Run.** Run: `cargo nextest run -p repo-tasks`. Expected: pass.
- [ ] **Step 5: Commit.** `git commit -m "feat(repo-tasks): reminder and command line"` (after `git add`).

### Task 9: The four wrappers (14, 10.2)

**Files:**
- Create for each of the three Rust wrappers (`claude-code/plugins/repo-tasks-mcp-cc`, `qwen/plugins/repo-tasks-mcp-qwen`, `antigravity/plugins/repo-tasks-mcp-agy`): `Cargo.toml`, `src/bin/mcp.rs` (name `repo-tasks-cc-mcp`, `repo-tasks-qwen-mcp`, `repo-tasks-agy-mcp`; the qwen and agy wrappers' opencode-ctl twins keep the same file name under `src/`: read `qwen/plugins/opencode-ctl-mcp-qwen/src/main.rs` and `antigravity/plugins/opencode-ctl-mcp-agy/src/bin/` first and mirror their layout), the host manifest, the host's MCP config, the host's hooks file, `README.md`, `CHANGELOG.md`, `.gitignore` (its single pattern is `/bin/` with the leading slash, as `claude-code/plugins/opencode-ctl-mcp-cc/.gitignore` explains: a bare `bin/` would ignore `src/bin/`), `tests/e2e.rs`.
- Host manifests (mirror the opencode-ctl twin of each host, reading the twin first): Claude Code `.claude-plugin/plugin.json` and `.mcp.json` plus a `SessionStart` hook for the sources `startup`, `resume`, `clear` and `compact` running `repo-tasks-cc-mcp reminder --event claude-code`; Qwen `qwen-extension.json` with `mcpServers` and `SessionStart` hooks for `startup`, `resume`, `clear` (as `qwen/plugins/re-ghidra-mcp-qwen/qwen-extension.json:17-43` declares) running `reminder --event qwen`; Antigravity `plugin.json`, `mcp_config.json`, `hooks.json` with a `PreInvocation` hook acting only when `invocationNum` is at most 1, running `reminder --event antigravity`.
- Create the OpenCode wrapper: `opencode/plugins/repo-tasks-mcp-opencode/{package.json,plugin.ts,INSTALL.md,README.md,CHANGELOG.md,tests/hooks.test.ts,bin/}` (mirror `opencode/plugins/opencode-ctl-mcp-opencode/`; the MCP entry shape is `INSTALL.md:125-140` there; the plugin uses the `Plugin.define` shape of `opencode/plugins/re-ghidra-mcp-opencode/plugin.ts:189-215`).

**Interfaces:**
- Each Rust `mcp.rs` is the 12-line pattern of `claude-code/plugins/opencode-ctl-mcp-cc/src/bin/mcp.rs` with `Host::Claude | Host::Qwen | Host::Agy` and no other logic.
- OpenCode `plugin.ts` exports `Plugin.define({ id: "repo-tasks-mcp-opencode", setup })`; on the first `context` event per `sessionID` and on `compaction` it runs `<binary> reminder --event opencode --workspace <ctx.location.directory>` with `spawnSync` (as `opencode/plugins/rtk-mcp-opencode/plugin.ts` runs its binary), pushes `{ type: "text", text: <line> }` onto `event.system` only when `event.system` is an array (as `re-ghidra-mcp-opencode/plugin.ts:203-204`), pushes nothing when nothing comes back, never throws, and contains no code that parses the list. The binary's path is configured with an environment override as `opencode/plugins/rtk-mcp-opencode/plugin.ts:14` and `:86-87` do.

- [ ] **Step 1: Write the failing tests:** per Rust wrapper, `tests/e2e.rs` copied in shape from `claude-code/plugins/opencode-ctl-mcp-cc/tests/e2e.rs` starting the built binary (`env!("CARGO_BIN_EXE_...")`) and asserting `tools/list` has the four tools and a `task_create` then `task_list` round trip in a temporary git repository, plus `reminder_prints_hook_json` (the exact wrapped form for that host); for OpenCode, `tests/hooks.test.ts` with `bun test`: `pushes_once_per_session`, `pushes_object_form`, `silent_when_no_line`, `silent_when_system_not_array`, `compaction_pushes`, `never_throws_when_binary_missing`, `contains_no_list_parsing` (the source has no `JSON.parse` and no `.json` file read).
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p repo-tasks-mcp-cc` and `bun test` in the OpenCode folder. Expected: build errors (crates missing).
- [ ] **Step 3: Create the wrappers** from the twins, with versions `0.1.0` (not the workspace version, as the twins use their own: `claude-code/plugins/opencode-ctl-mcp-cc/Cargo.toml` shows `version = "0.2.0"` with `[package.metadata.release] pre-release-replacements` keeping the manifest in step; copy that block with the right manifest file name for each host) and `[package.metadata.dist.binaries]` listing the shipped binary only.
- [ ] **Step 4: Run.** Run: `cargo nextest run --workspace` and `bun test` in `opencode/plugins/repo-tasks-mcp-opencode`. Expected: pass. Then, before the other wrappers are finished, build the Claude Code wrapper (`just build-repo-tasks-mcp-cc` from Task 10 Step 3, written first for this wrapper only) and measure it as `just footprint-regen` does (`Justfile:133`); if `tiers.resident.bytes` is above the 3,000 design target, say so to the owner before going on, since 3,690 blocks all four wrappers.
- [ ] **Step 5: Commit.** One commit per wrapper: `git commit -m "feat(repo-tasks-mcp-cc): the Claude Code wrapper"` and so on.

### Task 10: Registration, footprint and the stop-line check (13, 17.3)

**Files (each edit mirrors the `opencode-ctl-mcp-*` entry found by `grep -n "opencode-ctl-mcp-cc" <file>`; read the entry before copying):**
- Modify: `.release-please-manifest.json` (lines 4, 7, 13, 14 show the four existing entries), `release-please-config.json` (`:33` for Claude Code with `extra-files` `.claude-plugin/plugin.json`; `:61` Qwen with `qwen-extension.json`; `:103` OpenCode; the Antigravity entry), `agents.json` (the four `plugins` arrays), `.claude-plugin/marketplace.json` (entry at line 95), the Qwen and Antigravity marketplace files that the Justfile recipes `qwen-marketplace` and `agy-marketplace` validate, `scripts/check-package-versions.sh` (the pair list at lines 40 to 54 and the "must not inherit the workspace version" list at line 62), `scripts/lib/registry.sh` (the expected-order assertions at lines 57 to 61), `Justfile` (a `build-repo-tasks-mcp-<x>` recipe copied from `Justfile:419`; the `check` line at 360 stays), `.github/workflows/ci.yml` (add the build lines beside `ci.yml:259` to `:265`) and `.github/workflows/footprint-freshness.yml` (beside lines 38 to 44), `README.md` (the plugin list near lines 136 to 147), `QWEN.md` (the table at lines 26 to 29), `ROADMAP.md` (the entry 12 requires: the overlap of `todov2-opencode` with OpenCode 2.3.0's built-in `todowrite`).
- Create: `docs/footprints/repo-tasks-mcp-<cc|agy|opencode>.json` (generated, not written by hand: `cargo run -q -p plugin-footprint --bin plugin-footprint -- measure ...` as `just footprint-regen` does at `Justfile:133`), entries in `docs/footprints/budgets.json` (`deltaBytes` 500, `headroomBytes` 2000, `residentBytes` = measured plus 2000), `tools/plugin-footprint/tests/repo_tasks_stop_line.rs`.

**Interfaces:**
- The stop-line test reads each `docs/footprints/repo-tasks-mcp-*.json` and fails when `tiers.resident.bytes` is 3,690 or more (13); it must itself be shown to fail by pointing it at a fixture document with 3,690.

- [ ] **Step 1: Write the failing test** `stop_line_fails_at_3690_and_passes_below` using two small fixture documents under the test folder, plus `all_three_footprints_exist` (the test first asserts that `repo-tasks-mcp-cc.json`, `repo-tasks-mcp-agy.json` and `repo-tasks-mcp-opencode.json` exist in `docs/footprints/`, so an ungenerated document cannot pass the stop line by being absent; Qwen has no footprint document today, as `docs/footprints/` shows), and `registry_lists_repo_tasks` is covered by the existing `scripts/lib/registry.sh` self-checks edited in Step 3.
- [ ] **Step 2: Run to fail.** Run: `cargo nextest run -p plugin-footprint --test repo_tasks_stop_line`. Expected: FAIL, because the footprint documents do not exist yet (`all_three_footprints_exist`).
- [ ] **Step 3: Apply the registration edits** listed above; build the three binaries with the new `just build-repo-tasks-mcp-*` recipes; regenerate the footprints; if any wrapper measures 3,690 bytes or more, STOP and report to the owner (13) before recording a budget.
- [ ] **Step 4: Run the gate.** Run: `just check`. Expected: exit 0 (every recipe in the `check` line at `Justfile:360`, including `versions`, `wiring`, the three marketplace validators, `footprint` and `footprint-budgets`).
- [ ] **Step 5: Commit.** `git commit -m "chore(repo-tasks): register the plugins, footprints and the stop line"` (after `git add`).

### Task 11: CI on Windows and macOS, canonicalisation, docs (7.5, 8.3, 15.6)

**Files:**
- Create: `shared/repo-tasks/tests/windows_canonical.rs`, `shared/repo-tasks/README.md`, `shared/repo-tasks/CHANGELOG.md`
- Modify: `docs/superpowers/specs/2026-11-18-repo-tasks-design.md` (section 15.6 and 15.7: record what CI measured)

**Interfaces:**
- `windows_canonical.rs` (`#[cfg(windows)]`): a directory reached as `c:\x` and as `C:\X` gives one store key (settles 15.6's canonicalisation unknown); it prints the two canonical forms.

- [ ] **Step 1: Write the test** above and confirm it compiles on Linux as an empty module (`cargo nextest run -p repo-tasks` passes).
- [ ] **Step 2: Push nothing.** Ask the owner to let CI run the branch (a push is the owner's decision); until then the Windows and macOS results are `not measured` and the README says so.
- [ ] **Step 3: After CI has run,** read the Windows job output for `windows_reader_and_writer_never_fail` and `windows_canonical`, and record in 15.6 and 15.7 what they printed (the error kind seen on a refused rename, the two canonical forms); if either fails, the failing rule in 8.2, 8.3 or 7.5 is revised with the owner before anything else lands.
- [ ] **Step 4: Write `README.md`** (what it is, the four tools, environment variables, the roots deprecation risk in rmcp's `list_roots`, the list's location, how to read the archive) and a first `CHANGELOG.md` entry.
- [ ] **Step 5: Commit.** `git commit -m "docs(repo-tasks): readme, changelog and the CI measurements"` (after `git add`).

---

## Self-review

**Spec coverage.** 7.1 table and roots: Tasks 1, 6, 7. 7.2 to 7.6: Tasks 1, 3 (modes, unwritable store: Task 3, test `store_unwritable_names_path`). 8.1 to 8.4: Tasks 2, 3. 8.5: Tasks 4, 7 (start). 9.1 to 9.6: Task 5. 10: Task 8 (+ Task 9 wrappers). 11: Task 8. 12 (ROADMAP entry): Task 10. 13 (stop line): Task 10. 14 (names): Tasks 1, 9, 10. 16: done. 17.3 calls 1 to 26: each is implemented by the task that owns its section; call 12 (a person's `updated_by` is `cli`) is Task 8. 18 (done looks like): Task 7's `five_step_scenario` and `startup_archives_old_completed_list`, Task 8's `list_archived_forms`; a final acceptance test `acceptance_section_18` in `shared/repo-tasks/tests/acceptance.rs` walks all eleven sentences with two server processes and the `reminder` subcommand and is in Task 8 Step 1.

**Gaps closed here (the exhaustiveness audit).** (1) `store_unwritable_names_path` (7.6) is in Task 3's test list. (2) The acceptance walk is named. (3) `list_roots` is deprecated in rmcp 3.10.0 (`service/server.rs:931`); the plan keeps the owner's ruling, uses `#[allow(deprecated)]`, and Task 11 puts the risk in the README. (4) Qwen's hook format and Antigravity's `PreInvocation` details are read from the existing `re-ghidra-mcp-*` files by the implementer in Tasks 8 and 9 (the plan names the file and lines, not a copy of their content).

**Left to the builder (spec 17.1, unchanged):** the JSON writing style, crate choices for git discovery (the engine reads `.git` itself, so none is expected), error wording beyond what sections 7 to 10 fix, how OpenCode's `plugin.ts` learns the binary's path (Task 9 gives the two precedents).

**Left open on purpose:** exact RFC 3339 helper tests (Task 1 uses known epoch values: `0` is `1970-01-01T00:00:00Z`, `1_700_000_000` is `2023-11-14T22:13:20Z`); Windows and macOS behaviour stays `not measured` until Task 11 Step 3.

**Proportion.** The plan is a fraction of the spec's length and carries signatures and test names rather than bodies.

**Panel round 1 on this plan** (the peer returned FINDINGS-3, checked against the spec and the files): folded `directory.txt` (7.5, Task 3), the stop-line test's existence check (Task 10), and the `/bin/` ignore pattern (Task 9). Its citation check found nothing wrong. Its Q1 suggestion, to measure the footprint early, is adopted in Task 9 Step 4: the Claude Code wrapper is measured as soon as it builds, and an early report is due above 3,000 bytes.
