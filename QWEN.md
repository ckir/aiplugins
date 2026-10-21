# AI Plugins Marketplace — QWEN.md

## Project Overview

**AI Plugins Marketplace** is a Rust monorepo that builds, manages, and distributes plugins for autonomous AI agents — **Claude Code**, **Antigravity**, **Qwen Code**, and **OpenCode**. The project provides two flagship reverse-engineering plugins:

1. **re-ghidra-mcp** — Drives a persistent headless Ghidra JVM from an AI agent via MCP, providing 19 reverse-engineering tools (decompile, navigate, search, durable rename/comment/write operations).
2. **rtk-mcp** — A PreToolUse hook that transparently rewrites shell commands through `rtk` (Rust Token Killer) for 60-90% token savings, plus analytics as MCP tools.

The architecture follows a **shared engine + agent-specific front** pattern:
- `shared/` — Agent-agnostic crates used by multiple plugins
- `claude-code/plugins/`, `qwen/plugins/`, `antigravity/plugins/`, `opencode/plugins/` — Thin agent-specific plugin wrappers (manifests, skills, hooks, settings)

### Shared Crates

| Crate | Purpose |
|---|---|
| `shared/ghidra-ipc` | Wire protocol, framing, and error envelopes for the Ghidra worker channel |
| `shared/ghidra-worker-ctl` | Headless Ghidra JVM lifecycle: boot, launch, connection, Windows Job Object / Unix process-group containment |
| `shared/ghidra-mcp` | MCP server with 19 reverse-engineering tools, embedded Ghidra worker script, and driver skill |

### Agent Plugins

| Agent | re-ghidra-mcp | rtk-mcp | opencode-ctl-mcp |
|---|---|---|---|
| Claude Code | `claude-code/plugins/re-ghidra-mcp-cc` | `claude-code/plugins/rtk-mcp-cc` | `claude-code/plugins/opencode-ctl-mcp-cc` |
| Qwen Code | `qwen/plugins/re-ghidra-mcp-qwen` | `qwen/plugins/rtk-mcp-qwen` | `qwen/plugins/opencode-ctl-mcp-qwen` |
| Antigravity | `antigravity/plugins/re-ghidra-mcp-agy` | `antigravity/plugins/rtk-mcp-agy` | `antigravity/plugins/opencode-ctl-mcp-agy` |
| OpenCode | `opencode/plugins/re-ghidra-mcp-opencode` | `opencode/plugins/rtk-mcp-opencode` (plus `opencode/plugins/todov2-opencode`: todowrite tool + TUI sidebar) | `opencode/plugins/opencode-ctl-mcp-opencode` |

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

Installs dev tools (`cargo-nextest`, `cargo-deny`, `bacon`, `typos-cli`, `lefthook`, `mlc`) and git hooks.

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
| Full check | `just check` | All local pre-flight checks (CI runs these plus generated-manifest `--check` gates and the opencode suites — see the `check` recipe comment) |
| Clean stale | `just clean-stale` | Remove orphaned executables |
| Full clean | `just clean` | Remove all build artifacts |

### Building Individual Plugin Binaries

```bash
just build-claude-example   # claude-code/plugins/example plugin
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
├── claude-code/plugins/    # Claude Code plugins (re-ghidra-mcp-cc, rtk-mcp-cc, example)
├── claude-code/{agents,skills,commands,models,themes,hooks}/  # Standalone components
├── antigravity/plugins/    # Antigravity plugins
├── antigravity/{agents,skills,commands,models,themes,hooks}/
├── qwen/plugins/           # Qwen Code extensions
├── qwen/{agents,skills,commands,models,themes,hooks}/
├── opencode/plugins/        # OpenCode plugins (re-ghidra-mcp-opencode, rtk-mcp-opencode, todov2-opencode)
├── opencode/{agents,skills,commands,models,themes,hooks}/
├── shared/                # Agent-agnostic crates (ghidra-ipc, ghidra-worker-ctl, ghidra-mcp)
├── tools/                 # Maintainer tooling (plugin-footprint)
├── scripts/               # CI and justfile verification scripts
├── .agents/skills/        # Qwen Code skills (brainstorming, debugging, TDD, etc.)
├── .github/workflows/     # CI: ci.yml, plugin-bundles.yml, qwen-extension-bundles.yml, agy-bundles.yml, opencode-bundles.yml, release.yml, release-please.yml, docs.yml, e2e-ghidra.yml, footprint-freshness.yml
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
- `lefthook` manages pre-commit hooks (formatting, linting, typos) and pre-push hooks (formatting, linting, `mlc` link check)
- `just check` is the local gate; CI runs the same core checks plus generated-manifest `--check` gates, the opencode test/smoke suites, and deny/typos jobs (link checking stays hook-only)

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

Workspace: **0.7.2**. Plugin releases are per-package: **0.8.0** (Claude/Qwen/Antigravity ghidra + rtk), **0.7.3** (`rtk-mcp-opencode`, `re-ghidra-mcp-opencode`), **0.3.0** (`todov2-opencode`). See `.release-please-manifest.json` for the registry.

License: **PolyForm-Noncommercial-1.0.0**

Repository: https://github.com/ckir/aiplugins
