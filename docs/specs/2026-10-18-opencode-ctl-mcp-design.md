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
  tracing-subscriber, thiserror, uuid, sha2, lru, tracing-appender. No HTTP client crate; adding one would ratchet
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
  mutations unless `allow_mutation: true`; streaming routes are refused outright.
- `session_create` defaults `directory` to the host working directory.
- Preflight: `doctor` skill only. Settings keys: `opencode_bin`, `wait_timeout_s`, `poll_s`, `permission_policy`,
  `output_max_bytes`, `server_url`, `default_model`; author added `wait_max_s` (slice cap, default 300) after the
  builder check, and `stall_s` (90), `max_turn_s` (1800), `body_max_bytes`, `permissions_locked` (false) after the panel.
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
qwen/plugins/opencode-ctl-mcp-qwen/          qwen-extension.json (mcpServers), QWEN.md, src/main.rs, tests/e2e.rs
antigravity/plugins/opencode-ctl-mcp-agy/    plugin.json, mcp_config.json, src/bin/mcp.rs, skills/, tests/e2e.rs
opencode/plugins/opencode-ctl-mcp-opencode/  plugin.ts, opencode.jsonc (mcp.servers.opencode-ctl, environment
                                             OPENCODE_CTL_HOST=opencode), package.json, README.md, INSTALL.md,
                                             skills/, tests/*.test.ts
```

Binary names follow the siblings: `opencode-ctl-cc-mcp`, `opencode-ctl-qwen-mcp`, `opencode-ctl-agy-mcp`; the
OpenCode front stages `opencode-ctl-cc-mcp` (as rtk does). Registry keys: `opencode-ctl-mcp-cc`, `-qwen`, `-agy`,
`-opencode`; published name `opencode-ctl-mcp` via `agents.json` `publishedNames`.

### Transport

```rust
pub trait Transport: Send + Sync {
    async fn call(&self, op: &Operation, params: &Params, body: Option<&Value>, deadline: Duration) -> Result<Value, TransportError>;
    async fn lifecycle(&self, verb: Lifecycle) -> Result<String, TransportError>;   // start | stop | restart | status
    async fn wait_idle(&self, session_id: &str, deadline: Duration) -> Result<WaitOutcome, TransportError>; // Idle | DeadlineHit
}
```

`CliTransport` spawns `opencode api <operationId> [--param k=v ...] [-d <json>] [--server <url>]` with
`tokio::process::Command`, STDOUT REDIRECTED TO A TEMP FILE under a 0700 per-process directory (never a pipe: the
pipe truncates at 256 KiB, measured), stderr captured for the error message, a kill-on-drop deadline, the child in
its own process group (Windows: job object) so the deadline kills grandchildren too. It never reads
`service.json`; the CLI resolves auth, port and relocation. The operation id is always passed as an id (never the
`METHOD /path` form); a `--param` NAME must be a parameter the operation table declares, compared on the part before
any `[` (so `location[directory]` matches the declared deepObject `location`); a VALUE is passed as one argv element
(argv is not a shell, so no quoting layer exists or is needed) and may contain anything except NUL and newline; `-H`
is never forwarded. A body larger than `body_max_bytes` (default 100 000; Windows
command lines cap near 32 KB so the Windows default is 24 000) returns `BODY_TOO_LARGE` with the hint to attach a
file instead. Error mapping, in order: spawn failure = `OPENCODE_NOT_FOUND`; exit 1 with stdout JSON carrying
`_tag` = `SessionNotFoundError` -> `SESSION_NOT_FOUND`, `UnauthorizedError` -> `AUTH_FAILED`, other tags ->
`CLI_ERROR` with the tag and message; exit 1 with stderr containing `rejected the password` or `requires a password`
-> `AUTH_FAILED`; exit 1 with empty stdout and a `[cause]:` line containing `Operation not found` ->
`UNKNOWN_OPERATION` (for any `UnknownError` the `[cause]:` line is the message, not the first line); stderr containing `ECONNREFUSED` or a reachability probe (`opencode service status`) failing
-> `SERVICE_DOWN`; anything else -> `CLI_ERROR` with the first stderr line only (never a stack trace). Empty stdout with
exit 0 on an operation whose declared success status is 204 is SUCCESS (`{ok: true}`); on a 200 operation it is
retried once for GET operations only (never a mutation), then `CLI_ERROR` (`"reason": "unparseable stdout"`, and for
a mutation `"outcome": "unknown"` with the hint to check the inbox and messages before repeating it). Every tool
validates a session id against `^ses_[A-Za-z0-9]{26}$` before spawning (`SESSION_NOT_FOUND` with the hint).
Envelope keys beside `data` and `cursor` (for example `location` on `vcs.status`) are kept under `meta`.
When `server_url` is set, the CLI needs credentials for that server: `OPENCODE_SERVER_PASSWORD` is passed through
from the environment, and `doctor` reports when `server_url` is set without it. `wait_idle` runs `opencode api
experimental.session.wait --param sessionID=<id>` with the deadline; exit 0 = idle, killed by deadline =
`DeadlineHit`, anything else = error. Binary resolution: settings `opencode_bin`, else `opencode` on PATH, else
`~/.opencode/bin/opencode` (Windows: `opencode.exe` and `%USERPROFILE%\.opencode\bin`). HTTP is a second impl
only when a tool needs streaming or the polling task's CPU time exceeds 10 % of a turn's wall time measured over
ten real turns (the co-signed trigger, restated: the original "2 s cumulative per turn" would fire on every normal
turn at two 0.53 s spawns per 10 s poll).

The CLI's JSON output is `{"data": ..., "cursor"?: {...}}` for list/get operations and a raw object or array for
some (`server.info`, `location.get`, `config.get`). The engine unwraps `data` and KEEPS `cursor` as the tool's
`next_cursor`; error JSON (a top-level `_tag`) is never treated as data.

### Pairing contract (owned by this plugin)

Environment, read by the MCP server (inherited from Claude) and set by the launcher:

| Variable | Meaning |
|---|---|
| `OPENCODE_CTL_SESSION` | the paired session id; tools with an optional `session_id` default to it |
| `OPENCODE_CTL_DIR` | the paired workspace directory; `session_create`/`session_list` default to it, else host cwd |
| `OPENCODE_CTL_SERVER` | optional server URL passed as `--server`; empty = background service |
| `OPENCODE_CTL_PAIR` | pair id; names the fallback file below |
| `OPENCODE_CTL_HOST` | `opencode` only, set by the OpenCode front's `opencode.jsonc` (it reuses the cc binary); the cc, qwen and agy binaries carry their host as a compile-time constant passed to `cli::dispatch`. Lifecycle tools fail CLOSED: unknown host = refused |

Fallback file `~/.opencode-ctl/pair.<pair-id>.json` (directory mode 0700, files 0600, written to a temp name and
renamed): `{"pair_id", "session_id", "directory", "server_url", "created"}` (pids live in their own sentinels, see below). The
pair id is 12 hex characters from a uuid v4, validated as `^[0-9a-f]{12}$` everywhere, and the launcher refuses an
id that already exists. Exit signals are SEPARATE
sentinel files, each written once by one writer: `pair.<pair-id>.tui-pid` by the tab wrapper at start (its own
`$$` plus the process start time, `pid:starttime`; the terminal CLIENT the launcher spawns hands the tab to a daemon
and exits in 0.22 s, measured, so the launcher's child pid is worthless for liveness), `pair.<pair-id>.claude-pid`
by the launcher right after it spawns Claude (same `pid:starttime` form), `pair.<pair-id>.tui-exited` by the
wrapper's exit trap, and `pair.<pair-id>.claude-exited` by the launcher when Claude returns. A pid is "alive" only
when it exists AND the sentinel's `pid:boot_id:starttime` matches (Linux: `/proc/sys/kernel/random/boot_id` plus
`/proc/<pid>/stat` field 22 parsed after the last `)`; Windows: process creation time, absolute), so a recycled pid
after a reboot never reads as alive. Sentinels are same-user files and are not a security boundary (see
Stand-downs). When Claude returns the launcher also posts a
display-only notice into the worker session (`session.synthetic` with `resume: false`, which admits the message
without scheduling a model turn) so the human in the TUI tab sees "controller exited"; a 409 (session busy) is
retried once with `delivery: queue`, and a failure is logged, never fatal. Liveness is computed, not stored: `tui_alive` =
no tui-exited sentinel AND `tui_pid` alive; `claude_alive` likewise; an abnormal death (SIGKILL, terminal closed,
power loss) therefore reads as dead within one pid check even though no sentinel was written. Identity comes from
env (read once at spawn) and the file is re-read on every `pair_status` / `service_status` call for liveness only;
`attach` never changes an existing pair's session. `opencode-pair gc` deletes every `pair.<pair-id>.*` file (sentinels and wrapper included) of a pair whose both
sides are dead, whose session is NOT in `session.active`, and whose newest file mtime is older than 24 h (a
still-running worker keeps its record); `pair` runs `gc` first on every start, and `gc` can be run by hand.
`attach <session-id | pair-id>` ALWAYS creates a NEW pair record (fresh pair id, no claude side; a pair id
argument only resolves the session id), because sentinels are write-once and a re-opened tab needs fresh ones; the
old record is left for `gc`.

### Tools (all return JSON text; every description states "spends model tokens: yes/no")

Common error shape: `{"error": {"code": CODE, "message": "...", "hint": "..."}}` with codes `OPENCODE_NOT_FOUND`,
`SERVICE_DOWN`, `AUTH_FAILED`, `SESSION_NOT_FOUND`, `NO_SESSION` (no `session_id` and no pairing), `REFUSED_ON_HOST`,
`CONFIRM_REQUIRED`, `PERMISSIONS_LOCKED`, `MUTATION_BLOCKED`, `BLOCKED_OPERATION`, `STREAMING_REFUSED`,
`NON_JSON_RESPONSE`, `UNKNOWN_OPERATION`, `BODY_TOO_LARGE`, `LAUNCHER_CONFLICT`, `TIMEOUT`, `CLI_ERROR`. This list
is closed: a builder adding a code adds it here.

| Tool | Arguments | Returns | Tokens | Guards |
|---|---|---|---|---|
| `service_status` | none | `{running, urls: [..], pid, version, reachable, paired: {session_id, directory, tui_alive, claude_alive} \| null}` | no | |
| `service_start` | none | same as status after `opencode service start` (idempotent) | no | |
| `service_stop` | `confirm: bool` | `{stopped: true}` | no | `confirm` required; refused when host is opencode or unknown; description: "stops the daemon for EVERY session on this machine; to stop one turn use session_interrupt" |
| `service_restart` | `confirm: bool` | status | no | same |
| `session_list` | `directory?`, `all?: bool`, `search?`, `limit?=20`, `cursor?` | `{sessions: [{id, title, directory, outcome, updated, idle, tokens, cost}], next_cursor}` | no | default directory = paired dir or cwd; `all` drops the filter; fields are exactly what the list route returns (no N+1 `session.get`) |
| `session_create` | `title?`, `directory?`, `agent?`, `model?` (`provider/model[#variant]`, split on the first `/` and `#` into `Model.Ref`), `permissions?: Rule[]`, `parent_id?` | `{session_id, directory, agent, model}` | no | directory default as above; model default = settings `default_model`; permissions default per `permission_policy`; a per-call `permissions` is refused with `PERMISSIONS_LOCKED` when settings `permissions_locked: true` |
| `session_get` | `session_id?` | `{id, title, directory, outcome, agent?, model?, tokens, cost, time, pending_permissions, pending_forms}` | no | |
| `session_prompt` | `session_id?`, `text`, `delivery?='steer'`, `files?: [path]` | `{message_id, delivery}` and the hint "call session_wait with after=<message_id> on EVERY slice" | YES | `message_id` is the stored user message id (verified for steer; for `queue` it is the inbox item's id and resolution rule (i) handles it, to confirm); steering into a busy turn joins that turn (route description), so `steer` is the default in every state and `queue` is opt-in |
| `session_wait` | `session_id?`, `after?: message_id`, `timeout_s?=25`, `poll_s?=10`, `hard?: bool=false` | `{state: idle\|failed\|interrupted\|running\|awaiting_permission\|awaiting_input\|stalled\|budget_exceeded\|timeout, outcome?, inbox_pending: bool, elapsed_s, turn_s, pair?: {tui_alive, claude_alive}, pending_permissions: [{id, action, resources, save, message}], pending_forms: [{id, title, fields: [{key, type, label?, options?}]}], last_assistant_text, tokens}` | no | ONE SLICE per call: `timeout_s` capped at settings `wait_max_s` (default 300); `running` means "call again"; `timeout` only with `hard: true` |
| `session_messages` | `session_id?`, `limit?=20`, `cursor?`, `order?='desc'`, `type?` (one of the route's enum values), `reasoning?=false`, `tools?='summary'` (`none\|summary\|full`) | `{messages: [{id, type, outcome?, text, tools: [{name, status}]}], next_cursor, truncated}` | no | compaction (below) |
| `session_changes` | `session_id?`, `from?: message_id`, `to?: message_id`, `paths?: [path]`, `patches?: bool=true` | `{directory, from, to, files: [{file, status, additions, deletions, patch?, patch_truncated?}], omitted_patches: n}` from `session.diff`; DEFAULT RANGE = the whole session (`from` = the session's first `user` message, `to` = its newest `user` message, both found with one `session.message.list type=user` read each; the route requires `to` to be a user message, per its parameter description), so a re-prompt loop's every turn is included; pass `from` = a prompt's `message_id` for that turn only | no | the cap drops PATCHES first (`omitted_patches`), never files; a single patch larger than the cap comes back cut with `patch_truncated: true`; only if even the file list exceeds the cap does `files_truncated: true` appear and the skill pages with `paths`; `omitted_patches > 0`, any `patch_truncated` or `files_truncated` is a signal the driver skill must act on (fetch by `paths`, or read the file itself with the host's own file tools: it is in the workspace), never a pass |
| `workspace_status` | `directory?` | `{directory, status: [{path, kind}]}` from `vcs.status` with `location[directory]` | no | directory-wide view; a non-git directory returns `{vcs: null}` |
| `session_interrupt` | `session_id?`, `resume?: bool=false` | `{interrupted: bool}` (false = the idle no-op) | no | `resume: true` lets pending steering input continue; queued prompts stay parked either way |
| `session_delete` | `session_id`, `confirm: bool` | `{deleted: true}` | no | `confirm` required; never defaults to the paired session |
| `permission_reply` | `session_id?`, `request_id`, `decision: once\|always\|reject`, `message?`, `confirm?: bool` | `{replied: true, saved: bool}` (`saved` = decision was `always` and the request's `save` list was non-empty; the route itself answers 204) | no | `always` persists a saved rule beyond the session (the request's `save` list says what) and requires `confirm: true`; the driver skill defaults to `once` |
| `form_reply` | `session_id?`, `form_id`, `answer` | `{replied: true}` | no | the worker's question channel; `session_wait` reports `awaiting_input` with the form |
| `saved_permissions` | `project_id?`, `remove_id?`, `confirm?: bool` | `{rules: [...]}`; with `remove_id` + `confirm` removes one | no | inspect and revoke what `always` created; `project_id` is the route's filter |
| `opencode_operations` | `filter?` | `{operations: [{id, method, path, summary, mutating}]}` from the cached OpenAPI document | no | |
| `opencode_api` | `operation`, `params?`, `body?`, `allow_mutation?=false` | compacted response | depends | gating (below) |
| `pair_status` | none | the pairing file contents plus liveness | no | |

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
The OpenCode front's `opencode.jsonc` sets `timeout.execution` to `wait_max_s + 30` seconds (330000 ms).

1. Resolve `session_id`; `timeout_s` from arg, else settings `wait_timeout_s`, else 25, capped at `wait_max_s`;
   `poll_s` likewise, else 10. Resolve the ANCHOR, all from the server (nothing is remembered in the process, so it
   survives MCP restarts and holds for a second controller): the `after` arg if given, else the newest `user` message
   (`session.message.list order=desc type=user limit=1`; `synthetic` messages never anchor, so the controller-exit
   notice cannot make an idle session look busy). An EXPLICIT `after` is never replaced: a human typing into the paired TUI
   creates a newer user message, and the controller must keep waiting on ITS turn, not the human's. Resolution
   order for an explicit `after`: (i) the inbox (`session.inbox.list`) still holds it: `running` with
   `inbox_pending: true`, whatever the message list says; (ii) it is in the message list: compare by `time.created`
   (server-stamped epoch milliseconds; the service is local, so the engine's clock is the same clock); (iii) found in
   neither: `running` (not yet admitted) until `stall_s`, then `stalled`. Only when no `after` is given does "a session
   with NO user message" mean `idle`.
2. Run two tasks concurrently until the slice deadline (`tokio::select`): (a) the wait child, `wait_idle` with
   `min(remaining, 30 s)`, restarted while the slice lasts; (b) a poll every `poll_s` (default 10) of the two stall
   channels only, `session.permission.list` and `session.form.list` (two CLI spawns, about 1 s of CPU per cycle,
   measured 0.53 s each). The message, inbox and `session.active` reads happen once whenever the wait child returns
   and once at the slice end, never per poll. Return the first terminal state reached:
   - newest message is `idle`, created after the anchor: `idle` / `failed` / `interrupted` from its `outcome`;
   - a pending permission: `awaiting_permission`; a pending form: `awaiting_input`;
   - STATELESS stall rule: inbox empty, no message newer than the anchor, the session ABSENT from `session.active`
     (the server's own list of sessions with a running loop), and server time minus the anchor's `time.created`
     greater than `stall_s` (default 90): `stalled` (the prompt produced nothing; the skill re-reads `session_get`
     and re-prompts or gives up). A session the server lists as active is `running` regardless of the message list,
     so a slow-starting provider or a long tool call can never read as stalled. An input parked in the inbox longer than `stall_s` is reported as
     `running` with `inbox_pending: true` and `parked_s`; the skill decides whether to interrupt.
   `wait_idle` returning idle while the newest message is older than the anchor means the loop has not started:
   sleep `poll_s` before restarting the wait child (no back-to-back spawns), keep polling.
   - TURN BUDGET: when server time minus the anchor's `time.created` exceeds settings `max_turn_s` (default 1800) the
     slice returns `budget_exceeded` even if the session is still in `session.active`; the driver skill then calls
     `session_interrupt` and reports, so a hung provider or a tool waiting on stdin cannot hold the controller
     forever.
3. At the slice deadline return `running` (or `timeout` when `hard: true`). Every return carries `outcome` when
   known, `last_assistant_text` (newest `text` part), `tokens`, `elapsed_s`, `turn_s` (since the anchor) and, when
   paired, `pair: {tui_alive, claude_alive}` so the controller learns in the same call that the human's tab is gone. A `SERVICE_DOWN` or
   `SESSION_NOT_FOUND` from either task ends the slice with that error.
4. If the operation table has no `experimental.session.wait` (route removed upstream), task (a) is skipped and
   the poll alone drives the state machine; `doctor` reports the missing route.
Measured on the spike: 5 to 7 wait children of 30 s per 2 to 3 minute turn, 0.5 s CLI overhead per poll call.

### Compaction (`compact.rs`)

- Message list: keep `id`, `type`, `time.created`; `text` parts concatenated; `reasoning` dropped unless asked;
  `tool` parts reduced to `{name, status}` (`summary`), omitted (`none`), or kept whole (`full`); user messages keep
  their text. Measured input: 68 KB raw for 18 reasoning, 11 text, 24 tool parts in one short session.
- Every tool result passes a final cap of `output_max_bytes` (default 16384) and is ALWAYS valid JSON: arrays are
  cut at an element boundary, then the longest string fields are shortened with a `...[+N bytes]` tail, and the
  result carries `"truncated": true` with the original byte count. Where the route offers a filter (`type` on the
  message list, `limit`, `paths` on `session_changes`) the fetch is narrowed BEFORE compaction.
- The escape hatch applies a depth cap of 6 and the same byte cap.

### Escape hatch gating (`openapi.rs`)

- The OpenAPI document (280 KB live) is fetched once per process via `opencode api GET /openapi.json` and reduced
  to the operation table `operationId -> (method, path, summary)` (14 KB). A built-in copy of that TABLE only
  (`openapi.table.json`, generated from v2.0.32 and committed with the version it came from) is the fallback when the
  live fetch fails, so `opencode_operations` still answers. Live always wins; every `opencode_operations` and
  `opencode_api` result carries `source: live | snapshot(v2.0.32)`, and when the snapshot is used while
  `server.info.version` differs from it the result adds a `warning`. A mutating operation is never run from the
  snapshot when the live document could not be fetched (`SERVICE_DOWN` first).
- `operation` accepts an operation id ONLY (the `METHOD /path` form is rejected, so no raw path ever reaches the
  CLI). Method resolution happens BEFORE execution. GET operations run unless on the GET DENY-LIST (`BLOCKED_OPERATION`):
  `server.connect` (redeems a pairing code into a server credential), the whole `fs.*` group (`fs.read` returns raw
  file bytes as `application/octet-stream` from anywhere on the host; `fs.list` / `fs.find` enumerate it),
  `session.log` (`text/event-stream`), `event.subscribe`, and everything that returns terminal buffers or secrets:
  `shell.*`, `pty.*`, `server.experimental.persistentPty.*` (measured: `shell.output`, `persistentPty.read` and
  `.snapshot` return buffers as JSON), `credential.*`, `config.*`. Non-GET operations require
  `allow_mutation: true` (`MUTATION_BLOCKED` otherwise) AND must be on the MUTATION ALLOW-LIST, which is closed and
  enumerated here (eight ids): `session.compact`, `session.switchAgent`, `session.switchModel`, `session.view`,
  `session.inbox.cancel`, `session.inbox.update`, `session.form.cancel`, `session.fork`. Dropped after review:
  `session.update` (its body carries `permissions`, measured, which would bypass `permissions_locked`) and
  `session.background` (lets a turn end while a tool keeps editing, which defeats verification by diff).
  `session.switchModel` / `session.switchAgent` stay allowed and are NOT covered by `permissions_locked`; that is a
  cost exposure, not an authority one, and is accepted. Every other mutating operation is `BLOCKED_OPERATION` even with `allow_mutation`, which covers
  `server.pair`, `rpc.call`, `session.shell`, `session.command`, `session.environment`, `session.generate`,
  `session.permission.reply` (only `permission_reply` may answer, so `always` cannot skip `confirm`),
  `session.revert.*`, `session.move`, `project.update`, `worktree.*`, `experimental.session.instructions.*`,
  `experimental.*` writes, `credential.*`, `integration.*`, `plugin.*`, `shell.*`, `pty.*`, `permission.saved.*`,
  `vcs.init`, `location.reload`, `debug.*`. Ids are matched whole and case-sensitively; a `*` in these lists is a
  glob over the rest of the id. The protocol test enumerates every mutating operation id from the snapshot table
  and asserts each is either on the allow-list or refused. An operation is runnable only when it
  declares a 204 success (result `{ok: true}`, 46 operations) or a 200 with `application/json`; anything else
  (`application/octet-stream`, `text/event-stream`) is refused as `NON_JSON_RESPONSE` before execution; stdout that
  fails to parse as JSON is never returned to the model. `event.subscribe`, the pty connect routes and any `text/event-stream` response
  are `STREAMING_REFUSED`. Unknown ids are `UNKNOWN_OPERATION` with the three nearest ids as a hint. The cached
  table is invalidated when `service_status` observes a different service pid or version.

### Permission handling

- `session_create` sends `permissions` as given; when absent, `permission_policy` decides: `inherit` sends null,
  `allow` sends `[{action:"*",resource:"*",effect:"allow"}]`, `ask` sends `[{..., effect:"ask"}]`, `deny` sends
  `[{..., effect:"deny"}]`.
- `session_wait` surfaces pending requests and forms; `permission_reply` / `form_reply` answer them; the driver
  skill tells the controller to read the request's `action`, `resources` and `save` before replying, to default to
  `once`, and never to reply `always` without the human's say-so. `permissions_locked: true` in settings forbids
  per-call `permissions` on `session_create` and `permission_policy` other than the file's value, for users who
  want the controller unable to widen the worker's authority.

### Host-specific behaviour

- Host identity: each per-host binary passes a `Host` constant (`Claude`, `Qwen`, `Agy`) into `cli::dispatch`. The
  OpenCode front stages the cc binary and runs it as `serve --host opencode` (an ARGUMENT in `opencode.jsonc`'s
  command array, which survives any environment handling; `OPENCODE_CTL_HOST=opencode` is set as well). Either
  signal alone is enough: `Opencode` wins whenever the argument OR the variable says so, from ANY constant
  (`Claude`, `Qwen`, `Agy`), and nothing can widen a constant. `Unknown` exists only for test builds. On `Opencode` or `Unknown`,
  `service_stop` and `service_restart` return `REFUSED_ON_HOST` (fail closed) and the mutation allow-list contains no service- or instance-level operation, so the refusal cannot be bypassed
  through the hatch. On
  `Opencode` the `pair` and `attach` subcommands are not offered (the TUI is already open).
- The Claude Code and OpenCode fronts read `.claude/opencode-ctl-mcp-cc.local.md` and
  `.opencode/opencode-ctl-mcp-opencode.local.md` (YAML front matter: `key: value` lines between `---` fences, the
  keys above, env overrides win), the re-ghidra precedent. BUT a workspace file is part of a checkout a stranger may
  have written, so it may set ONLY the tuning keys: `wait_timeout_s`, `wait_max_s`, `poll_s`, `stall_s`,
  `max_turn_s`, `output_max_bytes`, `body_max_bytes`. The keys that choose what runs, where data goes (a provider
  receives the code), or how much authority the worker gets (`opencode_bin`, `server_url`, `default_model`,
  `permission_policy`, `permissions_locked`) are read
  ONLY from the environment or from the USER-LEVEL file `~/.opencode-ctl/settings.local.md` (same format, written
  by the user, mode 0600), env winning; a workspace file that names them is ignored for those keys with a logged
  warning. Qwen and Antigravity fronts read the user-level file and env only in v1. Env names: `OPENCODE_CTL_` + the key upper-cased
  (`OPENCODE_CTL_WAIT_TIMEOUT_S`, ...), with one documented exception: `opencode_bin` is `OPENCODE_CTL_BIN`.
- Logging: `serve` writes a per-instance file log (`tracing-appender`, like the siblings) under
  `~/.opencode-ctl/logs/`; stdout is the protocol channel and never receives a log line; `doctor` prints the path
  of the newest log and its last error.

### Launcher (`opencode-ctl-cc-mcp pair`, shipped in the same binary)

`pair [directory] [--model <provider/model>] [--title <t>] [--no-tab] [--dry-run] [-- <claude args>]`:
1. Ensure the service runs (`opencode service start`, idempotent).
2. Create the session in `directory` (default cwd) with `default_model` or `--model`; title default
   `pair <basename> <HH:MM>`.
3. Write `~/.opencode-ctl/pair.<pair-id>.json` (pair id = 12 hex chars of a uuid v4).
4. Unless `--no-tab`: write a wrapper script under `~/.opencode-ctl/` (mode 0700) that runs `opencode <directory>
   -s <id>` and writes the `tui-exited` sentinel on exit; the wrapper is `~/.opencode-ctl/pair.<pair-id>.wrapper.sh`
   (Windows: `.wrapper.ps1`, single-quoted values with `''` escaping; no `.cmd`, whose `%` and `^` expansion cannot be
   quoted away). Open it in a new tab of the terminal the user is in: FIRST the terminal found by walking the parent
   process names (clavity's measured approach: mate-terminal, gnome-terminal, xfce4-terminal), and ONLY when no
   parent matches, the first of those three found on PATH, in that order; forms `mate-terminal --tab -e <script>`,
   `gnome-terminal --tab -- <script>`, `xfce4-terminal --tab -e <script>`. On Windows: `wt -w 0 nt -d <directory>
   <pwsh|powershell.exe> -NoProfile -ExecutionPolicy Bypass -File <script.ps1>` (`pwsh` when on PATH, else the stock
   `powershell.exe`; `doctor` reports which; `Bypass` because the stock client policy refuses `.ps1` files). When no
   terminal is found at all, print the attach command and continue without a tab. Until the wrapper has written
   `tui-pid` (expected within seconds) `tui_alive` reports `starting`; after 30 s without it, `unknown`.
5. Spawn `claude <claude args>` as a CHILD with inherited stdio in the current terminal, with the four
   `OPENCODE_CTL_*` variables set (`SESSION`, `DIR`, `SERVER`, `PAIR`; `HOST` is the binary's constant); write the
   `claude-pid` sentinel from the child's pid and start time; then block on the child. When it exits write the
   `claude-exited` sentinel and print "worker session <id> is still running; attach with `opencode <dir> -s <id>`".
`attach <session-id | pair-id>` opens step 4 for an existing session (lazy binding made visible).
`pair`, `attach`, `gc` and `install-launcher` live in `cli::dispatch`, so the cc, qwen and agy binaries all have
them; the OpenCode front does not offer them. `install-launcher` is idempotent: on POSIX it creates `~/.local/bin/opencode-pair`
as a symlink to the running binary; an existing symlink is replaced when its target's file name matches
`opencode-ctl-*-mcp` (host-agnostic, no cache-path patterns), and a regular file or a symlink elsewhere is never
touched (`LAUNCHER_CONFLICT` with the path). On Windows it COPIES the running exe to `%LOCALAPPDATA%\opencode-ctl\opencode-pair.exe` and writes
`opencode-pair.exe.source` beside it (the staged path and the plugin version); an existing copy is replaced only
when that source file exists. No `.cmd` shim exists, so cmd's `%`, `^` and `&` expansion never touches user input.
`doctor` compares the symlink target (POSIX) or the copy's hash against the running binary and says "re-run
install-launcher" after a plugin update. The link target
goes stale when the plugin updates, so `doctor` compares the link target with the running binary and the README
says to re-run `install-launcher` after an update. How a human reaches the one command from a terminal: the owner chose (see "Decision: how the human invokes
the one command") the `install-launcher` subcommand that links the staged binary as
`opencode-pair` into `~/.local/bin` (Windows: `%LOCALAPPDATA%\opencode-ctl\opencode-pair.cmd`) and prints the PATH
hint, so the plugin's `bin/` location never has to be typed.
`--dry-run` prints every command it would run and writes nothing; the launcher's tests use it.
Values that reach a shell: the session id must match `^ses_[A-Za-z0-9]{26}$`, the pair id `^[0-9a-f]{12}$`, the
directory is canonicalised and must exist; the wrapper script single-quotes every value (`'\''` escaping) on POSIX
and `''` escaping in the `.ps1` on Windows; a value that fails validation aborts the launcher before any file is
written. A unit test feeds a directory containing a quote, a space, a `$`, a `%`, a `^` and an `&`.

### Skills

Both skills are embedded in the engine crate and printed by `emit-skill opencode-ctl-driver` / `emit-skill
opencode-ctl-doctor`; the Justfile recipe `emit-opencode-ctl-skills` writes the eight committed copies (two skills,
four hosts) and `tests/skill_emit.rs` in the cc front pins every committed copy to the emitted text. The names
carry the plugin prefix because Antigravity and OpenCode do not namespace skills and `doctor` already exists in the
re-ghidra siblings (measured: three `skills/doctor/SKILL.md` in the tree).

- `opencode-ctl-driver` (description: "Delegate bounded implementation work to an OpenCode worker session and
  verify it. Use when the user asks to hand a task, plan or file to opencode, to run something in the opencode
  worker, to check what the worker did, or when a opencode-ctl tool returns awaiting_permission, awaiting_input,
  stalled or omitted_patches"):
  when to delegate; the loop: `service_status` first, and if `paired` USE THAT SESSION (never create a second one
  beside the human's tab unless a parallel worker is wanted on purpose), else `session_create`; then
  `session_prompt -> session_wait with after=<message_id> on EVERY slice until a terminal state -> handle
  awaiting_permission / awaiting_input / failed / interrupted / stalled / budget_exceeded (interrupt and report) /
  tui_alive=false (tell the user the tab is gone, keep driving) -> session_changes + session_messages -> verify by
  files and diff -> on a gap re-prompt the SAME session with the specific finding`;
  which tools spend tokens; how to answer permissions and forms; `omitted_patches > 0` or `stalled` are never a
  pass; the raw-CLI fallback (`opencode service start`,
  `opencode api session.create -d ...`, `opencode api session.prompt ...`, `opencode api
  experimental.session.wait ...`) for a host without the binary; never trust the worker's closing summary.
- `opencode-ctl-doctor` (description: "Diagnoses why opencode-ctl-mcp tools fail or do not appear. Use when the user
  runs /opencode-ctl-mcp:opencode-ctl-doctor, when a tool returns OPENCODE_NOT_FOUND, SERVICE_DOWN, AUTH_FAILED,
  UNKNOWN_OPERATION or REFUSED_ON_HOST, when opencode-pair is missing or stale, or when the user asks why the worker
  does not respond"): checks in order: `opencode` binary found and version >= 2.0 (warns when `server.info.version` is outside the
  tested `2.0.x` range, since `session.diff` turn semantics and the experimental wait route are version behaviours); service registration file present
  and pid alive; `opencode api server.info` answers; the operation table contains `experimental.session.wait`;
  pairing (if any) resolves to a live session; `opencode-pair` link resolves to the running binary; `server_url`
  without `OPENCODE_SERVER_PASSWORD`; newest log file and its last error; pairs whose Claude side is dead while the TUI side is alive (the
  orphaned-worker case an abnormal controller death leaves behind); prints the resolved settings when invoked with
  `verbose`.

### Release and registry wiring

- `agents.json`: add the four keys to each agent's `plugins`, `publishedNames` entries mapping to `opencode-ctl-mcp`.
- Justfile: `build-opencode-ctl-mcp-cc|qwen|agy|opencode` recipes cloning the rtk ones; footprint and smoke
  enumerations pick the plugin up from the registry; `check-package-versions.sh` gets the four pairs.
- `bash scripts/gen-release-please-packages.sh` and `bash scripts/gen-marketplaces.sh` regenerate; `just
  footprint-regen` ratchets budgets; `just check` must be green before the capstone.
- Versions start at 0.1.0 in every manifest; tags are minted by release-please (HACK.md: never by hand).

### Testing

- `tests/bin/mock_opencode.rs`: a stand-in `opencode` selected by `OPENCODE_CTL_BIN`. It answers `api <op>` from a
  fixture directory (`MOCK_OPENCODE_FIXTURES`), `service status|start|stop` from `MOCK_OPENCODE_SERVICE`, and forces
  failure modes via `MOCK_OPENCODE_MODE`: `fail`, `garbage`, `silent`, `slow` (wait never returns), `permission`
  (permission list non-empty after N polls).
- The mock records every argv it receives into `MOCK_OPENCODE_ARGV_LOG`, so tests assert the exact parameters sent
  (deep-object `location[directory]`, `type=user`, `order=desc`), and it can emit a 300 000-byte stdout to prove the
  temp-file read (a pipe-based transport would see 262144 bytes and fail `transport_reads_large_stdout_whole`).
- Engine protocol test (rmcp in-process client), named tests, each tied to a ledger defect class:
  `tools_declare_token_cost`; `hatch_get_runs`, `hatch_mutation_needs_flag`, `hatch_allowlist_is_exactly_these_eight`
  (pins the ids; any addition or removal goes red), `hatch_get_denylist_blocks_buffers_and_secrets` (every id listed),
  `hatch_204_is_ok_true`, `hatch_non_json_refused`, `hatch_unknown_id`, `hatch_method_path_form_rejected`,
  `hatch_param_name_prefix_and_value_rules`, `hatch_never_forwards_header`; `wait_idle_after_anchor`,
  `wait_stale_idle_before_anchor_is_running`, `wait_explicit_after_not_replaced_by_human_message`,
  `wait_synthetic_never_anchors`, `wait_inbox_pending`, `wait_after_unknown_then_stalled`, `wait_failed_outcome`,
  `wait_awaiting_permission_shape`, `wait_awaiting_input_fields`, `wait_active_session_never_stalled`,
  `wait_budget_exceeded`, `wait_no_spin_on_early_idle`; `transport_204_no_retry_of_mutation`,
  `transport_error_mapping_table` (one case per code), `transport_kills_grandchild`; `compact_valid_json_under_cap`,
  `changes_drop_patches_before_files`, `changes_default_range_ends_at_user_message`; `pairing_env_then_file_then_none`,
  `pairing_ids_validated`, `liveness_boot_id_and_starttime`, `gc_keeps_active_session`;
  `host_narrowing_from_each_constant`, `host_unknown_refuses_lifecycle`, `settings_workspace_cannot_set_policy_keys`,
  `settings_env_beats_user_file`.
- Per-host e2e (child-process rmcp client against the staged binary): handshake, `service_status`, one refusal.
- Launcher: `--dry-run` snapshot tests for each terminal candidate with a fake parent chain; the sentinel writer.
- OpenCode front: `bun test` for the plugin's own hooks (pattern from rtk-mcp-opencode).
- A reproduction of the spike through the MCP tools, run by hand before the capstone, is the acceptance test.

### Install and verification

Claude Code: `claude plugin install opencode-ctl-mcp@aiplugins` (registry key `opencode-ctl-mcp-cc`); verify with
`/opencode-ctl-mcp:doctor`. Other hosts follow their siblings' INSTALL.md / README patterns. First-use path: run
`service_status`, then the driver skill's loop on a throwaway directory.

## To confirm while building (resolved in the plan, not here)

- Whether killing the CLI child's process group also reaches bun's grandchildren (measured: the binary is a native
  ELF; the kill behaviour is not yet measured); the mock's `slow` mode has a grandchild to prove it.
- The `Form.Answer` shape for `form_reply` and what `session.form.list` rows contain.
- `session.inbox.list` row shape for the `stalled` check.
- `files` attachments on `session.prompt` (`PromptInput.FileAttachment` shape) for `session_prompt.files`.
- Windows: `wt` argument quoting and whether `opencode.exe` is on PATH after the official installer.
- Whether `wt` honours `-d` with a path containing spaces when the script path is also quoted.

## Decision: how the human invokes the one command (owner, 2026-10-18)

`install-launcher` it is: the subcommand links the staged binary as `opencode-pair` into `~/.local/bin` (Windows:
a COPY of the exe as `%LOCALAPPDATA%\opencode-ctl\opencode-pair.exe`, no shim script at all), prints a PATH hint when
that directory is not on PATH, and the `doctor` skill checks the link (or copy) matches the currently staged binary.
The binary dispatches on its own name: invoked as `opencode-pair` it behaves as `<binary> pair`, so no wrapper and no
command-line quoting layer exists on either platform. The human types `opencode-pair [dir]`. FRESH-MACHINE
BOOTSTRAP: the first `install-launcher` is run from inside the host, where the plugin root is known: the
`opencode-ctl-doctor` skill accepts the argument `install` and runs `${CLAUDE_PLUGIN_ROOT}/bin/opencode-ctl-cc-mcp
install-launcher` (per-host path in each front's README); the README's one-liner uses the same variable. The human
never types the plugin's `bin/` path.
Owner approved this document at commit 79ea151 as the basis for the panel review and the plan.

## Stand-downs (durable record of the panel review)

- UNVERIFIED-ACCEPTED (owner, 2026-10-18): prompt text and request bodies travel on the CLI's argv (`-d`), so they
  are visible to other local users in `ps` and bounded by the command-line limit. The plugin guards the length
  (`BODY_TOO_LARGE`) and the OpenCode CLI itself exposes prompts the same way (`opencode run "<text>"`).
- UNVERIFIED-ACCEPTED (owner, 2026-10-18): a controller may widen a worker's permissions per call
  (`session_create.permissions`, `permission_policy: allow`); the controller acts with the user's authority.
  `permissions_locked` exists for users who want that door shut.
- UNVERIFIED-ACCEPTED (owner ruling pending): the worker runs as the same OS user as the plugin, so a
  prompt-injected worker could rewrite `~/.opencode-ctl/settings.local.md` (including `opencode_bin`) or the pair
  sentinels; the same user can also edit `~/.claude` itself, so this is the OS trust boundary, not a plugin one.
  Mitigation offered: `doctor` prints the settings file's hash and mtime; nothing else.
- DISCARDED-BELOW-FLOOR: a session driven by a `--standalone` TUI is absent from `session.active` and could read
  as stalled; unreachable because pairing goes only through the shared background service (Decisions, L3) and a
  standalone instance has no discoverable endpoint for the plugin to bind to.
- DISCARDED-BELOW-FLOOR: pair-id collision between two launches in the same second; the id is 48 bits of a uuid v4
  and the launcher refuses an existing id (Pairing contract), so a collision cannot produce two pairs.

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
standalone-TUI stall case (above). UNVERIFIED-ACCEPTED pending the owner: same-user worker can rewrite the plugin's
own files (above).

## Parked work (do not lose)

- HTTP transport with SSE event tailing (`event_tail` tool) when the co-signed trigger fires.
- macOS terminal tab support (`osascript` / iTerm) in the launcher.
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
