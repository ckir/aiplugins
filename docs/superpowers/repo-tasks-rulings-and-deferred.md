# repo-tasks: rulings made and deferred minors

Written at merge of `feat/repo-tasks` (engine `shared/repo-tasks` and four wrappers). It is the one place the
decisions taken without asking, and the findings not acted on, are recorded after the git-ignored SDD workspace
is gone. Spec: `docs/superpowers/specs/2026-11-18-repo-tasks-design.md`. Reviews: `docs/agy-capstone-ledger.md`
and `docs/agy-test-audit-ledger.md`. CI: run 44426553379, green on Linux, Windows and macOS.

## Rulings made

Each row: what was decided, why, and what it costs if wrong. "(derived)" means the ledger gave no cost and it was
worked out afterwards.

| # | Ruling | Why | Cost if wrong |
|---|---|---|---|
| 1 | `tests/bin/serve.rs` calls `server::serve_with(host, version, env, ServeOptions{roots_window})` directly; `serve` is `serve_with` with the defaults. A test-only host and window are read from environment variables inside that test binary only. | Avoids a forward dependency on the CLI task. | One extra file edit. |
| 2 | The wrapper task adds only the `build-repo-tasks-mcp-cc` Justfile recipe; the registration task adds the others. | The footprint had to be measurable first. | Duplicate recipe name, caught by `just --list`. |
| 3 | The roots API changed on purpose: `Action::SendRequest(u64 seq)`, `on_reply(seq, roots, now)`, `call_started` returning `(CallGate, Action)`, `next_deadline()` returning `Option<Instant>`. | Spec 7.1 (one more request when a request settles; a refresh reply is always used) cannot hold otherwise. | The server had to use these shapes; it does. |
| 4 | The task `updated_by` value `cli` is never written. | No subcommand edits a task; archiving moves tasks as they stood (8.5). | None today. |
| 5 | `opencode.jsonc` follows the twin plugin's `./bin/<bin>` form, not the plan's `<DEST>` placeholder. | The probe could not launch the placeholder. | (derived) The OpenCode footprint would stop being measurable. |
| 6 | The wiring and smoke checkers take the first word of a hook command. | They treated the whole command as a path, so hooks with a subcommand failed. | (derived) A checker could accept a wrong first word. |
| 7 | One fix wave after the final review: A `deny_unknown_fields` and protocol capped at `V_2025_11_25`; B retry observer replaces `eprintln!`, CI notes upload, ASCII-case and `assert_ne` in the Windows canonical test, best-effort folder flush after rename; C Antigravity reminder prints `{}` when silent, Qwen hook in single-string form; D the `repo-tasks` command on hosts that have commands; E false "git-tracked" docs fixed, first-word-then-basename in 5 scripts, archive symlink refusal, output-side control-character escaping in the CLI, dependabot/lefthook/install-qwen help lists. | The final review found no Critical and 7 Important findings. | Visible, undoable rework. |
| 8 | New input refuses C1 controls U+0080 to U+009F; the read path is unchanged. Spec 8.1 and one line of 17.3 were edited to match. | The spec's purpose is no terminal escape sequences, and U+009B is a CSI introducer; tightening later would break existing lists. | A subject holding a C1 character is refused. |
| 9 | No `bun.lock` in the fix wave. | It needs network. The owner approved it later (`aec2f89`). | Closed. |
| 10 | The Qwen hook uses the single-string command form. | Qwen v0.28.0 does not honour `args` (`hooks/types.ts:91-104`, `hookRunner.ts:1284-1285`). | (derived) If a later Qwen honours `args`, the single string still works. |
| 11 | A `repo-tasks` command exists on hosts that have commands, none on Antigravity. | Spec section 11 approved commands; Antigravity has no documentation page for them. | (derived) Antigravity users lack the slash command. |
| 12 | The Windows, macOS and smoke CI failures were test-side defects and were fixed there; the engine rules stayed. | Measured in the CI run above. | Six OpenCode hook tests skip on Windows: a real coverage loss on that platform. |
| 13 | The capstone was GREEN after 2 rounds (round 1 was thin, so round 2 carried an evidence rule), confirmed by the owner. | See `docs/agy-capstone-ledger.md`. | A clean round is a coverage claim, not proof. |
| 14 | In the test audit, the peer's claim that the Antigravity `NoWorkspace` path is untested was rejected, and two verified gaps were closed with tests (`5320836`). | The path is covered at `shared/repo-tasks/tests/server_e2e.rs:361` and `antigravity/plugins/repo-tasks-mcp-agy/tests/e2e.rs:198`. Only test code changed, so the capstone stands. | None. |

## Deferred minors (still open)

Task 1:
- `Display` of `EnvDir` and `ListName` interpolates the raw value (`error.rs`).
- A relative `XDG_DATA_HOME` is accepted.
- No subprocess test that `GIT_DIR`, `GIT_COMMON_DIR` and `GIT_CEILING_DIRECTORIES` are ignored.

Task 2:
- `to_bytes` uses `unwrap_or_default`.
- The `rejects_each_rule` rows assert only `is_err`.
- The "names both numbers" assertion is weak.
- No missing-member test.
- Key order inside unknown members can change on rewrite.

Task 3:
- A rule broken by the closure's output is reported as `ListFile`, not `ListInvalid` (`store.rs:333-335`).
- Lock-timeout tests use `contains("lock")`.
- The helper child is not kill-on-drop.
- `store_unwritable_names_path` passes vacuously as root.

Task 4:
- No test for the under-lock re-check or an unparseable `updated_at`.
- No test for a line holding both `archived_at` and `deleted_at`.
- A busy start-up lock says "within 0 seconds".

Task 5:
- A refused `task_create` leaves an empty store folder and lock.
- Per-entry create errors carry no position.
- `check_acyclic` runs on every update.
- No test pins `[blocked by]` on a completed task.

Task 6:
- A Windows decoded path without a drive letter is returned as `/foo`; a query or fragment is not stripped.
- `on_reply` is tested only on the non-Windows path.
- A late reply to an earlier settled refresh can overwrite a newer root if replies arrive out of order.
- A leftover `let _ = q;` in `tests/roots.rs`.

Task 7:
- No doc line saying the guard relies on `current_thread` ordering.
- A call whose `store_for` fails is answered before `ensure_startup`.
- No Antigravity success-arm test.
- Busy-lock stderr wording and `on_roots_list_changed` are untested end to end.
- Tool descriptions are duplicated (a drift test exists).
- A cancelled call mid `get_or_init` can overlap two start-up runs (harmless under the lock).
- The 100 ms lower-bound timing margin in the silent-client tests could flake.

Task 8:
- The mode-000 unreadable-file test passes vacuously as root.
- Claude Code `wrap` omits `systemMessage` (the re-ghidra hook prints both).
- Acceptance S8 asserts nothing right after the takeover update.
- Unused `_workspace` and `_e` parameters.
- `reminder` usage errors exit 2.
- `--workspace=` with an empty value is accepted and ignored.

Task 9:
- The `plugin.ts` comment on once-per-session behaviour.
- Events without `sessionID` share the key `""`.
- The Antigravity README omits the `cwd` fallback.

Task 10:
- Path stripping before the cut at whitespace in `smoke-bundle.sh` and `probe-plugin-bin.sh`, and no test pinning the first-word behaviour.
- No stop-line fixture for a missing path or a document with no `tiers.resident.bytes`.
- Cosmetic capitals in the wrapper README bullets.

Task 11:
- `README.md` should say that "one to 50 tasks" is a proposal and that the 2-second roots window is unmeasured.
- The deprecation note should state the consequence for Antigravity (rmcp `server.rs:921-923`).
- "Task 11 measurements pending" is plan jargon, and `archive` is not described.
- `windows_canonical.rs` unwraps before cleanup, which leaks the temp dir.

Fix wave:
- The `Justfile` `mlc` ignore list lacks `repo-tasks-mcp-opencode/node_modules`.
- `scripts/install-qwen-extension.ps1` has its own extension list, not updated.
- Other Qwen manifests declare `"args": []`, which Qwen does not read (harmless).
- INSTALL.md step 4b is redundant now that the bundle ships the command.

## Still unmeasured

- Which error kind triggers the Windows retry; the notes record only call counts.
- Compaction in Antigravity, OpenCode and Qwen.
- OpenCode through its background service.
- The proposed limits (subject 200, description 8000, owner 64) and windows.

## Closed since the final review

- The OpenCode command is bundled and `bun.lock` is committed (`aec2f89`).
- The `eprintln!` on every retry poll was replaced by the retry observer.
- The stray doc line in the Claude Code e2e test was fixed.
- The Windows canonical test uses ASCII-case and `assert_ne`.
- The `MAX_OUTSTANDING` eviction and the `$HOME/.local/share` fallback are pinned (`5320836`).
