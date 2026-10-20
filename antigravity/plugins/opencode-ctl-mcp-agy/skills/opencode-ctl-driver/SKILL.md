---
name: opencode-ctl-driver
description: Delegate bounded implementation work to an OpenCode worker session on a cheaper or free model and verify it. Use when the user asks to hand a task, plan or file to opencode, to delegate to a cheaper or free model, to have another agent (not the host's own built-in subagents) implement a plan, to run something in the opencode worker, to check what the worker did, or when an opencode-ctl tool returns awaiting_permission, awaiting_input, stalled, budget_exceeded, omitted_patches or patch_truncated. Not for questions about OpenCode itself
allowed-tools: Bash, Read, Write
---

You are running as Antigravity; this skill is the macro workflow for delegating bounded implementation work
to an OpenCode worker session on a cheaper or free model and verifying it, driven through the
`opencode-ctl-mcp` tools. Read each tool's own description for parameters and error codes; read this for the
loop.

# When to delegate

Delegate to an OpenCode worker session when the user asks to hand a task, a plan or a file to opencode; to
delegate work to a cheaper or free model; to have another agent (not the host's own built-in subagents)
implement a plan; to run something in the opencode worker; or to check what the worker did. The worker's
model is a configuration the loop checks, not a property of the tool: "cheaper or free" is what the user
configured, so say so when the server default is used instead. Do not use this skill for questions about
OpenCode itself.

# The loop

1. Call `service_status` first. When it reports not running, call `service_start`; on the OpenCode host the
   service is up by definition.
2. If the result is `paired`, USE THAT SESSION: never create a second one beside the human's tab unless a
   parallel worker is wanted on purpose. Otherwise call `session_create`, and tell the user which `model`
   the result names when it is the server default rather than a configured cheap one.
3. Call `session_prompt`, then `session_wait` with `after = <message_id>` on EVERY slice, dispatching on the
   result's `state` by this table:

| `state` | action |
| --- | --- |
| `running` | call again with the same `after` |
| `idle` | go to VERIFY |
| `failed` | report `error`, ask the user before re-prompting |
| `interrupted` | report, ask the user |
| `awaiting_permission` | read `action`, `resources`, `save`, answer with `permission_reply` (`once` by default; `reject` rejects every pending request of that session; `always` needs the human's say-so), then wait again |
| `awaiting_input` | `form_reply` with visible required fields, then wait again |
| `stalled` | if the previous slice returned `SERVICE_DOWN`, read `session_messages` for the restart notice and `session_get` for the state before anything else (the server resumes the turn by itself); otherwise `session_interrupt`, report, ask the user |
| `budget_exceeded` | `session_interrupt`, report, ask the user |
| `timeout` | only with `hard: true`; treat as `running` |

Independently of the state, when the result carries `concurrent_turn = <id>` the human's turn is running:
wait PASSIVELY with `after = <id>` for at most `max_turn_s`, answer none of its permissions or forms, never
interrupt it, tell the user once what it is waiting on, then VERIFY when it ends (a human prompt steered INTO
the controller's turn joins it, so the controller's `session_changes` range then includes the human's work and
the report says so).

The `pair` line is a separate rule; each value has its own action:
- `tui = dead` or `unknown` -> tell the user the tab is gone (`opencode-pair attach <pair-id>` reopens it),
  keep driving.
- `tui = starting` -> say nothing yet.
- `tui = none` -> no tab was requested, say nothing.
- a tab without `server_url` reconnects itself after `service_restart`.

When `server_url` is set, two behaviours of this loop change:
- `service_start` with `server_url` set only re-checks reachability, so when it returns `running: false`
  tell the user the remote server at `server_url` must be started by hand, and when it returns an error
  (`AUTH_FAILED`: a wrong or missing password for that server) run `/opencode-ctl-mcp-agy:opencode-ctl-doctor` instead.
- `service_stop` and `service_restart` are `REFUSED_ON_HOST` with `server_url` set; the Related line routes
  that code to the doctor.

# Verify

VERIFY the work once `session_wait` reports `idle`:
- Call `session_changes` with `from` = the prompt id for this turn, plus `session_messages`.
- Verify by files and diff, not by the worker's prose.
- Any of `omitted_patches`, `patch_truncated` or `omitted_files` is never a pass: page by `paths` or read the
  file.
- On a gap re-prompt the SAME session with the specific finding.
- Never trust the worker's closing summary.

# Which tools spend worker tokens

- `session_prompt` spends worker tokens: yes.
- `opencode_api` depends on the operation it calls.
- All other tools spend none.

# Answering permissions and forms

- For `awaiting_permission`, read `action`, `resources` and `save` before answering.
- Answer with `permission_reply`, `once` by default.
- `reject` rejects every pending request of that session.
- `always` needs the human's say-so.
- For `awaiting_input`, answer with `form_reply`, filling only the visible required fields.

# Raw-CLI fallback

For a host without the binary, drive the worker with the raw CLI. In managed mode (no `server_url`) the four
commands are:
- `opencode service start`
- `opencode api session.create -d '{"title":...,"location":{"directory":...}}'`
- `opencode api session.prompt --param sessionID=<id> -d '{"text":...}'`
- `opencode api experimental.session.wait --param sessionID=<id>`

When `server_url` is set, the list is instead the LAST THREE of those, each with `--server <url>` appended,
because a plain `opencode api` targets or starts the LOCAL managed service, and `opencode service start` is
dropped from it because it has no `--server` flag and starts only the local service:
- `opencode api session.create -d '{"title":...,"location":{"directory":...}}' --server <url>`
- `opencode api session.prompt --param sessionID=<id> -d '{"text":...}' --server <url>`
- `opencode api experimental.session.wait --param sessionID=<id> --server <url>`

# Cheap model check

When `session_create` returns the server default model rather than a configured cheap or free one, tell the
user: the description's "cheaper or free model" is a configuration the loop checks, not a property of the
tool.

# Related

This skill is `/opencode-ctl-mcp-agy:opencode-ctl-driver`; when a tool returns OPENCODE_NOT_FOUND, SERVICE_DOWN, AUTH_FAILED, UNKNOWN_OPERATION or REFUSED_ON_HOST, run `/opencode-ctl-mcp-agy:opencode-ctl-doctor`.
