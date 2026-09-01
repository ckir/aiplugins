# AI Plugins Marketplace — QWEN.md

## Project Overview

**AI Plugins Marketplace** is a Rust monorepo that builds, manages, and distributes plugins for autonomous AI agents — initially **Claude Code**, **Antigravity**, and **Qwen Code**. The project provides two flagship reverse-engineering plugins:

1. **re-ghidra-mcp** — Drives a persistent headless Ghidra JVM from an AI agent via MCP, providing 19 reverse-engineering tools (decompile, navigate, search, durable rename/comment/write operations).
2. **rtk-mcp** — A PreToolUse hook that transparently rewrites shell commands through `rtk` (Rust Token Killer) for 60-90% token savings, plus analytics as MCP tools.

The architecture follows a **shared engine + agent-specific front** pattern:
- `shared/` — Agent-agnostic crates used by multiple plugins
- `claude-code/`, `qwen/`, `antigravity/` — Thin agent-specific plugin wrappers (manifests, skills, hooks, settings)

### Shared Crates

| Crate | Purpose |
|---|---|
| `shared/ghidra-ipc` | Wire protocol, framing, and error envelopes for the Ghidra worker channel |
| `shared/ghidra-worker-ctl` | Headless Ghidra JVM lifecycle: boot, launch, connection, Windows Job Object / Unix process-group containment |
| `shared/ghidra-mcp` | MCP server with 19 reverse-engineering tools, embedded Ghidra worker script, and driver skill |

### Agent Plugins

| Agent | re-ghidra-mcp | rtk-mcp |
|---|---|---|
| Claude Code | `claude-code/re-ghidra-mcp-cc` | `claude-code/rtk-mcp-cc` |
| Qwen Code | `qwen/re-ghidra-mcp-qwen` | `qwen/rtk-mcp-qwen` |
| Antigravity | `antigravity/re-ghidra-mcp-agy` | `antigravity/rtk-mcp-agy` |

## Key Technologies

- **Language:** Rust (Edition 2021)
- **MCP Framework:** `rmcp` (Rust MCP SDK)
- **Async Runtime:** Tokio
- **Serialization:** Serde + Serde JSON
- **Error Handling:** Thiserror, Anyhow
- **Testing:** `cargo-nextest`, `insta` (snapshot testing), `proptest`
- **CI/CD:** GitHub Actions, `cargo-dist` for releases
- **Task Runner:** `just`
- **Git Hooks:** `lefthook`

## Building and Running

### Prerequisites

- Rust toolchain (stable)
- `cargo-binstall` (for fast tool installation)
- **For live Ghidra tests:** Ghidra 12.1.2 + JDK 21, with an analyzed fixture project

### Setup

```bash
just setup
```

Installs dev tools (`cargo-nextest`, `cargo-deny`, `bacon`, `typos-cli`, `lefthook`) and git hooks.

### Common Tasks

| Task | Command | Description |
|---|---|---|
| Format | `just fmt` | Format all Rust code |
| Lint | `just lint` | Clippy with `-D warnings` |
| Test | `just test` | Run all tests via nextest |
| Watch test | `just watch-test` | Background test runner with bacon |
| Dependency check | `just deny` | cargo deny check |
| Spellcheck | `just spellcheck` | typos-cli |
| Plugin wiring | `just wiring` | Verify plugins point at correct binaries |
| Marketplace manifest | `just marketplace` | Verify root manifest matches plugins |
| Dispatch check | `just dispatch` | Test bin dispatcher branches |
| Smoke test | `just smoke` | Assemble and start bundled plugins |
| Full check | `just check` | All pre-flight checks (CI-equivalent) |
| Clean stale | `just clean-stale` | Remove orphaned executables |
| Full clean | `just clean` | Remove all build artifacts |

### Building Individual Plugin Binaries

```bash
just build-claude-example   # claude-code/example plugin
just build-rtk-mcp-cc       # Claude Code rtk plugin
just build-re-ghidra-mcp-cc # Claude Code ghidra plugin
```

### Live Ghidra E2E Tests

```bash
just test-live-ghidra
```

Requires `GHIDRA_MCP_E2E=1` environment variable and a pre-analyzed Ghidra fixture project (see `shared/ghidra-mcp/tests/fixtures/README.md`). Runs with `-j1` to avoid project lock collisions.

### Regenerating Ghidra Skills

```bash
just emit-ghidra-skill
```

Regenerates the committed SKILL.md copies for all agent plugins from the canonical source at `shared/ghidra-mcp/skill/SKILL.md`.

## Repository Structure

```
aiplugins/
├── .claude-plugin/        # marketplace.json — what `claude plugin marketplace add` reads
├── claude-code/           # Claude Code plugins (re-ghidra-mcp-cc, rtk-mcp-cc, example)
├── antigravity/           # Antigravity plugins
├── qwen/                  # Qwen Code extensions
├── shared/                # Agent-agnostic crates (ghidra-ipc, ghidra-worker-ctl, ghidra-mcp)
├── scripts/               # CI and justfile verification scripts
├── .agents/skills/        # Qwen Code skills (brainstorming, debugging, TDD, etc.)
├── .github/workflows/     # CI: ci.yml, plugin-bundles.yml
├── Cargo.toml             # Workspace root
├── Justfile               # Task runner
├── deny.toml              # cargo deny configuration
├── cliff.toml             # git-cliff changelog config
├── dist-workspace.toml    # cargo-dist release configuration
└── lefthook.yml           # Git hooks
```

## Development Conventions

### Coding Style
- Rust Edition 2021, workspace-level dependency management
- Clippy warnings are errors (`-D warnings`)
- Use `cargo fmt --all` before committing
- Snapshot tests use `insta` for deterministic output verification

### Testing
- Tests use `cargo-nextest` for faster parallel execution
- `insta` for snapshot testing (JSON format)
- `proptest` for property-based testing
- Live Ghidra tests are gated behind `GHIDRA_MCP_E2E` env var; without it they early-return and pass

### Git Hooks
- `lefthook` manages pre-commit hooks (formatting, linting, tests)
- CI runs the same checks as `just check`

### Plugin Bundle Distribution
- Plugins are distributed as release assets: `<plugin-name>-*.zip` and `<plugin-name>-*.tar.xz`
- Each bundle contains binaries for Windows x64, Linux x86_64, and macOS (x86_64 + aarch64)
- `scripts/bundle-plugin.sh` creates the bundles; `.github/workflows/plugin-bundles.yml` runs on release
- **Unix-only bundling**: Run `just bundle-plugins` from WSL on Windows; MSYS2 would overwrite binaries

### Per-Developer Configuration
- Plugin settings files use the `*.local.md` convention and belong in `.gitignore`
- Claude Code: `.claude/settings.local.json`, `.claude/*.local.md`
- Qwen Code: `.qwen/*.local.md`

### Important Notes
- **Windows Job Objects**: The Ghidra worker JVM is held in a Job Object so it cannot outlive its parent
- **Unix process groups**: `setsid()` + `SIGKILL` for clean JVM teardown on Linux/macOS
- **One Ghidra project = one live server**: No open GUI allowed (project.lock collision)
- **Durable writes**: Five of the 19 Ghidra tools write directly to the project (no undo stack) — back up projects before agent use
- **`clean-stale` before `clean`**: Cargo never deletes orphaned executables when bin targets are renamed; `just clean-stale` removes them without a full rebuild

## Version

Current version: **0.6.3** (2026-09-01)

License: **PolyForm-Noncommercial-1.0.0**

Repository: https://github.com/ckir/aiplugins
