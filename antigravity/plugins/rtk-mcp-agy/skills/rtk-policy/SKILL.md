---
name: rtk-policy
description: Explains how rtk (Rust Token Killer) rewrites shell commands in Antigravity and which CLI tools to prefer for token-cheap output. Use when a command comes back rewritten or prefixed with `rtk`, when output looks unexpectedly abbreviated, when choosing between grep/rg or find/fd or cat/bat, when deciding whether to prefix a command with rtk manually, or when the user asks about rtk, token savings, or why a command was changed.
---

# Working with rtk in Antigravity

`rtk` (Rust Token Killer) rewrites shell commands into token-cheaper equivalents.
In Antigravity, the `PreInvocation` hook directs shell operations to `rtk_run` on the `rtkmcp` server.

## The one rule that matters

**Write ordinary commands. Do not prefix anything with `rtk` yourself.**

The tool handles rewriting automatically. Writing `rtk grep foo` produces a command that is either double-processed or redundant.

To see what rtk would do to a specific command without running it, call the `rtk_check` tool from this plugin's MCP server.

## Preferring token-cheap tools

Where a choice exists, prefer modern CLI tools:
- `rg` (ripgrep) instead of `grep`
- `fd` instead of `find`
- `sd` instead of `sed -i`
- `eza` instead of `ls`
- `jq` / `yq` instead of manual text munging
