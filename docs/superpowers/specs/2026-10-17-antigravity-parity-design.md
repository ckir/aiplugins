# Spec: Antigravity Parity with Claude Code — Port Missing Plugins, Skills, Gates & Footprint

**Status:** SPEC, brainstormed 2026-10-17; approved in chat. All four design sections presented and validated. Approach 1 (Unified Specification with Workstreams A and B) chosen to achieve full first-class parity across the Antigravity front.

**Date:** 2026-10-17  
**Scope:** Close every remaining `claude-code/` → `antigravity/` parity gap across functional plugins, skills, agents, template scaffolds, repo validation gates, CI workflows, and context footprint measurement. Complements and aligns with the precedent in `docs/superpowers/specs/2026-10-17-opencode-parity-design.md`.

---

## 1. Background and Goal

The AI Plugins Marketplace monorepo supports autonomous agents including Claude Code, OpenCode, Qwen, and Antigravity. While Claude Code and OpenCode have achieved complete feature parity, continuous integration checks, PR-time smoke testing, and context footprint ratcheting, a comprehensive gap analysis revealed several areas where Antigravity lags behind:

| # | Gap Area | Details |
|---|---|---|
| **G1** | `repo-user-guide-agy` | Missing skills-only plugin for PDF user guide generation with verified examples (exists in `claude-code` and `opencode`). |
| **G2** | `antigravity/plugins/example` | Skeletal crate with no manifest, hooks, or skills (Claude Code has a full reference implementation). |
| **G3** | `rtk-mcp-agy` skills & analytics | Lacks `skills/gain` and `skills/rtk-policy`; MCP server only exposes `rtk_run` and lacks the analytics tools (`rtk_gain`, `rtk_check`, `rtk_discover`). |
| **G4** | `re-ghidra-mcp-agy` skills & subagent | Lacks `skills/doctor`, `skills/ghidra-re-driver`, subagent `agents/re-analyst.md`, and example settings file. `Justfile` recipe `emit-ghidra-skill` ignores Antigravity. |
| **G5** | Wiring & Marketplace Gates | No `scripts/check-agy-wiring.sh` or `scripts/check-agy-marketplace.sh` to validate plugin IDs, configurations, [[bin]] resolution, or marketplace manifest consistency. |
| **G6** | PR-Time Bundle Smoke | No `scripts/smoke-agy-bundle.sh` or CI job to verify that assembled Antigravity bundles unpack and launch correctly on PRs across supported OSs. |
| **G7** | Context Footprint & Ratcheting | `tools/plugin-footprint` cannot parse Antigravity manifests (`plugin.json`, `mcp_config.json`); Antigravity has no footprint baselines, no budget keys in `budgets.json`, and no rows in the root `README.md` comparison table. |
| **G8** | Registry & Justfile Integration | `scripts/lib/registry.sh` self-tests do not assert Antigravity plugins; `Justfile` lacks build, staging, smoke, and bundle recipes for Antigravity. |

**Goal:** Elevate Antigravity to full first-class parity with Claude Code and OpenCode across functional behavior (G1–G4), repo verification gates and CI (G5–G6, G8), and context footprint governance (G7).

---

## 2. Workstream A — Functional Ports & Components

### 2.1 G1: `repo-user-guide-agy` (Skills-Only Plugin)
Port `claude-code/plugins/repo-user-guide-cc` following the existing `aiplugins-devtools-agy` pattern.

* **Directory:** `antigravity/plugins/repo-user-guide-agy/`
* **Manifest (`plugin.json`):**
  ```json
  {
    "name": "repo-user-guide",
    "version": "0.3.0",
    "description": "Antigravity plugin that ships the repo-user-guide skill: generates PDF user guides with verified examples."
  }
  ```
  `name` matches the published family name (enforced by `publishedNames` mapping in `agents.json`). Version begins at `0.3.0` matching its `repo-user-guide-cc` and `repo-user-guide-opencode` siblings.
* **Skills Tree (`skills/repo-user-guide/`):**
  Byte-identical copies of portable assets from `claude-code/plugins/repo-user-guide-cc/skills/repo-user-guide/`:
  * `SKILL.md`
  * `assets/guide-template.html`
  * `scripts/check_pre.py`
  * `scripts/escape_pre.py`
  * `scripts/html_to_pdf.sh`
* **Documentation:**
  * `README.md`: Documents capabilities, prerequisites (Chrome/Chromium, poppler, Python 3), installation, and context footprint placeholder.
  * `INSTALL.md`: Agent-facing install guide for global (`~/.gemini/config/skills/repo-user-guide/`) and project-local (`.agents/skills/repo-user-guide/`) installs.
* **Registry & Release Wiring:**
  * `agents.json`: In `antigravity`, add `"repo-user-guide-agy"` to `plugins[]` and `skillsOnly[]`, and map `"repo-user-guide-agy": "repo-user-guide"` in `publishedNames`.
  * `release-please-config.json`: Regenerated via `scripts/gen-release-please-packages.sh`.
  * `.agy-plugin/marketplace.json`: Regenerated via `scripts/gen-marketplaces.sh` with release asset URL `repo-user-guide-agy-agy-plugin.zip`.
  * `scripts/check-package-versions.sh`: Add self-compare row for `repo-user-guide-agy`.
  * `scripts/check-skills-bundles.sh`: Add `antigravity:repo-user-guide-agy` to `pairs`.

### 2.2 G2: `antigravity/plugins/example` (Template Scaffold Plugin)
Flesh out `antigravity/plugins/example` into a complete reference implementation matching `claude-code/plugins/example`.

* **Crate Configuration (`antigravity/plugins/example/Cargo.toml`):**
  * Crate name: `agy-example`, with `package.metadata.dist.dist = false` (unpublished).
  * Declares two binaries:
    * `[[bin]] name = "agy-example-mcp" path = "src/bin/mcp.rs"`
    * `[[bin]] name = "agy-example-hook" path = "src/bin/hook.rs"`
  * Dependencies: workspace `rmcp`, `tokio`, `serde`, `serde_json`, `schemars`, `tracing`, `tracing-subscriber`.
* **Plugin Files:**
  * `plugin.json`: `"name": "example", "version": "0.1.0"`.
  * `mcp_config.json`: Exposes MCP server `"example"` with `"command": "agy-example-mcp"`.
  * `hooks.json`: Exposes `"PreInvocation"` hook running `"agy-example-hook"`.
  * `src/`: Core logic in `src/lib.rs`, stdio MCP server in `src/bin/mcp.rs`, and hook in `src/bin/hook.rs`.
  * `skills/example-skill/SKILL.md` & `agents/example-agent.md`: Demonstrates skill and agent discovery in an Antigravity plugin.
  * `README.md` & `tests/`: End-to-end unit and integration tests.
* **Registry & Justfile:**
  * `agents.json`: Add `"example"` to `antigravity.notPublished`.
  * `Justfile`: Add `build-agy-example` recipe to compile and stage binaries into `antigravity/plugins/example/bin/`.

### 2.3 G3: `rtk-mcp-agy` Analytics Tools and Skills
Expand `rtk-mcp-agy` to provide rtk analytics tools alongside `rtk_run`, and add the missing skills.

* **MCP Server Analytics Tools (`src/bin/rtk-mcp.rs` & `src/lib.rs`):**
  Extend the synchronous JSON-RPC request dispatcher in `handle_line()` to recognize and execute:
  * `rtk_gain`: Parses arguments (`history`, `project_only`, `format`) and runs `rtk gain`.
  * `rtk_check`: Takes `command` string and runs `rtk check -- <command>`.
  * `rtk_discover`: Parses arguments (`project`, `limit`, `since`, `all_projects`, `format`) and runs `rtk discover`.
  Keeps the server fast, lightweight, and synchronous without pulling in Tokio or rmcp.
* **Skills:**
  * `skills/gain/SKILL.md`: Directs the agent to query `rtk_gain` and `rtk_discover` on MCP server `rtkmcp`, falling back to `rtk gain` CLI if needed.
  * `skills/rtk-policy/SKILL.md`: Explains Antigravity command rewriting via `rtk_run`, how to avoid manual `rtk` prefixes, token-efficient tool preferences (`rg`, `fd`, `sd`, `eza`, `jq`), and rewrite inspection via `rtk_check`.

### 2.4 G4: `re-ghidra-mcp-agy` Skills, Agent & Settings
Align the Ghidra reverse-engineering plugin with its Claude and OpenCode twins.

* **Skills:**
  * `skills/doctor/SKILL.md`: Host-ported diagnostic procedure. Validates binaries in `antigravity/plugins/re-ghidra-mcp-agy/bin/`, checks MCP registration on server `re-ghidra-mcp`, validates `GHIDRA_INSTALL_DIR`, checks JDK ≥ 21, and verifies project settings in `.agents/re-ghidra-mcp-agy.local.md`.
  * `skills/ghidra-re-driver/SKILL.md`: Generated from canonical `shared/ghidra-mcp` via `emit-skill`.
  * `Justfile`: Extend `emit-ghidra-skill` recipe to emit into `antigravity/plugins/re-ghidra-mcp-agy/skills/ghidra-re-driver/SKILL.md`.
* **Autonomous Subagent (`agents/re-analyst.md`):**
  Add autonomous binary analysis agent prompt and configuration into `antigravity/plugins/re-ghidra-mcp-agy/agents/re-analyst.md`.
* **Settings Example (`examples/re-ghidra-mcp-agy.local.md`):**
  Provide example frontmatter configuration for project settings in `.agents/`.

---

## 3. Workstream B — Repo Gates, Verification Scripts, CI & Footprint

### 3.1 G5: Antigravity Wiring and Marketplace Gates
* **`scripts/check-agy-wiring.sh`**:
  A dedicated verification gate mirroring `check-opencode-wiring.sh`:
  1. **Manifest Integrity:** Every Antigravity plugin directory carries a valid `plugin.json` with a semver version.
  2. **Config Validation:** `hooks.json` and `mcp_config.json` parse as valid JSON.
  3. **Binary Resolution:** Every command named in `hooks.json` and `mcp_config.json` resolves to a real `[[bin]]` target in `cargo metadata`.
  4. **Skill Frontmatter:** Every `skills/*/SKILL.md` frontmatter `name:` matches its parent directory name.
  5. **Agent Frontmatter:** Every `agents/*.md` defines a `description:`.
  6. **Copy-Staleness Gate:**
     * `cmp` byte-identity check: `antigravity/plugins/re-ghidra-mcp-agy/skills/ghidra-re-driver/SKILL.md` matches `claude-code/plugins/re-ghidra-mcp-cc/skills/ghidra-re-driver/SKILL.md`.
     * `diff -r` byte-identity check: `antigravity/plugins/repo-user-guide-agy/skills/` matches `claude-code/plugins/repo-user-guide-cc/skills/`.
* **`scripts/check-agy-marketplace.sh`**:
  Verifies that `.agy-plugin/marketplace.json` remains in sync with `agents.json`, asserting plugin names, descriptions, licenses, and tag-pinned archive URLs.

### 3.2 G6: PR-Time Bundle Smoke Testing
* **`scripts/smoke-agy-bundle.sh`**:
  * Assembles host-only bundles for Antigravity plugins using local debug binaries and `scripts/bundle-agy-plugin.sh`.
  * Extracts generated `.zip` files and asserts the presence and executable permissions of all required files (`plugin.json`, `hooks.json`, `mcp_config.json`, skills, `bin/` dispatchers).
  * Runs `scripts/probe-plugin-bin.sh` against the staged tree to ensure entry points launch and return accurate `--version` output.
* **CI Integration:**
  * Add `Check antigravity wiring` and `Check antigravity marketplace` steps to the `wiring` job in `.github/workflows/ci.yml`.
  * Add `smoke-agy-bundle.sh` execution to CI so every PR validates Antigravity bundles.

### 3.3 G7: Footprint Measurement Engine Extension
Extend `tools/plugin-footprint` to support Antigravity plugins:
* **`tools/plugin-footprint/src/manifest.rs`**:
  * Update `looks_like_a_plugin()`: Return true if `dir.join("plugin.json").is_file()`.
  * Add `read_agy_servers(plugin_dir: &Path) -> Result<Vec<ServerSpec>, ManifestError>`:
    * Parses `<plugin_dir>/mcp_config.json` if present.
    * Resolves bare binary commands by checking `plugin_dir.join("bin").join(&command)` (and `.exe` on Windows).
    * Enforces `is_confined` to ensure the binary is within the plugin tree.
* **`tools/plugin-footprint/src/main.rs`**:
  * In `measure()`, detect Antigravity when `plugin.json` exists without `.claude-plugin/` or `package.json`.
  * Dispatches to `read_agy_servers()` and stamps document `agent: "antigravity"`.
* **`docs/footprints/budgets.json` & Footprint Documents:**
  * Add budget entries for `aiplugins-devtools-agy`, `repo-user-guide-agy`, `rtk-mcp-agy`, and `re-ghidra-mcp-agy` (baselines matched to their Claude Code siblings).
  * Measure each plugin and generate `docs/footprints/<plugin>.json`.
  * Add `<!-- footprint:begin -->` / `<!-- footprint:end -->` blocks to Antigravity plugin READMEs.
  * Run `plugin-footprint publish` to update the comparison table in the root `README.md`.

### 3.4 G8: Registry & Justfile Integration
* **`scripts/lib/registry.sh`**:
  Update `--self-test` to assert Antigravity's plugin list:
  `[ "$(registry_plugins antigravity | tr '\n' ' ')" = "rtk-mcp-agy re-ghidra-mcp-agy aiplugins-devtools-agy repo-user-guide-agy " ]`.
* **`Justfile`**:
  * `agy-wiring`: runs `bash scripts/check-agy-wiring.sh`.
  * `agy-marketplace`: runs `bash scripts/check-agy-marketplace.sh`.
  * `smoke-agy`: runs `bash scripts/smoke-agy-bundle.sh`.
  * `bundle-agy tag:`: runs `scripts/bundle-agy-plugin.sh` over published plugins.
  * `build-rtk-mcp-agy` and `build-re-ghidra-mcp-agy`: stages binaries into `antigravity/plugins/*/bin/`.
  * Update `footprint-regen` to loop over Antigravity plugins.
  * Update `footprint` freshness to compare `antigravity/plugins/*/README.md`.
  * Add `agy-wiring` and `agy-marketplace` to `just check`.

---

## 4. Error Handling & Guardrails

* **Rust MCP Servers (`rtk-mcp`, `re-ghidra-agy-mcp`, `agy-example-mcp`):**
  * Fail-safe execution: unrecognized or failing external tools do not crash the MCP server process; errors are returned as JSON-RPC error responses.
  * Strict stdout hygiene: stdout is exclusively reserved for valid JSON-RPC lines.
* **Preflight Hooks:**
  * Silent when healthy; never block or exit non-zero; degrade gracefully if settings files are missing.
* **Shell Scripts:**
  * All scripts use `set -euo pipefail` and explicit error reporting.
  * Path and CRLF normalization using `tr -d '\r'` on all Windows-compatible jq captures.
* **Footprint Bounds:**
  * Staged binaries must stay confined to their respective plugin directory; escaping paths are rejected at measurement time.
  * Zero-byte false positives are prevented by verifying manifest existence before probing.

---

## 5. Sequencing

1. **Phase 1 — Functional Ports (Workstream A):**
   * Scaffold `repo-user-guide-agy` (unblocks registry and footprint tracking).
   * Flesh out `antigravity/plugins/example` and update `agents.json.notPublished`.
   * Add skills and analytics tools to `rtk-mcp-agy`.
   * Add skills, subagent, and example settings to `re-ghidra-mcp-agy`; update `emit-ghidra-skill`.
2. **Phase 2 — Repo Gates & Verification (Workstream B.1 & B.2):**
   * Implement `scripts/check-agy-wiring.sh` and `scripts/check-agy-marketplace.sh`.
   * Implement `scripts/smoke-agy-bundle.sh`.
   * Update `scripts/lib/registry.sh` self-tests and `Justfile` recipes.
   * Wire checks and bundle smoke into `.github/workflows/ci.yml`.
3. **Phase 3 — Footprint & Budgets (Workstream B.3 & B.4):**
   * Extend `tools/plugin-footprint` for Antigravity manifest parsing.
   * Add staging build recipes (`build-rtk-mcp-agy`, `build-re-ghidra-mcp-agy`).
   * Add budget baselines to `docs/footprints/budgets.json`.
   * Run `just footprint-regen` to generate documents, update READMEs, and publish comparison table.
   * Verify entire repository passes `just check`.

---

## 6. Verification & Success Criteria

1. **Compilation & Unit Tests:** `cargo test --workspace` passes cleanly with all new and updated crates.
2. **Staleness & Copy Gates:** `scripts/check-agy-wiring.sh` confirms byte-identity for `ghidra-re-driver` and `repo-user-guide` skills.
3. **Marketplace Parity:** `scripts/check-agy-marketplace.sh` confirms `.agy-plugin/marketplace.json` matches `agents.json`.
4. **Smoke Verification:** `scripts/smoke-agy-bundle.sh` verifies bundle assembly, permissions, and entry point execution on local machine.
5. **Footprint Gate:** `just footprint-regen` succeeds and `cargo run -q -p plugin-footprint --bin footprint-gate -- origin/main` passes against the baseline budgets.
6. **Full Repo Check:** `just check` passes with all checks green (formatting, lints, tests, deny, spellcheck, wiring, marketplaces, dispatch, smoke, footprints, budgets).
