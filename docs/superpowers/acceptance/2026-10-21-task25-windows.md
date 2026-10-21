# Task 25 hand acceptance on Windows (2026-10-21)

Plan: `docs/superpowers/plans/2026-10-18-opencode-ctl-mcp.md`, Task 25. Spec background: the "To confirm while
building" section of `docs/specs/2026-10-18-opencode-ctl-mcp-design.md`. This file records results only. The spec and
the plan are left unchanged for the session that folds these results in.

## Environment

- Windows 11 Pro 10.0.26200, ANSI code page 1252, Windows Terminal (`wt`), Git Bash for the driver commands.
- Branch `feat/opencode-ctl-mcp` at `d55f711` (worktree `E:\Rust\aiplugins-ctl`). The plugin was built with
  `bash scripts/smoke-bundle.sh opencode-ctl-mcp-cc` ("Probed 2 entry point route(s) in opencode-ctl-mcp-cc 0.1.0,
  all sound"), and every run used `target/smoke/opencode-ctl-mcp-cc`.
- Claude Code 2.1.306, Antigravity (`agy`) 1.4.4, OpenCode `opencode v2.0.32` (an npm/fnm install with an
  `opencode.cmd` shim, so no official installer), Windows PowerShell 5.1.26100.9711, and PowerShell 7 (`pwsh`) on
  PATH unless stated otherwise.
- The OpenCode service was isolated from the user's own: `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME` and
  `XDG_CACHE_HOME` pointed under `E:\tmp\oci`, `OPENCODE_CTL_HOME` pointed under it too, and
  `opencode service set port 45999` was applied. The user's own service was stopped before the session and
  is still stopped.
- **Code fixes needed to run the plugin: none.** No commits besides this file.

## Step 1: spike reproduction through the MCP tools

**Ran.** This was the top-level session. A throwaway git repository was created per run (`E:\tmp\oc1`, `E:\tmp\oc2`)
holding a 4-task `PLAN.md` (`greet.py`, `mathx.py`, `test_all.py`, `README.md`) committed as the only file. The
isolated service was **stopped** before each run (`opencode service stop`, and `opencode service status` printed
`stopped`). Then, from the project directory:

```
claude -p "$(cat prompt1.txt)" --plugin-dir /e/Rust/aiplugins-ctl/target/smoke/opencode-ctl-mcp-cc \
  --allowedTools "mcp__plugin_opencode-ctl-mcp-cc_opencode-ctl-mcp-cc" --output-format stream-json --verbose
```

The prompt told the controller to use only the plugin's tools in this order: `service_status`, then `service_start`
if not running, then `session_create` with model `opencode/deepseek-v4.1-flash-free`, then `session_prompt`
("Implement every task in PLAN.md in this directory. Create all four files."), then one `session_wait` per slice
until the state is not running, then `session_changes`, and a re-prompt naming any missing file (at most 2). Wall
time was measured around the `claude` process. Spawns were counted from the `cli starting` lines in the isolated
`opencode.log`, between the start and the end of the run. CPU load was 35 % just before the runs (`btop`, Chrome and
other desktop apps were open), so the machine was not idle and background load was not controlled.

**Observed.**

| | Run 1 | Run 2 |
|---|---|---|
| Wall time | 59.4 s | 82.5 s |
| Tool calls | `service_status` (`running: false`) → `service_start` (`running: true`, pid 6088) → `session_create` → `session_prompt` (`delivery: "steer"`) → `session_wait` → `session_changes` | same sequence, `service_start` pid 19148 |
| Slices | 1: `idle`, `elapsed_s` 13, `turn_s` 14 | 1: `idle`, `elapsed_s` 34, `turn_s` 35 |
| Re-prompts | 0 | 0 |
| `session_changes` (`from` = the prompt id, default `to` = the same id) | `README.md` +13, `greet.py` +6, `mathx.py` +11, `test_all.py` +49, three `__pycache__/*.pyc`, all `added` | `README.md` +7, `greet.py` +6, `mathx.py` +11, `test_all.py` +32, three `__pycache__/*.pyc`, all `added` |
| OpenCode CLI spawns | 22, plus the service's own `serve --service` | 26, plus `serve --service` |

Run 2's spawns by operation: `service status` 2, `service start` 1, `GET /openapi.json` 1, `server.info` 1,
`session.create` 1, `session.prompt` 1, `session.inbox.list` 2, `session.message.list` 4, `session.permission.list`
4, `session.form.list` 4, `experimental.session.wait` 1, `session.active` 1, `session.get` 2, `session.diff` 1.
Each slice ends with a `session.permission.list` + `session.form.list` + `session.inbox.list` + `session.active` +
`session.message.list` round. The service started by `service_start` was still running after `claude` exited
(`opencode service status` returned `http://127.0.0.1:45999`, and `tasklist` showed `opencode.exe` pid 19148).

Discarded run (setup artifact, kept for the record). The first attempt used an isolated data directory under the
long scratchpad path (`C:\Users\user\AppData\Local\Temp\claude\E--Rust-aiplugins\<uuid>\scratchpad\iso`). The worker
created all four files, but every `session_changes` returned `files: []` (`from` = `to`), so the controller
re-prompted twice (138.5 s wall, 3 slices). Calling `opencode api session.diff` directly also answered `{"data":[]}`,
including for the range first prompt → third prompt. The cause was in OpenCode, not the plugin: `opencode.log` had
24 entries of `WARN message="failed to capture snapshot" cause="Snapshot.Error: fatal: cannot stat
'C:/Users/.../scratchpad/iso/data/opencode/snapshot/<hash>/<hash>.init-10208-.../hooks/applypatch-msg.sample':
Filename too long"`. Snapshots under a data directory that deep exceed the Windows path limit, so `session.diff` has
nothing to diff. The runs above use `E:\tmp\oci` and do not hit it. Note for the folding session: the 2026-10-20 spike
also saw an empty `session_changes` and put it down to a non-git directory. It may be worth checking that run's
`opencode.log` for this warning.

**Verdict: MEASURED.** From a stopped service the sequence completed both times with one `idle` slice and no
re-prompt. Wall time was 59.4–82.5 s (background load uncontrolled, and the controller model's own turns are
included). There were 22–26 CLI spawns per run. `session_changes` listed all four files.

## Step 2: Antigravity: env inheritance, empty `OPENCODE_CTL_SERVER=`, parent pid

**Ran.** A stub MCP server (`stub_mcp.py`, Python 3.14.5 run as `python.exe -I`) appends one JSON line at spawn with its
pid, its parent pid and every `OPENCODE_CTL_*` variable, and serves one `slow` tool. It was registered with agy:

```
agy mcp add ocstub "C:\...\python.exe" -I "C:\...\stub_mcp.py"
```

`~/.gemini/config/mcp_config.json` was backed up first. agy was then launched from Python, because a Windows shell
cannot set a variable to the empty string, with `OPENCODE_CTL_SESSION=ses_accept25windows0000000` and
`OPENCODE_CTL_SERVER=""` added to the inherited environment:

```
python -I launch_host.py <out> E:\tmp\agy2 C:\Users\user\AppData\Local\agy\bin\agy.exe -p "Reply with the single word OK."
```

**Observed.** agy exited 0 with output `OK` after 91.1 s. Its pid from `Popen` was 9152. The stub's record:

```
{"pid": 3336, "ppid": 9152, "env": {"OPENCODE_CTL_SERVER": "", "OPENCODE_CTL_SESSION": "ses_accept25windows0000000"}, "OPENCODE_CTL_SERVER_present": true}
```

Two later agy launches in Step 3 recorded the same thing: launched pids 13816 and 17764, stub `ppid` 13816 and 17764,
both variables present and the server variable `""`. The run was headless (`-p`), not an interactive tab. Afterwards
`agy mcp remove ocstub` was run, and `cmp` against the backup confirmed that `mcp_config.json` is byte-identical.

**Verdict: MEASURED.** Antigravity 1.4.4 passes `OPENCODE_CTL_SESSION` to an MCP child, keeps
`OPENCODE_CTL_SERVER` as an empty string rather than dropping it, and the child's parent pid is the launched
`agy.exe` (3 of 3 launches), so `rebind_on_spawn` holds. No refusal branch is needed.

## Step 3: tool-call ceilings (Claude Code and Antigravity, a 130 s call)

**Ran.** The same stub, with `slow {seconds: 130}`, stands in for `wait_max_s` 100 + 30 s. The stub answers `ping`
while the call is in flight.

- Claude Code. `MCP_TOOL_TIMEOUT` was unset in the environment, and grep found it in neither the profile's
  `settings*.json` nor `~/.claude/settings*.json`. Run from `E:\tmp\cc3`:
  `claude -p "Call the ocstub slow tool with seconds=130 exactly once, then reply with its exact result text and nothing else." --mcp-config cc_stub.json --allowedTools mcp__ocstub__slow`
- Antigravity, without an allow rule: the same prompt with `seconds=5`, through `launch_host.py`.
- Antigravity, with a temporary allow rule. `"mcp(ocstub/*)"` was appended to `permissions.allow` in
  `~/.gemini/antigravity-cli/settings.json` (backed up first), and the 130 s prompt was run. Afterwards the file was
  restored from the backup.

**Observed.**

- Claude Code 2.1.306: printed `slept 130 s`, rc 0, 152 s wall. The stub logged `{"slow_done": 130, "took": 130.0}`.
- Antigravity 1.4.4 with no rule: `jetski: no output produced — a tool required the "mcp" permission that headless
  mode cannot prompt for, so it was auto-denied. Add an allow-rule under permissions.allow in settings.json (e.g.
  mcp(<target>)). Alternatively, re-run with --dangerously-skip-permissions to auto-approve all tools.` (33.7 s).
- Antigravity 1.4.4 with the rule: printed `slept 130 s`, rc 0, 173.3 s wall. The stub logged
  `{"slow_done": 130, "took": 130.0}`. `cmp` reported `SETTINGS_RESTORED_IDENTICAL`.

**Verdict: MEASURED.** Both hosts return a 130 s MCP call under their default timeouts (Claude Code with
`MCP_TOOL_TIMEOUT` unset, and Antigravity's default), so neither needs a manifest change or a lower `wait_max_s`
clamp. One caveat for headless agy only: an MCP call needs a `permissions.allow` `mcp(<server>/*)` rule, or it is
auto-denied.

## Step 4: Windows items

### 4a. `wt` quoting with spaces, and the drive-root `-d`

**Ran.**

- A real `pair` (4b below) in the directory `E:\tmp\proj with space`, with `OPENCODE_CTL_HOME='E:\tmp\ocä hömé ’q'`.
- `--dry-run` from `E:\` to get the launcher's drive-root form: `opencode-ctl-cc-mcp pair --controller qwen --dry-run`.
- The same `wt` argv form by hand, with a probe that writes its cwd:
  `subprocess.run(["wt","-w","0","nt","-d", d, python, "-I", r"E:\tmp\wtprobe.py", d])` for `d` = `E:\` and `C:\`.
- A `subst X: E:\tmp\droot` attempt.

**Observed.**

- The real pair spawned `wt -w 0 nt -d E:\tmp\proj with space powershell.exe -NoProfile -ExecutionPolicy Bypass
  -File E:\tmp\ocä hömé ’q\pair.33ef67c3ff69.g1.wrapper.ps1` (dry-run form; the log reads `tab spawned (wt, pid
  17572)`). The tab's `powershell.exe` (pid 2904) started `"...\node_modules\@opencode\cli\bin\opencode.exe" "E:\tmp\proj
  with space" -s ses_ea2343c86ffecHzQOviSPttc3l`, so the directory and the script path with spaces and non-ASCII
  characters arrived intact. The tab title was `OC | pair proj with space 09:29Z`.
- Dry-run at `E:\`: `would run: wt -w 0 nt -d E:\ powershell.exe ...`. The backslash is kept.
- Probe: `-d E:\` → `"cwd": "E:\\"`, and `-d C:\` → `"cwd": "C:\\"`. Both open the drive root.
- `subst`: the launcher canonicalizes `X:\` to the substituted target (`-d E:\tmp\droot`), so a `subst` root never
  reaches `wt` as a drive root.
- Side observation: a `wt` tab inherits the caller's environment (the probe saw `XDG_DATA_HOME` and an ad-hoc
  `FOO_ACCEPT=1` set on the `wt` command), which is what kept the paired TUI on the isolated service.

**Verdict: MEASURED.** Paths with spaces work for both `-d` and the script path, and a bare drive root `X:\` opens the
root. This matches `wt_directory_arg`, which passes the path through unchanged.

### 4b. `powershell.exe` 5.1 runs the BOM `.ps1` from a non-ASCII home; `tui-pid` bytes

**Ran.** `pwsh` was taken off PATH for the launcher (PowerShell 7 directory filtered out), so
`select_windows_shell` picks `powershell.exe`. Then:

```
OPENCODE_CTL_HOME='E:\tmp\ocä hömé ’q' opencode-ctl-cc-mcp pair --controller qwen -- -p "(a) b! c" "x!y" "(z)"
```

This ran from `E:\tmp\proj with space`. The `qwen` controller was the fake `qwen.cmd` from 4d. A control was also
run: a two-line script writing the literal `'hömé ’’q'` with `[System.IO.File]::WriteAllText`, saved in the same home
with and without a UTF-8 BOM, run by `powershell.exe -NoProfile -ExecutionPolicy Bypass -File`.

**Observed.**

- `powershell.exe -Command '$PSVersionTable.PSVersion'` → `5.1.26100.9711`, and the ANSI code page is `1252`. The tab
  process pid 2904 is `C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe`.
- The wrapper starts with the bytes `ef bb bf` (BOM), and the wrapper ran (it started `opencode.exe`, see 4a).
- `pair.33ef67c3ff69.g1.tui-pid` is 34 bytes, `2904:1792513380:134370485987487138`, with no BOM and no trailing
  newline (`xxd`: first byte `32`, last byte `38`).
- Read back by `read_sentinel`: `pair_status` (MCP, with the pair's env) answered `"tui":"alive"`.
- `tui-exited`, written when the TUI was killed at the end, is 13 bytes, `1792575295187`, with no BOM and no newline.
- Control: the BOM-less copy wrote `68 c3 83 c2 b6 6d c3 83 c2 a9 20 c3 a2 e2 82 ac e2 84 a2 ...` (mojibake: the
  script was decoded as ANSI). The BOM copy wrote `68 c3 b6 6d c3 a9 20 e2 80 99 71` = `hömé ’q` (the doubled
  `’’` folds to one `’`).
- Test artifact, not a defect: that `pair_status` probe started `serve` with `OPENCODE_CTL_PAIR` set, which rebinds
  the pair's controller to the probe's parent (`claude-pid` rewritten to `8804:...`, `claude-exited` removed), as
  `rebind_on_spawn` is designed to do.

**Verdict: MEASURED.** Windows PowerShell 5.1 runs the BOM wrapper from a non-ASCII home containing `’`. The
`tui-pid` and `tui-exited` sentinels are written without a BOM or newline and are read back as alive. The BOM is
required: without it the same file decodes as ANSI.

### 4c. `opencode.exe` on PATH after the official installer

**Ran.** `where.exe opencode`, then `opencode-ctl-cc-mcp doctor-json` with the isolated env.

**Observed.** `where.exe` lists only `...\fnm_multishells\8332_1792573317271\opencode` and `opencode.cmd` (the npm
shim). The doctor reported `binary ok: opencode found at C:\Users\user\AppData\Local\fnm_multishells\8332_1792573317271\node_modules/@opencode/cli/bin/opencode.exe`,
and `version_range` and `server_info` were `ok` (the message mixes `\` and `/` separators, which is cosmetic). The
TUI command line in 4a shows the same resolved `opencode.exe`, so the shim-following fix `9bef438` works for this
layout.

**Verdict: UNMEASURED.** The official OpenCode installer is not installed on this machine, and installing it was out
of scope. Only the npm shim layout was exercised (MEASURED working above).

### 4d. Does `cmd.exe /c` controller args need `(` `)` `!` in the refusal list?

**Ran.** A fake controller `E:\tmp\fake bin\qwen.cmd` (a path containing a space) was put first on PATH. It contains
one line: `@"C:\...\python.exe" -I "E:\tmp\dumpargs.py" %*`. The dumper logs `sys.argv` and `GetCommandLineW()`. Then
the real pair from 4b ran with the controller args `-p "(a) b! c" "x!y" "(z)"`. Dry-run form:
`cmd.exe /c E:\tmp\fake bin\qwen.CMD -p (a) b! c x!y (z)`.

**Observed.** `pair` exited 0 and printed the exit notice and the four `$env:OPENCODE_CTL_*` lines. The dumper
recorded:

```
{"argv": ["-p", "(a) b! c", "x!y", "(z)"], "cmdline": "\"C:\\...\\python.exe\"  -I \"E:\\tmp\\dumpargs.py\" -p \"(a) b! c\" x!y (z)"}
```

**Verdict: MEASURED.** `(`, `)` and `!` reach a `.cmd` controller unchanged, including when the `.cmd` path contains a
space and one argument is quoted (the `cmd_line` fix `81a65d6` holds). The refusal list needs no additions.

### 4e. Does the service survive a managed-mode call that auto-started it after the job object closes?

**Ran.** With the isolated env, `opencode service stop`, then `opencode service status` (`stopped`). Next, a scripted
MCP client (`mcp_client.py`) started `opencode-ctl-cc-mcp.exe serve`, called `session_list {limit: 2}` and exited.
Then `opencode service status` was run, and a second client process called `service_status`.

**Observed.**

```
raw before: stopped
   3.1s session_list {"next_cursor":null,"sessions":[]}
server exit 0
raw after server exit: http://127.0.0.1:45999
   2.0s service_status {"paired":null,"pid":8272,"reachable":true,"running":true,"urls":["http://127.0.0.1:45999"],"version":"2.0.32"}
```

Step 1 shows the same for `service_start`: the service it started outlived the `claude` process both times.

**Verdict: MEASURED.** The service auto-started by a managed-mode `session.*` call survives the call's job object and
the MCP server's exit (fix `478f64f`, the guard's `release()` on a normal exit).

## Step 5: managed TUI reconnect after `service_restart`

**Ran.** This used the paired tab from Step 4b: `wt` + `powershell.exe` 5.1, TUI `opencode.exe` pid 19768, managed
service pid 8272. `tuiconn.ps1` recorded the listener pid on :45999, whether the TUI is alive, the TUI's TCP
connections to :45999 and a screenshot, before and after. Through `mcp_client.py` (unpaired env), the calls were:
`service_restart {}`, then `service_restart {confirm: true}`, then `service_status`. After that came `session_prompt`
to `ses_ea2343c86ffecHzQOviSPttc3l` ("Reply with exactly: RECONNECT-CHECK-25. Do not use any tools."), then
`session_wait`, then `wt -w 0 focus-tab -t 3` and a screenshot.

**Observed.**

- Before: listener pid 8272, TUI alive, `tui -> :45999 Established: 1`. The tab showed the empty session.
- `service_restart {}` → `{"error":{"code":"CONFIRM_REQUIRED","message":"service_restart needs confirm: true"}}`,
  with no restart.
- `service_restart {confirm: true}` returned after 5.7 s with
  `{"paired":null,"pid":null,"reachable":false,"running":false,"urls":[],"version":null}`. Then `service_status`, 4.4 s
  later, gave `{"pid":13460,"reachable":true,"running":true,...}`. This reproduced on a second restart (5.7 s,
  `running: false`; `service_status` 3.8 s later showed pid 3832). A raw `opencode service restart` followed by
  `opencode service status` showed the URL both times. So **`service_restart`'s own result reports the service as
  stopped right after a restart that succeeded** (`tools/service.rs:108-113` reads `service_status` immediately after
  `Lifecycle::Restart`). A controller following the driver loop would read this as "not running".
- After: listener pid 13460, TUI pid 19768 still alive, `tui -> :45999 Established: 1`. The TUI made a new
  connection to the new service without any action in the tab.
- `session_wait` → `state: "idle"`, `last_assistant_text: "RECONNECT-CHECK-25"`. The screenshot of the focused tab
  shows the prompt, the reply `RECONNECT-CHECK-25`, `Build · MiniMax M3.1 Flash Free · 3.6s` and `Context 10,006
  tokens 1% used`. No error banner appeared, and the tab title stayed `OC | pair proj with space 09:29Z`.

**Verdict: MEASURED.** The managed TUI reconnects by itself after `service_restart` and shows post-restart turns
live. **FAILED** (a separate finding for the owner): the result of `service_restart {confirm: true}` says
`running: false, pid: null` although the restart succeeded (2 of 2). Only a later `service_status` shows the new
service.

## Leftovers

- On the user's desktop: two stray Windows Terminal tabs (`C:\WINDOWS\system32\cmd.exe` and `cmd`) from an early,
  malformed `wt ... cmd /c "set XDG > ..."` env probe. They were left open because they could not be told apart from
  the user's own tabs with certainty.
- Throwaway data under `E:\tmp`: `oci`, `oc1`, `oc2`, `proj with space`, `ocä hömé ’q`, `agy2`, `cc3`, `droot`,
  `fake bin`, plus the probe scripts. The isolated service is stopped. agy's `mcp_config.json` and `settings.json` are
  byte-identical to their backups.
