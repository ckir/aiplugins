# re-ghidra-mcp-qwen — Ghidra MCP for Qwen Code

This extension drives a persistent headless Ghidra JVM from Qwen Code via MCP,
providing **19 reverse-engineering tools**: decompile, navigate and search a
program, plus durable rename/comment/set-datatype/set-prototype/set-local writes
saved to the Ghidra project.

## Architecture

The reverse-engineering logic lives in the **shared `ghidra-mcp` crate** — the
same code the Claude Code and Antigravity plugins use. This Qwen Code extension
contributes only the Qwen-shaped glue:

- **`qwen-extension.json`** — MCP server registration + SessionStart hooks
- **`src/lib.rs`** — settings parsing (`.qwen/re-ghidra-mcp-qwen.local.md`) + preflight
- **`src/main.rs`** — MCP server binary (`re-ghidra-qwen-mcp`)
- **`src/bin/hook.rs`** — SessionStart preflight binary (`re-ghidra-qwen-hook`)

## Configuration

Create `.qwen/re-ghidra-mcp-qwen.local.md` in your project root:

```yaml
---
project_dir: C:\Users\you\ghidra-projects
project_name: crackme
bootstrap_program: crackme.exe
---
```

| Key | Required | Effect |
|---|---|---|
| `project_dir` | yes | Directory holding the `<name>.gpr` and `<name>.rep` |
| `project_name` | yes | Ghidra project name (`.gpr` filename without extension) |
| `bootstrap_program` | yes | Bare filename of a program already imported into the project |
| `bootstrap_program_path` | no | VFS path if bootstrap is in a subfolder (default: `/<name>`) |
| `ghidra_install_dir` | no | Ghidra install root (usually set via `GHIDRA_INSTALL_DIR`) |
| `max_heap` | no | JVM `-Xmx`, e.g. `4G` |

### Precedence

CLI flag → environment variable → settings file → built-in default.

| Variable | Overrides |
|---|---|
| `GHIDRA_INSTALL_DIR` | `ghidra_install_dir` |
| `GHIDRA_MCP_PROJECT_DIR` | `project_dir` |
| `GHIDRA_MCP_PROJECT_NAME` | `project_name` |
| `GHIDRA_MCP_BOOTSTRAP_PROGRAM` | `bootstrap_program` |
| `GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH` | `bootstrap_program_path` |
| `GHIDRA_MCP_MAX_HEAP` | `max_heap` |
| `GHIDRA_MCP_HOME` | Relocates the data directory (default `%USERPROFILE%\.ghidra-mcp`) |

## Prerequisites

1. Ghidra **12.1.2** installed, with `GHIDRA_INSTALL_DIR` set to its root
2. **JDK 21** or newer on `PATH`
3. A Ghidra project created, program imported, auto-analysis **finished**, and
   project **closed in the GUI**

One Ghidra project supports **one** live server and no open GUI — they collide
on Ghidra's `project.lock`.

## A note on writes

Five of the 19 tools write, and their writes are **durable**: they are saved
into the Ghidra project on success, with no undo stack. Back up a project you
care about before letting an agent loose on it.

`.qwen/*.local.md` is per-developer and belongs in `.gitignore`.
