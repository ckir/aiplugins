# re-ghidra-mcp-cc

A Claude Code plugin that attaches a **persistent headless Ghidra JVM** to your
session and exposes it as 19 reverse-engineering tools over MCP — 14 that read
and navigate a program, and 5 that write durably back into the Ghidra project.

It is the Claude Code member of a family. The reverse-engineering engine lives
in `shared/`, so an Antigravity and a Qwen plugin can front exactly the same
server without a second implementation of any of it.

## How it works

The expensive thing about Ghidra is starting it. `re-ghidra-cc-mcp serve`
launches one headless Ghidra JVM and **holds it for the life of the MCP
session**, so a tool call hits an already-warm process instead of paying Ghidra
startup cost per call.

```
Claude Code ──stdio JSON-RPC──> re-ghidra-cc-mcp ──loopback TCP──> Ghidra JVM
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
| Manifest | `.claude-plugin/plugin.json` | Name, version, metadata |
| MCP server | `.mcp.json` → `bin/re-ghidra-cc-mcp` | The 19 Ghidra tools |
| Hook | `hooks/hooks.json` → `bin/re-ghidra-cc-hook` | `SessionStart` preflight on `startup`/`resume`/`clear`; silent when healthy |
| Skill (auto) | `skills/ghidra-re-driver/SKILL.md` | How to drive the 19 tools, and when not to trust a decompile |
| Skill (invoked) | `skills/doctor/SKILL.md` | `/re-ghidra-mcp-cc:doctor` |
| Agent | `agents/re-analyst.md` | Autonomous naming/annotation sweep |
| Settings | `examples/re-ghidra-mcp-cc.local.md` | Per-project config via `.claude/*.local.md` |

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

`/re-ghidra-mcp-cc:doctor` checks all of it and tells you which one is wrong.

## Platform support

| Platform | Status |
|---|---|
| Windows | Supported. The worker JVM is bound to a Windows Job Object for cleanup. |
| Linux / macOS | Supported. The worker JVM runs in a Unix process group (via `setsid()`) and is killed via `SIGKILL` to the process group on host exit. |

## Building

The config files point at `bin/`, so the binaries must be built before the
plugin does anything:

```bash
just build-re-ghidra-mcp-cc
```

That stages `re-ghidra-cc-mcp` and `re-ghidra-cc-hook` into
`claude-code/re-ghidra-mcp-cc/bin/`, which is gitignored. Release builds for all
platforms come from CI via `cargo-dist`.

## Installing

This repository is a Claude Code marketplace, so the plugin installs like any
other — no clone, no build:

```bash
claude plugin marketplace add ckir/aiplugins
claude plugin install re-ghidra-mcp-cc@aiplugins
```

That fetches `re-ghidra-mcp-cc-plugin.zip` from the latest release, which
carries the binaries for every supported platform: Windows x64, and Linux and
macOS on both x86_64 and aarch64. Nothing is compiled at install time — Ghidra
and the JDK above are still yours to supply.

To run the working copy instead — what you want while changing the plugin —
build the binaries first and point Claude Code at the directory:

```bash
just build-re-ghidra-mcp-cc
claude --plugin-dir claude-code/re-ghidra-mcp-cc
```

Either way, configure the project next — see `examples/re-ghidra-mcp-cc.local.md`
and copy it to `.claude/re-ghidra-mcp-cc.local.md`.

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

## The `re-analyst` agent

An autonomous first-pass sweep: it finds the `FUN_`/`SUB_`/`undefined`
placeholders, gathers evidence per function, and **applies** names, prototypes
and comments rather than proposing them.

Its governing rule is that a guess is a comment, never a rename, and it will not
overwrite a symbol that already has a non-placeholder name. Even so — these are
durable writes to your project. Back it up first.

Its `tools:` list names the MCP tools explicitly rather than granting blanket
access, so if the server is not running the agent has no tools at all and fails
visibly. That is deliberate: a write-enabled agent should fail closed. Verify
the tools are present with `/mcp`.

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
claude-code/re-ghidra-mcp-cc/
├── .claude-plugin/plugin.json   # manifest (must be in .claude-plugin/)
├── .mcp.json                    # MCP server registration
├── hooks/hooks.json             # SessionStart preflight registration
├── agents/re-analyst.md
├── skills/
│   ├── ghidra-re-driver/SKILL.md   # GENERATED — see below
│   └── doctor/SKILL.md
├── examples/
│   └── re-ghidra-mcp-cc.local.md   # settings template
├── bin/                         # built binaries (gitignored)
├── src/
│   ├── lib.rs                   # settings parsing + preflight (pure, tested)
│   └── bin/
│       ├── mcp.rs               # thin front end over ghidra_mcp::cli
│       └── hook.rs              # SessionStart preflight binary
└── tests/
    ├── e2e.rs                   # drives both real binaries
    └── skill_emit.rs            # pins the generated skill copy
```

Component directories sit at the plugin **root**, not inside `.claude-plugin/`.
Only the manifest goes in `.claude-plugin/`.

### `skills/ghidra-re-driver/SKILL.md` is generated

The canonical driver skill lives at `shared/ghidra-mcp/skill/SKILL.md` and is
compiled into the binary with `include_str!`. This copy is regenerated from the
binary that embeds it:

```bash
just emit-ghidra-skill
```

Do not edit the copy — `tests/skill_emit.rs` compares it to the binary's output
byte for byte and will fail. The indirection exists because the skill is
agent-agnostic: when the agy and qwen plugins land, all three copies come from
one source rather than being hand-synced.

## Testing

```bash
cargo nextest run -p re-ghidra-mcp-cc -p ghidra-mcp -p ghidra-ipc -p ghidra-worker-ctl
```

Needs no Ghidra. The ~60 tests that drive a real JVM are gated at runtime on
`GHIDRA_MCP_E2E` — without it they early-return and pass, which is what keeps
the ubuntu and macos CI runners green. To actually run them you need a Ghidra
install and an analyzed fixture project (see
`shared/ghidra-mcp/tests/fixtures/README.md`):

```bash
just test-live-ghidra
```

CI runs that suite weekly on `windows-latest` via
`.github/workflows/e2e-ghidra.yml`, not on pull requests — it downloads Ghidra
and builds a fixture, which is far too slow to sit in front of a merge.
