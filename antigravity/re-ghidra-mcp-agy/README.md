# re-ghidra-mcp-agy

An Antigravity plugin that attaches a **persistent headless Ghidra JVM** to your
session and exposes it as 19 reverse-engineering tools over MCP — 14 that read
and navigate a program, and 5 that write durably back into the Ghidra project.

It is the Antigravity member of a family. The reverse-engineering engine lives
in `shared/`, so a Claude Code and a Qwen plugin can front exactly the same
server without a second implementation of any of it.

## How it works

The expensive thing about Ghidra is starting it. `re-ghidra-agy-mcp serve`
launches one headless Ghidra JVM and **holds it for the life of the MCP
session**, so a tool call hits an already-warm process instead of paying Ghidra
startup cost per call.

```text
Antigravity ──stdio JSON-RPC──> re-ghidra-agy-mcp ──loopback TCP──> Ghidra JVM
                                (Rust, shared/)                     (GhidraMcpWorker.java)
```

The worker is a GhidraScript embedded in the binary and extracted at boot into a
directory keyed by its own content hash, so the extracted copy can never be a
different version from the binary that wrote it.

It **attaches** to a Ghidra project you have already created and fully analyzed
in the GUI, then closed. It does not import or analyze binaries itself.

## What it provides

| Extension point | File | What it does here |
|---|---|---|
| Manifest | `plugin.json` | Name, version, metadata |
| MCP server | `mcp_config.json` → `bin/re-ghidra-agy-mcp` | The 19 Ghidra tools |
| Hook | `hooks.json` → `bin/re-ghidra-agy-hook` | `PreInvocation` preflight; silent when healthy |
| Settings | `examples/re-ghidra-mcp-agy.local.md` | Per-project config via `.agents/*.local.md` |

### The 19 tools

**Read / navigate (14):** `list_project_programs`, `attach_program`,
`inspect_function`, `find_functions`, `list_symbols`, `list_strings`,
`list_data_items`, `list_segments`, `resolve_symbol`, `describe_address`,
`get_xrefs`, `get_disassembly`, `read_bytes`, `get_datatype`.

**Write (5):** `rename`, `comment`, `set_datatype`, `set_prototype`,
`set_local`.

> **The writes are durable.** On success they are saved into your Ghidra project.
> There is no undo stack and no cross-session rollback — reverting means
> restoring a backup. Copy a project you care about before letting an agent
> write to it.

## Prerequisites

You supply these; nothing here bundles them.

- **Ghidra 12.1.2**, with `GHIDRA_INSTALL_DIR` set to its install root.
- **JDK 21** or newer on `PATH` (Ghidra 12.1.2 declares `application.java.min=21`).
- A Ghidra project already created, imported, **fully analyzed, and closed in
  the GUI**.

## Platform support

| Platform | Status |
|---|---|
| Windows | Supported. |
| Linux / macOS | Builds and unit-tests, but does not run. The worker JVM's lifetime is bound to a Windows Job Object (`shared/ghidra-worker-ctl/src/job_object.rs`), which has no non-Windows equivalent; the crate carries a `#[cfg(not(windows))]` stub so the workspace still compiles and the 3-OS CI matrix stays meaningful. |

## Building

The config files point at `bin/`, so the binaries must be built before the
plugin does anything:

```bash
cargo build -p re-ghidra-mcp-agy
```

That stages `re-ghidra-agy-mcp` and `re-ghidra-agy-hook` into the target directory. Ensure they are available on the PATH or adjust the paths in `mcp_config.json` and `hooks.json` to point to the built binaries.

## Installing

To install the plugin for Antigravity, add it to your project's `.agents/plugins.json` or explicitly reference the directory:

Then configure the project — see `examples/re-ghidra-mcp-agy.local.md` and copy
it to `.agents/re-ghidra-mcp-agy.local.md`.

## Configuration

Precedence, highest first: **CLI flag → environment variable → settings file →
default.**

| Setting | `.local.md` key | Environment variable |
|---|---|---|
| Ghidra install root | `ghidra_install_dir` | `GHIDRA_INSTALL_DIR` |
| Directory holding the `.gpr`/`.rep` | `project_dir` | `GHIDRA_MCP_PROJECT_DIR` |
| Ghidra project name | `project_name` | `GHIDRA_MCP_PROJECT_NAME` |
| Bare program filename to attach at boot | `bootstrap_program` | `GHIDRA_MCP_BOOTSTRAP_PROGRAM` |
| VFS path, if the program is in a subfolder | `bootstrap_program_path` | `GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH` |
| JVM `-Xmx` | `max_heap` | `GHIDRA_MCP_MAX_HEAP` |
| Data directory (logs, worker scripts) | — | `GHIDRA_MCP_HOME` |

**One server = one Ghidra project.** Two servers on the same project — or one
server and an open GUI — collide on Ghidra's `project.lock`. Different
workspaces must target different projects.

The environment sits above the settings file so you can retarget one session
without editing anything. An **empty** variable counts as unset.

## Troubleshooting

- **First call after a cold start returns `WORKER_WARMING`.** The JVM is still
  starting. Wait at least 10 seconds; do not retry in a tight loop.
- **Logs:** `%USERPROFILE%\.ghidra-mcp\logs\` (or under `GHIDRA_MCP_HOME`).
  They rotate daily with five kept, so the filename carries a date suffix —
  `worker-<pid>.log.<YYYY-MM-DD>`, not a bare `worker-<pid>.log`. Log verbosity
  is fixed — there is no `RUST_LOG`.
- **A boot that hangs rather than fails** is usually endpoint protection or a
  firewall blocking the worker's loopback bind.

## Layout

```text
antigravity/re-ghidra-mcp-agy/
├── plugin.json                  # manifest
├── mcp_config.json              # MCP server registration
├── hooks.json                   # PreInvocation preflight registration
├── examples/
│   └── re-ghidra-mcp-agy.local.md  # settings template
├── bin/                         # built binaries (gitignored)
├── src/
│   ├── lib.rs                   # settings parsing + preflight (pure, tested)
│   └── bin/
│       ├── mcp.rs               # thin front end over ghidra_mcp::cli
│       └── hook.rs              # PreInvocation preflight binary
```

## Testing

```bash
cargo nextest run -p re-ghidra-mcp-agy -p ghidra-mcp -p ghidra-ipc -p ghidra-worker-ctl
```

Needs no Ghidra. The ~60 tests that drive a real JVM are gated at runtime on
`GHIDRA_MCP_E2E` — without it they early-return and pass, which is what keeps
the ubuntu and macos CI runners green.
