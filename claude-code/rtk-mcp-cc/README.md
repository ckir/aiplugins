# rtk-mcp-cc

A Claude Code plugin that wires [`rtk`](https://github.com/ckir/rtk) (Rust Token
Killer) into every shell command Claude runs, for 60–90% fewer output tokens on
ordinary dev operations — plus rtk's analytics as MCP tools.

It is the Claude Code member of a family: `qwen/rtk-mcp-qwen` does this for Qwen
Code, and `antigravity/rtk-mcp-agy` for Antigravity.

## How it works

Claude Code's `PreToolUse` hook can rewrite a tool's input before it runs, by
returning `hookSpecificOutput.updatedInput`. `rtk` already speaks that exact
schema natively:

```console
$ echo '{"tool_name":"Bash","tool_input":{"command":"cat README.md"}}' | rtk hook claude
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecisionReason":"RTK auto-rewrite",
 "updatedInput":{"command":"rtk read README.md"}}}
```

So this plugin **owns no rewriting logic**. `rtk-cc-hook` pipes the event to
`rtk hook claude` and forwards the verdict. Re-implementing the rewrite here
would duplicate knowledge rtk owns and drift the moment rtk changes.

> This is the opposite of the Antigravity sibling, which needs an MCP proxy and
> a prompt injection *because* agy's `PreToolUse` cannot modify tool arguments.
> Claude Code can, so the direct hook is both simpler and more reliable.

### Fail-open, always

The hook always exits 0, and writes nothing when it has no opinion. Empty output
is Claude Code's "proceed unchanged" — and it is also rtk's own signal for
"nothing to rewrite", so the failure path and the no-op path are the same path
by construction. A missing, broken, or slow `rtk` therefore costs you nothing:
the command runs exactly as written.

Specifically, the command passes through untouched when rtk is not installed,
rtk exits non-zero, rtk emits anything that is not JSON, the payload is empty,
or the plugin is disabled.

## What it demonstrates

| Extension point | File | What it does here |
|---|---|---|
| Manifest | `.claude-plugin/plugin.json` | Name, version, metadata |
| Hook | `hooks/hooks.json` → `bin/rtk-cc-hook` | `PreToolUse` on `Bash`/`PowerShell`, delegates to `rtk hook claude` |
| MCP server | `.mcp.json` → `bin/rtk-cc-mcp` | `rtk_gain`, `rtk_discover`, `rtk_check`, `rtk_proxy` |
| Skill (auto) | `skills/rtk-policy/SKILL.md` | How to work with rtk; which CLI tools are token-cheap |
| Skill (invoked) | `skills/gain/SKILL.md` | `/rtk-mcp-cc:gain` |
| Settings | `examples/rtk-mcp-cc.local.md` | Per-project config via `.claude/*.local.md` |

## Prerequisites

`rtk` on your `PATH` (or `RTK_BIN` pointing at it). Verify with:

```bash
rtk --version   # should print: rtk 0.45.0 (or newer)
```

Without it the plugin installs and runs fine — it simply never rewrites
anything. Note the name collision: if `rtk gain` fails, you may have
reachingforthejack/rtk (Rust Type Kit) installed instead.

## Building

The config files point at `bin/`, so the binaries must be built before the
plugin does anything:

```bash
just build-rtk-mcp-cc
```

That stages `rtk-cc-hook` and `rtk-cc-mcp` into `claude-code/rtk-mcp-cc/bin/`,
which is gitignored. Release builds for all platforms come from CI via
`cargo-dist`.

## Installing

This repository is a Claude Code marketplace, so the plugin installs like any
other — no clone, no build:

```bash
claude plugin marketplace add ckir/aiplugins
claude plugin install rtk-mcp-cc@aiplugins
```

Working inside a clone of this repository, skip the first line: `.claude/settings.json`
declares the marketplace already.

That fetches `rtk-mcp-cc-plugin.zip` from the latest release, which carries the
binaries for every supported platform: Windows x64, and Linux and macOS on both
x86_64 and aarch64. Nothing is compiled at install time.

To run the working copy instead — what you want while changing the plugin —
build the binaries first and point Claude Code at the directory:

```bash
just build-rtk-mcp-cc
claude --plugin-dir claude-code/rtk-mcp-cc
```

### Replacing a hand-wired `rtk hook claude`

`rtk init -g` installs the hook directly into `~/.claude/settings.json`, which
looks like this:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash|PowerShell",
      "hooks": [{ "type": "command", "command": "rtk hook claude" }]
    }]
  }
}
```

**Remove that entry when you install this plugin** — it is the same hook, on the
same matcher, and the plugin supersedes it with settings, an escape hatch, and
fail-open handling the raw entry does not have.

Leaving it in place is *not* harmful, though. rtk's rewrite is idempotent:
feeding an already-rewritten command back through `rtk hook claude` produces
empty stdout and exit 0. Verified against rtk 0.45.0, and pinned by the
`hook_is_silent_when_there_is_nothing_to_rewrite` test. You will pay one extra
process spawn per command and gain nothing.

### `[rtk] /!\ No hook installed` while the hook is installed

You will keep seeing this on stderr after switching:

```console
[rtk] /!\ No hook installed — run `rtk init -g` for automatic token savings
```

rtk decides whether a hook exists by looking for the literal command
`rtk hook claude` in Claude Code's `settings.json`. This plugin supplies the
same hook from its own `hooks/hooks.json`, which rtk cannot see — so it reports
"no hook installed" while the hook is installed and rewriting. The notice is
wrong rather than merely noisy, and the remedy it proposes is the duplicate
entry the section above tells you to remove.

Claude Code ignores hook stderr on a zero exit, so nothing breaks. To silence it,
give rtk the string it is looking for in a hook that can never run:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "RtkHookIsProvidedByThePlugin",
        "hooks": [{ "type": "command", "command": "rtk hook claude" }]
      }
    ]
  }
}
```

A matcher is a regex tested against tool names — `Bash`, `Read`, `Edit` — so
this one matches nothing and the entry never executes. It exists only to be
read. rtk stops complaining, the plugin's hook stays the one doing the work, and
no command pays a second process spawn.

**Put it in the settings file of the config directory Claude Code is actually
using.** That is `~/.claude/settings.json` for an ordinary install, but
`$CLAUDE_CONFIG_DIR/settings.json` when Claude Code runs under a profile — and
under a profile the two disagree: marking only `~/.claude` leaves the plugin's
MCP tools still reporting the notice, because the rtk they spawn reads the
profile's file. If it persists, mark both.

This is a workaround, not a fix: it depends on rtk's check being a string search,
and it lives outside the repository, so a new machine or a new profile needs it
again. The fix belongs upstream — rtk recognising a plugin-provided hook, or
offering a switch to turn the notice off.

## Configuration

See `examples/rtk-mcp-cc.local.md` for the full table. In short — copy it to
`.claude/rtk-mcp-cc.local.md`, or use the environment, which takes precedence:

| Variable | Effect |
|---|---|
| `RTK_BIN` | Path to the rtk executable. Shared with `rtk-mcp-qwen`. |
| `RTK_CC_DISABLE=1` | Pass every command through untouched. |
| `RTK_CC_ULTRA_COMPACT=1` | Pass `--ultra-compact` to rtk. |
| `RTK_CC_SKIP_ENV=1` | Pass `--skip-env` to rtk. |

## MCP tools

| Tool | Runs a command? | Purpose |
|---|---|---|
| `rtk_gain` | no | Token-savings summary, optionally with per-command history |
| `rtk_discover` | no | Commands that could have been optimized but were not |
| `rtk_check` | no | How rtk *would* rewrite a given command — the way to answer "why did my command change?" |
| `rtk_proxy` | **yes** | Runs a command unfiltered but still tracked |

`rtk_proxy` is the one tool here that executes. It takes an **argv vector**, not
a shell line — `["git", "status"]`, not `"git status"` — and spawns it directly,
so no pipe, redirect, glob or `;` is ever interpreted. Prefer the ordinary
`Bash` tool unless you specifically need to bypass rtk's rewriting while keeping
the command in rtk's statistics.

`rtk gain --reset` is deliberately unreachable: it zeroes the user's saved
statistics, and an MCP tool that can silently destroy data is not worth the
convenience. The `gain_args_can_never_reach_reset` test keeps it that way.

## Layout

```text
claude-code/rtk-mcp-cc/
├── .claude-plugin/plugin.json   # manifest (must be in .claude-plugin/)
├── .mcp.json                    # MCP server registration
├── hooks/hooks.json             # hook registration
├── skills/
│   ├── rtk-policy/SKILL.md
│   └── gain/SKILL.md
├── examples/
│   └── rtk-mcp-cc.local.md      # settings template
├── bin/                         # built binaries (gitignored)
├── src/
│   ├── lib.rs                   # settings, argv building, hook decision (pure, tested)
│   └── bin/
│       ├── hook.rs              # PreToolUse hook binary
│       └── mcp.rs               # MCP server binary
└── tests/
    ├── bin/mock_rtk.rs          # fake rtk, so the suite needs no real one
    └── e2e.rs                   # drives both real binaries
```

Component directories sit at the plugin **root**, not inside `.claude-plugin/`.
Only the manifest goes in `.claude-plugin/`.

## Testing

```bash
cargo nextest run -p rtk-mcp-cc
```

The suite points `RTK_BIN` at the `mock-rtk-cc` fixture, so it runs identically on
a machine that has never had rtk installed — and can force the failure modes
(missing binary, non-zero exit, prose on stdout) rather than waiting for them.

`mock-rtk-cc` is a `[[bin]]` so the tests can reach it via `CARGO_BIN_EXE_mock-rtk-cc`,
but `[package.metadata.dist.binaries]` keeps it out of release artifacts.

## Naming

The `-mcp-` in the name matches the sibling plugins (`rtk-mcp-agy`,
`rtk-mcp-qwen`) rather than describing a hierarchy. Unlike `rtk-mcp-qwen`, which
carries no MCP server at all, this one genuinely does.
