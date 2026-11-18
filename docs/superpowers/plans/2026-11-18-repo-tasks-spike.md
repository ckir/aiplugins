# repo-tasks measurement spike Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fill a table of observed host behaviour for Claude Code, Qwen, Antigravity and OpenCode on this Linux machine, and add it to section 15 of the design document.

**Architecture:** A throwaway probe (one stdio MCP server and one hook script, both Python standard library) is wired into each host in a throwaway git repository outside this repository. The probe appends one JSON line per observation to a log file whose path is baked into the host configuration. Nothing is merged except the results table in the design document.

**Tech Stack:** Python 3 (stdlib only), `git`, `jq`, the installed hosts: `claude` 2.1.327, `agy` 1.5.11, `opencode` v2.3.0. `qwen` is not installed on this machine (checked 2026-11-18, `command -v qwen` finds nothing).

**Spec:** `docs/superpowers/specs/2026-11-18-repo-tasks-design.md`, section 16 step 1 (the five measurements) and section 15.6 (the unknowns these cells settle). Section 7.1 (how D is resolved) and 8.5 (start-up archiving) are the sections a result can contradict.

## Global Constraints

- The spike is throwaway and merges no product code (16, step 1). The only file this plan changes in the repository is the design document (Task 6), plus this plan.
- All probe files live in `SPIKE=/tmp/repo-tasks-spike`, outside the repository. Nothing is installed globally and no host configuration under the home directory is edited: every host is pointed at its configuration by flag, by environment variable or by a file in the throwaway repository. A host still writes its own session records where it always does (Claude Code and Antigravity under the home directory); that is recorded in `SPIKE/notes.md`, not prevented.
- Every cell of the result table holds what was observed with the command or log line that showed it, or the words `not measured` and the reason (16, step 1).
- `REPO_TASKS_DIR` and `REPO_TASKS_LIST` are the two variable names measured; the probe sets them to the values `/spike/dir` and `spike-list`.
- Each host is run from three starting places: the main checkout root `R`, a subdirectory `R/sub/deep`, and a linked worktree `W`. The log records which.
- Host runs use real model calls. Each prompt is the single word `ok`-style trivial request; no run needs a tool call, because every measurement except compaction is logged at server or hook start.
- Every host run is wrapped in `timeout 180` and is bracketed by a process census (`pgrep -af '<host>'` before and after). A process the run left behind, and that was absent before, is stopped by its own pid and noted in `SPIKE/notes.md`; a process present before is never touched.
- Every host configuration is generated per run from a template by `sed`, which writes the run's log path (`SPIKE/logs/<host>-<label>.jsonl`) and the fixed git directory (`SPIKE/repo/.git`) into the probe's two arguments (`probe_mcp.py <log> <fixed-git-dir>`, `probe_hook.py <log> <label>`). Each run therefore has its own log, and each run script takes `<place> <label> [prompt]` (and `<mode>` for OpenCode, before the prompt).
- A host may start an MCP server lazily. When a run leaves no `start` line, it is repeated once with a prompt that asks the model to call the probe tool by name; only when that run also leaves none is the cell `not measured: server did not start` with the debug-log reason.
- Windows and macOS are out of scope (17.2, item 2). Timing is not measured, so the timing-measurement rules do not apply.

## Review Focus

- A probe that logs nothing because the host never started it looks identical to a host that started it with an empty environment. Task 1 tests that a log line is written on start-up, and each host task checks that a `start` line exists before reading any cell.
- A host that resolves a relative path in the configuration against a different directory than the one the probe expects. The configuration uses absolute paths only (as `opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md:132` does).
- OpenCode's default background service may start the server with its own environment, not the shell's. Task 5 runs both `--standalone` and the default and records both.
- A hook that runs with the plugin folder as its working directory (15.5). The hook log records its own `cwd` next to the event's path.
- A compaction that cannot be triggered from a script. That cell may stay `not measured` with the reason, for the owner to accept at step 2 (16).

---

### Task 1: The probe and the fixture

**Files:**
- Create: `/tmp/repo-tasks-spike/probe_mcp.py`, `/tmp/repo-tasks-spike/probe_hook.py`, `/tmp/repo-tasks-spike/make_fixture.sh`, `/tmp/repo-tasks-spike/selftest.sh`

**Interfaces:**
- Produces: `probe_mcp.py <log> <fixed-git-dir>`: a stdio MCP server (newline-delimited JSON-RPC 2.0) answering `initialize`, `tools/list` (one tool `probe`) and `tools/call`; on start-up, before reading any input, it appends one line to `<log>`.
- Produces: `probe_hook.py <log> <label>`: reads standard input fully, appends one line to `<log>`, prints `{}` and exits 0.
- Produces the log line shape (one JSON object per line), with these keys: `kind` (`start` or `hook`), `label`, `ts` (RFC 3339 UTC), `cwd` (`os.getcwd()`), `argv`, `env` (an object with only the keys `REPO_TASKS_DIR`, `REPO_TASKS_LIST`, `PWD`, `CLAUDE_PROJECT_DIR`, each `null` when unset), `ppid`, and for `start` (with `label` set to `mcp`): `git_common_dir` (output of `git rev-parse --path-format=absolute --git-common-dir` run in `cwd`, or the error text), `mkdir` (`"ok"` or the OS error from creating `<git_common_dir>/repo-tasks-spike/` and removing it again, or `"skipped: no repository at cwd"` when `git_common_dir` is an error) and `mkdir_fixed` (the same create-and-remove in `<fixed-git-dir>`, always attempted; this one answers "may this process write under `.git`" independently of where the host started it); for `hook`: `stdin` (the raw text read, up to 64 KiB).
- Produces `make_fixture.sh`: creates `SPIKE/repo` (git repository `R` with one commit, a directory `sub/deep`), and a linked worktree `SPIKE/wt` (`W`) of it, and an empty `SPIKE/logs/`. Idempotent: it deletes and recreates `SPIKE/repo`, `SPIKE/wt` first. Any file a host needs inside the repository (Task 4's `.agents/` folder) is written and committed in `R` before `W` is added, so `W` holds it too.

- [ ] **Step 1: Write the self-test first (`selftest.sh`)**

It runs `make_fixture.sh`, then starts `python3 probe_mcp.py $SPIKE/logs/self-<n>.jsonl $SPIKE/repo/.git` (a new log file for each of its runs, so each log holds exactly one `start` line) with its working directory set to `SPIKE/repo/sub/deep` and `REPO_TASKS_DIR=/spike/dir` in its environment, sends one `initialize` request and one `tools/list` request on standard input, reads two reply lines, closes standard input, and asserts with `jq`: the log has exactly one `start` line; `cwd` ends in `/sub/deep`; `git_common_dir` equals `SPIKE/repo/.git`; `mkdir` and `mkdir_fixed` are `"ok"`; a start from `/tmp` (no repository) has `mkdir` `"skipped: no repository at cwd"` and `mkdir_fixed` `"ok"`; `env.REPO_TASKS_DIR` is `/spike/dir`; `env.REPO_TASKS_LIST` is `null`; the `tools/list` reply names a tool `probe`. It then pipes `{"hookEventName":"X","cwd":"/c"}` into `probe_hook.py $SPIKE/logs/self.jsonl lbl` and asserts a `hook` line whose `stdin` parses back to that object. A second start from `SPIKE/wt` asserts `git_common_dir` is still `SPIKE/repo/.git`, the oracle being `git rev-parse --path-format=absolute --git-common-dir` (the one 7.2 names).

- [ ] **Step 2: Run it to see it fail**

Run: `bash /tmp/repo-tasks-spike/selftest.sh`
Expected: FAIL (the probe files do not exist).

- [ ] **Step 3: Write `make_fixture.sh`, `probe_mcp.py` and `probe_hook.py` to the interfaces above**

Python standard library only. `probe_mcp.py` answers `initialize` with protocol version echoing the request's, an empty `capabilities.tools` object and server info `probe`; `tools/call` returns one text content `probed`. Unknown methods with an id get a JSON-RPC error `-32601`; notifications get no reply.

- [ ] **Step 4: Run the self-test**

Run: `bash /tmp/repo-tasks-spike/selftest.sh`
Expected: prints `selftest ok` and exits 0.

- [ ] **Step 5: Record**

No commit (the files are outside the repository). Append to the execution memory file the line `spike probe built, selftest ok`.

---

### Task 2: Claude Code

**Files:**
- Create: `/tmp/repo-tasks-spike/cc-mcp.json`, `/tmp/repo-tasks-spike/cc-settings.json`, `/tmp/repo-tasks-spike/run_cc.sh`, `/tmp/repo-tasks-spike/logs/cc-*.jsonl` (run output)

**Interfaces:**
- Consumes: Task 1's probe and log shape.
- Produces: `run_cc.sh <place> <log-label> [prompt]` where `<place>` is `R`, `R/sub/deep` or `W`; it runs `claude` from that directory with the probe wired in and `REPO_TASKS_DIR=/spike/dir REPO_TASKS_LIST=spike-list` set in its environment, and writes `logs/cc-<log-label>.jsonl`.

- [ ] **Step 1: Confirm the flags**

Run `claude --help` and confirm these exist (they did at 2.1.327): `-p`, `--mcp-config`, `--strict-mcp-config`, `--settings`, `--setting-sources`, `--debug-file`. Write the confirmation line into the plan's results scratch file `SPIKE/notes.md`. If any is missing, stop and report.

- [ ] **Step 2: Write the two configuration files and the script**

`cc-mcp.json` (a template, see Global Constraints) declares one server `probe` with command `python3`, arguments the absolute path of `probe_mcp.py`, the log path and the fixed git directory. `cc-settings.json` declares `SessionStart` hooks whose command runs `probe_hook.py` with the log path and label `cc-hook`, one entry per matcher source: `startup`, `resume`, `clear`, `compact`. Command and log paths are absolute. `run_cc.sh` runs `claude -p "reply with the word ok" --mcp-config cc-mcp.json --strict-mcp-config --settings cc-settings.json --setting-sources "" --debug-file SPIKE/logs/cc-debug-<label>.txt` and records the exit status.

- [ ] **Step 3: Run the three places**

Run `run_cc.sh R r`, `run_cc.sh R/sub/deep sub`, `run_cc.sh W wt`.
Expected for each: a log with one `start` line and at least one `hook` line. If a log has no `start` line the server did not start: read the debug file and record the reason; do not read any cell for that run.

- [ ] **Step 4: Compaction**

Whether `claude -p` runs `/compact` at all is not known [CLAIM, 2026-11-18]. Try, from `R`, in this order, stopping at the first that yields a `hook` line whose `stdin` carries a `source` of `compact`: (1) `run_cc.sh R compact "/compact"`; (2) a first run of the trivial prompt with `--output-format json` to learn its `session_id`, then `claude -p "/compact" --resume <session_id>` with the same flags. A run that leaves a `hook` line with another source, or none, is recorded as observed ("`-p` does not signal compaction") and the cell reads `not measured: a script cannot trigger a compaction in this host (<command>, <output>)`, for the owner to accept at step 2.

- [ ] **Step 5: Read the cells**

With `jq`, extract per place: `cwd` and `mkdir` and `git_common_dir` of the `start` line; `env.REPO_TASKS_DIR` and `env.REPO_TASKS_LIST` of the `start` line (the server) and of each `hook` line (the hook); the `hookEventName`/`source` and whether a workspace path key (`cwd`) is present in each hook `stdin`. Write the raw `jq` commands and their output into `SPIKE/notes.md` under a `## Claude Code` heading.

---

### Task 3: Qwen

**Files:**
- Modify: `/tmp/repo-tasks-spike/notes.md`

- [ ] **Step 1: Re-check that `qwen` is absent**

Run: `command -v qwen; ls ~/.qwen 2>&1 | head -3`
Expected: no `qwen` on the path. If it is now present, stop and say so: the plan then needs a Qwen task written like Task 2 against `qwen/plugins/re-ghidra-mcp-qwen/qwen-extension.json:17-43`.

- [ ] **Step 2: Record the cells**

Under `## Qwen` in `SPIKE/notes.md` write every cell as `not measured: qwen is not installed on this machine (command -v qwen printed nothing, <date>)`. Installing it is not part of this plan: it is a decision for the owner at step 2 (16), and the document's own wording says what rests on the reading of the repository's wrapper files.

---

### Task 4: Antigravity

**Files:**
- Create: `.agents/plugins/probe/` inside the fixture repository `R`, committed there before `W` is added (the customization root and plugin folder named in `hooks.md:8-9` and `plugins.md:13`; the fixture is outside this repository), `/tmp/repo-tasks-spike/run_agy.sh`

**Interfaces:**
- Consumes: Task 1's probe.
- Produces: `run_agy.sh <place> <label> [prompt]`, same contract as `run_cc.sh`, using `agy --print` (flags confirmed in `agy --help`: `--print`, `--log-file`, `--settings`, `--dangerously-skip-permissions`).

- [ ] **Step 1: Read how a plugin declares its server and hook**

Read, in this order, `~/.gemini/antigravity-cli/builtin/skills/agy-customizations/docs/plugins.md`, `mcp_servers.md`, `hooks.md` and `json_configs.md`, and the shipped example `antigravity/plugins/re-ghidra-mcp-agy/` (`plugin.json`, `mcp_config.json`, `hooks.json`). Write into `SPIKE/notes.md` the exact file names and keys you will use, each with the document and line you took it from. The documents name the workspace customization root `.agents/` with `hooks.json` directly in it (`hooks.md:8-9`) and per-plugin `plugins/<name>/mcp_config.json` (`mcp_servers.md:21-22`, the only workspace-level MCP location the document names); use those, in the fixture, so nothing is installed and nothing under the home directory changes. Because `.agents/` is found from the workspace, record for each place whether it was found (the `hooks.md:131` working directory is then `.agents/`, not the plugin). If the documents do not make a workspace-level `.agents/` take effect in a headless `--print` run, stop and report `STATE_MISMATCH`: the owner decides whether a throwaway `agy plugin install` and removal is allowed.

- [ ] **Step 2: Write the plugin folder and the script to those names**

`R/.agents/plugins/probe/` holds `mcp_config.json` (the probe server, absolute paths) and `hooks.json` (hooks of a plugin sit in the plugin's own folder, `plugins.md:63`), with the plugin metadata file and any enabling step `plugins.md` requires (read in step 1), and the hooks file has a `SessionStart` entry and a `PreInvocation` entry, each running `probe_hook.py` with labels `agy-sessionstart` and `agy-preinvocation`. `run_agy.sh` runs `agy --print "reply with the word ok"` from the chosen place, with `REPO_TASKS_DIR` and `REPO_TASKS_LIST` set.

- [ ] **Step 3: Run the three places and read the cells**

As Task 2, steps 3 and 5. For each `hook` line also record whether `stdin` has `workspacePaths`, `cwd`, or both, and the hook's own `cwd` (the document says it is the plugin folder, 15.5). Compaction: record whether any hook event named in `hooks.md` signals one, and whether `agy --help` or the documents name a command that triggers one in a `--print` run; if either is absent the cell reads `not measured: <which is absent> (<line or command>)`, for the owner to accept at step 2.

---

### Task 5: OpenCode

**Files:**
- Create: `/tmp/repo-tasks-spike/oc/opencode.json`, `/tmp/repo-tasks-spike/oc/plugins/probe.ts`, a symlink `/tmp/repo-tasks-spike/oc/node_modules` to `opencode/plugins/re-ghidra-mcp-opencode/node_modules` (so `@opencode/plugin` resolves with nothing installed; remove the link at the end), `/tmp/repo-tasks-spike/run_oc.sh`

**Interfaces:**
- Consumes: Task 1's probe.
- Produces: `run_oc.sh <place> <label> <mode> [prompt]` where `<mode>` is `standalone` or `service`; same log contract. It exports `REPO_TASKS_DIR=/spike/dir`, `REPO_TASKS_LIST=spike-list` and `OPENCODE_CONFIG_DIR=/tmp/repo-tasks-spike/oc` (the variable read at `packages/cli/src/index.ts:145` and `packages/util/src/global.ts:79` in the OpenCode source) before running the host, and `XDG_DATA_HOME`, `XDG_STATE_HOME` and `XDG_CACHE_HOME` set under `/tmp/repo-tasks-spike/xdg/` so the host's session data, state and locks do not land in the home directory (OpenCode's directories follow XDG, `packages/util/src/global.ts:5`; confirm in that file which variables it reads).

- [ ] **Step 1: Read the two shapes**

The MCP entry shape is in `opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md:125-140` (`mcp.servers.<name>` with `type: "local"`, an absolute `command` array, `environment`). The plugin shape is the `Plugin.define` block of `opencode/plugins/re-ghidra-mcp-opencode/plugin.ts` (the `context` hook at its line 207, the `compaction` hook at 201, and `event.system` guarded by `Array.isArray`). Confirm both against `opencode --help` and the source at `/tmp/claude-1001/-home-user-Development-Rust-aiplugins/5af8d5a4-c8d5-4fc6-b4c0-7e6f911789e0/scratchpad/opencode-src` (read only; never build or run anything in it). Write the confirmations with `path:line` into `SPIKE/notes.md`. Plugins load from `<config dir>/plugins/*.ts` (`opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md:52,57`), and `OPENCODE_CONFIG_DIR` moves the config directory, so nothing under the home directory is edited. Confirm in the source that a `.ts` file there is loaded without being named in `opencode.json`; if it must be named, add that entry.

- [ ] **Step 2: Write the configuration, the plugin and the script**

`opencode.json` declares the probe as in INSTALL.md with `environment` empty, so a result in `env` proves the host environment was merged (15.4 says it is: `stdio.ts:30-33`). `plugins/probe.ts` (shaped like the `Plugin.define` block of the re-ghidra plugin, importing `@opencode/plugin`) registers a `context` hook and a `compaction` hook that each spawn `probe_hook.py <log> oc-context|oc-compaction` with standard input `JSON.stringify({sessionID, directory: ctx.location.directory, keys: Object.keys(event)})` and push nothing onto `event.system`. `run_oc.sh` runs `opencode run`-style non-interactive prompting if `opencode --help` lists it, else `opencode --prompt "reply with the word ok" <place>` under `timeout 90`, with `--standalone` for `standalone` mode.

- [ ] **Step 3: Run and read**

For each of the three places in both modes (`service` mode may leave a background service running and a running service is reused by the next run, which hides that run's `start` line: before each `service` run, stop the service this plan started, found by the census; if a service was already running before the plan began, `service` mode is `not measured: a service was already running and is not ours to stop`): confirm a `start` line, then read the cells as in Task 2 step 5. Add the `ctx.location.directory` value from the `oc-context` line (the document's source of the workspace path, 10.2). Compaction: trigger a long enough session only if `opencode` offers a command for it; otherwise `not measured` with the reason.

---

### Task 6: The table, the revision list, the commit

**Files:**
- Modify: `docs/superpowers/specs/2026-11-18-repo-tasks-design.md` (section 15: add `### 15.7 Spike results (2026-11-18, Linux)` before `### 15.6`'s successor heading, keeping section numbers of 16 onward unchanged)

**Interfaces:**
- Consumes: `SPIKE/notes.md` and the logs.

- [ ] **Step 1: Build the table from the notes**

One row per host (four), one column per measurement of section 16 step 1: server working directory (per place R, `R/sub/deep`, W); mkdir in the git common directory; `REPO_TASKS_DIR`/`REPO_TASKS_LIST` reaching the server; reaching a session-start hook; session-start event name and whether its payload carries the workspace path; compaction signalled. Every cell is `<observed value> (<log line or command>)` or `not measured: <reason>`. Copy log lines verbatim into an indented block below the table; paths under the home directory are replaced by `~`.

- [ ] **Step 2: Write the contradiction list**

Below the table add a list headed `Sections this contradicts`: for each cell, say which of 7.1, 7.3, 7.6, 8.5, 10.2, 15.4, 15.5, 15.6 it confirms or contradicts, quoting the sentence. A cell that contradicts nothing is listed as `confirms <section>`. Remove from 15.6 each item now measured, and say in 15.6 which host cells remain `not measured` and why.

- [ ] **Step 3: Verify**

Run: `grep -c "not measured" docs/superpowers/specs/2026-11-18-repo-tasks-design.md` and read each hit in the new table: each must name a reason. Run `git diff --stat`: only the design document changed.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-11-18-repo-tasks-design.md
git commit -m "docs(spec): record the repo-tasks spike results"
```

(End the message with the attribution line the session gives.) Do not push. Step 2 of section 16 follows: the revision goes to the owner, and the implementation plan is written only after it.

## Review ledger (panel round 1, 2026-11-18, the peer returned FINDINGS-11)

All 11 were checked against the files before folding. Folded: the `start` line's `label`; `mkdir` conflated the server's directory with `.git` write permission (a `mkdir_fixed` probe added); a lazy server read as "did not start" (a repeat with a tool-calling prompt); Antigravity's loose plugin (workspace `.agents/` per `hooks.md:8-9`, nothing installed); OpenCode's plugin not registered and `@opencode/plugin` unresolved (`OPENCODE_CONFIG_DIR`, `plugins/probe.ts`, a read-only `node_modules` link); `run_oc.sh` omitting the two variables; `claude -p "/compact"` unproven (two routes, then `not measured`); the OpenCode background service left running and missing timeouts (a process census and `timeout 180`); Antigravity compaction read from documents only (a command search added). None rejected.

Panel round 2 (FINDINGS-8, checked against `mcp_servers.md:16-22`, `plugins.md:8-16`, `global.ts:5`): folded all eight: the two probe arguments and per-run logs and prompts by template (Global Constraints); the workspace MCP location is a plugin folder; `W` receives `.agents/` through a commit in `R`; the self-test uses one log per run; a reused OpenCode service; OpenCode's XDG directories moved under `SPIKE`. None rejected.
