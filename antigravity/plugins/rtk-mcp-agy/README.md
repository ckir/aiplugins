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

1. **Download the versioned plugin bundle**:
   pick the version you want (`0.7.2` is current as of this writing) and
   fetch its `-agy-plugin.zip` — the doubled name is
   `<package>-agy-plugin.zip` for package `rtk-mcp-agy`:

   ```bash
   curl -fsSL -o /tmp/rtk-mcp-agy-agy-plugin.zip \
     https://github.com/ckir/aiplugins/releases/download/rtk-mcp-agy-v0.7.2/rtk-mcp-agy-agy-plugin.zip
   unzip -q -o /tmp/rtk-mcp-agy-agy-plugin.zip -d /tmp/rtk-mcp-agy
   ```

   The zip carries the plugin files plus the binaries for every supported
   platform — Windows x64, and Linux and macOS on both x86_64 and
   aarch64 — so nothing is compiled at install time. (Hacking on the
   binaries instead? `cargo build -p rtk-mcp-agy` from a checkout builds
   them from source.)

2. **Install the plugin** from the extracted directory into your
   user-scoped global configuration:

   ```bash
   agy plugin install /tmp/rtk-mcp-agy
   ```

   This copies the directory into place — after downloading a newer
   bundle, install again from its fresh extraction.

   If you prefer a **Repo-Scoped** installation instead, point your
   project's `.agents/plugins.json` (at the root of your workspace) at
   the extracted directory:
   ```json
   {
     "entries": [
       {
         "path": "path/to/aiplugins/antigravity/plugins/rtk-mcp-agy"
       }
     ]
   }
   ```
   *(Alternatively, you can simply copy the extracted `rtk-mcp-agy` directory directly into your project's `.agents/plugins/` folder).*

3. **Put the installed `bin/` on `PATH`.** `hooks.json` and
   `mcp_config.json` name bare binaries (`rtk-hook-preinvocation`,
   `rtk-mcp`) and never learn about platforms: the host resolves them
   through `PATH` — it does not add the plugin's `bin/` itself. On
   macOS/Linux the bare names hit the `bin/<name>` dispatcher, which
   execs the build for the running machine (`bin/<target>/<name>`); on
   Windows they resolve to the `bin/<name>.exe` siblings. So export the
   installed copy's `bin/` (under your agy user config, e.g.
   `~/.gemini/config/plugins/rtk-mcp-agy/bin`), then confirm:

   ```bash
   agy plugin validate <installed plugin directory>
   ```

   which fails `not found on PATH` until the binaries resolve.



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
