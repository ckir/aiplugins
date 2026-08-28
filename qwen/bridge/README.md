# qwen-bridge — RTK Command Rewriter Hook

A Qwen Code extension that installs a **PreToolUse hook** to intercept
`run_shell_command` tool calls and rewrite the command via the external
`rtk` CLI before execution.

## Purpose

When the AI agent is about to run a shell command, this hook gives `rtk` a
chance to produce a safer, more idiomatic, or context-appropriate version of
the command. If `rtk` is unavailable or declines to rewrite, the original
command passes through unchanged (fail-open).

## Structure

```
qwen/bridge/
├── qwen-extension.json   # Extension manifest with PreToolUse hook wiring
├── Cargo.toml            # Rust workspace crate
├── src/
│   └── main.rs           # Hook binary (stdio JSON-RPC)
└── QWEN.md               # Extension context
```

## Building

```bash
cargo build -p qwen-bridge --bin rtk-mcp-qwen
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

The `rtk` binary must be on your `PATH`. Override by setting `RTK_BIN`:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "^run_shell_command$",
      "hooks": [{
        "type": "command",
        "command": "cargo",
        "args": ["run", "-p", "qwen-bridge", "--bin", "rtk-mcp-qwen", "--quiet"],
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
qwen extensions link ./qwen/bridge
```
