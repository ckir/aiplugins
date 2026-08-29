# rtk-mcp-agy

An Antigravity hook integration for `rtk` (Rust Token Killer), designed to optimize token usage for shell commands executed by the Antigravity (agy) agent.

## Overview

The `rtk-mcp-agy` crate provides a mechanism to transparently intercept and rewrite shell commands executed by the Antigravity agent through `rtk rewrite`. By using optimized tools (e.g., `rg`, `fd`, `sd`) instead of native tools (like `grep`, `find`, `sed`), it achieves significant token savings.

Because Antigravity's `PreToolUse` hook schema lacks a way to modify tool arguments directly, this project uses an MCP (Model Context Protocol) Server proxy alongside a `PreInvocation` hook injection to instruct the agent to use the optimized MCP tool.

## Components

This crate provides two main binaries:

1. **`rtk-hook-preinvocation`**: A `PreInvocation` hook that injects an `ephemeralMessage` at the start of the agent's turn. It instructs the model to use the `rtk_run` MCP tool instead of its native `run_command` tool.
2. **`rtk-mcp`**: A JSON-RPC MCP server over stdio that exposes the `rtk_run` tool. When invoked, it passes the given command to `rtk rewrite`, executes the optimized command (or falls back to the original if un-rewritable), and returns the output to the agent.

## Installation and Configuration

1. **Get the Binaries**:
   Download the latest precompiled binaries from the [Releases page](https://github.com/ckir/aiplugins/releases) and place them in your environment PATH.

   *(Alternatively, if building locally: run `cargo build --release` and ensure the output binaries `rtk-hook-preinvocation` and `rtk-mcp` are in your PATH).*

2. **Install the Plugin (Repo-Scoped)**:
   It is recommended to install this plugin repo-scoped for your project. Clone this repository and configure it in your project's `.agents/plugins.json`:
   ```bash
   git clone https://github.com/ckir/aiplugins.git
   ```
   Then create or update your project's `.agents/plugins.json` (at the root of your workspace) to point to the cloned directory:
   ```json
   {
     "entries": [
       {
         "path": "path/to/aiplugins/antigravity/rtk-mcp-agy"
       }
     ]
   }
   ```
   *(Alternatively, you can simply copy the `rtk-mcp-agy` directory directly into your project's `.agents/plugins/` folder).*



## Configuration

Both binaries read their configuration from the environment.

| Variable | Default | Effect |
|---|---|---|
| `RTK_BIN` | `rtk` (on `PATH`) | The rtk executable used for rewriting. Shared with the sibling `rtk-mcp-qwen` and `rtk-mcp-cc` plugins, so relocating rtk needs one variable, not three. |
| `RTK_AGY_SHELL` | `pwsh` on Windows, `sh` elsewhere | The shell that runs the rewritten command. It is invoked as `<shell> -c "<command>"`. |

A blank value (`RTK_BIN=`) counts as unset and falls back to the default.

Neither is required. If `rtk` cannot be spawned there is simply no rewrite and
the original command runs — rtk only covers specific known commands, so most
invocations take that path anyway.

> **Note for non-Windows hosts.** The shell used to be a hardcoded `pwsh`, which
> meant `rtk_run` could not execute anything on Linux or macOS unless PowerShell 7
> happened to be installed. The platform default now handles that; set
> `RTK_AGY_SHELL` only if you want something other than `sh`.

## Design Decisions

For more detailed architectural choices and the reasoning behind this proxy approach, see [DESIGN.md](DESIGN.md).
