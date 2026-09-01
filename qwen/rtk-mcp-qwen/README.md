# rtk-mcp-qwen — RTK Command Rewriter Hook

A Qwen Code extension that installs a **PreToolUse hook** to intercept
`run_shell_command` tool calls and rewrite the command via the external
`rtk` CLI before execution.

## Purpose

When the AI agent is about to run a shell command, this hook gives `rtk` a
chance to produce a safer, more idiomatic, or context-appropriate version of
the command. If `rtk` is unavailable or declines to rewrite, the original
command passes through unchanged (fail-open).

## Installation

### Via Qwen Code extension bundle (recommended)

```bash
qwen extensions install https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-extension.zip
```

> **Note:** As of Qwen Code v0.22.3, `qwen extensions install` from an archive
> URL may exit silently without installing (tracked in
> [QwenLM/qwen-code#10741](https://github.com/QwenLM/qwen-code/issues/10741)).
> If that happens, use the manual install below.

### Manual install (workaround)

1. Download the extension bundle for your platform:

   | Platform | URL |
   |---|---|
   | Windows | https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-extension.zip |
   | Linux x86_64 | https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-extension.zip |
   | macOS x86_64 | https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-extension.zip |
   | macOS aarch64 | https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-extension.zip |

2. Extract it to your Qwen extensions directory:

   ```bash
   # Linux / macOS
   mkdir -p ~/.qwen/extensions/rtk-mcp-qwen
   unzip rtk-mcp-qwen-extension.zip -d ~/.qwen/extensions/rtk-mcp-qwen/
   ```

   ```powershell
   # Windows (PowerShell)
   New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.qwen\extensions\rtk-mcp-qwen"
   Expand-Archive -Path rtk-mcp-qwen-extension.zip -DestinationPath "$env:USERPROFILE\.qwen\extensions\rtk-mcp-qwen" -Force
   ```

3. Verify it was detected:

   ```bash
   qwen extensions list
   ```

   You should see `RTK MCP Qwen — RTK Command Rewriter` listed and enabled.

### Building from source

```bash
git clone https://github.com/ckir/aiplugins.git
cd aiplugins
cargo build -p rtk-mcp-qwen --release
qwen extensions link ./qwen/rtk-mcp-qwen
```

### Updating

```bash
qwen extensions update rtk-mcp-qwen
```

Check your current version:

```bash
qwen extensions list | grep rtk
```

## Structure

```
qwen/rtk-mcp-qwen/
├── qwen-extension.json   # Extension manifest with PreToolUse hook wiring
├── Cargo.toml            # Rust workspace crate
├── src/
│   └── main.rs           # Hook binary (stdio JSON-RPC)
└── QWEN.md               # Extension context
```

## Hook contract

**Input** (stdin):
```json
{
  "tool_name": "run_shell_command",
  "tool_input": { "command": "rm -rf /tmp/old" }
}
```

**Output** (stdout) — on rewrite:
```json
{
  "hook_specific_output": {
    "hook_event_name": "PreToolUse",
    "permission_decision": "allow",
    "permission_decision_reason": "RTK rewrite applied: rm -rf /tmp/old -> rm -rf /tmp/old",
    "updated_input": { "command": "<rewritten>" }
  }
}
```

**Output** (stdout) — on pass-through:
```json
{
  "hook_specific_output": {
    "hook_event_name": "PreToolUse",
    "permission_decision": "allow",
    "permission_decision_reason": "No RTK rewrite available for: ..."
  }
}
```

## Configuration

The `rtk` binary must be on your `PATH`. Override by setting `RTK_BIN`. A blank
value (`RTK_BIN=`) counts as unset and falls back to `rtk` on `PATH` — the same
resolution the sibling `rtk-mcp-agy` and `rtk-mcp-cc` plugins use, so relocating
rtk takes one variable rather than three.

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "^run_shell_command$",
      "hooks": [{
        "type": "command",
        "command": "cargo",
        "args": ["run", "-p", "rtk-mcp-qwen", "--bin", "rtk-mcp-qwen", "--quiet"],
        "cwd": "${extensionPath}${/}..${/}..",
        "env": { "RTK_BIN": "/opt/rtk/bin/rtk" },
        "name": "rtk-command-rewrite",
        "timeout": 10000
      }]
    }]
  }
}
```

## rtk exit codes

| Code | Meaning |
|------|---------|
| 0    | Success — rewritten command on stdout |
| 1    | No rewrite available |
| 3    | Success — rewritten command on stdout (special case) |
| Other| Failure — hook passes through with original command |

## Linking

```bash
qwen extensions link ./qwen/rtk-mcp-qwen
```
