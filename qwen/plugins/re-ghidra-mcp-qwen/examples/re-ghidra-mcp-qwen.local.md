---
project_dir: C:\Users\you\ghidra-projects
project_name: crackme
bootstrap_program: crackme.exe
---

# re-ghidra-mcp-qwen settings

Copy this file to `.qwen/re-ghidra-mcp-qwen.local.md` in your project root to
tell the extension which Ghidra project this workspace works on. Every key is
optional; anything you omit (or misspell) simply contributes nothing, and the
environment or a CLI flag supplies it instead.

**This file is what makes the extension usable across more than one project.**
One running server attaches to exactly one Ghidra project, so the project
location, its name and the bootstrap program are per-workspace by nature.
Without this file, each workspace needs a hand-written `mcpServers` block that
re-declares the whole MCP server just to carry three strings.

| Key | Required | Effect |
|---|---|---|
| `project_dir` | yes | Directory holding the `<name>.gpr` and `<name>.rep`. Not the `.rep` itself. |
| `project_name` | yes | The Ghidra project name, i.e. the `.gpr` filename without its extension. |
| `bootstrap_program` | yes | A **bare** program filename already imported into the project, e.g. `crackme.exe`. The worker attaches to this at boot; you can `attach_program` elsewhere afterwards. |
| `bootstrap_program_path` | no | The `/`-prefixed VFS path, if the bootstrap program lives in a project subfolder. Defaults to `/<bootstrap_program>`. |
| `ghidra_install_dir` | no | Ghidra install root. Usually left unset — `GHIDRA_INSTALL_DIR` is machine-wide, not per-project. |
| `max_heap` | no | JVM `-Xmx`, e.g. `4G`. Raise it for large binaries. |

## Precedence

Highest to lowest: **CLI flag → environment variable → this file → built-in
default.** The environment sits above this file so you can point a single
session at a different project without editing anything:

| Variable | Overrides |
|---|---|
| `GHIDRA_INSTALL_DIR` | `ghidra_install_dir` — the one variable you should set machine-wide, not here. |
| `GHIDRA_MCP_PROJECT_DIR` | `project_dir` |
| `GHIDRA_MCP_PROJECT_NAME` | `project_name` |
| `GHIDRA_MCP_BOOTSTRAP_PROGRAM` | `bootstrap_program` |
| `GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH` | `bootstrap_program_path` |
| `GHIDRA_MCP_MAX_HEAP` | `max_heap` |
| `GHIDRA_MCP_HOME` | Relocates the data directory (logs and extracted worker scripts), default `%USERPROFILE%\.ghidra-mcp`. |

An **empty** environment variable counts as unset, so `GHIDRA_MCP_PROJECT_NAME=`
leaves this file's value in place rather than blanking it.

## Before it will work

The extension attaches to a project you have already prepared; it does not import
or analyze binaries itself.

1. Ghidra **12.1.2** installed, with `GHIDRA_INSTALL_DIR` set to its root.
2. **JDK 21** or newer on `PATH` (Ghidra 12.1.2 declares `application.java.min=21`).
3. A Ghidra project created, the program imported, auto-analysis **finished**,
   and the project **closed in the GUI**.

That last point is not optional. One Ghidra project supports **one** live server
and no open GUI — they collide on Ghidra's `project.lock`. Two workspaces must
point at two different projects.

## A note on writes

Five of the 19 tools write, and their writes are **durable**: they are saved
into the Ghidra project on success, with no undo stack and no cross-session
rollback. Back up (or copy) a project you care about before letting an agent
loose on it.

`.qwen/*.local.md` is per-developer and belongs in `.gitignore`. There is no
committed, team-shared variant; to give a whole team the same setting, use the
environment variable instead.
