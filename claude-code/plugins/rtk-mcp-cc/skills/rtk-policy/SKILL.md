---
name: rtk-policy
description: Explains how rtk (Rust Token Killer) rewrites shell commands in this session and which CLI tools to prefer for token-cheap output. Use when a command comes back rewritten or prefixed with `rtk`, when output looks unexpectedly abbreviated, when choosing between grep/rg or find/fd or cat/bat, when deciding whether to prefix a command with rtk manually, or when the user asks about rtk, token savings, or why a command was changed.
---

# Working with rtk

`rtk` (Rust Token Killer) rewrites shell commands into token-cheaper
equivalents. This plugin installs it as a `PreToolUse` hook, so the rewrite
happens **between** the command being written and the command being run.

## The one rule that matters

**Write ordinary commands. Do not prefix anything with `rtk` yourself.**

The hook already does it. Writing `rtk grep foo` produces a command that is
either double-processed or — because rtk's rewrite is idempotent — silently
identical to what the hook would have produced anyway. Either way the manual
prefix buys nothing and makes the command harder to read.

Equally: do not try to escape the rewrite by piping around it, wrapping the
command in `bash -c`, or splitting it to dodge the pattern match. If a rewrite
is actively wrong for a task, use the escape hatches below rather than
obfuscating the command.

## What gets rewritten

rtk recognizes specific, well-known commands and swaps them for optimized
equivalents with terser output. Commands it does not recognize — `git`, `npm`,
`cargo`, project scripts — pass through and simply run.

This means the rewrite is **not** something to plan around. Write the command
that expresses the intent; rtk handles the rest or gets out of the way.

To see what rtk would do to a specific command without running it, call the
`rtk_check` tool from this plugin's MCP server. That is the right way to answer
"why did my command change?" — it reports the rewrite without executing
anything.

## Preferring token-cheap tools

Where a choice exists, prefer the modern Rust-family CLI tools. They emit less
noise per useful line, which is the whole point:

| Instead of | Prefer | Why |
|---|---|---|
| `grep` | `rg` (ripgrep) | Faster, respects `.gitignore`, terser output |
| `find` | `fd` | Shorter syntax, sane defaults, less chatter |
| `sed -i` | `sd` | Literal-string mode, no regex-escaping traps |
| `ls` | `eza` | Cleaner columns |
| manual JSON parsing | `jq` / `yq` | Structured, no ad-hoc text munging |

Two cautions on `bat`:

- Always pass `--style=plain --color=never --paging=never`. Its default colors,
  line-number gutter, and pager are pure token noise, and the pager can hang a
  non-interactive session outright.
- Prefer the native `Read` tool over `bat` for reading files. `Read` is already
  optimized, whereas `bat` is not covered by rtk's rewrite rules and so bypasses
  the savings entirely.

The native `Read`, `Grep`, and `Glob` tools already use ripgrep-class machinery.
Keep using them for reading and searching; the guidance above governs commands
that actually go through a shell.

## Escape hatches

When a rewrite genuinely gets in the way:

- **One command, unfiltered but still counted** — call the `rtk_proxy` MCP tool
  with an argv vector. It runs the command without rewriting while keeping it in
  rtk's statistics. It executes, so use it deliberately.
- **A whole session** — set `RTK_CC_DISABLE=1`, or put `enabled: false` in
  `.claude/rtk-mcp-cc.local.md`. Both make the hook pass every command through
  untouched.

## When rtk is not installed

The hook fails open by design: if `rtk` is missing, errors, or declines to
rewrite, the command runs exactly as written and nothing is reported. So a
command running unmodified is never evidence of a problem — it is the expected
behavior in an environment without rtk.

If the user expects savings and is not getting them, check that `rtk` is on
`PATH` (or that `RTK_BIN` points at it) rather than assuming the hook is broken.
