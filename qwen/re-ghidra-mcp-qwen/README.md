# re-ghidra-mcp-qwen

A Qwen Code extension that attaches a **persistent headless Ghidra JVM** to your
session and exposes it as 19 reverse-engineering tools over MCP — 14 that read
and navigate a program, and 5 that write durably back into the Ghidra project.

It is the Qwen Code member of a family. The reverse-engineering engine lives
in `shared/`, so the Claude Code and Antigravity plugins front exactly the same
server without a second implementation of any of it.

## How it works

The expensive thing about Ghidra is starting it. `re-ghidra-qwen-mcp serve`
launches one headless Ghidra JVM and **holds it for the life of the MCP
session**, so a tool call hits an already-warm process instead of paying Ghidra
startup cost per call.

```
Qwen Code ──stdio JSON-RPC──> re-ghidra-qwen-mcp ──loopback TCP──> Ghidra JVM
                              (Rust, shared/)                    (GhidraMcpWorker.java)
```

The worker is a GhidraScript embedded in the binary and extracted at boot into a
directory keyed by its own content hash, so the extracted copy can never be a
different version from the binary that wrote it.

It **attaches** to a Ghidra project you have already created and fully analyzed
in the GUI, then closed. It does not import or analyze binaries itself.

## What it provides

| Extension point | File | What it does here |
|---|---|---|
| Manifest | `qwen-extension.json` | MCP server registration + SessionStart hooks |
| MCP server | `src/main.rs` → `re-ghidra-qwen-mcp` | The 19 Ghidra tools |
| Hook | `src/bin/hook.rs` → `re-ghidra-qwen-hook` | `SessionStart` preflight on `startup`/`resume`/`clear`; silent when healthy |
| Skill (auto) | `skills/ghidra-re-driver/SKILL.md` | How to drive the 19 tools, and when not to trust a decompile |
| Settings | `examples/re-ghidra-mcp-qwen.local.md` | Per-project config via `.qwen/*.local.md` |

### The 19 tools

**Read / navigating (14):** `list_project_programs`, `attach_program`,
`inspect_function`, `find_functions`, `list_symbols`, `list_strings`,
`list_data_items`, `list_segments`, `resolve_symbol`, `describe_address`,
`get_xrefs`, `get_disassembly`, `read_bytes`, `get_datatype`.

**Write (5):** `rename`, `comment`, `set_datatype`, `set_prototype`,
`set_local`.

> **The writes are durable.** On success they are saved into your Ghidra project.
> There is no undo stack and no cross-session rollback — reverting means
> restoring a backup. Copy a project you care about before letting an agent
> write to it.

## Installation

### Via Qwen Code CLI (recommended)

```bash
qwen extensions install https://github.com/ckir/aiplugins/releases/download/v0.5.0/re-ghidra-mcp-qwen-x86_64-pc-windows-msvc.zip
```

Or for other platforms:

```bash
# Linux
qwen extensions install https://github.com/ckir/aiplugins/releases/download/v0.5.0/re-ghidra-mcp-qwen-x86_64-unknown-linux-gnu.tar.xz

# macOS
qwen extensions install https://github.com/ckir/aiplugins/releases/download/v0.5.0/re-ghidra-mcp-qwen-x86_64-apple-darwin.tar.xz
```

### Building from source

```bash
git clone https://github.com/ckir/aiplugins.git
cd aiplugins
cargo build -p re-ghidra-mcp-qwen --release
qwen extensions link ./qwen/re-ghidra-mcp-qwen
```

### Updating

```bash
qwen extensions update re-ghidra-mcp-qwen
```

Check your current version:

```bash
qwen extensions list | grep re-ghidra
```

## Prerequisites

You supply these; nothing here bundles them.

- **Ghidra 12.1.2**, with `GHIDRA_INSTALL_DIR` set to its install root.
- **JDK 21** or newer on `PATH` (Ghidra 12.1.2 declares `application.java.min=21`).
- A Ghidra project already created, imported, **fully analyzed, and closed in
  the GUI**.

## Platform support

| Platform | Status |
|---|---|
| Windows | Supported. The worker JVM is bound to a Windows Job Object for cleanup. |
| Linux / macOS | Supported. The worker JVM runs in a Unix process group (via `setsid()`) and is killed via `SIGKILL` to the process group on host exit. |

## Building

The extension uses `cargo run` from `qwen-extension.json`, so `cargo` must be
on your `PATH`. For faster iteration you can build the binaries directly:

```bash
cargo build -p re-ghidra-mcp-qwen
```

Release builds for all platforms come from CI via `cargo-dist`.

## Configuration

Create `.qwen/re-ghidra-mcp-qwen.local.md` in your project root:

```yaml
---
project_dir: C:\Users\you\ghidra-projects
project_name: crackme
bootstrap_program: crackme.exe
---
```

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
- **Hook and MCP changes need a restart.** Both are loaded at session start and
  cannot be hot-swapped.
- **The preflight deliberately does not run after a context compaction.**
  `SessionStart` matches on how the session began, and `compact` is one of those
  sources. The hook is registered for `startup`, `resume` and `clear` only, so a
  diagnostic block is not re-injected every time the context is compacted.

## Layout

```text
qwen/re-ghidra-mcp-qwen/
├── qwen-extension.json          # Extension manifest (MCP + hooks)
├── Cargo.toml                   # Rust workspace crate
├── QWEN.md                      # Extension context
├── examples/
│   └── re-ghidra-mcp-qwen.local.md  # settings template
├── skills/
│   └── ghidra-re-driver/SKILL.md    # GENERATED — see below
├── src/
│   ├── lib.rs                   # settings parsing + preflight (pure, tested)
│   ├── main.rs                  # MCP server binary (thin front end)
│   └── bin/
│       └── hook.rs              # SessionStart preflight binary
└── tests/
    ├── e2e.rs                   # drives both real binaries
    └── skill_emit.rs            # pins the generated skill copy
```

### `skills/ghidra-re-driver/SKILL.md` is generated

The canonical driver skill lives at `shared/ghidra-mcp/skill/SKILL.md` and is
compiled into the binary with `include_str!`. This copy is regenerated from the
binary that embeds it:

```bash
cargo run -p re-ghidra-mcp-qwen --bin re-ghidra-qwen-mcp -- emit-skill > qwen/re-ghidra-mcp-qwen/skills/ghidra-re-driver/SKILL.md
```

Do not edit the copy — `tests/skill_emit.rs` compares it to the binary's output
byte for byte and will fail. The indirection exists because the skill is
agent-agnostic: all three plugins (cc, agy, qwen) generate their copy from one
source rather than hand-syncing.

## Testing

```bash
cargo test -p re-ghidra-mcp-qwen
```

Needs no Ghidra. The ~60 tests that drive a real JVM are gated at runtime on
`GHIDRA_MCP_E2E` — without it they early-return and pass, which is what keeps
the ubuntu and macos CI runners green. To actually run them you need a Ghidra
install and an analyzed fixture project (see
`shared/ghidra-mcp/tests/fixtures/README.md`):

```bash
cargo test -p re-ghidra-mcp-qwen -- --ignored
```

CI runs that suite weekly on `windows-latest` via
`.github/workflows/e2e-ghidra.yml`, not on pull requests — it downloads Ghidra
and builds a fixture, which is far too slow to sit in front of a merge.
