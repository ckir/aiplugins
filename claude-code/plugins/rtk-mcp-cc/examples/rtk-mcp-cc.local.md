---
enabled: true
rtk_bin: rtk
ultra_compact: false
skip_env: false
---

# rtk-mcp-cc settings

Copy this file to `.claude/rtk-mcp-cc.local.md` in your project root to change
how the plugin behaves there. Every key is optional; anything you omit (or
misspell) falls back to the default shown above.

| Key | Default | Effect |
|---|---|---|
| `enabled` | `true` | When `false`, the hook passes every command through untouched. |
| `rtk_bin` | `rtk` | The rtk executable. Looked up on `PATH` unless it contains a path separator. |
| `ultra_compact` | `false` | Pass `--ultra-compact` to rtk (Level 2 optimizations: ASCII icons, inline format). |
| `skip_env` | `false` | Pass `--skip-env` to rtk, setting `SKIP_ENV_VALIDATION=1` for child processes (Next.js, tsc, lint, prisma). |

## Environment overrides

Environment variables take precedence over this file, so you can change
behavior for a single session without editing anything:

| Variable | Effect |
|---|---|
| `RTK_BIN` | Path to the rtk executable. Shared with the sibling `rtk-mcp-qwen` extension, so relocating rtk needs only one variable. |
| `RTK_CC_DISABLE=1` | Pass every command through untouched. |
| `RTK_CC_ULTRA_COMPACT` | Overrides `ultra_compact`. |
| `RTK_CC_SKIP_ENV` | Overrides `skip_env`. |

Truthy values are `1`, `true`, `yes`, `on`; falsy are `0`, `false`, `no`, `off`
(case-insensitive). `RTK_CC_DISABLE=0` therefore leaves the hook **active** —
unset it or use `RTK_CC_DISABLE=1` to actually turn things off.

`.claude/*.local.md` is per-developer and belongs in `.gitignore`. Only the
`.local.md` name is read — there is no committed, team-shared variant. To give a
whole team the same setting, set the environment variable instead.
