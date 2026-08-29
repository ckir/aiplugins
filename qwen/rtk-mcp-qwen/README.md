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

### From release artifacts (recommended)

Download the latest release from <https://github.com/ckir/aiplugins/releases>
and extract the `rtk-mcp-qwen` archive for your platform:

**Windows (x86_64):**
```powershell
curl -LO https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-x86_64-pc-windows-msvc.zip
Expand-Archive rtk-mcp-qwen-x86_64-pc-windows-msvc.zip -DestinationPath rtk-mcp-qwen
```

**macOS (x86_64 / Apple Silicon):**
```bash
curl -LO https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-$(uname -m)-apple-darwin.tar.xz
tar xf rtk-mcp-qwen-*.tar.xz
```

**Linux (x86_64 / aarch64):**
```bash
curl -LO https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-qwen-$(uname -m)-unknown-linux-gnu.tar.xz
tar xf rtk-mcp-qwen-*.tar.xz
```

Then place the binary in your `PATH` (e.g. `~/.local/bin`, `~/bin`, or `/usr/local/bin`):

```bash
mkdir -p ~/.local/bin
cp rtk-mcp-qwen* ~/.local/bin/   # add the platform-specific binary
chmod +x ~/.local/bin/rtk-mcp-qwen
```

Verify the installation:

```bash
rtk-mcp-qwen --version
rtk-mcp-qwen --help
```

### Updating

To update, simply download the latest release again and replace the binary
in your `PATH`. Check your current version first:

```bash
rtk-mcp-qwen --version
```

### Building from source

```bash
cargo build -p rtk-mcp-qwen --bin rtk-mcp-qwen --release
```

The binary will be at `target/release/rtk-mcp-qwen`.

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
