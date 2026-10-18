# opencode-ctl-mcp: drive an OpenCode worker from Claude Code (and three other hosts) as a subagent

Design document. Written 2026-10-18 on branch `feat/opencode-ctl-mcp` (from `feat/antigravity-parity` @4aeeb54).
Status: APPROVED by the owner on 2026-10-18 (at 79ea151); panel review next, then the plan.

## Intent and why

The owner wants Claude Code to drive OpenCode as a subagent: Claude keeps the plan and the judgment, hands bounded
work to an OpenCode session that runs a cheap (often free) model, waits for it, verifies what it did, and sends it
back when something is missing. The reason is cost: development with OpenCode's free models is cheap, and the owner
has watched OpenCode implement a Claude-written plan correctly.

Measured on 2026-10-18 (throwaway spike, two fresh sessions on `opencode/nemotron-3.5-super-vl-free`, a four-task
Claude-written plan, driven through the raw `opencode api` CLI): both reached idle unattended in 133 s and 202 s,
0 permission requests, cost 0, the workers' own tests passed (9 and 17) when run independently. One run skipped a task
while its final message claimed all four were done; a follow-up prompt naming the gap fixed it in 37 s. The plugin
exists so Claude can do that loop through typed tools instead of shell commands, with compacted output (the raw
message list of one short session is 68 KB) and verification built into the workflow.

The owner also wants one command that starts Claude and its OpenCode worker side by side in two terminal tabs, so a
human can watch (and type into) the worker while Claude drives it. Each Claude gets its own OpenCode session.

## Goals

1. Claude Code, Qwen Code, Antigravity and OpenCode itself can create, prompt, wait on, read, verify, interrupt and
   delete OpenCode sessions through MCP tools, and can start the OpenCode background service if it is not running.
2. Output is compacted: a tool result never dumps raw server JSON; every result fits a stated byte cap.
3. The worker's permission requests surface to the controller, which answers them; nothing stalls unattended.
4. Lazy binding is the core: Claude can create worker sessions on demand with no terminal tab. A `pair` launcher is a
   convenience that pre-creates a session, opens the TUI tab and starts Claude bound to it.
5. The plugin is self-contained: no dependency on clavity, no new Rust dependency beyond the workspace set.
6. Ships through the repo's existing pipeline for all four hosts (registry, marketplaces, release-please, bundles,
   footprint budgets, `just check`).

## Non-goals

- Streaming the worker's events live into the controller (SSE). The CLI transport cannot stream; see Transport.
- Driving a private OpenCode server per pair. `--standalone` starts a child with a random password and no
  discoverable endpoint (measured in the binary: `cli.standalone.endpoint`); pairs share the background service.
- A SessionStart preflight hook binary. v1 ships a `doctor` skill instead (co-signed, see Decisions).
- Managing OpenCode's own config (models, providers, agents) beyond passing `agent` and `model` per session.
- macOS terminal-tab support in the launcher (follow-up; the MCP tools themselves work on macOS).

## Anti-goals (a failure even if it technically works)

- A controller that trusts the worker's closing summary. The spike proved a worker claims a task done that it did
  not do. The driver skill must require verification by file list and diff; the tool set must make that cheap.
- A `service_stop` that kills the service hosting the OpenCode-side plugin, or that a model reaches for when it meant
  "stop this turn". Lifecycle tools are refused on the OpenCode host and require `confirm: true` everywhere else.
- A generic escape hatch that lets a model run a mutating route by accident. 77 of the 143 operations mutate.
- Context blow-up: a result larger than the cap, or reasoning blocks returned by default.
- Two Claudes silently sharing one worker session because both defaulted to "the newest session in this directory".

## Constraints (measured, not assumed)

- OpenCode v2.0.32. `opencode serve --service` is the background service: HTTP/1.1 JSON under `/api/...` on
  `127.0.0.1:49374` (channel "latest"), not gRPC (grpcurl cannot dial; h2c gets nothing). 119 paths, 143 operations
  in `/openapi.json`. Auth: HTTP Basic `opencode:<password>`; registration file `~/.local/state/opencode/service.json`
  = `{id, version, url, pid, password}`. The CLI client `opencode api <operationId | METHOD path> [--param k=v]
  [-d body] [--server url]` authenticates by itself. Lifecycle: `opencode service start|restart|status|stop|get|set|unset`.
- `session.prompt` ENQUEUES (returns `Session.Inbox.User` at once); `experimental.session.wait` (POST, no body)
  blocks until the agent loop is idle (204) and returns at once when the session is already idle. When a turn
  ends the server appends a message of type `idle` to the session's message list (measured: the spike sessions end
  `user, assistant..., idle`), so "newest message is `idle` and newer than the prompt" is the end-of-turn signal. `Session.Inbox.Delivery` = `steer | queue`; the TUI sends every prompt
  with `steer`; `queue` items are held until the current turn ends. The wait route does not report permissions;
  pending ones are read from `session.permission.list` (per session) or `permission.request.list` (per location).
- `session.create` (POST /api/session, JSON body `{title?, parentID?, agent?, model?, location?: {directory}, permissions?}`)
  without `location` lands in the SERVER default directory (`/home/user` here), not the host workspace. `session.list`
  (GET /api/session) filters SERVER-side by query params `directory`, `project`, `search`, `parentID`, `subpath`,
  with `limit`, `order`, `cursor` paging. Session ids look like `ses_` + 26 alphanumerics. Sessions carry `agent`, `model {providerID, id, variant}`, `permissions: Permission.Rule[] | null`
  (`{action, resource, effect: allow|deny|ask}`), `time {created, updated, idle, viewed}`, `tokens`, `cost`.
  Reply decisions: `once | always | reject`. The owner's global config is allow-all.
- Wire facts measured 2026-10-18 for the contract: list routes return `{data: [...], cursor: {previous, next}}`;
  `server.info` returns `{version, pid, urls: [..], paths, capabilities}` (`urls` is an ARRAY); session rows from
  `session.list` and `session.get` carry `id, projectID, title, location{directory}, outcome, time{created, updated,
  idle, viewed?}, tokens, cost` and `agent`, `model` only when set; the end-of-turn `idle` message carries
  `outcome: succeeded | failed | interrupted`; `Permission.Request` is `{id, sessionID, action, resources: string[],
  save: string[], message, metadata, source}`; forms are a second stall channel (`session.form.list`,
  `session.form.reply {answer: Form.Answer}`, `form.list`); `session.diff` (params `sessionID, from?, to?, context?`, `from`/`to` are message ids)
  returns `{data: [{file, patch, additions, deletions, status}]}` for ONE TURN by default ("from the first prompt
  after the session was last idle until its next idle marker; prompts steered in while it was busy belong to the
  same turn; `to` extends the range through a later turn; in sessions without any idle marker a prompt's turn spans
  until the next user message", route description); `vcs.status` / `vcs.diff`
  take `location` as a deepObject query parameter (`--param 'location[directory]=<dir>'`; a bare `directory`
  param is silently ignored and resolves to the server default), `vcs.diff` also `mode` (required, `Vcs.Mode` =
  `working | branch | committed`), `base`, `context`; the id returned by
  `session.prompt` IS the stored user message id (verified on the spike). `opencode run --model` documents the
  model string as `provider/model#variant`.
- 204 routes (`experimental.session.wait`, `session.interrupt`, `session.remove`, `session.permission.reply`,
  `session.form.reply`, `permission.saved.remove`, ...) print NOTHING on stdout with exit 0 (measured on the wait
  route). `session.interrupt` takes `resume` (query; `true` resumes pending steering input while queued prompts stay
  parked). `session.message.list` `type` is a single enum value and `order` defaults to oldest-first.
  `Form.Info` = `{id, sessionID, title, metadata, fields}`; `Form.Answer` is a map of field key to `Form.Value`.
  `permission.saved.list` filters by `projectID`. Both `OPENCODE_SERVER_PASSWORD` and `OPENCODE_PASSWORD` reach the
  server; with `--server` and a wrong or missing password the CLI prints `Error: Server at <url> rejected the
  password` / `... requires a password` on stderr (no `_tag` JSON), exit 1. An `UnknownError` stack trace carries the
  real message on its `[cause]:` line.
- CLI error contract measured: a server error prints the error JSON (`{"_tag": "SessionNotFoundError", ...}`) on
  STDOUT without a `data` envelope, `HTTP 404 Not Found` on stderr, exit 1; an unknown operation id prints an
  `UnknownError` stack trace on stderr with empty stdout, exit 1. CLI STDOUT IS TRUNCATED AT 262144 BYTES WHEN
  READ THROUGH A PIPE in 1 of 8 and 4 of 10 runs (exit 0, no error); redirected to a file it is complete every time.
- `opencode api` costs 0.48 to 0.62 s per call (bun startup); curl 0.1 s. `opencode api event.subscribe` emits
  nothing (cannot stream). Message content parts are `text`, `reasoning`, `tool {id, name, state, time, executed}`.
- Workspace deps (root `Cargo.toml`): anyhow, serde, serde_json, tokio, rmcp 3.1, schemars, tracing,
  tracing-subscriber, thiserror, uuid, sha2, lru, tracing-appender, plus `nix` (Unix) and `windows` (cfg-gated);
  `shared/ghidra-worker-ctl/src/job_object.rs` already implements the process-group kill and the Windows job object
  and is REUSED, not re-specified (the plan decides whether that is a dependency on `ghidra-worker-ctl` or a lift
  of the module into a shared process-utility crate; either way the dependency is named there). The Windows parent-pid read for re-bind needs the
  `Win32_System_Diagnostics_ToolHelp` feature on the existing `windows` dependency (a feature, not a crate), and
  macOS has no `/proc`, so liveness there uses the `ps` / `sysctl` path that the macOS CI leg exercises. No `async_trait` and no HTTP client crate; adding one would ratchet
  `docs/footprints/budgets.json` and pass `deny.toml`.
- Repo pattern for multi-host MCP plugins: engine lib crate in `shared/<x>` (rmcp `#[tool_router]` server,
  `cli::dispatch`, an `emit-skill` subcommand that prints the crate-embedded driver skill; the Justfile
  `emit-ghidra-skill` recipe writes the four committed `skills/<name>/SKILL.md` copies from it, and the cc front's
  `tests/skill_emit.rs` pins the committed copy to the emitted text), thin per-host crates `claude-code/plugins/<x>-cc`, `qwen/plugins/<x>-qwen`,
  `antigravity/plugins/<x>-agy` with `src/bin/*.rs`, host manifests (`.claude-plugin/plugin.json` + `.mcp.json`;
  `qwen-extension.json` with `mcpServers`; `plugin.json` + `mcp_config.json` (`{command, args}` only, the binary is
  found by NAME on PATH, no environment block) + `hooks.json`), and an OpenCode front
  `opencode/plugins/<x>-opencode` (`plugin.ts`, `opencode.jsonc` with `mcp.servers.<x>`, `package.json`,
  `README.md`, `INSTALL.md`; bundler requires all five). `/bin/` holds built binaries and is gitignored; Justfile
  `build-<plugin>` recipes stage them. `agents.json` is the registry; `scripts/check-package-versions.sh` lists
  crate-manifest pairs by hand; `just footprint-regen` ratchets budgets; HACK.md documents the rest.
- MCP stdio servers spawned by Claude Code inherit Claude's environment (measured on a live `rtk-cc-mcp` child).
  The OpenCode-hosted front runs under the SERVICE process, not under the TUI, so it does not see TUI env.
- Antigravity wiring, marketplace and bundle-smoke gates plus footprint support for Antigravity manifests exist only
  on `feat/antigravity-parity` (12 commits ahead of main); this branch builds on it.

## Decisions made by the owner (2026-10-18)

- Shape: MCP / binary plugin, shipped to all four hosts.
- Vehicle: build the binary now; the driver skill is written first and documents the raw-CLI fallback.
- Transport: CLI wrapper for everything, behind a `Transport` trait.
- Tool surface: curated typed tools plus one gated generic `opencode_api` tool.
- Name: `opencode-ctl-mcp`.
- Policy when the worker skips something: send it back to finish (re-prompt the same session with the specific
  finding), do not take over. Measured: 37 s fix on the spike.
- Process: full ceremony (brainstorm, builder check, adversarial panel review, plan, subagent-driven development,
  agy capstone, test audit).

## Decisions co-signed with agy (AGY-FIRST rounds 1 to 4, all ALIGNED; every claim re-measured by the author)

- Default permission policy: inherit the host's OpenCode config (`permissions: null`), per-call override on
  `session_create`, settings key `permission_policy`.
- `session_wait`: run the wait route as a child with a deadline, poll the session's permission list every `poll_s`
  seconds, return `awaiting_permission` when non-empty. `session_prompt` defaults to `steer`.
- `service_stop`, `service_restart`, `session_delete` require `confirm: true`; the first two are refused on the
  OpenCode host. The escape hatch resolves operation ids to HTTP methods through the OpenAPI document and blocks
  mutations unless `allow_mutation: true`; streaming routes are deny-listed (later rounds replaced the mutation
  rule with a closed allow-list).
- `session_create` defaults `directory` to the host working directory.
- Preflight: `doctor` skill only. Settings keys: `opencode_bin`, `wait_timeout_s`, `poll_s`, `permission_policy`,
  `output_max_bytes`, `server_url`, `default_model`; author added `wait_max_s` (slice cap, default 300) after the
  builder check, and `stall_s` (90), `max_turn_s` (1800), `body_max_bytes`, `permissions_locked` (false), `call_timeout_s` (30) after the
  panel.
- Launcher: the plugin owns the pairing contract and ships a minimal launcher; clavity may implement the same
  contract later. Pairing travels as environment variables with a file fallback. Shared background service.
  Linux terminals (mate, gnome, xfce `--tab -e`) and Windows Terminal (`wt`) in v1. Lifecycle desync between the
  two tabs is the real hazard: the contract carries exit sentinels in both directions.
- Branch: from `feat/antigravity-parity`.

## Design

### Layout

```
shared/opencode-ctl/                      engine: lib crate `opencode_ctl`
  src/lib.rs, cli.rs (dispatch: serve [--host h] | pair | attach | gc | install-launcher | emit-skill <name> |
  doctor-json | --version; no arguments = usage on stderr, exit 2),
  src/transport.rs (trait + CliTransport), src/openapi.rs (operation table, gating),
  src/tools/*.rs (one file per tool group), src/compact.rs, src/pairing.rs, src/launcher/{mod,posix,windows}.rs,
  src/settings.rs, skill/ (canonical driver + doctor skills, emitted per host like ghidra-mcp's `skill/`),
  tests/ (protocol test with rmcp in-process client; unit tests), tests/bin/mock_opencode.rs
claude-code/plugins/opencode-ctl-mcp-cc/     .claude-plugin/plugin.json, .mcp.json, src/bin/mcp.rs, src/lib.rs,
                                             skills/{opencode-ctl-driver,opencode-ctl-doctor}/SKILL.md, examples/*.local.md, tests/e2e.rs, tests/skill_emit.rs
qwen/plugins/opencode-ctl-mcp-qwen/          qwen-extension.json (mcpServers), QWEN.md, src/main.rs, skills/,
                                             tests/e2e.rs, tests/skill_emit.rs
antigravity/plugins/opencode-ctl-mcp-agy/    plugin.json, hooks.json (`{}`: v1 ships no hook, but
                                             scripts/bundle-agy-plugin.sh:229 requires the file and deletes the zip
                                             without it, while every other check tolerates its absence, so the first
                                             tag-time bundle job would be the first thing to fail), mcp_config.json
                                             (`{command, args}`, a bare name resolved on PATH as the siblings do),
                                             src/bin/mcp.rs, skills/, tests/e2e.rs, tests/skill_emit.rs
opencode/plugins/opencode-ctl-mcp-opencode/  plugin.ts (a hook plugin with `id: "opencode-ctl-mcp-opencode"`, the
                                             directory name, and the shared `@opencode/plugin` pin, both enforced by
                                             scripts/check-opencode-wiring.sh), opencode.jsonc (mcp.servers.opencode-ctl
                                             with an ABSOLUTE `command`, args `serve --host opencode`, environment
                                             OPENCODE_CTL_HOST=opencode), package.json, README.md, INSTALL.md (the
                                             bundle is SOURCE ONLY: scripts/bundle-opencode-plugin.sh ships no binary
                                             and scripts/smoke-opencode-bundle.sh rejects a `bin/`; INSTALL.md fetches
                                             `opencode-ctl-cc-mcp` from the cc release zip, names the minimum cc
                                             version that knows `--host`, and says why a relative `command` fails),
                                             skills/, tests/*.test.ts
```

Binary names follow the siblings: `opencode-ctl-cc-mcp`, `opencode-ctl-qwen-mcp`, `opencode-ctl-agy-mcp`; the
OpenCode front stages `opencode-ctl-cc-mcp` (as rtk does). Registry keys: `opencode-ctl-mcp-cc`, `-qwen`, `-agy`,
`-opencode`; published name `opencode-ctl-mcp` via `agents.json` `publishedNames`.

### Transport

```rust
// Used as a generic parameter (`Engine<T: Transport>`), never as `dyn Transport`: a bare `async fn` in a trait is
// not dyn-compatible and the workspace carries no `async_trait` dependency. Each method is WRITTEN in the
// `fn ..() -> impl Future<Output = ..> + Send` form (shown as `async fn` below for brevity) because rmcp's tool
// handlers and `tokio::spawn` need `Send` futures, which a bare `async fn` in a trait does not promise.
pub trait Transport: Send + Sync {
    async fn call(&self, op: &Operation, params: &Params, body: Option<&Value>, deadline: Duration) -> Result<Value, TransportError>;
    async fn lifecycle(&self, verb: Lifecycle) -> Result<String, TransportError>;   // start | stop | restart | status
    async fn wait_idle(&self, session_id: &str, deadline: Duration) -> Result<WaitOutcome, TransportError>; // Idle | DeadlineHit
    async fn fetch_openapi(&self) -> Result<Value, TransportError>; // the ONE call that is not an operation id
}
```

`CliTransport` spawns `opencode api <operationId> [--param k=v ...] [-d <json>] [--server <url>]` with
`tokio::process::Command`, STDOUT REDIRECTED TO A TEMP FILE under a 0700 per-process directory (never a pipe: the pipe truncates at 256 KiB,
measured; the file is deleted right after it is parsed, and `gc` sweeps per-process directories whose owner pid is
gone), every non-wait call bounded by `call_timeout_s`, stderr captured for the error message, a kill-on-drop deadline, the child in
its own process group (Windows: job object) so the deadline kills grandchildren too. It never reads
`service.json`; the CLI resolves auth, port and relocation. The operation id is always passed as an id (never the
`METHOD /path` form), with exactly one exemption, `fetch_openapi`, which runs `opencode api GET /openapi.json` and
nothing else; a `--param` NAME must be a parameter the operation table declares, compared on the part before
any `[` (so `location[directory]` matches the declared deepObject `location`); a VALUE is passed as one argv element
(argv is not a shell, so no quoting layer exists or is needed) and may contain anything except NUL and newline; `-H`
is never forwarded. A body larger than `body_max_bytes` (default 100 000; Windows
command lines cap near 32 KB so the Windows default is 24 000) returns `BODY_TOO_LARGE` with the hint to shorten the
prompt or write it into the workspace and reference the path (`files` attachments are deferred). Error mapping, in order: spawn failure = `OPENCODE_NOT_FOUND`; exit 1 with stdout JSON carrying
`_tag` = `SessionNotFoundError` -> `SESSION_NOT_FOUND`, `UnauthorizedError` -> `AUTH_FAILED`, other tags ->
`CLI_ERROR` with the tag and message; exit 1 with stderr containing `rejected the password` or `requires a password`
-> `AUTH_FAILED`; exit 1 with empty stdout and a `[cause]:` line containing `Operation not found` ->
`UNKNOWN_OPERATION` (for any `UnknownError` the `[cause]:` line is the message, not the first line); stderr containing `ECONNREFUSED` or a reachability probe (`opencode service status`) failing
-> `SERVICE_DOWN`; anything else -> `CLI_ERROR` with the first stderr line only (never a stack trace). Empty stdout with
exit 0 on an operation whose declared success status is 204 is SUCCESS (`{ok: true}`); on a 200 operation it is
retried once for GET operations only (never a mutation), then `CLI_ERROR` with `details.reason = "unparseable
stdout"` and, for a mutation, `details.outcome = "unknown"` with the hint to check the inbox and messages before
repeating it. `InvalidRequestError` and `InvalidCursorError` map to `INVALID_REQUEST`; `ServiceUnavailableError` (503,
declared on the wait and `vcs.diff` routes) to `SERVICE_DOWN`; `UnknownError` (500) to `CLI_ERROR`; the 409 tags
`ConflictError`, `SessionBusyError`, `FormAlreadySettledError` AND the 404 tags `PermissionNotFoundError`,
`FormNotFoundError` (the request vanished before the reply) to `CONFLICT`. Every tool
validates a session id against `^ses_[A-Za-z0-9]{26}$` before spawning (`SESSION_NOT_FOUND` with the hint).
Envelope keys beside `data` and `cursor` (for example `location` on `vcs.status`) are kept under `meta`.
When `server_url` is set, the CLI needs credentials for that server: `OPENCODE_SERVER_PASSWORD` is passed through
from the environment, and `doctor` reports when `server_url` is set without it. The CLI reads `OPENCODE_PASSWORD`
BEFORE `OPENCODE_SERVER_PASSWORD` (measured in the binary), so `doctor` flags a set `OPENCODE_PASSWORD` too. The
local service rotates its password on every restart, so `server_url` is for REMOTE servers only: for the local
service it stays unset and the CLI resolves the password itself; an `AUTH_FAILED` while `server_url` is set carries
the hint "password rotated? unset OPENCODE_CTL_SERVER for the local service, or re-export the new password", and
the skill tells the user to re-run `attach` after a `service_restart` because the paired TUI tab lost its server. `wait_idle` runs `opencode api
experimental.session.wait --param sessionID=<id>` with the deadline; exit 0 = idle, killed by deadline =
`DeadlineHit`, anything else = error. Binary resolution: settings `opencode_bin`, else `opencode` on PATH, else
`~/.opencode/bin/opencode` (Windows: `opencode.exe` and `%USERPROFILE%\.opencode\bin`). HTTP is a second impl
only when a tool needs streaming or the polling task's measured CPU time (not wall time) exceeds 25 % of one
core over ten real turns. Measured 2026-10-18 (three `server.info` calls, background load uncontrolled): 0.17 to
0.21 s wall and 0.26 to 0.28 s CPU (user+sys) per spawn, against 0.53 s wall under the spike's load. The spawn
arithmetic under session_wait gives 0.37 to 0.47 spawns per second at the shipped 120 s slice (10 to 13 % of one
core at 0.27 s CPU each) and 0.72 to 0.96 per second at a 25 s slice (19 to 26 %, straddling the trigger), so the
default slice length is what keeps polling under the line. This is a scheduled MEASUREMENT on the first ten real
turns, not a prediction that HTTP will not be needed; if it fires, the parked HTTP transport is the first v1.1 item.

The CLI's JSON output is `{"data": ..., "cursor"?: {...}}` for list/get operations and a raw object or array for
some (`server.info`, `location.get`, `config.get`). The engine unwraps `data` and KEEPS `cursor` as the tool's
`next_cursor`; error JSON (a top-level `_tag`) is never treated as data.

### Pairing contract (owned by this plugin)

Environment, read ONCE by the MCP server at spawn (inherited from Claude) and set by the launcher:

| Variable | Meaning |
|---|---|
| `OPENCODE_CTL_SESSION` | the paired session id; tools with an optional `session_id` default to it |
| `OPENCODE_CTL_DIR` | the paired workspace directory; `session_create` / `session_list` default to it, else host cwd |
| `OPENCODE_CTL_SERVER` | optional server URL passed as `--server` to every CLI call AND to the TUI wrapper; empty = background service. This is also the env form of the settings key `server_url` (second documented naming exception) |
| `OPENCODE_CTL_PAIR` | pair id (`^[0-9a-f]{12}$`); names the pair record below; invalid = `INVALID_ARGUMENT` at startup, logged, unpaired |
| `OPENCODE_CTL_HOST` | `opencode` only, set by the OpenCode front (see Host-specific behaviour) |

Pair directory `~/.opencode-ctl/` (POSIX: mode 0700, every file 0600, wrappers 0700; Windows: the directory inherits the profile's ACL and modes are not applied; every write goes to a temp name then
`rename`). Files of one pair, all named `pair.<pair-id>.*`:

| File | Writer (once) | Content |
|---|---|---|
| `pair.<id>.json` | launcher, once (and `attach`, which rewrites only `generation` and `tab`) | `{"pair_id", "session_id", "directory", "server_url", "created": <epoch ms>, "controller": "claude" \| "qwen" \| "agy" \| null, "generation": <n>, "tab": <bool>}` |
| `pair.<id>.dead-seen` | `gc`, the first time every side reads `dead`, `none` or unknown-past-1-h; deleted by `gc`, `attach` or a re-bound MCP server (Launcher) when any side reads `alive` or `starting` | `<epoch ms>` |
| `pair.<id>.g<n>.wrapper.sh` / `.ps1` | launcher or `attach` | the tab script for generation `n` |
| `pair.<id>.g<n>.tui-pid` | the wrapper, at its start | one line `<pid>:<boot_id>:<starttime>` |
| `pair.<id>.g<n>.tui-exited` | the wrapper's exit trap | `<epoch ms>` |
| `pair.<id>.claude-pid` | launcher, right after spawning the controller | one line `<pid>:<boot_id>:<starttime>` |
| `pair.<id>.claude-exited` | launcher, when the controller returns | `<epoch ms>` |

Liveness line format on every platform: `<pid>:<boot_id>:<starttime>`. POSIX wrapper (bash): pid `$$`; `boot_id` =
`/proc/sys/kernel/random/boot_id` (macOS: `sysctl -n kern.boottime` epoch); `starttime` = field 22 of
`/proc/$$/stat` parsed after the last `)` (macOS: `ps -o lstart= -p $$` as epoch). Windows (`.ps1`): `$PID`,
`(Get-CimInstance Win32_OperatingSystem).LastBootUpTime` as epoch SECONDS truncated to the minute,
`(Get-Process -Id $PID).StartTime` as FILETIME. The Rust side computes the Windows boot time as now minus
`GetTickCount64`, truncated the same way, and the two boot values compare within 120 s (clock adjustments drift
them; a reboot moves them by far more); `starttime` compares exactly on every platform. The launcher writes the
controller line the same way from its child handle. A side is `alive` only when the pid exists AND its boot value
and start time match the sentinel; a recycled pid after a reboot therefore reads `dead`.

Liveness VALUE for each side is one of `alive | dead | starting | unknown | none`:
- `none`: the pair has no such side: the record's `tab` is false (`--no-tab`, or no terminal was found at launch), or an
  `attach` record, whose `controller` is null;
- `starting`: the pid sentinel is not yet written and that generation's wrapper file is younger than 30 s (`attach`
  creates a fresh one, so a re-opened tab is `starting`, not `unknown`; the record's `created` is not used here);
- `unknown`: no pid sentinel after 30 s (the tab never opened), or the sentinel is unparsable;
- `dead`: an exited sentinel exists, or the pid check fails;
- `alive`: otherwise.
`tui_*` reads the generation named in the record; when that generation has no `tui-pid` sentinel yet, it falls back
to the highest generation that HAS one (a failed `attach` tab never hides a live older tab). `attach` creates the
`g<n+1>` files with exclusive create (`O_EXCL`), so two concurrent attaches cannot both claim `n+1`, and deletes
the files of generations older than `n`. A tab re-opened with `attach` is thus seen by the controller that still
holds the same `OPENCODE_CTL_PAIR`. The file is
re-read on every `pair_status`, `service_status` and `session_wait` call. Identity (session, directory, server)
comes from `OPENCODE_CTL_SESSION` / `_DIR` / `_SERVER` when they are set; when only `OPENCODE_CTL_PAIR` is set, the
record supplies them: that is the file fallback the Decisions name (`pairing_env_then_file_then_none`), and it is
what lets `resume` and a hand restart work from the pair id alone. `created` is the fallback age for `gc` when no sentinel exists; `server_url` is read by the
wrapper writer and by `doctor`.

`gc` (also run by `pair` right after the service is confirmed up, never before): a side `unknown` for more than
1 h counts as dead; a side `unknown` for less than that keeps the pair alive. The first time every side of a pair
reads `dead`, `none` or unknown-past-1-h, `gc` writes `dead-seen`; whenever any side reads `alive` or `starting`,
`gc` (and `attach`) DELETE `dead-seen`, so a pair revived by `attach` or by a re-bound controller (Launcher) starts
its grace again. A pair is deleted (every `pair.<id>.*` file) only when `dead-seen` is older than 24 h AND every
side STILL reads `dead`, `none` or unknown-past-1-h at deletion time AND the session is absent from `session.active`, so a controller killed a
minute ago is not collected by the next `pair`, and a pair re-attached after a day is not collected either (an idle
worker is absent from `session.active` by definition, so that check alone never protected it). When the
service is down, `gc` cannot read `session.active` and deletes nothing. Sentinel files without a `pair.<id>.json`
are deleted when older than 24 h. `gc` also deletes `opencode-pair.exe.old` under `%LOCALAPPDATA%\opencode-ctl\`,
stale per-process stdout directories (see Transport), and log files beyond the retention rule (see Logging).

### Tools (all return JSON text; every description states "spends model tokens: yes/no", meaning WORKER tokens; every result also costs CONTROLLER context, at most `output_max_bytes`, and a wait slice is one such result)

Common error shape: `{"error": {"code": CODE, "message": "...", "hint": "...", "details"?: {...}}}` (`details` carries
structured extras such as `reason` or `outcome: "unknown"`). Codes, closed list (a builder adding one adds it here):
`OPENCODE_NOT_FOUND`, `SERVICE_DOWN`, `AUTH_FAILED`, `SESSION_NOT_FOUND`, `NO_SESSION` (no `session_id` and no
pairing), `PAIR_NOT_FOUND`, `INVALID_ARGUMENT` (a tool argument, id, `--param` name or value, or launcher value fails
validation), `INVALID_REQUEST` (the server answered 400, e.g. a rejected diff range), `REFUSED_ON_HOST`,
`CONFIRM_REQUIRED`, `PERMISSIONS_LOCKED`, `MUTATION_BLOCKED` (allow-listed id without the flag), `BLOCKED_OPERATION`,
`NON_JSON_RESPONSE`, `UNKNOWN_OPERATION`, `BODY_TOO_LARGE`, `CONFLICT` (a busy session, an already-settled form, or a
permission or form that vanished before the reply; the permission reply route declares NO 409, its vanished case is
a 404 `PermissionNotFoundError`, so `CONFLICT` is the engine's mapping, not a status code), `DEADLINE` (a non-wait CLI call hit `call_timeout_s`; for a mutation it
carries `details.outcome = "unknown"`), `IO_ERROR` (a settings, log or pair file could not be written),
`LAUNCHER_CONFLICT`, `CONTROLLER_NOT_FOUND` (the launcher could not resolve the controller executable),
`CLI_ERROR`. `timeout` is a `session_wait` STATE, not an error. `pair_status` with no record
for the env pair id returns `PAIR_NOT_FOUND`.

| Tool | Arguments | Returns | Tokens | Guards |
|---|---|---|---|---|
| `service_status` | none | `{running, urls: [..], pid, version, reachable, paired: {pair_id, session_id, directory, server_url, tui, claude} \| null}` (liveness values per the pairing contract) | no | |
| `service_start` | none | same as status after `opencode service start` (idempotent) | no | |
| `service_stop` | `confirm: bool` | `{stopped: true}` | no | `confirm` required; refused when host is opencode or unknown; description: "stops the daemon for EVERY session on this machine; to stop one turn use session_interrupt" |
| `service_restart` | `confirm: bool` | status | no | same |
| `session_list` | `directory?`, `all?: bool`, `search?`, `limit?=20`, `cursor?` | `{sessions: [{id, title, directory, outcome, updated, idle, tokens, cost}], next_cursor}` | no | default directory = paired dir or cwd; `all` drops the filter; fields are exactly what the list route returns (no N+1 `session.get`) |
| `session_create` | `title?`, `directory?`, `agent?` (`PERMISSIONS_LOCKED` under the lock; default = protected setting `default_agent`), `model?` (`provider/model[#variant]`, split on the first `/` and `#` into `Model.Ref`), `permissions?: Rule[]`, `parent_id?` | `{session_id, directory, agent, model}` | no | directory default as above; model default = settings `default_model`; permissions default per `permission_policy`; a per-call `permissions` is refused with `PERMISSIONS_LOCKED` when settings `permissions_locked: true` |
| `session_get` | `session_id?` | `{id, title, directory, outcome, agent?, model?, tokens, cost, time, pending_permissions, pending_forms}` | no | |
| `session_prompt` | `session_id?`, `text`, `delivery?='steer'` (`files` attachments are deferred out of v1; the route's `PromptInput.FileAttachment` shape is parked) | `{message_id, delivery}` and the hint "call session_wait with after=<message_id> on EVERY slice" | YES | `message_id` is the stored user message id (verified for steer; for `queue` it is the inbox item's id and resolution rule (i) handles it, to confirm); steering into a busy turn joins that turn (route description), so `steer` is the default in every state and `queue` is opt-in |
| `session_wait` | `session_id?`, `after?: message_id`, `timeout_s?` (default settings `wait_timeout_s`, else 120), `poll_s?=10`, `hard?: bool=false` | the result object defined under "session_wait mechanics": `state` is one of `idle \| failed \| interrupted \| running \| awaiting_permission \| awaiting_input \| stalled \| budget_exceeded \| timeout` | no | ONE SLICE per call; `running` means "call again with the same `after`"; `timeout` only with `hard: true` |
| `session_messages` | `session_id?`, `limit?=20`, `cursor?`, `order?='desc'`, `type?` (one of the route's enum values), `reasoning?=false`, `tools?='summary'` (`none\|summary\|full`) | `{messages: [{id, type, outcome?, text, tools: [{name, status}]}], next_cursor, truncated}` | no | compaction (below) |
| `session_changes` | `session_id?`, `from?: message_id`, `to?: message_id`, `paths?: [workspace-relative exact paths]`, `patches?: bool=true`, `context?: int` | `{directory, from, to, files: [{file, status: added\|modified\|deleted (the route's whole `FileDiff.Info` enum), additions, deletions, patch?, patch_truncated?}], omitted_patches: n, omitted_files: [file names]}` from `session.diff` (`from`/`to` are user message ids; DEFAULT = the whole session: `from` = the first `user` message, `to` = the newest `user` message, found with two `session.message.list type=user limit=1` reads, `order=asc` and `order=desc`; the route requires `to` to be a user message; in a paired session that range includes the human's turns, so the skill passes `from=<its prompt id>` for its own turn) | no | `paths` is an ENGINE-SIDE filter applied after the fetch (the route has no such parameter); the cap drops patches first (`omitted_patches`), then, if the file list alone exceeds it, lists the omitted FILE NAMES in `omitted_files` so the skill can page with `paths`; a single patch larger than the cap is cut with `patch_truncated: true`; `patches: false` fetches names and counts only and sets `omitted_patches` to the file count; any of `omitted_patches > 0`, `patch_truncated`, non-empty `omitted_files` is a signal the skill must act on (page by `paths`, or read the file itself with the host's own file tools), never a pass; a range the route rejects (location change, non-user `to`) is `INVALID_REQUEST` |
| `workspace_status` | `directory?` | `{directory, status: [{path, kind}]}` from `vcs.status` with `location[directory]` | no | directory-wide view; a non-git directory returns `{vcs: null}` |
| `session_interrupt` | `session_id?`, `resume?: bool=false` | `{interrupted: bool}` (false = the idle no-op) | no | `resume: true` lets pending steering input continue; queued prompts stay parked either way |
| `session_delete` | `session_id`, `confirm: bool` | `{deleted: true}` | no | `confirm` required; never defaults to the paired session |
| `permission_reply` | `session_id?`, `request_id`, `decision: once\|always\|reject`, `message?`, `confirm?: bool` | `{replied: true, saved: bool}` (the tool re-reads the request before replying; `saved` = decision was `always` and the request's `save` list was non-empty; the route itself answers 204; a vanished request is `CONFLICT`) | no | `always` persists a saved rule beyond the session (the request's `save` list says what), requires `confirm: true`, and is `PERMISSIONS_LOCKED` when settings `permissions_locked: true` (a standing rule widens the worker's authority beyond this session, which is what the lock promises to prevent); the driver skill defaults to `once` |
| `form_reply` | `session_id?`, `form_id`, `answer` | `{replied: true}` | no | the worker's question channel; `session_wait` reports `awaiting_input` with the form |
| `saved_permissions` | `project_id?`, `remove_id?`, `confirm?: bool` | `{rules: [...]}`; with `remove_id` + `confirm` removes one | no | inspect and revoke what `always` created; `project_id` is the route's filter |
| `opencode_operations` | `filter?` | `{operations: [{id, method, path, summary, mutating}]}` from the cached OpenAPI document | no | |
| `opencode_api` | `operation`, `params?`, `body?`, `allow_mutation?=false` | compacted response | depends | gating (below) |
| `pair_status` | none | the pair record plus `{tui, claude}` liveness and `generation` | no | |

Twenty tools. `pair_status`, `session_changes`, `workspace_status`, `form_reply` and `saved_permissions` are
additions to the co-signed list (the owner can cut them): the first because the launcher's lifecycle sentinels
need a reader, the next two because the spike proved verification by diff is the whole point, the last two because
the panel showed forms are a second stall channel and `always` creates standing authority that needs a reader.
`confirm` flags are FRICTION, not a guard: the same model sets them. The guards that hold are the host refusal, the
escape-hatch deny-list, `permissions_locked`, and the driver skill never recommending the destructive tools.

### session_wait mechanics (slice protocol)

Hosts bound a single MCP tool call (the OpenCode front's sibling manifests set `timeout.execution: 30000` ms; the
other hosts have their own defaults), and a worker turn took 133 to 202 s on the spike. So one `session_wait`
call is a SLICE, never the whole wait: it returns `running` when its slice ends and the driver skill calls it again.
The OpenCode front's `opencode.jsonc` sets `timeout.execution` to `wait_max_s + 30` seconds (330000 ms), which is why
`wait_max_s` is not workspace-settable.

Result object. Always present: `state`, `elapsed_s` (this slice), `turn_s` (server time since the start of the
current non-idle stretch, see below; `null` only when the session has no user message), `inbox_pending` (bool),
`tokens` (`{input, output, reasoning, cache_read}` from `session.get`, or `null` when that read fails),
`last_assistant_text` (newest `text` part, or `null`). Present whenever non-empty, in ANY state: `pending_permissions`
and `pending_forms` (each form entry `{id, title, fields}` where every field is the route's `Form.Field` object
reduced to `key, type, title, description, required, hidden, when` plus its per-type extras; the skill answers
visible, required fields only); these two arrays are exempt from the output cap's array cut (their entries'
`metadata` is shortened instead). Present only in their state: `outcome` (`idle | failed | interrupted`), `parked_s`
(when `inbox_pending`), `verify_hint` (on `idle`: the `session_changes` call for this turn), `concurrent_turn: <user message id> | null`
(on the three terminal outcomes, rule 3.1). Present whenever the
server is paired: `pair: {tui, claude}` with the liveness values above. "Server time" is the engine's clock: every timestamp compared is server-stamped
epoch milliseconds and the service runs on the same machine.

1. Resolve `session_id` (validated); `timeout_s` from arg, else settings `wait_timeout_s`, else 120 (a 2 to 3 minute turn costs the
   controller one or two slices in its context rather than six to eight; the OpenCode front's own manifest allows
   330 s, its siblings' 30 s default does not apply to it), capped at `wait_max_s`; `poll_s` likewise, else 10. Resolve the ANCHOR from the server (nothing is remembered in the
   process): the `after` arg if given, else the newest `user` message (`session.message.list order=desc type=user
   limit=1`; `synthetic` is its own type and never anchors). An explicit `after` is never replaced: a human typing
   into the paired TUI creates a newer user message, and the controller must keep waiting on ITS turn. Its
   timestamp: (i) if the inbox (`session.inbox.list`) holds it, the item's `time` and the state is `running` with
   `inbox_pending: true` and `parked_s`, whatever the message list says; (ii) if the message list holds it, its
   `time.created`; (iii) if neither, the session's newest message `time.created` (a vanished prompt adds no message,
   so the clock keeps running) and the state is `running` until `stall_s`, then `stalled`. Without `after`, a
   session with no user message is `idle`. `turn_s` is measured from the start of the CURRENT NON-IDLE STRETCH: the
   oldest user message that has no `idle` marker after it (or the anchor, whichever is older), so neither a
   re-prompt nor a fresh `after` resets the budget while the worker has not gone idle.
2. Run two tasks concurrently until the slice deadline (`tokio::select`): (a) the wait child, `wait_idle` with
   `min(remaining, 30 s)`; when it returns before the turn is done (an idle marker older than the anchor), sleep
   `poll_s` before restarting it; (b) a poll of the two stall channels only, `session.permission.list` and
   `session.form.list` (two CLI spawns, 0.17 to 0.21 s wall each measured idle, 0.53 s under the spike's load), run once at the slice START, every `poll_s`
   after that, and once at the slice END regardless of `poll_s`. The message, inbox and `session.active` reads
   happen once per wait-child return and once at the slice end, never per poll; the anchor is resolved once per
   slice. The message read is UNFILTERED: `idle` is not a value of the list route's `type` filter (measured: 400
   `InvalidRequestError`) and the route has no after-id parameter, so the engine reads `session.message.list
   order=desc limit=50` and follows `cursor` until it has seen the anchor itself or a message older than the anchor's
   timestamp (then every message after the anchor has been seen). There is NO page cap: a turn of N messages costs
   `ceil(N/50)` spawns per read, bounded only by the slice deadline (a read cut off by the deadline reports
   `running` and the next slice reads again; the first idle marker after the anchor is found on the way back, never
   by giving up). The round-6 text asked the skill for "a newer `after`" on exhaustion; no such user message exists
   inside a single turn, so that rule was wrong and is withdrawn. The `session_messages.type` enum is the route's (no
   `idle`); the wait's marker search filters idle markers in the engine, while `session_messages` still returns them
   as `{type: idle, outcome}` rows. Spawn arithmetic per slice: `2 (anchor + session.get) + 2 x polls + wait_children
   x (3 + pages)` where polls = slice start, every `poll_s`, and slice end, and each wait-child return costs the
   child plus inbox, `session.active` and `pages` message reads. At a 25 s slice: 4 polls, up to 2 children, 1 to 4
   pages = 18 to 24 spawns, 0.72 to 0.96 spawns per second. At the shipped 120 s slice: 13 polls, up to 4 children =
   44 to 56 spawns, 0.37 to 0.47 per second, because the fixed per-slice costs amortise; see the Transport trigger.
3. Terminal-state evaluation, in this ORDER, first match wins:
   1. the FIRST `idle` message created after the anchor exists: `idle | failed | interrupted` from its `outcome`
      (the first marker after the anchor is the controller's own turn even if the human ran a later one). For an
      anchor delivered with `queue` ONLY, the marker also needs at least one `assistant` message between the anchor
      and itself (a parked prompt must not borrow the busy turn's marker); for `steer` anchors rule (i) already
      reports a parked prompt as `running`, and a turn that ended with no assistant message, succeeded or failed, is
      terminal rather than sitting in `running` until `max_turn_s` (whether an early failure stores an assistant
      message is to confirm). On a terminal state `turn_s` ends at the marker's `time.created`, and
      `concurrent_turn` carries the id of the newest `user` message after the marker (null when none) when such a
      message exists or the session is in `session.active`: the human started a turn after ours and is editing the
      workspace the controller is about to verify. The skill then waits PASSIVELY with `after` = that id: it never
      answers that turn's permissions or forms, never interrupts it, reports what is pending to the user, and
      verifies its own turn only once the human's has ended;
   2. a pending permission: `awaiting_permission`; a pending form: `awaiting_input`;
   3. `turn_s > max_turn_s` (default 1800): `budget_exceeded`, even if the session is in `session.active`;
   4. session absent from `session.active`, the newest message is not an `idle` marker after the anchor, and no
      message has arrived for `stall_s` (default 90): `stalled` (covers both a prompt that produced nothing and a
      loop that died mid-turn after emitting messages);
   5. otherwise `running`; at the slice deadline `running` (or `timeout` when `hard: true`).
4. If the operation table has no `experimental.session.wait` (route removed upstream), task (a) is skipped and the
   poll alone drives the state machine; `doctor` reports the missing route. ANY mapped error from either task ends the slice with
   that error (a wait child that exits non-zero goes through the error mapping and then the reachability probe like
   every other call), with one exception: a failed stall-channel poll is retried at the next tick and ends the slice
   only when two consecutive polls fail. A service restart kills the in-flight turn: the worker never resumes it, the
   wait reports `stalled` after `stall_s`, and the skill re-prompts. Step 3 is evaluated only after step 1 has assigned the anchor
   and its timestamp.
Measured on the spike: 5 to 7 wait children of 30 s per 2 to 3 minute turn; each CLI spawn 0.53 s wall under that
load (0.17 to 0.21 s idle, Transport).

### Compaction (`compact.rs`)

- Message list: keep `id`, `type`, `time.created`; `text` parts concatenated; `reasoning` dropped unless asked;
  `tool` parts reduced to `{name, status}` (`summary`), omitted (`none`), or kept whole (`full`); user messages keep
  their text. Measured input: 68 KB raw for 18 reasoning, 11 text, 24 tool parts in one short session.
- Every tool result passes a final cap of `output_max_bytes` (default 16384) and is ALWAYS valid JSON: arrays are
  cut at an element boundary, then the longest string fields are shortened with a `...[+N bytes]` tail, and the
  result carries `"truncated": true` with the original byte count. Where the route offers a filter (`type` on the
  message list, `limit`, `paths` on `session_changes`) the fetch is narrowed BEFORE compaction.
- The escape hatch applies a depth cap of 6 and the same byte cap. `pending_permissions` and `pending_forms` are
  exempt from the array cut (see the wait result). The last `session.diff` fetch is cached per (session, from, to)
  for 10 s so paging by `paths` does not re-read a multi-megabyte diff per call.

### Escape hatch gating (`openapi.rs`)

- The OpenAPI document (280 KB live) is fetched once per process via `opencode api GET /openapi.json` and reduced
  to the operation table `operationId -> (method, path, declared parameters, success status, response media type,
  summary)` (14 KB). A built-in copy of that TABLE only (`openapi.table.json`, generated from v2.0.32 and committed
  with the version it came from) is the fallback when the live fetch fails. Live always wins; the cache is
  invalidated when any tool observes a different service pid or version (`server.info` is re-read before every
  MUTATING hatch call, one extra spawn, and after any `SERVICE_DOWN` recovery); every `opencode_operations` and
  `opencode_api` result carries `source: live | snapshot(v2.0.32)` and, when the snapshot is used while
  `server.info.version` differs, a `warning`. A mutating operation is never run from the snapshot when the live
  document could not be fetched (`SERVICE_DOWN` first).
- `operation` accepts an operation id ONLY (the `METHOD /path` form is `INVALID_ARGUMENT`). An operation is runnable
  only when it declares a 204 success (result `{ok: true}`, 46 operations) or a 200 with `application/json`;
  anything else is `NON_JSON_RESPONSE`.
- GET DENY-LIST (`BLOCKED_OPERATION`): `server.connect` (redeems a pairing code into a credential), `fs.*` (raw
  file bytes from anywhere on the host, listings), `session.log` (`text/event-stream`), `event.subscribe`,
  `shell.*`, `pty.*`, `persistentPty.*`, `server.experimental.persistentPty.*` (terminal buffers; measured:
  `shell.output`, `persistentPty.read` and `.snapshot` return buffers as JSON, and `persistentPty.connect` is a GET
  with a JSON 200 that opens a terminal connection), `credential.*`, `config.*`, `mcp.*` (server configs carry an
  `environment` map whose VALUES the key-name redaction cannot see), `integration.*` (auth-attempt status and
  output), `experimental.session.export` (full transcripts). The GET side is open-world by id, so an id the built-in
  TABLE does not know (a route added upstream after v2.0.32) is `BLOCKED_OPERATION` with `details.reason =
  "unknown_to_snapshot"` whatever its live method, until a plugin release ships a regenerated table; the live table
  still supplies method and parameters for every KNOWN id.
- MUTATION ALLOW-LIST, closed, seven ids: `session.compact`, `session.switchAgent`, `session.switchModel`,
  `session.view`, `session.inbox.cancel`, `session.inbox.update`, `session.form.cancel`. `session.fork` was dropped:
  it creates a session outside the `session_create` policy path (its body is `{before?}` only, measured, so it cannot
  set permissions, but whether a fork inherits the parent's ruleset is unverified). An allow-listed id without
  `allow_mutation: true` is `MUTATION_BLOCKED`; every other non-GET id is `BLOCKED_OPERATION` whatever the flag (that
  covers `server.pair`, `rpc.call`, `session.shell`, `session.command`, `session.environment`, `session.generate`,
  `session.permission.reply` (only `permission_reply` may answer, so `always` can never skip `confirm`),
  `session.fork`, `session.update` (its body carries `permissions`), `session.background` (lets a turn end while a
  tool keeps editing), `session.revert.*`, `session.move`, `project.update`, `worktree.*`, `experimental.*` writes,
  `credential.*`, `integration.*`, `plugin.*`, `shell.*`, `pty.*`, `permission.saved.*`, `vcs.init`,
  `location.reload`, `debug.*`). `session.switchModel` is a cost exposure and is accepted; `session.switchAgent` changes
  AUTHORITY (measured: `agent.list` returns a `permissions: [{action, resource, effect}]` ruleset per agent), so it is
  `PERMISSIONS_LOCKED` under `permissions_locked` and accepted otherwise. Ids are matched whole and case-sensitively; `*` is a glob over the rest of the id.
- RESPONSE REDACTION, applied to EVERY tool result: any JSON key whose name contains `header`, `apikey`, `api_key`,
  `token` (but not `tokens`, the usage counter), `secret`, `password`, `auth` or `credential` (case-insensitive
  substring) has its value replaced by `"<redacted>"`, and any string value that parses as a URL has its userinfo
  part removed. Provider, model and agent records carry `headers` fields at schema level; `config.get` carries
  `client_secret`, `env` and `environment` and is deny-listed anyway.
- Unknown ids are `UNKNOWN_OPERATION` with the three nearest ids as a hint. Hatch results are `{source, warning?,
  meta?, data}` and pass the common cap.

### Permission handling

- `session_create` sends `permissions` as given; when absent, `permission_policy` decides: `inherit` sends null,
  `allow` sends `[{action:"*",resource:"*",effect:"allow"}]`, `ask` sends `[{..., effect:"ask"}]`, `deny` sends
  `[{..., effect:"deny"}]`.
- `session_wait` surfaces pending requests and forms; `permission_reply` / `form_reply` answer them; the driver
  skill tells the controller to read the request's `action`, `resources` and `save` before replying, to default to
  `once`, and never to reply `always` without the human's say-so. `permissions_locked: true` in settings is the ONE switch
  for users who want the controller unable to widen the worker's authority; it closes every path that would:
  per-call `permissions` and per-call `agent` on `session_create` (an agent carries its own `permissions` ruleset,
  measured; the worker runs the protected `default_agent` or the server default), `permission_reply` with
  `always`, `session.switchAgent` through the hatch, and a `permission_policy` other than the locking source's value.

### Host-specific behaviour and settings

- Host identity: each per-host binary passes a `Host` constant (`Claude`, `Qwen`, `Agy`) into `cli::dispatch`. The
  OpenCode front stages the cc binary and runs it as `serve --host opencode` (an ARGUMENT in `opencode.jsonc`'s
  command array, which survives any environment handling; `OPENCODE_CTL_HOST=opencode` is set as well). Either
  signal alone is enough: `Opencode` wins whenever the argument OR the variable says so, from ANY constant, and
  nothing can widen a constant. `Unknown` exists only for test builds. On `Opencode` or `Unknown`, `service_stop`
  and `service_restart` return `REFUSED_ON_HOST` (fail closed), and the mutation allow-list contains no service- or
  instance-level operation, so the refusal cannot be bypassed through the hatch. On `Opencode` the `pair`, `attach`
  and `install-launcher` subcommands are not offered (the TUI is already open). A Claude launched from inside an
  OpenCode terminal inherits the variable and is refused too: accepted, fail-closed.
- On `Opencode` the host spawns the front against the SESSION DIRECTORY (re-ghidra `INSTALL.md`: a relative
  `command` fails with `NotFound: ChildProcess.spawn` for exactly that reason), so the host-cwd default of
  `session_create` holds there and the workspace settings file is found. The controller is itself an OpenCode
  session that can never observe its own idle marker, so on `Opencode` every session tool except `session_list`,
  `session_get`, `session_messages`, `session_changes` and `workspace_status` accepts only a session id that a controller
  in THIS directory created with `session_create`: the ids are appended to `~/.opencode-ctl/created.<12 hex of
  sha256(directory)>.json` (0600, atomic rewrite, pruned of ids `session.get` no longer finds on each append) and
  read at startup, so an MCP server restart (`opencode reload`, a crash) keeps the worker reachable; any other id is
  `REFUSED_ON_HOST` with the hint "create the worker session first". The hatch's session-scoped mutations
  (`sessionID` parameter) are gated by the same set on this host. There is no pairing on this host: the service never sets `OPENCODE_CTL_*`.
- Settings sources, in precedence order (first wins): environment; user-level file
  `~/.opencode-ctl/settings.local.md` (written by the user, mode 0600); workspace file
  `.claude/opencode-ctl-mcp-cc.local.md` on Claude Code or `.opencode/opencode-ctl-mcp-opencode.local.md` on
  OpenCode (the re-ghidra precedent; Qwen and Antigravity fronts read env and the user-level file only in v1);
  defaults. Grammar of both files: YAML front matter between `---` fences, one `key: value` per line; integers for
  seconds and bytes, `true`/`false` for booleans, unquoted strings otherwise; an unknown key or an invalid value is
  ignored with a logged warning and the default stands. `default_model` is `provider/model[#variant]`.
- Key table. TUNING keys, settable anywhere, each CLAMPED to a range (a value outside it is clamped, logged, and
  listed by `doctor`): `wait_timeout_s` (120; 5..`wait_max_s`), `poll_s` (10; 2..60), `stall_s` (90; 30..600),
  `max_turn_s` (1800; 60..7200), `call_timeout_s` (30; 5..120, the deadline of every non-wait CLI call),
  `output_max_bytes` (16384; 1024..65536 from a workspace file, up to 1048576 from env or the user file),
  `body_max_bytes` (100000; 1024..100000; 24000 on Windows).
  PROTECTED keys, read ONLY from environment or the user-level file (a workspace file is part of a checkout a
  stranger may have written; a workspace value for these is ignored with a logged warning): `opencode_bin`
  (`OPENCODE_CTL_BIN`), `server_url` (`OPENCODE_CTL_SERVER`), `default_model`, `default_agent` (unset = the server's
  default agent), `permission_policy` (`inherit`),
  `permissions_locked` (`false`), `wait_max_s` (300; 30..600, and CLAMPED to 300 on the OpenCode host, whose
  manifest `timeout.execution` is the static 330000 ms).
  Env names are `OPENCODE_CTL_` + the key upper-cased except the two noted. The lock is STICKY:
  `permissions_locked: true` from ANY protected source wins over `false` from another, and `permission_policy` is
  then read from the source that set the lock; per-call `permissions` on `session_create` is `PERMISSIONS_LOCKED`.
  The environment is trusted because the host trusts it; a project-level env block (Claude Code's
  `.claude/settings.json`) is part of the checkout, so `doctor` flags every protected key whose env value differs
  from the user-level file.
- Logging: `serve` writes a per-instance file log (`tracing-appender`, like the siblings) to
  `~/.opencode-ctl/logs/serve-<pid>.log`, and `pair` / `attach` write `~/.opencode-ctl/logs/pair-<pair-id>.log`;
  stdout is the protocol channel and never receives a log line; retention: `gc` keeps the newest 20 log files and
  deletes any older than 14 days; `doctor` prints the path of the newest log and its last error, and lists every
  settings entry it ignored or clamped (file, key, value).

### Launcher (`pair`, `attach`, `gc`, `install-launcher`; in `cli::dispatch`, so the cc, qwen and agy binaries all have them)

Name-based dispatch: the binary compares the file stem of `argv[0]` (directory and `.exe` stripped) with
`opencode-pair`; when it matches and `argv[1]` is one of
`attach`, `gc`, `install-launcher`, `doctor-json`, `--version`, that subcommand runs; any other `argv[1]` (a
directory, a flag, or nothing) runs `pair`. The long binary name always takes an explicit subcommand.

`pair [directory] [--model <provider/model[#variant]>] [--title <t>] [--controller <claude|qwen|agy>] [--no-tab]
[--dry-run] [-- <controller args>]` (a directory literally named `attach`, `gc` or `install-launcher` must be given
as `./attach` etc. under name dispatch):
1. Ensure the service runs (`opencode service start`, idempotent), then run `gc`.
2. Create the session in `directory` (default cwd, canonicalised, must exist) through the engine's `session_create`
   path, so `default_model` or `--model`, `permission_policy` and `permissions_locked` apply exactly as for the tool;
   title default `pair <basename> <HH:MM>`; `server_url` from settings becomes the record's `server_url`.
3. Select the terminal (the walk described in step 4) and then write `pair.<pair-id>.json` ONCE (pair id = 12 hex
   chars of a uuid v4; an existing id is refused, `INVALID_ARGUMENT`), `generation: 1`, `controller` = `--controller`
   or the binary's host constant (`claude`, `qwen` or `agy`), `tab: true` unless `--no-tab` or no terminal was found.
   The record is never rewritten by `pair`. If
   step 3 or 4 fails (`IO_ERROR`, validation), the session created in step 2 is removed (`session.remove`) so no
   recordless session is left behind.
4. Unless `--no-tab`: write `~/.opencode-ctl/pair.<id>.g1.wrapper.sh` (Windows: `.wrapper.ps1`) that writes
   `~/.opencode-ctl/pair.<id>.g1.tui-pid` (absolute paths baked into the script), runs `opencode <directory> -s
   <session-id> [--server <server_url>]` (with `OPENCODE_SERVER_PASSWORD` passed through when a server URL is set),
   and writes `~/.opencode-ctl/pair.<id>.g1.tui-exited` from its exit trap. Open it in a new tab of the terminal
   the user is in: FIRST the terminal found by walking the parent process names (mate-terminal, gnome-terminal and
   gnome-terminal-server, xfce4-terminal, konsole; the gnome client execs and the shell's ancestor is the server
   process), and ONLY when no parent matches, the first of those four on PATH, in that order; forms
   `mate-terminal --tab -e '<script>'`, `gnome-terminal --tab -- <script>`, `xfce4-terminal --tab -e '<script>'`,
   `konsole --new-tab -e <script>`
   (the `-e` forms take ONE command string that the terminal splits, so the script path is single-quoted; a HOME
   with a space is in the quoting test). On Windows: `wt -w 0 nt -d <directory> <pwsh|powershell.exe> -NoProfile
   -ExecutionPolicy Bypass -File <script.ps1>` (`pwsh` when on PATH, else the stock `powershell.exe`; `doctor`
   reports which; `Bypass` because the stock client policy refuses `.ps1` files). When no terminal was found in step 3,
   print the attach command and continue without a tab (`tui: none`).
5. Spawn the controller CLI (`claude`, `qwen` or `agy`, by `--controller` or the host constant; on Windows the npm
   install is `claude.cmd`, which `CreateProcess` does not find for a bare name, so the launcher resolves the
   executable through `PATHEXT` itself and runs a `.cmd` via `cmd.exe /c`) as a CHILD with inherited
   stdio in the current terminal, with `OPENCODE_CTL_SESSION`, `OPENCODE_CTL_DIR`, `OPENCODE_CTL_SERVER` and
   `OPENCODE_CTL_PAIR` set; write `claude-pid` from the child's pid, boot id and start time; block on the child.
   When it exits: write `claude-exited`; post a display-only notice into the worker session (`session.synthetic`
   with `resume: false`, no model turn; a 409 is retried once with `delivery: queue`; failure logged, never fatal);
   print "worker session <id> is still running; attach with `opencode-pair attach <pair-id>`". RE-BIND ON SPAWN: an
   MCP server that starts with `OPENCODE_CTL_PAIR` set while `claude-exited` exists OR `claude-pid` is absent,
   unparsable or names a dead process (a SIGKILLed launcher or a closed terminal never writes `claude-exited`)
   deletes `claude-exited`, deletes `dead-seen`, and writes `claude-pid` from its PARENT process (the controller that spawned it; measured on Claude
   Code: the MCP child's parent is `claude`), so a controller restarted by hand with the same environment reads
   `alive` again instead of `dead` forever; it never re-binds over a controller that reads `alive`. `attach` reopens
   only the tab; `resume <pair-id>` (below) re-launches the controller. On exit the launcher also prints the four
   `export OPENCODE_CTL_*=...` lines (Windows: `$env:` lines) so a hand restart has the values.
   A controller executable that cannot be resolved is `CONTROLLER_NOT_FOUND`: the record and the tab stay (the
   worker is usable from the tab) and the launcher prints the `attach` and `resume` commands.
`--dry-run` prints every command it would run and writes nothing; the launcher's tests use it.

`resume <pair-id>`: reads the record, sets the four `OPENCODE_CTL_*` variables from it, and runs step 5 only (the
controller named by the record's `controller`, or `--controller`); the tab is untouched. This is how a controller is
restarted after an exit, a crash or a reboot without retyping the environment.

`attach <session-id | pair-id>`: with a pair id, reads the record, claims the lowest unclaimed generation above `n`
by the exclusive create of its wrapper file (the claim token; tries `n+1` to `n+5`, so orphaned claims from a
crashed `attach` are skipped, then `LAUNCHER_CONFLICT`), rewrites `generation` only AFTER the claim and only upward,
sets `tab: true`, deletes `dead-seen`, and runs step 4 for the new generation, so a controller still holding that
pair id sees the re-opened tab; with a session id, resolves the directory via `session.get`, writes a NEW record with `controller: null` and
runs step 4. A pair id whose record is gone is `PAIR_NOT_FOUND`.

`install-launcher [--force]`: creates the target directory when missing (0700 on POSIX); POSIX creates
`~/.local/bin/opencode-pair` as a symlink to the running binary; an existing symlink is replaced only when its
target's file name is THIS host's binary name (so installing from Qwen never silently re-points a link Claude made;
another host's link is `LAUNCHER_CONFLICT` with the hint `--force`); a regular file or a symlink elsewhere is never
touched (`LAUNCHER_CONFLICT`). Windows copies the running exe to `%LOCALAPPDATA%\opencode-ctl\opencode-pair.exe`
(an in-use copy is first renamed to `opencode-pair.exe.old`, which `gc` deletes) and writes
`opencode-pair.exe.source` with two lines, `path=<staged path>` and `version=<plugin semver>` (LF, split on the first
`=`; any parse failure counts as absent); an existing copy is replaced only when that file parses AND its `path=`
names this host's staged binary (`--force` overrides). Both print a PATH hint when the directory is not on PATH.
`doctor` compares the symlink target (POSIX) or the copy's hash and `version=` line (Windows) with the running
binary and says "re-run install-launcher" after a plugin update (or "install-launcher --force" when the link
belongs to another host's binary, which plain `install-launcher` would refuse with `LAUNCHER_CONFLICT`). Name dispatch relies on the invoker passing the
link name as `argv[0]` (execve does; an invoker that resolves the link first gets the usage error, and `<binary>
pair` always works).

Values that reach a shell: the session id must match `^ses_[A-Za-z0-9]{26}$`, the pair id `^[0-9a-f]{12}$`, the
directory is canonicalised and must exist (on Windows the `\\?\` verbatim prefix `std::fs::canonicalize` returns is
stripped, and a drive root's trailing backslash is kept out of the quoted `-d` argument; both cases are in the
quoting test); the wrapper single-quotes every value (`'\''` escaping on POSIX, `''` in
the `.ps1`); a value that fails validation aborts the launcher before any file is written (`INVALID_ARGUMENT`). On Windows a
directory containing `;` is refused (`INVALID_ARGUMENT`: `wt` splits its command line on `;`), and a newline is
refused everywhere; on Windows `-- <controller args>` travel through `cmd.exe /c`, so an argument containing `&`,
`|`, `<`, `>`, `^`, `"` or `%` is refused there (`INVALID_ARGUMENT`). A unit test feeds a directory containing a quote, a space, a `$`, a `%`, a `^`, an `&`, a
leading `-`, and (expecting refusal) a `;` on Windows and a newline.

### Skills

Both skills are embedded in the engine crate and printed by `emit-skill <opencode-ctl-driver|opencode-ctl-doctor>
--host <cc|qwen|agy|opencode>` (the precedent `emit-skill` has no host selector and the re-ghidra Justfile recipe
writes four byte-identical copies from the cc binary; here the copies differ by invocation form, so the host is an
argument); the Justfile recipe `emit-opencode-ctl-skills` writes the eight committed copies (two skills, four hosts)
and a `tests/skill_emit.rs` in EACH Rust front pins that front's copies (a test can only run its own package's
`CARGO_BIN_EXE_`; the opencode copies are pinned by the cc test, whose binary emits them with `--host opencode`;
precedent: `re-ghidra-mcp-qwen::skill_emit` caught a stale qwen copy the cc test could not see). The names
carry the plugin prefix because Antigravity and OpenCode do not namespace skills and `doctor` already exists in the
re-ghidra siblings (measured: three `skills/doctor/SKILL.md` in the tree). The OpenCode copy of
`opencode-ctl-doctor` omits the `install` argument (`install-launcher` is not offered on that host).

- `opencode-ctl-driver` (description: "Delegate bounded implementation work to an OpenCode worker session on a
  cheaper or free model and verify it. Use when the user asks to hand a task, plan or file to opencode, to delegate
  to a cheaper or free model, to have another agent or a subagent implement a plan, to run something in the
  opencode worker, to check what the worker did, or when an opencode-ctl tool returns awaiting_permission,
  awaiting_input, stalled, budget_exceeded, omitted_patches or patch_truncated. Not for questions about OpenCode
  itself"; the OpenCode host receives the same skill, where the controller is an OpenCode agent driving a sibling
  session; the emitted copy carries the host's own invocation form, never another host's slash path):
  when to delegate; the loop: `service_status` first (`service_start` when it reports not running; on the OpenCode
  host the service is up by definition), and if `paired` USE THAT SESSION (never create a second one
  beside the human's tab unless a parallel worker is wanted on purpose), else `session_create`; then
  `session_prompt -> session_wait with after=<message_id> on EVERY slice until a terminal state -> handle
  a terminal state with concurrent_turn = <id> (the human's turn is running: wait PASSIVELY with after = that id,
  answer none of its permissions or forms, never interrupt it, tell the user what it is waiting on, verify once it
  ends; a human prompt steered INTO the controller's turn joins it, so the controller's session_changes range then
  includes the human's work and the report says so) / awaiting_permission / awaiting_input / failed / interrupted /
  stalled / budget_exceeded (interrupt, report, ask the user before re-prompting) / after a service_restart tell the
  user to re-run attach (the paired tab lost its server) / pair.tui = dead or unknown (tell the user the tab is gone, keep driving) /
  pair.tui = starting (say nothing yet) / pair.tui = none (no tab was requested, say nothing) ->
  session_changes (from = the prompt id for this turn) + session_messages -> verify by files and diff; any of
  omitted_patches, patch_truncated, omitted_files is never a pass: page by paths or read the file -> on a gap
  re-prompt the SAME session with the specific finding`;
  which tools spend tokens; how to answer permissions and forms; `omitted_patches > 0`, `patch_truncated`, `omitted_files`, `stalled` and `budget_exceeded` are never a
  pass; the raw-CLI fallback (`opencode service start`,
  `opencode api session.create -d ...`, `opencode api session.prompt ...`, `opencode api
  experimental.session.wait ...`) for a host without the binary; never trust the worker's closing summary.
- `opencode-ctl-doctor` (description: "Diagnoses why opencode-ctl-mcp tools fail or do not appear. Use when the user
  runs /opencode-ctl-mcp:opencode-ctl-doctor, when a tool returns OPENCODE_NOT_FOUND, SERVICE_DOWN, AUTH_FAILED,
  UNKNOWN_OPERATION or REFUSED_ON_HOST, when opencode-pair is missing or stale, when the user wants
  opencode-pair installed (argument `install`), or when the user asks why the worker does not respond"): checks in order: `opencode` binary found and version >= 2.0 (warns when `server.info.version` is outside the
  tested `2.0.x` range, since `session.diff` turn semantics and the experimental wait route are version behaviours); service registration file present
  and pid alive; `opencode api server.info` answers; the operation table contains `experimental.session.wait`;
  pairing (if any) resolves to a live session, its record generation has a sentinel, and both liveness values
  are shown with their meaning (`starting`, `unknown`, `none` included); which source set `permissions_locked` and
  `permission_policy`; every protected key whose env value differs from the user-level file; the Windows copy's
  `.source` and any `.old` leftovers; `opencode-pair` link resolves to the running binary; `server_url`
  without `OPENCODE_SERVER_PASSWORD`; newest log file and its last error; pairs whose controller side is `dead` (not `none`) while the TUI side is `alive`, and sessions in
  `session.active` whose pair has no living side (orphaned workers); prints the resolved settings when invoked with
  `verbose`.

### Release and registry wiring

- `agents.json`: add the four keys to each agent's `plugins`. PUBLISHED NAME, OPEN FORK (owner decision pending,
  round 7): the co-signed name decision publishes all four fronts as `opencode-ctl-mcp` through `publishedNames`,
  which `scripts/lib/registry.sh` implements and the marketplace checks enforce (the manifest `name` must then equal
  the published name). But every binary plugin today is published under its REGISTRY KEY, and the four bundle
  workflows (`agy-bundles.yml`, `qwen-extension-bundles.yml`, `plugin-bundles.yml`, `opencode-bundles.yml`) resolve
  the tag with `name="${TAG%-v*}"` and `select(.name == $n)` against the marketplace, so tag
  `opencode-ctl-mcp-agy-v0.1.0` looks for an entry named `opencode-ctl-mcp-agy` that does not exist and every
  release job of the four fronts dies; only `skills-bundles.yml` resolves through the registry. Options: (A) drop
  `publishedNames`, publish under the registry keys like every binary sibling (install
  `opencode-ctl-mcp-cc@aiplugins`, slash form `/opencode-ctl-mcp-cc:opencode-ctl-doctor`); (B) keep the published
  name and change the four workflow lookups to `registry_plugin_for_published` plus a manifest-name check. The plan
  waits for the ruling; the Install section below is written for (B) and marked.
- Justfile: `build-opencode-ctl-mcp-cc|agy|opencode` recipes cloning the rtk ones, and `build-opencode-ctl-mcp-qwen`
  as the FIRST qwen build recipe (none exists: the Justfile builds cc, opencode and agy fronts only, and the
  footprint loops, `scripts/check-footprint-budgets.sh` and `docs/footprints/budgets.json` know no qwen front, so the
  qwen front is added to all three or stays explicitly outside the footprint gate); a `just agy-validate` recipe
  running `agy plugin validate --strict` on the agy front, wired into `check` (nothing runs it today; measured: it
  passes for a command that is not on PATH and fails only name/folder/frontmatter errors). Manifest conventions the
  tooling enforces: the qwen command is `${extensionPath}${/}bin${/}<bin>` (a bare name aborts
  `bundle-qwen-extension.sh`) with `contextFileName`, `license` and `description`; each front's `Cargo.toml` carries
  `pre-release-replacements` and `[package.metadata.dist.binaries]` and no `version.workspace`;
  `check-package-versions.sh` has two hand tables that gain the new crates; `probe-plugin-bin.sh` requires
  `--version` to print `<per-host binary name> <plugin.json version>` and exit 0. The registry does NOT feed every enumeration: rtk and re-ghidra are hand-named in
  `.github/workflows/{ci,release,release-refresh,footprint-freshness,plugin-bundles,opencode-bundles,
  qwen-extension-bundles,agy-bundles}.yml`, `scripts/check-skills-release-wiring.sh`, `scripts/smoke-opencode.sh`,
  `scripts/install-qwen-extension.sh` and `.ps1`, `scripts/check-agy-wiring.sh`, `scripts/check-opencode-wiring.sh`
  (per-skill copy pairs), `scripts/check-skills-bundles.sh`, `scripts/probe-plugin-bin.sh`,
  `.release-please-manifest.json`, `release-please-config.json` and the Justfile, so the plan lists each of those
  edits; `just check` and `agy plugin validate --strict` on the agy front gate them.
- `bash scripts/gen-release-please-packages.sh` and `bash scripts/gen-marketplaces.sh` regenerate; `just
  footprint-regen` ratchets budgets ONCE, after the tool descriptions are final: the first landing is uncapped but
  every later PR may move a front by at most 500 resident bytes (`budgets.json` `deltaBytes`), and a 19-tool sibling
  front measures about 21.5 KB. The footprint prober runs `serve` with an ALLOWLISTED environment
  (`tools/plugin-footprint/src/child_env.rs`: PATH, HOME, temp and locale variables only, no `OPENCODE_CTL_*`) on a
  runner with no `opencode` installed and needs `tools/list` to answer, so `serve` must complete the MCP handshake
  and list its tools without resolving the opencode binary (resolution is lazy, at the first call). `just check`
  must be green before the capstone.
- Versions start at 0.1.0 in every manifest; tags are minted by release-please (HACK.md: never by hand).

### Testing

- `mock_opencode`: a stand-in `opencode` selected by `OPENCODE_CTL_BIN`. ONE source file in the engine crate
  (`shared/opencode-ctl/tests/bin/mock_opencode.rs`) that every Rust front includes as its own `[[bin]]` via
  `#[path]` and lists OUT of `[package.metadata.dist.binaries]`, because `CARGO_BIN_EXE_*` is per package and the
  cc, qwen and agy e2e tests must reach it (precedent: `rtk-mcp-qwen`'s `mock-rtk`). Fixtures are keyed by
  OPERATION AND PARAMETERS, not by call order alone: `MOCK_OPENCODE_FIXTURES/<operationId>[@<sorted k=v params>]
  [.<n>].json`, each `{exit, stdout, stderr}` (a bare JSON body means exit 0 with that stdout), the `.<n>` sequence
  consumed per key, so a paged `session.message.list` answers `cursor=...` pages deterministically whatever the
  timer does. Built-in rules: `session.message.list` with `type=idle` answers the real 400 `InvalidRequestError`;
  `GET /openapi.json` is served from `fixtures/openapi.json`; `session.active` answers a map. Failure modes are
  PER OPERATION and composable (`MOCK_OPENCODE_MODE=<op>:<mode>[,<op>:<mode>]`): `fail`, `garbage`, `silent`,
  `slow` (wait never returns; it spawns a grandchild that records its own pid so the kill test can check it
  died), `permission` and `form` (pending lists non-empty after N polls, both at once allowed), `409`, `404-vanished`,
  `big` (300 000 bytes, and when its stdout is a PIPE rather than a file it truncates at 262144 like the real CLI,
  recording the stdout kind, so a pipe-reading transport fails `transport_reads_large_stdout_whole` for the real
  reason). It records every call into `MOCK_OPENCODE_ARGV_LOG` (one JSON line: argv, plus the PRESENCE of an
  allowlisted set of environment names, `OPENCODE_PASSWORD`, `OPENCODE_SERVER_PASSWORD`, `OPENCODE_CTL_*`, never
  their values) so tests assert exact parameters (deep-object `location[directory]`, `type=user`, `order=desc`,
  `--server`) and the password pass-through. The engine reads "now" through an injected clock (a `Clock` trait with
  a test implementation), because stall, budget, `parked_s`, the 30 s `starting` window and the 1 h / 24 h `gc`
  rules compare against now and their clamp floors forbid shortening them; fixtures carry `{{now-N}}` templates for
  server-stamped times. Doctor output is pinned by `doctor_json_reports_*` tests (one per doctor rule: settings
  hash and mtime, ignored and clamped keys, env-vs-file, lock source, `.source` and `.old`, orphaned workers, missing
  wait route, version range, log path). Windows-only resolution (`PATHEXT`, the verbatim prefix) is a pure
  function tested with injected PATH and PATHEXT on every platform, and the `attach` race test runs two real
  threads against one directory.
- Engine protocol test (rmcp in-process client), named tests, each tied to a ledger defect class:
  `tools_declare_token_cost`; `hatch_get_runs`, `hatch_mutation_needs_flag`, `hatch_allowlist_is_exactly_these_seven`
  (pins the ids; any addition or removal goes red), `hatch_get_denylist_blocks_every_listed_id`,
  `hatch_every_snapshot_mutation_is_allowlisted_or_blocked` (enumerates every non-GET id in the snapshot table and
  asserts `MUTATION_BLOCKED` for the seven or `BLOCKED_OPERATION` for the rest; a regenerated snapshot that adds a
  runnable mutation goes red),
  `hatch_204_is_ok_true`, `hatch_non_json_refused`, `hatch_unknown_id`, `hatch_method_path_form_rejected`,
  `hatch_param_name_prefix_and_value_rules`, `hatch_never_forwards_header`, `hatch_redacts_secret_keys`, `hatch_unknown_to_snapshot_blocked`,
  `hatch_live_openapi_wins`, `hatch_snapshot_fallback_reports_source`, `hatch_mutation_never_from_snapshot`,
  `hatch_cache_invalidated_on_pid_change`, `hatch_server_info_reread_before_mutation`,
  `hatch_switch_agent_locked`, `permission_reply_always_locked`, `session_create_agent_locked`,
  `opencode_host_refuses_foreign_session`, `opencode_host_created_set_survives_restart`,
  `opencode_host_hatch_session_mutation_gated`, `changes_status_enum_three_values`, `auth_failed_rotation_hint`;
  `wait_idle_after_anchor`, `wait_first_idle_after_anchor_not_newest`, `wait_stale_idle_before_anchor_is_running`,
  `wait_explicit_after_not_replaced_by_human_message`, `wait_synthetic_never_anchors`, `wait_inbox_pending`,
  `wait_after_unknown_then_stalled`, `wait_failed_outcome`, `wait_awaiting_permission_shape`,
  `wait_awaiting_input_fields`, `wait_active_session_never_stalled`, `wait_budget_exceeded`,
  `wait_idle_beats_budget`, `wait_no_spin_on_early_idle`, `wait_result_field_matrix` (per state),
  `wait_queued_prompt_does_not_borrow_previous_idle`, `wait_budget_survives_reprompt`, `wait_dead_loop_is_stalled`,
  `wait_polls_at_slice_start_and_end`, `wait_stall_arrays_survive_cap`, `wait_failed_marker_without_assistant`,
  `wait_concurrent_turn_flag`, `wait_any_task_error_ends_slice`, `wait_one_failed_poll_is_retried`,
  `wait_idle_search_pages_to_anchor_timestamp`, `wait_search_cut_by_deadline_is_running`,
  `wait_queue_anchor_needs_assistant_message`, `wait_steer_anchor_terminal_without_assistant`,
  `wait_concurrent_turn_carries_message_id`, `wait_turn_s_ends_at_marker`, `wait_default_timeout_120`,
  `wait_permission_and_form_pending_together`, `wait_two_failed_polls_end_slice`, `wait_spawn_bound_per_slice`,
  `wait_max_s_clamped_on_opencode_host`;
  `transport_204_no_retry_of_mutation`, `transport_error_mapping_table` (one case per code),
  `transport_kills_grandchild`, `transport_reads_large_stdout_whole`, `transport_session_id_validated`,
  `transport_mutation_unknown_outcome` (unparseable stdout and deadline), `transport_409_is_conflict`, `transport_conflict_tags_and_vanished_404`, `transport_503_is_service_down`,
  `transport_call_timeout_is_deadline`, `transport_temp_file_removed`, `transport_password_passthrough_env`,
  `transport_no_password_in_argv`, `transport_binary_resolution_order`, `serve_handshakes_without_opencode`,
  `version_flag_prints_name_and_version`, `host_unknown_host_flag_rejected`, `confirm_required_on_destructive`;
  `compact_valid_json_under_cap`, `changes_drop_patches_before_files`, `changes_files_truncated_lists_names`,
  `changes_default_range_ends_at_user_message`, `changes_paths_filter_engine_side`;
  `pairing_env_then_file_then_none`, `pairing_ids_validated`, `pairing_bin_env_exception`,
  `liveness_boot_id_and_starttime`, `liveness_values_per_side` (alive, dead, starting, unknown, none),
  `liveness_reads_record_generation_then_fallback`, `attach_exclusive_create`, `gc_keeps_active_session`,
  `gc_skips_when_service_down`, `gc_dead_seen_grace`, `gc_unknown_after_grace`, `gc_rechecks_liveness_before_delete`,
  `gc_sweeps_old_exe_and_stdout_dirs`, `gc_deletes_none_sided_pair`, `gc_log_retention`,
  `rebind_on_spawn_clears_claude_exited`, `rebind_on_dead_claude_pid`, `rebind_never_over_alive_controller`,
  `liveness_starting_from_wrapper_mtime`, `liveness_windows_boot_tolerance`, `pair_status_missing_record`,
  `pairing_file_fallback_from_pair_id`;
  `host_narrowing_from_each_constant`, `host_unknown_refuses_lifecycle`,
  `settings_workspace_cannot_set_protected_keys`, `settings_workspace_caps`, `settings_env_beats_user_file`,
  `settings_lock_is_sticky`, `settings_clamps_every_tuning_key`, `redaction_substring_and_url_userinfo`.
- Launcher tests (`--dry-run`): `pair_commands_per_terminal` (fake parent chain per terminal and the PATH
  fallback), `pair_passes_server_url_to_wrapper`, `pair_argv0_dispatch_table` (every subcommand name and a
  directory), `attach_bumps_generation`, `attach_loser_retries_then_conflict`, `attach_clears_dead_seen`,
  `attach_session_id_new_record`, `attach_sets_tab_true`, `attach_skips_orphaned_claims`, `pair_controller_flag`,
  `pair_applies_permission_policy`, `pair_record_written_once`, `pair_runs_gc_after_service_up`,
  `pair_no_terminal_tab_false`, `wrapper_paths_absolute`, `logs_never_on_stdout`, `resume_execs_controller_from_record`,
  `controller_not_found_keeps_record`, `launcher_rejects_semicolon_dir_windows`,
  `launcher_rejects_cmd_metachars_in_controller_args_windows`, `launcher_strips_verbatim_prefix_windows`,
  `launcher_resolves_cmd_controller_windows`, `doctor_conflict_hint_names_force`, `install_launcher_conflict_rules`,
  `install_launcher_source_file`, `install_launcher_other_host_conflict`, `pair_removes_session_on_record_failure`,
  `exit_notice_409_retry`, the sentinel writer (one code path writes and reads the liveness string, exact equality),
  and the quoting test (directory and script path, including a HOME with a space, a leading `-`, and the refused `;` and
  newline).
- Per-host e2e (child-process rmcp client against the staged binary): handshake, `service_status`, one refusal;
  `tests/skill_emit.rs` in each Rust front (Skills), asserting the four host copies DIFFER in their invocation lines.
- OpenCode front: `bun test` for the plugin's own hooks (pattern from rtk-mcp-opencode).
- A reproduction of the spike through the MCP tools, run by hand before the capstone, is the acceptance test.

### Install and verification

Claude Code: `claude plugin marketplace add ckir/aiplugins` once (as the siblings' READMEs), then
`claude plugin install opencode-ctl-mcp@aiplugins` (registry key `opencode-ctl-mcp-cc`; under option (A) of the
published-name fork this reads `opencode-ctl-mcp-cc@aiplugins` and the slash form gains the `-cc` suffix); verify with
`/opencode-ctl-mcp:opencode-ctl-doctor`. Other hosts follow their siblings' INSTALL.md / README patterns. First-use path: run
`service_status`, then the driver skill's loop on a throwaway directory.

## To confirm while building (resolved in the plan, not here)

- Whether killing the CLI child's process group also reaches bun's grandchildren (measured: the binary is a native
  ELF; the kill behaviour is not yet measured); the mock's `slow` mode has a grandchild to prove it.
- The per-type extras of `Form.Field` (`external` has `url`; `multiselect` has options) and what
  `session.form.list` rows contain.
- CLOSED from the schema (round 6): the inbox item `time` is `{created}`; `Session.Inbox.Info` is `User | Synthetic
  | Compaction | Move`; `Form.Answer` is a map of key to `string | number | boolean | string[]` and the
  `session.form.reply` body is `{answer}`; `session.active` returns `{data: {<sessionID>: {type: "running"}}}`, a MAP
  keyed by session id, not an array.
- Whether Qwen Code and Antigravity pass the controller's environment to MCP children (measured only for Claude
  Code; a stub MCP server that prints its env settles it). A host that does not inherit has no pairing in v1, and
  `pair --controller` for it prints that instead of a half-pair.
- Whether a turn that fails before its first model output stores an `assistant` message (rule 3.1's relaxed case).
- `files` attachments on `session.prompt` (`PromptInput.FileAttachment` shape) for `session_prompt.files`.
- Windows: `wt` argument quoting and whether `opencode.exe` is on PATH after the official installer.
- Whether `wt` honours `-d` with a path containing spaces when the script path is also quoted.
- The MCP tool-call ceiling of each host must exceed `wait_max_s` + 30 s: Claude Code (`MCP_TOOL_TIMEOUT`), Qwen
  Code (the extension's `timeout`), Antigravity (`mcp_config.json`); set it in each manifest where configurable,
  measure where not, and lower that host's `wait_max_s` clamp if it cannot be raised.
- The `konsole --new-tab -e` form (unmeasured; mate, gnome and xfce are the measured three).
- Which id `session_prompt` returns for `queue` delivery (the inbox item's id, resolution rule (i)).

## Decision: how the human invokes the one command (owner, 2026-10-18)

`install-launcher` it is: the human types `opencode-pair [dir]`, a symlink (POSIX) or copy (Windows) of the plugin
binary that dispatches on its own name; the full rules live in the Launcher section. FRESH-MACHINE BOOTSTRAP: the
first `install-launcher` runs from inside the host, where the plugin root is known: the `opencode-ctl-doctor` skill
accepts the argument `install` and runs `<plugin root>/bin/<host binary> install-launcher` (`${CLAUDE_PLUGIN_ROOT}`
on Claude Code; each front's README gives its own path form); the human never types the plugin's `bin/` path.
Owner approved this document at commit 79ea151 as the basis for the panel review and the plan. Per host, the skill names the binary as: Claude Code `${CLAUDE_PLUGIN_ROOT}/bin/opencode-ctl-cc-mcp`; Qwen `~/.qwen/extensions/<registry key>/bin/opencode-ctl-qwen-mcp` (`${extensionPath}` expands only inside the manifest, never in a skill); Antigravity the bare `opencode-ctl-agy-mcp` from `mcp_config.json`, which is already on PATH (agy gives skills no plugin-root variable); OpenCode has no `install` step.

## Stand-downs (durable record of the panel review)

- UNVERIFIED-ACCEPTED (owner, 2026-10-18): prompt text and request bodies travel on the CLI's argv (`-d`), so they
  are visible to other local users in `ps` and bounded by the command-line limit. The plugin guards the length
  (`BODY_TOO_LARGE`) and the OpenCode CLI itself exposes prompts the same way (`opencode run "<text>"`).
- UNVERIFIED-ACCEPTED (owner, 2026-10-18): a controller may widen a worker's permissions per call
  (`session_create.permissions`, `permission_policy: allow`); the controller acts with the user's authority.
  `permissions_locked` exists for users who want that door shut.
- UNVERIFIED-ACCEPTED (owner, 2026-10-18, accepted for v1): the worker runs as the same OS user as the plugin, so a
  prompt-injected worker could rewrite `~/.opencode-ctl/settings.local.md` (including `opencode_bin`) or the pair
  sentinels; the same user can also edit `~/.claude` itself, so this is the OS trust boundary, not a plugin one.
  Mitigation offered: `doctor` prints the settings file's hash and mtime; nothing else.
- DISCARDED-BELOW-FLOOR: a session driven by a `--standalone` TUI is absent from `session.active` and could read
  as stalled; unreachable because pairing goes only through the shared background service (Decisions, L3) and a
  standalone instance has no discoverable endpoint for the plugin to bind to.
- UNVERIFIED-ACCEPTED (owner, 2026-10-18, prose-only accepted): the driver skill's verification rule is prose the engine cannot
  enforce; a controller that never calls `session_changes` passes nothing the engine can see. `verify_hint` on
  every `idle` result is the nudge offered.
- DISCARDED-BELOW-FLOOR: name dispatch under an invoker that resolves the `opencode-pair` symlink before exec;
  the explicit `<binary> pair` form is always available and documented.
- DISCARDED-BELOW-FLOOR: pair-id collision between two launches in the same second; the id is 48 bits of a uuid v4
  and the launcher refuses an existing id (Pairing contract), so a collision cannot produce two pairs.

- OPEN (owner decision pending, round 7): the published name. `publishedNames` = `opencode-ctl-mcp` makes every
  bundle workflow's tag lookup fail (Release and registry wiring); option (A) publish under the registry keys like
  every binary sibling, option (B) keep the name and change the four workflow lookups. Neither is folded until the
  owner rules; the ruling is recorded here.

## Panel review ledger

Round 1 (2026-10-18, solo panel of two reviewers with nine seats, plus the agy escalation that reviewed the
pre-fold text and returned 13 findings, 10 of them duplicates of the solo panel's): 68 findings raised, disposed as
follows. From agy specifically FOLDED: TUI liveness from the wrapper's own pid (terminal client exits at once);
`fs.*` on the deny-list plus the non-JSON response refusal; `location[directory]` deep-object encoding;
display-only controller-exit notice via `session.synthetic resume:false`. From agy REJECTED: "the raw-CLI fallback
names `session.delete`" (the document never contains that string; the tool is `session_delete` and the route id
`session.remove` is what the engine calls). Owner rulings 2026-10-18: argv exposure ACCEPTED for v1; per-call
permission widening KEPT with `permissions_locked` available. Those two move from pending to UNVERIFIED-ACCEPTED. FOLDED: idle `outcome` read (failed / interrupted states); forms as a second stall channel
(`awaiting_input`, `form_reply`); concurrent permission/form polling every `poll_s` (the earlier text polled only
at the 30 s deadline); server-derived wait anchor (newest input message) instead of process memory; `queue`
delivery when a turn is running (superseded in round 2: `steer` in every state); `stalled` terminal state; CLI stdout to a temp file (256 KiB pipe truncation);
`{data, cursor}` envelope kept; CLI error mapping table (`_tag` on stdout, stack traces on stderr); process-group
kill of CLI children; `Permission.Request` shape with `resources[]` and `save[]`; `always` requires `confirm` and
`saved_permissions` can revoke; `session_changes` from the session-scoped `session.diff` with `paths` and
`omitted_patches`, plus `workspace_status`; `session_list` returns the list route's real fields; `server.info.urls`
array; model string split into `Model.Ref`; escape hatch accepts ids only, validates params, never forwards `-H`,
carries a deny-list, and cannot reach service-level operations; cache invalidation on service pid/version change;
`opencode_bin` and `server_url` accepted only from the environment; `--host` argument on the OpenCode front that can
only narrow; file modes 0700/0600, atomic writes, separate sentinel files, pid-based liveness, `gc`; PowerShell
wrapper on Windows; `install-launcher` idempotency and conflict rule; skills renamed with the plugin prefix and given
descriptions; `emit-skill <name>` for both skills with a pinning test; file logging and doctor coverage; `hard` and
`stall_slices` (superseded in round 2 by `stall_s`) added to the contract; `body_max_bytes`; always-valid JSON truncation; experimental wait route
fallback and doctor check; subcommand list completed. REJECTED: none. DEFERRED-TO-ANOMALIES: the CLI's 256 KiB pipe
truncation is an upstream OpenCode defect, recorded in `.clavity/local-anomalies.md` (2026-10-18) for reporting
upstream; the plugin works around it. The two UNVERIFIED-ACCEPTED items above carry the owner's ruling.

Round 2 (2026-10-18, solo panel: Fold Auditor over the enumerated round-1 edits, Dependency Cynic, Literal
Implementer on the new sections; plus the agy escalation): 20 solo findings. FOLDED: anchor from the newest `user`
message only (synthetics excluded), inbox-held input = `running` with `inbox_pending`, no-input session = idle;
stateless stall rule on server time (`stall_s`); `steer` default in every state (route: steered prompts join the
busy turn); `session_changes` `from`/`to` with whole-session default (turn-scoped route); 204 = success, no retry of
mutations; param names matched before `[`, values as raw argv elements; GET deny-list plus a closed mutation
allow-list (credential-minting `server.pair`/`server.connect`, `rpc.call`, `session.environment`, instructions,
worktrees, reverts, `session.permission.reply` all blocked); policy keys (`permission_policy`, `permissions_locked`)
and executable keys only from env or the user-level settings file; pair id 12 hex everywhere; `claude-pid` sentinel
after spawn, `pid:starttime` liveness, `gc` deletes all sentinels and runs on every `pair`; `attach` creates its own
pair record; `pwsh` else `powershell.exe`; Windows shim `.cmd` with marker header and fixed path, replace-if-marker;
`install-launcher` replaces a link whose target is named `opencode-ctl-*-mcp`; closed error-code list including
`AUTH_FAILED` from the password stderr lines and `[cause]:` extraction; `type` singular and engine-side
`order=desc,limit=1`; `pending_forms` with compacted `fields`; `saved` inference and `project_id` filter;
`session_interrupt.resume`; host narrowing from any constant; four launcher variables; doctor version-range warning;
`meta` envelope keys kept. From the agy escalation (reviewing the pre-fold text, 7 findings, 3 already folded) FOLDED:
204-only operations runnable through the hatch (`{ok: true}`); an explicit `after` is never overwritten by a newer
human message; `stalled` requires absence from `session.active`; poll task limited to the two stall channels with
`poll_s` 10; doctor lists orphaned workers after an abnormal controller death. REJECTED: none. The grandchildren-kill
question moved to "to confirm".

Round 3 (2026-10-18, solo panel: Procedure Walker, Test-Plan Auditor, Fold Auditor over round-2 edits, Mechanism
Gamer; plus the agy escalation with a Resource Vampire): 41 solo findings and 5 from agy (2 duplicates). FOLDED:
fresh-machine bootstrap through the doctor skill's `install` argument; argv[0] dispatch (`opencode-pair` = `pair`),
Windows copy of the exe instead of a `.cmd` shim; wrapper file names; Claude spawned as a child with inherited
stdio before the sentinel write; terminal selection order (parent chain first); `-ExecutionPolicy Bypass`; `attach`
always a new record; `tui_alive: starting | unknown`; exit notice retried on 409; `gc` keeps active sessions and
deletes sentinels; boot id in sentinels; explicit `after` resolution order (inbox, messages, neither); no wait-child
spin; server-stamped time; turn budget `max_turn_s` with `budget_exceeded` even for active sessions; `pair` liveness
and `turn_s` on every wait result; driver loop uses the paired session, passes `after` on every slice and handles
every terminal state; `session_changes` default range ends at the newest USER message (route requirement), never
drops files, `patch_truncated` / `files_truncated`, and the read-the-file exit; `session.update` and
`session.background` removed from the allow-list (eight ids remain, pinned by a test); GET deny-list extended to
`shell.*`, `pty.*`, `persistentPty.*`, `credential.*`, `config.*`; `default_model` moved to the protected keys;
`OPENCODE_CTL_BIN` named as the one env exception; mutation with empty stdout reports `outcome: unknown`; session ids
validated in the engine; the HTTP-transport trigger restated as a CPU share (the 2 s wording would fire on every
turn); a named test per ledger defect class, the allow-list pinned, a 300 KB stdout fixture, argv recording in the
mock; stale ledger wording marked superseded. REJECTED: "`type=user limit=1` can return the synthetic notice" (the
message type enum lists `synthetic` as its own type, so the filter excludes it). DISCARDED-BELOW-FLOOR: the
standalone-TUI stall case (above). UNVERIFIED-ACCEPTED (owner ruled 2026-10-18): same-user worker can rewrite the plugin's
own files (above).

Round 4 (2026-10-18, solo panel: Fold Auditor over round-3 edits, Procedure Walker re-walk, Protocol Pedant;
plus the agy escalation): 28 solo findings, most of them contradictions introduced by rounds 1 to 3's incremental
patches. Disposition: CONSOLIDATION REWRITE of the Pairing contract, session_wait mechanics, Escape hatch, Host and
settings, Launcher and Testing sections as single texts, which FOLDS: name-based dispatch that still reaches `attach`,
`gc`, `install-launcher`; the last `.cmd` shim mention removed, the in-use Windows copy renamed before replacing,
`.source` file format; one sentinel format `<pid>:<boot_id>:<starttime>` with the wrapper's way of computing it per
platform; the wrapper passes `--server`; pair GENERATIONS so `attach` reuses the controller's pair id; liveness values
`alive | dead | starting | unknown | none` for both sides and a loop branch for each; doctor orphan rule on `dead`,
not `none`; the unknown-anchor stall clock from the inbox item's time or the session's `time.updated`; terminal-state
ORDER (first idle marker after the anchor wins over budget and stall); `paths` named as an engine-side filter and
`omitted_files` names so `files_truncated` is no dead end; the whole-session range documented as including the
human's turns, with `from=<prompt id>` for one turn; GET deny-list extended to `integration.*` and
`experimental.session.export` plus RESPONSE REDACTION of secret-named keys; workspace caps on `output_max_bytes`,
`wait_max_s` made protected, `server_url` env name as the second exception, lock-source rule; HTTP trigger restated
on measured CPU at 25 %; `pair` runs `gc` after the service is up, `gc` deletes nothing when the service is down;
the controller spawned by host constant (`claude`, `qwen`, `agy`); `session_wait` result field matrix; `Form.Field`
reduction; `session_changes` request and response fully typed; file-content grammars; settings grammar; error list
closed with `INVALID_ARGUMENT`, `INVALID_REQUEST`, `DEADLINE`, `IO_ERROR`, `PAIR_NOT_FOUND` and `details`,
`TIMEOUT` removed as an error; `source`/`warning`/`meta` in the hatch rows; stale doctor command name; the test list
extended to every round-3 and round-4 fold. From the agy escalation (6 findings on the pre-rewrite text): all six
already resolved by the consolidation; two walker guesses folded (installer creates its directory; name dispatch
strips directory and `.exe`). REJECTED: none. The pre-rewrite text is in git history (395e3df).

Round 5 (2026-10-18, full fresh panel on the consolidated text: two solo reviewers with eleven seats plus a Fold
Auditor, and the agy escalation with every seat): 47 solo findings (15 material) and 4 from agy (2 material, both
new). FOLDED: `persistentPty.*` added to the GET deny-list (the connect route's id has no `server.experimental`
prefix); absolute sentinel paths baked into the wrapper; `session.fork` dropped (seven ids) and the blocked-side
enumeration test restored; redaction by substring plus URL userinfo; every tuning key clamped, `call_timeout_s`,
`wait_max_s` protected and clamped on the OpenCode host; sticky lock; env-trust dependency named and checked by
doctor; `dead-seen` grace for `gc`, `unknown` as dead after 1 h, `.old` and log retention and temp-file sweep in
`gc`; exclusive-create generations and record-generation liveness with fallback; stall rule covering a loop that
died after emitting messages; turn budget measured from the current non-idle stretch; `queue`d prompts cannot
borrow an earlier idle marker; polls at slice start and end; per-slice spawn bound; stall-channel arrays exempt from
the cap; `CONFLICT` code, `STREAMING_REFUSED` removed, `pair_status` missing-record result, `DEADLINE` with unknown
outcome; `pair` removes its session when the record cannot be written; `--controller`, same-host symlink rule and
`--force`, `.source` parse rule and path check; script path quoting; launcher log file; `files` attachments deferred;
two reads for the default range; `permission_reply` re-reads the request; skill descriptions widened to the cost
use case and narrowed away from questions about OpenCode, `budget_exceeded` removed from doctor's triggers, per-host
invocation form in the emitted copy, loop lines for `starting` and `none`; doctor checks for lock source, generation
sanity, env-vs-file, `.source` and `.old`, ignored and clamped settings. REJECTED: "a forked session escapes
`permissions_locked`" as stated (the fork body cannot carry permissions, measured) while the id was dropped on the
unverified-inheritance ground. DISCARDED-BELOW-FLOOR: name dispatch under an invoker that resolves symlinks before
exec (the explicit `<binary> pair` form always works). UNVERIFIED-ACCEPTED (owner ruled 2026-10-18): the verification rule
("never trust the closing summary") is a skill rule the engine cannot enforce; `verify_hint` is the only nudge.

Round 6 (2026-10-18, owner waived the round cap; two solo reviewers with five seats, two of them new: Measured-Claims
Verifier against the live v2.0.32 binary and OpenAPI document, Cross-Host Parity Auditor against the repo's
precedents; plus the agy escalation with Axiom, Cascade, Measured-Claims and First-Use Operator seats): 17 solo-A,
19 solo-B and 13 agy findings, about 28 material after de-duplication; 49 factual claims re-verified against the
binary, 2 contradicted. FOLDED: the stale round-4 `install-launcher` paragraph deleted (it contradicted the
same-host and `.source` rules); `dead-seen` re-checks liveness at deletion time and is cleared by `attach`, `gc` or a
re-bound controller, unknown counts as dead only after 1 h; `tab: false` in the record defines `none` for a launch
that found no terminal; `Transport` is a generic parameter with an explicit `fetch_openapi` (the one non-id call);
the HTTP trigger restated from the measured spawn cost (0.26 to 0.28 s CPU, 20 spawns per slice sits AT 25 % of a
core) as a scheduled measurement; the token column defined as WORKER tokens with the controller-context cost named,
and the default slice 120 s off the OpenCode host; `permissions_locked` now covers `permission_reply always` and
`session.switchAgent` (agents carry their own `permissions` ruleset, measured); `mcp.*` on the GET deny-list and
unknown-to-snapshot ids blocked; the OpenCode host's cwd mechanism stated (spawned against the session directory)
and a created-here session guard so the controller never waits on itself; any task error ends a wait slice, one
failed poll is retried; 503, 500, `InvalidCursorError`, the 409 tags and the vanished-request 404 tags mapped, and
`CONFLICT` redefined (the permission reply route declares no 409); the idle-marker search specified as unfiltered,
paged reads (`type=idle` is a 400, measured) with the spawn bound updated; a `failed` or `interrupted` marker
without an assistant message is terminal when the anchor is not inbox-held; `turn_s` ends at the marker;
`concurrent_turn` on terminal states and the skill's wait-before-verify rule; `session_changes.status` is the
route's three-value enum; `OPENCODE_PASSWORD` precedence and the rotation hint, `server_url` for remote servers
only; `attach` claims a generation by exclusive create with one retry; re-bind on spawn clears `claude-exited`;
Windows resolves `claude.cmd` through `PATHEXT`, refuses `;` in directories, and the quoting test covers `-`, `;`
and newline; konsole added to the terminal table, four more terminals parked; the skill loop starts the service
when down; `emit-skill --host` with a `skill_emit.rs` per Rust front; the OpenCode front's source-only bundle,
absolute `command`, `plugin.ts` id and pin stated; `nix`/`windows` deps and `job_object.rs` reuse; the hand-named
CI and script enumerations listed; four "to confirm" items closed from the schema and two added (env inheritance
on Qwen/Antigravity, assistant message on an early failure); `call_timeout_s` in the author-added tail of the co-signed list; 23 named
tests added. REJECTED: "Windows install does not document PATH" (the section already prints a PATH hint when the directory
is not on PATH); "the inbox path parameter is `inboxID`, not `messageID`" (the document never names it; the
operation table supplies parameter names); "the OpenCode front's working directory is the service's" as stated
(re-ghidra `INSTALL.md`: the host spawns against the session directory; its self-session half was folded);
"`check-marketplace.sh` enforces `publishedNames`" (rejected on a grep of `scripts/*.sh`; OVERTURNED in round 7: `scripts/lib/registry.sh` implements it and the marketplace checks call it, which opened the published-name fork). DISCARDED-BELOW-FLOOR:
none. DEFERRED-TO-ANOMALIES: none.

Round 7 (2026-10-18, two solo reviewers: Fold Auditor over the ENUMERATED round-6 edits plus a Procedure Walker on
Linux and Windows, Cross-Host Parity on the Qwen and Antigravity fronts plus a Test-Plan Auditor; the agy escalation
with Fold Auditor, Cascade on the new mechanisms, a lens on the driver's own dispositions, and a coverage
disclosure): 33 solo-A, 17 solo-B and 6 agy findings. The round-6 `pages_exhausted` rule was WRONG and is withdrawn (the
driver's own disposition, caught by agy and by the solo Fold Auditor independently): no user message newer than the
anchor exists inside one turn, so the search now pages back to the anchor's timestamp with no cap. FOLDED: the
deletion gate accepts `none` sides (an `attach` or `--no-tab` pair was uncollectable); `dead-seen` deleters include
the re-bound server; the record is written ONCE after terminal detection and `attach` sets `tab: true`; trait
methods are `impl Future + Send`; re-bind fires on a dead or missing `claude-pid` too and never over an alive
controller; `resume <pair-id>` re-launches the controller from the record, the launcher prints the export block on
exit, and the file fallback is defined (pair id alone suffices); `starting` is measured from the generation's
wrapper file; Windows boot values compare within 120 s with the Rust computation stated; modes are POSIX-only;
Windows strips the verbatim prefix and handles a drive root; controller args refuse `cmd.exe` metacharacters;
`gnome-terminal-server` in the parent walk; `pair` creates its session through the policy path; doctor names
`--force`; marketplace add in Install; the lock covers `session_create.agent` (protected `default_agent`),
`always`, `switchAgent` and the policy in one sentence; the created-here set is persisted per directory and gates
hatch session mutations; honest spawn arithmetic (18 to 24 per 25 s slice, 44 to 56 per 120 s) and the default
slice is 120 s on every host, which is what keeps polling at 10 to 13 % of a core; the assistant-message
requirement applies to `queue` anchors only; `concurrent_turn` carries the human's message id and the skill waits
PASSIVELY (no replies, no interrupt); `session_messages` returns idle rows; the `BODY_TOO_LARGE` hint no longer
names a deferred feature; the OpenCode doctor copy has no `install`; `job_object.rs` reuse names its dependency
question; six more hand-named wiring files; `CONTROLLER_NOT_FOUND`; three to-confirm items added (host tool-call
ceilings, konsole flag, `queue` message id); 30 named tests added; the ledger's test count corrected to 23.
REJECTED: "`call_timeout_s` sits in the author-added tail, not the co-signed list" (the tail is the record of what
the author added after the panel, which is where that key came from). From the Qwen/Antigravity parity and test-plan seats FOLDED: `hooks.json` `{}` in the agy front (the bundler
requires it); the qwen front's first build recipe and footprint entries; `just agy-validate` in `check`; the qwen
manifest command form and fields, per-front `Cargo.toml` metadata, the version-check tables, the `--version` line
format; `serve` handshakes under the prober's allowlisted environment with no opencode installed; footprint regen
once after descriptions are final (500-byte delta cap); per-host binary paths for the doctor skill; the `windows`
feature and the macOS liveness path; the mock rewritten (per-front `[[bin]]` from one source, fixtures keyed by
operation and parameters with `{exit, stdout, stderr}`, built-in 400 for `type=idle`, composable per-operation
modes, pipe-aware `big`, env-presence log, injected clock, doctor-json tests, pure Windows resolvers, a real
two-thread attach race) and 20 more named tests for the round-5 and round-6 rules that had none. REJECTED:
"`.release-please-manifest.json` is registry-generated" (`gen-release-please-packages.sh` never names it; it stays in
the hand-named list). OPEN: the published-name fork (Stand-downs).

## Parked work (do not lose)

- HTTP transport with SSE event tailing (`event_tail` tool) when the co-signed trigger fires.
- macOS terminal tab support (`osascript` / iTerm) in the launcher; Linux ptyxis, kitty, alacritty and wezterm tabs
  (flags to measure; today they get `tui: none` and the printed attach command).
- clavity: an opencode tab option writing this pairing contract.
- A composite `delegate` tool (create + prompt + wait + changes in one call) once the primitives are measured.
- Per-pair private servers if OpenCode ever publishes the standalone endpoint.

## Builder check (2026-10-18, fresh reviewer, read-only)

Seven questions, one broken reference. Folded as the author's calls: slice protocol for `session_wait` with
`after` and `wait_max_s`; request shapes and server-side `directory` filtering stated; `idle` marker semantics;
host identity as a binary constant with fail-closed refusals; launcher input validation and quoting; OpenAPI
snapshot reduced to the operation table with live-wins policy; skill emission cited as `emit-skill` + Justfile
recipe + pinning test. Left to the owner: how the human invokes `pair`.

## Self-audit of this document (exhaustiveness)

- Contracts: every tool has arguments, return shape, token flag and guards; error codes enumerated; env variables
  and pairing file fields enumerated; settings keys and defaults stated; compaction rules and caps stated.
- No placeholders: every "to confirm" item names where it is resolved (the plan).
- Edge cases covered: unpaired use, service down (snapshot fallback), two controllers (session ids, never-default
  delete), TUI closed (sentinels), wait before the loop starts, permission stall, mutation via operation id or
  method, streaming routes, output over cap, OpenCode host self-stop, binary missing.
- Every goal maps to a section: 1 Tools, 2 Compaction, 3 Permission handling, 4 Pairing + Launcher, 5 Transport +
  Constraints, 6 Release wiring.
