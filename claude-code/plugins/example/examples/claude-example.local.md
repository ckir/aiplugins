---
require_owner: true
kinds: [TODO, FIXME, HACK]
max_results: 200
---

# claude-example settings

Copy this file to `.claude/claude-example.local.md` in your project root to
change how the plugin behaves there. Every key is optional; anything you omit
(or misspell) falls back to the default shown above.

| Key | Default | Effect |
|---|---|---|
| `require_owner` | `true` | When `false`, the PostToolUse hook stops reporting unowned markers. |
| `kinds` | `[TODO, FIXME, HACK]` | Which markers the hook and the MCP server look for. |
| `max_results` | `200` | Cap on markers returned by a single `scan_todos` scan. |

`.claude/*.local.md` is per-developer and belongs in `.gitignore`. Commit a
shared default as `.claude/claude-example.md` only if your team wants one.
