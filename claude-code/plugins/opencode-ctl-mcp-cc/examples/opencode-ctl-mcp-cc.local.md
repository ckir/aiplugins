---
wait_timeout_s: 120
poll_s: 10
stall_s: 90
max_turn_s: 1800
call_timeout_s: 30
output_max_bytes: 16384
body_max_bytes: 100000
---

# opencode-ctl-mcp-cc settings

Copy this file to `.claude/opencode-ctl-mcp-cc.local.md` in your project root to change how the plugin
behaves there. Every key is optional; anything you omit (or misspell) falls back to the default shown.

| Key | Default | Range | Effect |
|---|---|---|---|
| `wait_timeout_s` | `120` | 5..`wait_max_s` | Length of one `session_wait` slice. |
| `poll_s` | `10` | 2..60 | Polling interval inside a wait. |
| `stall_s` | `90` | 30..600 | Seconds without progress before a turn is reported `stalled`. |
| `max_turn_s` | `1800` | 60..7200 | Budget for one turn before `budget_exceeded`. |
| `call_timeout_s` | `30` | 5..120 | Deadline for a single `opencode` call. |
| `output_max_bytes` | `16384` | 1024..65536 | Cap on one tool result. |
| `body_max_bytes` | `100000` | 1024..100000 | Cap on a request body (24000 on Windows). |

## Protected keys

These keys are read from the environment or from the per-user file `~/.opencode-ctl/settings.local.md`
only, never from the workspace file above, so a cloned repository cannot redirect the plugin:
`opencode_bin`, `server_url`, `default_model`, `default_agent`, `permission_policy` (`inherit`),
`permissions_locked` (`false`) and `wait_max_s` (300, range 30..600). A `server_url` carrying userinfo
(`user:pass@`) is ignored with a note.

## Environment overrides

Environment variables take precedence over both files. The name is `OPENCODE_CTL_` plus the key upper-cased,
except `OPENCODE_CTL_BIN` (`opencode_bin`) and `OPENCODE_CTL_SERVER` (`server_url`). An empty variable still
overrides the files. `OPENCODE_CTL_HOME` relocates `~/.opencode-ctl/`; `OPENCODE_CTL_LOG=debug` raises the
log level.

`.claude/*.local.md` is per-developer and belongs in `.gitignore`.
