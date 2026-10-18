# opencode-ctl-mcp: drive an OpenCode worker from Claude Code (and three other hosts) as a subagent

Design document. Written 2026-10-18 on branch `feat/opencode-ctl-mcp` (from `feat/antigravity-parity` @4aeeb54).
Status: DRAFT, awaiting owner approval. Nothing is built until it is approved.

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
  builder check.
- Launcher: the plugin owns the pairing contract and ships a minimal launcher; clavity may implement the same
  contract later. Pairing travels as environment variables with a file fallback. Shared background service.
  Linux terminals (mate, gnome, xfce `--tab -e`) and Windows Terminal (`wt`) in v1. Lifecycle desync between the
  two tabs is the real hazard: the contract carries exit sentinels in both directions.
- Branch: from `feat/antigravity-parity`.

## Design

### Layout

```
shared/opencode-ctl/                      engine: lib crate `opencode_ctl`
  src/lib.rs, cli.rs (dispatch: serve | pair | attach | doctor-json | --version),
  src/transport.rs (trait + CliTransport), src/openapi.rs (operation table, gating),
  src/tools/*.rs (one file per tool group), src/compact.rs, src/pairing.rs, src/launcher/{mod,posix,windows}.rs,
  src/settings.rs, skill/ (canonical driver + doctor skills, emitted per host like ghidra-mcp's `skill/`),
  tests/ (protocol test with rmcp in-process client; unit tests), tests/bin/mock_opencode.rs
claude-code/plugins/opencode-ctl-mcp-cc/     .claude-plugin/plugin.json, .mcp.json, src/bin/mcp.rs, src/lib.rs,
                                             skills/{opencode-ctl-driver,doctor}/SKILL.md, examples/*.local.md, tests/e2e.rs
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
`tokio::process::Command`, stdout captured, stderr captured for the error message, a kill-on-drop deadline. It never
reads `service.json`; the CLI resolves auth, port and relocation. `wait_idle` runs `opencode api
experimental.session.wait --param sessionID=<id>` with the deadline; exit 0 = idle, killed by deadline =
`DeadlineHit`, anything else = error. Binary resolution: settings `opencode_bin`, else `opencode` on PATH, else
`~/.opencode/bin/opencode` (Windows: `opencode.exe` and `%USERPROFILE%\.opencode\bin`). HTTP is a second impl
only when a tool needs streaming or permission polling exceeds 2 s cumulative per turn (co-signed trigger).

The CLI's JSON output is `{"data": ...}` for most list/get operations and the raw object for some (`server.info`,
`location.get`, `config.get` returns an array). The engine unwraps `data` when present and keeps the rest.

### Pairing contract (owned by this plugin)

Environment, read by the MCP server (inherited from Claude) and set by the launcher:

| Variable | Meaning |
|---|---|
| `OPENCODE_CTL_SESSION` | the paired session id; tools with an optional `session_id` default to it |
| `OPENCODE_CTL_DIR` | the paired workspace directory; `session_create`/`session_list` default to it, else host cwd |
| `OPENCODE_CTL_SERVER` | optional server URL passed as `--server`; empty = background service |
| `OPENCODE_CTL_PAIR` | pair id; names the fallback file below |
| `OPENCODE_CTL_HOST` | `opencode` only, set by the OpenCode front's `opencode.jsonc` (it reuses the cc binary); the cc, qwen and agy binaries carry their host as a compile-time constant passed to `cli::dispatch`. Lifecycle tools fail CLOSED: unknown host = refused |

Fallback file `~/.opencode-ctl/pair.<pair-id>.json`:
`{"pair_id", "session_id", "directory", "server_url", "created", "tui": {"pid", "exited_at"}, "claude": {"pid",
"exited_at"}}`. Written by the launcher; `exited_at` fields are written by the tab wrapper script (trap on exit)
and by the launcher when Claude exits. The MCP server reads env first, then the file named by `OPENCODE_CTL_PAIR`,
then nothing (unpaired: tools require an explicit `session_id`). `service_status` reports `paired` with
`tui_alive` (pid alive and no `exited_at`) so Claude knows when the human's tab is gone.

### Tools (all return JSON text; every description states "spends model tokens: yes/no")

Common error shape: `{"error": {"code": CODE, "message": "...", "hint": "..."}}` with codes `OPENCODE_NOT_FOUND`,
`SERVICE_DOWN`, `SESSION_NOT_FOUND`, `NO_SESSION` (no `session_id` and no pairing), `REFUSED_ON_HOST`,
`CONFIRM_REQUIRED`, `MUTATION_BLOCKED`, `STREAMING_REFUSED`, `UNKNOWN_OPERATION`, `TIMEOUT`, `CLI_ERROR`.

| Tool | Arguments | Returns | Tokens | Guards |
|---|---|---|---|---|
| `service_status` | none | `{running, url, pid, version, reachable, paired: {session_id, directory, tui_alive} \| null}` | no | |
| `service_start` | none | same as status after `opencode service start` (idempotent) | no | |
| `service_stop` | `confirm: bool` | `{stopped: true}` | no | `confirm` required; refused when host = opencode |
| `service_restart` | `confirm: bool` | status | no | same |
| `session_list` | `directory?`, `all?: bool`, `search?`, `limit?=20` | `{sessions: [{id, title, directory, agent, model, updated, idle}]}` | no | default directory = paired dir or cwd; `all` drops the filter |
| `session_create` | `title?`, `directory?`, `agent?`, `model?`, `permissions?: Rule[]`, `parent_id?` | `{session_id, directory, agent, model}` | no | directory default as above; model default = settings `default_model`; permissions default per `permission_policy` |
| `session_get` | `session_id?` | `{id, title, directory, agent, model, tokens, cost, time, pending_permissions}` | no | |
| `session_prompt` | `session_id?`, `text`, `delivery?='steer'`, `files?: [path]` | `{message_id, delivery}` and the hint "call session_wait with after=<message_id>" | YES | the server remembers `message_id` per session for `session_wait.after` |
| `session_wait` | `session_id?`, `after?: message_id`, `timeout_s?=25`, `poll_s?=2` | `{state: idle\|running\|awaiting_permission\|timeout, elapsed_s, pending_permissions: [{id, action, resource}], last_assistant_text, tokens}` | no | ONE SLICE per call: `timeout_s` capped at settings `wait_max_s` (default 300); `state: running` means "call again" |
| `session_messages` | `session_id?`, `limit?=20`, `cursor?`, `reasoning?=false`, `tools?='summary'` (`none\|summary\|full`) | `{messages: [{id, type, text, tools: [{name, status}]}], next_cursor, truncated}` | no | compaction (below) |
| `session_changes` | `session_id?` | `{directory, status: [{path, kind}], diff_stat, diff?}` from the vcs routes for the session's directory | no | diff included only under the byte cap |
| `session_interrupt` | `session_id?` | `{interrupted: true}` | no | |
| `session_delete` | `session_id`, `confirm: bool` | `{deleted: true}` | no | `confirm` required; never defaults to the paired session |
| `permission_reply` | `session_id?`, `request_id`, `decision: once\|always\|reject`, `message?` | `{replied: true}` | no | |
| `opencode_operations` | `filter?` | `{operations: [{id, method, path, summary, mutating}]}` from the cached OpenAPI document | no | |
| `opencode_api` | `operation`, `params?`, `body?`, `allow_mutation?=false` | compacted response | depends | gating (below) |
| `pair_status` | none | the pairing file contents plus liveness | no | |

Seventeen tools. `pair_status` and `session_changes` are the author's additions to the co-signed list (the owner
can cut them): the first because the launcher's lifecycle sentinels need a reader, the second because the spike
proved verification by diff is the whole point.

### session_wait mechanics (slice protocol)

Hosts bound a single MCP tool call (the OpenCode front's sibling manifests set `timeout.execution: 30000` ms; the
other hosts have their own defaults), and a worker turn took 133 to 202 s on the spike. So one `session_wait`
call is a SLICE, never the whole wait: it returns `running` when its slice ends and the driver skill calls it again.
The OpenCode front's `opencode.jsonc` sets `timeout.execution` to `wait_max_s + 30` seconds (330000 ms).

1. Resolve `session_id`; `timeout_s` from arg, else settings `wait_timeout_s`, else 25, capped at `wait_max_s`;
   `poll_s` likewise, else 2. Resolve `after`: the arg, else the id of the last prompt this server process sent to
   that session, else none.
2. Loop until the slice deadline: run `wait_idle` with `min(remaining, 30 s)`; on `Idle`, read the newest message
   (`order=desc`, `limit=1`); if its type is `idle` AND (no `after`, or it was created after the `after` message)
   return `idle`, otherwise (the loop has not started yet, or the idle marker is stale) sleep `poll_s` and continue.
   On `DeadlineHit`, read `session.permission.list`; if non-empty return `awaiting_permission` with the requests.
3. At the slice deadline return `running` (or `timeout` when the caller's own `timeout_s` was the binding limit and
   the skill asked for a hard stop via `hard: true`). Each return carries `last_assistant_text` (newest `text` part),
   `tokens` and `elapsed_s`.
Measured on the spike: 5 to 7 wait children of 30 s per 2 to 3 minute turn, 0.5 s CLI overhead per poll.

### Compaction (`compact.rs`)

- Message list: keep `id`, `type`, `time.created`; `text` parts concatenated; `reasoning` dropped unless asked;
  `tool` parts reduced to `{name, status}` (`summary`), omitted (`none`), or kept whole (`full`); user messages keep
  their text. Measured input: 68 KB raw for 18 reasoning, 11 text, 24 tool parts in one short session.
- Every tool result passes a final cap of `output_max_bytes` (default 16384): JSON is truncated at the top-level
  array boundary when possible, otherwise as a string with a `"truncated": true` marker and the original byte count.
- The escape hatch applies a depth cap of 6 and the same byte cap.

### Escape hatch gating (`openapi.rs`)

- The OpenAPI document (280 KB live) is fetched once per process via `opencode api GET /openapi.json` and reduced
  to the operation table `operationId -> (method, path, summary)` (14 KB). A built-in copy of that TABLE only
  (`openapi.table.json`, generated from v2.0.32 and committed with the version it came from) is the fallback when the
  live fetch fails, so `opencode_operations` still answers. Live always wins; every `opencode_operations` and
  `opencode_api` result carries `source: live | snapshot(v2.0.32)`, and when the snapshot is used while
  `server.info.version` differs from it the result adds a `warning`. A mutating operation is never run from the
  snapshot when the live document could not be fetched (`SERVICE_DOWN` first).
- `operation` accepts an operation id or `METHOD /path`. Method resolution happens BEFORE execution; anything but
  GET requires `allow_mutation: true` else `MUTATION_BLOCKED`. `event.subscribe`, the pty connect routes and any
  `text/event-stream` response are `STREAMING_REFUSED`. Unknown ids are `UNKNOWN_OPERATION` with the three nearest
  ids as a hint.

### Permission handling

- `session_create` sends `permissions` as given; when absent, `permission_policy` decides: `inherit` sends null,
  `allow` sends `[{action:"*",resource:"*",effect:"allow"}]`, `ask` sends `[{..., effect:"ask"}]`, `deny` sends
  `[{..., effect:"deny"}]`.
- `session_wait` surfaces pending requests; `permission_reply` answers them; the driver skill tells the controller
  to read the request's `action` and `resource` before replying and never to reply `always` on a reject-worthy
  action.

### Host-specific behaviour

- Host identity: each per-host binary passes a `Host` constant (`Claude`, `Qwen`, `Agy`) into `cli::dispatch`; the
  cc binary alone also honours `OPENCODE_CTL_HOST=opencode` because the OpenCode front stages that binary. On
  `Opencode` or `Unknown`, `service_stop` and `service_restart` return `REFUSED_ON_HOST` (fail closed; the hint names
  the variable). On `Opencode` the `pair` and `attach` subcommands are not offered (the TUI is already open).
- The Claude Code and OpenCode fronts read `.claude/opencode-ctl-mcp-cc.local.md` and
  `.opencode/opencode-ctl-mcp-opencode.local.md` (YAML front matter, keys above, env overrides win), the re-ghidra
  precedent. Qwen and Antigravity fronts are env-only in v1 (builder may add files following the same code path).

### Launcher (`opencode-ctl-cc-mcp pair`, shipped in the same binary)

`pair [directory] [--model <provider/model>] [--title <t>] [--no-tab] [--dry-run] [-- <claude args>]`:
1. Ensure the service runs (`opencode service start`, idempotent).
2. Create the session in `directory` (default cwd) with `default_model` or `--model`; title default
   `pair <basename> <HH:MM>`.
3. Write `~/.opencode-ctl/pair.<pair-id>.json` (pair id = 8 hex chars of a uuid v4).
4. Unless `--no-tab`: write a wrapper script under `~/.opencode-ctl/` that runs `opencode <directory> -s <id>` and
   records `tui.exited_at` on exit; open it in a new tab of the terminal the user is in, trying in order
   `mate-terminal --tab -e <script>`, `gnome-terminal --tab -- <script>`, `xfce4-terminal --tab -e <script>`; on
   Windows `wt -w 0 nt -d <directory> <script.cmd>`. The terminal is detected by walking the parent process names
   (clavity's measured approach); when none is found, print the attach command and continue without a tab.
5. Run `claude <claude args>` in the current terminal with the five `OPENCODE_CTL_*` variables set; when it exits,
   record `claude.exited_at` and print "worker session <id> is still running; attach with `opencode <dir> -s <id>`".
`attach <session-id | pair-id>` opens step 4 for an existing session (lazy binding made visible).
`pair` and `attach` live in `cli::dispatch`, so the cc, qwen and agy binaries all have them; the OpenCode front does
not offer them. How a human reaches the one command from a terminal is the owner's decision (PENDING, see
"Decisions still open"); the provisional default is an `install-launcher` subcommand that links the staged binary as
`opencode-pair` into `~/.local/bin` (Windows: `%LOCALAPPDATA%\opencode-ctl\opencode-pair.cmd`) and prints the PATH
hint, so the plugin's `bin/` location never has to be typed.
`--dry-run` prints every command it would run and writes nothing; the launcher's tests use it.
Values that reach a shell: the session id must match `^ses_[A-Za-z0-9]{26}$`, the pair id `^[0-9a-f]{8}$`, the
directory is canonicalised and must exist; the wrapper script single-quotes every value (`'\''` escaping) on POSIX
and uses `""` quoting in the `.cmd` on Windows; a value that fails validation aborts the launcher before any file is
written. A unit test feeds a directory containing a quote, a space and a `$`.

### Skills

- `opencode-ctl-driver` (canonical in `shared/opencode-ctl/skill/`, emitted to each host like ghidra-mcp does):
  when to delegate; the loop `session_create -> session_prompt -> session_wait -> session_changes +
  session_messages -> verify by files and diff -> on a gap re-prompt the SAME session with the specific finding`;
  which tools spend tokens; how to answer permissions; the raw-CLI fallback (`opencode service start`,
  `opencode api session.create -d ...`, `opencode api session.prompt ...`, `opencode api
  experimental.session.wait ...`) for a host without the binary; never trust the worker's closing summary.
- `doctor`: checks in order: `opencode` binary found and version >= 2.0; service registration file present and pid
  alive; `opencode api server.info` answers; pairing (if any) resolves to a live session; prints the resolved
  settings when invoked with `verbose`.

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
- Engine protocol test (rmcp in-process client): every tool listed with a description containing "spends model
  tokens"; gating table cases (GET allowed, POST blocked, POST with `allow_mutation`, streaming refused, unknown id);
  `session_wait` state machine against the mock (idle, awaiting_permission, timeout); compaction caps; pairing
  resolution (env, file, none); host refusals.
- Per-host e2e (child-process rmcp client against the staged binary): handshake, `service_status`, one refusal.
- Launcher: `--dry-run` snapshot tests for each terminal candidate with a fake parent chain; the sentinel writer.
- OpenCode front: `bun test` for the plugin's own hooks (pattern from rtk-mcp-opencode).
- A reproduction of the spike through the MCP tools, run by hand before the capstone, is the acceptance test.

### Install and verification

Claude Code: `claude plugin install opencode-ctl-mcp@aiplugins` (registry key `opencode-ctl-mcp-cc`); verify with
`/opencode-ctl-mcp:doctor`. Other hosts follow their siblings' INSTALL.md / README patterns. First-use path: run
`service_status`, then the driver skill's loop on a throwaway directory.

## To confirm while building (resolved in the plan, not here)

- The exact `vcs.status` / `vcs.diff` request shape (`directory` query param) and whether `session.diff` ("Diff
  session turns") is the better source for `session_changes`.
- Whether `opencode api` exits non-zero on 4xx/5xx or prints the error JSON with exit 0 (the transport must handle
  both; the mock exercises both).
- `files` attachments on `session.prompt` (`PromptInput.FileAttachment` shape) for `session_prompt.files`.
- Windows: `wt` argument quoting and whether `opencode.exe` is on PATH after the official installer.
- Antigravity's `mcp_config.json` environment key for `OPENCODE_CTL_HOST`.

## Decisions still open (owner)

- How the human invokes the one command: `install-launcher` symlink (provisional), the release binary installed on
  PATH by the user, or a clavity verb. The plan's install docs and the launcher tests depend on it.

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
