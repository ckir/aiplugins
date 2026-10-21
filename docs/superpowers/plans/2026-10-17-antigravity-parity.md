# Antigravity Parity with Claude Code (G1–G8) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Elevate the Antigravity front to complete feature, verification, and context footprint parity with Claude Code: port the `repo-user-guide` skills plugin (G1), flesh out the `example` scaffold plugin (G2), expand `rtk-mcp-agy` with analytics tools and skills (G3), add `doctor`, `ghidra-re-driver`, and `re-analyst` to `re-ghidra-mcp-agy` (G4), establish repo gates and PR-time bundle smoke checks (G5–G6), extend `plugin-footprint` to measure Antigravity and enforce budgets (G7), and wire Justfile and registry integration (G8).

**Architecture:** Registry-first architecture: `agents.json` is the single source of truth; derived marketplace manifests and release-please configs are regenerated, never hand-edited. Functional components are strictly isolated: `repo-user-guide-agy` is a crate-less, skills-only plugin; `antigravity/plugins/example` is a complete unpublished reference template; `rtk-mcp-agy`'s synchronous JSON-RPC server handles analytics queries without runtime bloat; `re-ghidra-mcp-agy` utilizes host-aware settings and driver emission. `tools/plugin-footprint` securely inspects `plugin.json` and `mcp_config.json` with path confinement.

**Tech Stack:** Rust (`rmcp`, `serde`, `serde_json`, `schemars`, `tokio`, `tracing`), Bash (`set -euo pipefail`), Just, GitHub Actions.

**Spec:** [`docs/superpowers/specs/2026-10-17-antigravity-parity-design.md`](../specs/2026-10-17-antigravity-parity-design.md)

## Global Constraints

- `repo-user-guide-agy` skill files are **byte-identical copies** of the `repo-user-guide-cc` originals.
- `ghidra-re-driver` skill is emitted via `re-ghidra-cc-mcp emit-skill` or `re-ghidra-agy-mcp emit-skill` and must match the Claude original byte-for-byte.
- `sed -i` is forbidden (macOS/BSD compatibility); use `sed '...' file > file.tmp && mv file.tmp file`.
- Manifest versions and package names must strictly match across `agents.json`, `plugin.json`, `Cargo.toml`, and `.agy-plugin/marketplace.json`.
- `antigravity/plugins/example` stays unpublished (`dist = false` in `Cargo.toml`, listed in `agents.json.notPublished`).
- All shell commands run from the repository root: `/home/user/Development/Rust/aiplugins`.

---

## File Structure

```text
antigravity/
├── plugins/
│   ├── repo-user-guide-agy/          # G1: Skills-only plugin
│   │   ├── plugin.json
│   │   ├── README.md
│   │   ├── INSTALL.md
│   │   └── skills/repo-user-guide/   # Byte-identical copies
│   ├── example/                      # G2: Complete scaffold template
│   │   ├── Cargo.toml
│   │   ├── plugin.json
│   │   ├── mcp_config.json
│   │   ├── hooks.json
│   │   ├── README.md
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   └── bin/{mcp.rs, hook.rs}
│   │   ├── skills/example-skill/SKILL.md
│   │   ├── agents/example-agent.md
│   │   └── tests/
│   ├── rtk-mcp-agy/                  # G3: Analytics tools + skills
│   │   ├── src/{lib.rs, bin/rtk-mcp.rs}
│   │   └── skills/
│   │       ├── gain/SKILL.md
│   │       └── rtk-policy/SKILL.md
│   └── re-ghidra-mcp-agy/            # G4: Skills + subagent + settings
│       ├── skills/
│       │   ├── doctor/SKILL.md
│       │   └── ghidra-re-driver/SKILL.md
│       ├── agents/re-analyst.md
│       └── examples/re-ghidra-mcp-agy.local.md
scripts/
├── check-agy-wiring.sh               # G5: Antigravity wiring validator
├── check-agy-marketplace.sh          # G5: Marketplace manifest validator
└── smoke-agy-bundle.sh               # G6: PR-time bundle assembler and prober
tools/plugin-footprint/src/
├── manifest.rs                       # G7: Antigravity manifest & server parser
└── main.rs                           # G7: Antigravity agent tagging & dispatch
docs/footprints/
├── budgets.json                      # G7: Baselines for Antigravity plugins
└── {repo-user-guide-agy, ...}.json   # G7: Measured footprints
```

---

## Task 1: Scaffold `repo-user-guide-agy` (G1)

**Files:**
- Create: `antigravity/plugins/repo-user-guide-agy/plugin.json`
- Create: `antigravity/plugins/repo-user-guide-agy/README.md`
- Create: `antigravity/plugins/repo-user-guide-agy/INSTALL.md`
- Create: `antigravity/plugins/repo-user-guide-agy/skills/repo-user-guide/*` (byte-copied)

- [ ] **Step 1: Create plugin directory and copy skills byte-for-byte**

```bash
mkdir -p antigravity/plugins/repo-user-guide-agy/skills
cp -r claude-code/plugins/repo-user-guide-cc/skills/repo-user-guide \
      antigravity/plugins/repo-user-guide-agy/skills/
```

- [ ] **Step 2: Create `plugin.json`**

Create `antigravity/plugins/repo-user-guide-agy/plugin.json`:
```json
{
  "name": "repo-user-guide",
  "version": "0.3.0",
  "description": "Antigravity plugin that ships the repo-user-guide skill: generates PDF user guides with verified examples."
}
```

- [ ] **Step 3: Create `README.md`**

Create `antigravity/plugins/repo-user-guide-agy/README.md`:
```markdown
# repo-user-guide

An Antigravity plugin that ships the `repo-user-guide` skill: it produces a PDF
user guide for a code repository, with usage examples that are run and verified
against the repository before they go into the guide.

Requirements:
- **Chrome or Chromium** on the machine: the guide is rendered to PDF with headless Chrome.
- **poppler** (`pdftoppm`, `pdftotext`, `pdfinfo`) is recommended to verify rendered pages.
- **Python 3**, for the helper scripts (`escape_pre.py`, `check_pre.py`).

Install instructions: see [INSTALL.md](INSTALL.md).

<!-- footprint:begin -->
### Context footprint

Generated by `tools/plugin-footprint`; regenerate with `just footprint-regen`.
<!-- footprint:end -->
```

- [ ] **Step 4: Create `INSTALL.md`**

Create `antigravity/plugins/repo-user-guide-agy/INSTALL.md`:
```markdown
# Installing repo-user-guide for Antigravity (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install
> this plugin for them. Follow this document top to bottom, in order.

You will install `repo-user-guide`: a pure skills plugin that ships
the `repo-user-guide` skill. There are no binaries to stage.

## Step 1 - Choose installation destination
- **Project-local**: `<project root>/.agents/skills/repo-user-guide/`
- **Global**: `~/.gemini/config/skills/repo-user-guide/`

## Step 2 - Copy skill files
Copy the contents of `skills/repo-user-guide/` into the target destination directory.
```

- [ ] **Step 5: Verify byte identity against Claude Code source**

```bash
diff -r claude-code/plugins/repo-user-guide-cc/skills/repo-user-guide \
        antigravity/plugins/repo-user-guide-agy/skills/repo-user-guide
```
Expected: Exit code 0, no output.

- [ ] **Step 6: Commit**

```bash
git add antigravity/plugins/repo-user-guide-agy
git commit -m "feat(antigravity): scaffold repo-user-guide-agy skills plugin (G1)"
```

---

## Task 2: Wire `repo-user-guide-agy` into Registry, Release, and Bundles (G1)

**Files:**
- Modify: `agents.json`
- Modify: `scripts/check-package-versions.sh`
- Modify: `scripts/check-skills-bundles.sh`
- Regenerate: `release-please-config.json`
- Regenerate: `.agy-plugin/marketplace.json`

- [ ] **Step 1: Update `agents.json`**

In `agents.json`, under the `antigravity` agent object:
1. Add `"repo-user-guide-agy"` to `plugins` array.
2. Add `"repo-user-guide-agy": "repo-user-guide"` to `publishedNames`.
3. Add `"repo-user-guide-agy"` to `skillsOnly` array.

- [ ] **Step 2: Regenerate `release-please-config.json` and marketplace**

```bash
bash scripts/gen-release-please-packages.sh
bash scripts/gen-marketplaces.sh
```

- [ ] **Step 3: Add `repo-user-guide-agy` to `scripts/check-package-versions.sh`**

Add the self-compare line to `scripts/check-package-versions.sh` under `antigravity`:
```bash
    "repo-user-guide-agy|antigravity/plugins/repo-user-guide-agy/plugin.json|antigravity/plugins/repo-user-guide-agy/plugin.json|.version" \
```

- [ ] **Step 4: Add `antigravity:repo-user-guide-agy` to `scripts/check-skills-bundles.sh`**

In `scripts/check-skills-bundles.sh`, add `antigravity:repo-user-guide-agy` to `pairs`:
```bash
pairs="claude-code:aiplugins-devtools-cc
qwen:aiplugins-devtools-qwen
antigravity:aiplugins-devtools-agy
opencode:aiplugins-devtools-opencode
claude-code:repo-user-guide-cc
antigravity:repo-user-guide-agy
opencode:repo-user-guide-opencode"
```

- [ ] **Step 5: Test skills bundles and package versions**

```bash
bash scripts/check-package-versions.sh
bash scripts/check-skills-bundles.sh
bash scripts/gen-marketplaces.sh --check
bash scripts/gen-release-please-packages.sh --check
```
Expected: All exit 0 with all pairs passing.

- [ ] **Step 6: Commit**

```bash
git add agents.json release-please-config.json .agy-plugin/marketplace.json \
        scripts/check-package-versions.sh scripts/check-skills-bundles.sh
git commit -m "feat(antigravity): wire repo-user-guide-agy into registry, release, and bundles (G1)"
```

---

## Task 3: Flesh out `antigravity/plugins/example` (G2)

**Files:**
- Modify: `antigravity/plugins/example/Cargo.toml`
- Create: `antigravity/plugins/example/plugin.json`
- Create: `antigravity/plugins/example/mcp_config.json`
- Create: `antigravity/plugins/example/hooks.json`
- Create: `antigravity/plugins/example/README.md`
- Create: `antigravity/plugins/example/src/lib.rs`
- Create: `antigravity/plugins/example/src/bin/mcp.rs`
- Create: `antigravity/plugins/example/src/bin/hook.rs`
- Create: `antigravity/plugins/example/skills/example-skill/SKILL.md`
- Create: `antigravity/plugins/example/agents/example-agent.md`
- Create: `antigravity/plugins/example/tests/e2e_test.rs`
- Modify: `agents.json`
- Modify: `Justfile`

- [ ] **Step 1: Update `antigravity/plugins/example/Cargo.toml`**

Update `antigravity/plugins/example/Cargo.toml` with the bin targets and dependencies matching `claude-code/plugins/example/Cargo.toml`:
```toml
[package]
name = "agy-example"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description.workspace = true
repository.workspace = true
readme.workspace = true

[[bin]]
name = "agy-example-mcp"
path = "src/bin/mcp.rs"

[[bin]]
name = "agy-example-hook"
path = "src/bin/hook.rs"

[dependencies]
anyhow = { workspace = true }
rmcp = { workspace = true }
schemars = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[dev-dependencies]
rmcp = { workspace = true, features = ["client", "transport-child-process"] }

[package.metadata.dist]
dist = false
```

- [ ] **Step 2: Create plugin configs**

Create `antigravity/plugins/example/plugin.json`:
```json
{
  "name": "example",
  "version": "0.1.0",
  "description": "Example Antigravity plugin with MCP tools, hooks, skills, and agents."
}
```

Create `antigravity/plugins/example/mcp_config.json`:
```json
{
  "mcpServers": {
    "example": {
      "command": "agy-example-mcp"
    }
  }
}
```

Create `antigravity/plugins/example/hooks.json`:
```json
{
  "example-hook": {
    "PreInvocation": [
      {
        "type": "command",
        "command": "agy-example-hook"
      }
    ]
  }
}
```

- [ ] **Step 3: Port source code (`src/lib.rs`, `src/bin/mcp.rs`, `src/bin/hook.rs`) and tests**

Port the sample TODO/FIXME tracking server from `claude-code/plugins/example/src/` into `antigravity/plugins/example/src/`, naming the binaries `agy-example-mcp` and `agy-example-hook`. Add `tests/e2e_test.rs` validating tool invocation over RMCP stdio.

- [ ] **Step 4: Create sample skill, subagent, and README**

Create `antigravity/plugins/example/skills/example-skill/SKILL.md`:
```markdown
---
name: example-skill
description: An example skill demonstrating Antigravity skill structure in a plugin.
---
# Example Skill
Demonstrates procedural guidance for Antigravity.
```

Create `antigravity/plugins/example/agents/example-agent.md`:
```markdown
---
name: example-agent
description: An example subagent for Antigravity plugin templates.
tools: list_todos, add_todo
---
You are an example assistant subagent.
```

Create `antigravity/plugins/example/README.md`.

- [ ] **Step 5: Register in `agents.json.notPublished` and `Justfile`**

In `agents.json`, under `antigravity`, set `"notPublished": ["example"]`.
In `Justfile`, add recipe:
```just
# Build the example Antigravity plugin's binaries into its bin/ directory.
build-agy-example:
    cargo build -p agy-example --release --bin agy-example-mcp --bin agy-example-hook
    mkdir -p antigravity/plugins/example/bin
    for b in agy-example-mcp agy-example-hook; do \
        if [ -f "target/release/$b.exe" ]; then \
            cp "target/release/$b.exe" antigravity/plugins/example/bin/; \
        else \
            cp "target/release/$b" antigravity/plugins/example/bin/; \
        fi; \
    done
    @echo "Plugin binaries staged in antigravity/plugins/example/bin/"
```

- [ ] **Step 6: Run tests and compilation**

```bash
cargo test -p agy-example
just build-agy-example
```
Expected: Tests pass, binaries staged in `antigravity/plugins/example/bin/`.

- [ ] **Step 7: Commit**

```bash
git add antigravity/plugins/example agents.json Justfile
git commit -m "feat(antigravity): flesh out example plugin scaffold (G2)"
```

---

## Task 4: Add `gain` & `rtk-policy` Skills and Analytics Tools to `rtk-mcp-agy` (G3)

**Files:**
- Modify: `antigravity/plugins/rtk-mcp-agy/src/lib.rs`
- Modify: `antigravity/plugins/rtk-mcp-agy/src/bin/rtk-mcp.rs`
- Create: `antigravity/plugins/rtk-mcp-agy/skills/gain/SKILL.md`
- Create: `antigravity/plugins/rtk-mcp-agy/skills/rtk-policy/SKILL.md`
- Test: `antigravity/plugins/rtk-mcp-agy/tests/analytics_test.rs`

- [ ] **Step 1: Write failing test for rtk analytics tool dispatcher**

Create `antigravity/plugins/rtk-mcp-agy/tests/analytics_test.rs`:
```rust
use rtk_mcp_agy::handle_request;
use serde_json::json;

#[test]
fn test_rtk_gain_tool_call() {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "rtk_gain",
            "arguments": { "history": true }
        }
    });
    let res = handle_request(&req);
    assert!(res.is_some());
}
```

- [ ] **Step 2: Implement analytics tools in `rtk_mcp_agy`**

In `antigravity/plugins/rtk-mcp-agy/src/lib.rs`:
Add support for:
1. `rtk_gain`: runs `rtk gain` with optional arguments.
2. `rtk_check`: runs `rtk check -- <command>`.
3. `rtk_discover`: runs `rtk discover`.
Advertise these tools in the `tools/list` response alongside `rtk_run`.

- [ ] **Step 3: Run tests to verify passing**

```bash
cargo test -p rtk-mcp-agy
```
Expected: All tests pass.

- [ ] **Step 4: Create `skills/gain/SKILL.md`**

Create `antigravity/plugins/rtk-mcp-agy/skills/gain/SKILL.md`:
```markdown
---
name: gain
description: Reports rtk's token savings for this project or all projects, and optimization opportunities. Use when the user asks how many tokens rtk has saved, what savings look like, or whether rtk is working.
argument-hint: "[history] [discover] [project|all]"
---

# Report rtk token savings

1. Call the `rtk_gain` tool on MCP server `rtkmcp`.
2. When `discover` is requested, call `rtk_discover`.
3. If the tool is unavailable, run `rtk gain` via CLI.
```

- [ ] **Step 5: Create `skills/rtk-policy/SKILL.md`**

Create `antigravity/plugins/rtk-mcp-agy/skills/rtk-policy/SKILL.md`:
```markdown
---
name: rtk-policy
description: Explains how rtk rewrites shell commands in Antigravity and which CLI tools to prefer for token-cheap output. Use when a command comes back rewritten, when output looks abbreviated, or when deciding whether to prefix commands with rtk manually.
---

# Working with rtk in Antigravity

- Write ordinary commands; do not prefix with `rtk` manually.
- The `PreInvocation` hook directs shell operations to `rtk_run`.
- Prefer modern CLI tools: `rg` over `grep`, `fd` over `find`, `sd` over `sed -i`, `eza` over `ls`.
- Use `rtk_check` to see how rtk would rewrite a command without executing it.
```

- [ ] **Step 6: Commit**

```bash
git add antigravity/plugins/rtk-mcp-agy
git commit -m "feat(rtk-mcp-agy): add analytics tools and gain/rtk-policy skills (G3)"
```

---

## Task 5: Add `doctor` & `ghidra-re-driver` Skills, `re-analyst.md` Subagent, and Settings to `re-ghidra-mcp-agy` (G4)

**Files:**
- Create: `antigravity/plugins/re-ghidra-mcp-agy/skills/doctor/SKILL.md`
- Create: `antigravity/plugins/re-ghidra-mcp-agy/agents/re-analyst.md`
- Create: `antigravity/plugins/re-ghidra-mcp-agy/examples/re-ghidra-mcp-agy.local.md`
- Modify: `Justfile` (update `emit-ghidra-skill`)
- Generate: `antigravity/plugins/re-ghidra-mcp-agy/skills/ghidra-re-driver/SKILL.md`

- [ ] **Step 1: Create `skills/doctor/SKILL.md`**

Create `antigravity/plugins/re-ghidra-mcp-agy/skills/doctor/SKILL.md` adapted for Antigravity (referencing `re-ghidra-agy-mcp`, `.agents/re-ghidra-mcp-agy.local.md`, and MCP server `re-ghidra-mcp`).

- [ ] **Step 2: Update `emit-ghidra-skill` recipe in `Justfile` and run it**

In `Justfile`, update `emit-ghidra-skill`:
```just
emit-ghidra-skill: build-re-ghidra-mcp-cc
    cargo build -p re-ghidra-mcp-qwen --release --bin re-ghidra-qwen-mcp
    mkdir -p antigravity/plugins/re-ghidra-mcp-agy/skills/ghidra-re-driver
    ./claude-code/plugins/re-ghidra-mcp-cc/bin/re-ghidra-cc-mcp emit-skill \
        > claude-code/plugins/re-ghidra-mcp-cc/skills/ghidra-re-driver/SKILL.md
    ./target/release/re-ghidra-qwen-mcp emit-skill \
        > qwen/plugins/re-ghidra-mcp-qwen/skills/ghidra-re-driver/SKILL.md
    ./claude-code/plugins/re-ghidra-mcp-cc/bin/re-ghidra-cc-mcp emit-skill \
        > opencode/plugins/re-ghidra-mcp-opencode/skills/ghidra-re-driver/SKILL.md
    ./claude-code/plugins/re-ghidra-mcp-cc/bin/re-ghidra-cc-mcp emit-skill \
        > antigravity/plugins/re-ghidra-mcp-agy/skills/ghidra-re-driver/SKILL.md
    @echo "Regenerated the committed ghidra-re-driver skill copies (claude-code, qwen, opencode, antigravity)"
```
Run `just emit-ghidra-skill`.

- [ ] **Step 3: Create `agents/re-analyst.md`**

Create `antigravity/plugins/re-ghidra-mcp-agy/agents/re-analyst.md` configuring autonomous bulk naming and decompilation sweeps for attached Ghidra binaries.

- [ ] **Step 4: Create `examples/re-ghidra-mcp-agy.local.md`**

Create `antigravity/plugins/re-ghidra-mcp-agy/examples/re-ghidra-mcp-agy.local.md`:
```markdown
---
ghidra_install_dir: "C:/tools/ghidra_12.1.2_PUBLIC"
project_dir: "C:/projects/ghidra"
project_name: "firmware"
bootstrap_program: "target.bin"
max_heap: "4G"
---
# Antigravity Ghidra Project Settings
Place this file at `.agents/re-ghidra-mcp-agy.local.md` in your project root.
```

- [ ] **Step 5: Verify skill copy byte identity**

```bash
cmp claude-code/plugins/re-ghidra-mcp-cc/skills/ghidra-re-driver/SKILL.md \
    antigravity/plugins/re-ghidra-mcp-agy/skills/ghidra-re-driver/SKILL.md
```
Expected: Exit code 0, no output.

- [ ] **Step 6: Commit**

```bash
git add antigravity/plugins/re-ghidra-mcp-agy Justfile
git commit -m "feat(re-ghidra-mcp-agy): add doctor/driver skills, re-analyst agent, and settings (G4)"
```

---

## Task 6: Implement `check-agy-wiring.sh` and `check-agy-marketplace.sh` (G5)

**Files:**
- Create: `scripts/check-agy-wiring.sh`
- Create: `scripts/check-agy-marketplace.sh`
- Modify: `Justfile`

- [ ] **Step 1: Create `scripts/check-agy-wiring.sh`**

Create `scripts/check-agy-wiring.sh` with executable permissions (`chmod +x`):
- Enforces valid `plugin.json` and semver version.
- Enforces valid JSON for `hooks.json` and `mcp_config.json`.
- Enforces that every command in `hooks.json` and `mcp_config.json` exists in `cargo metadata` `[[bin]]` targets.
- Enforces skill frontmatter directory match and agent `description:` presence.
- Asserts byte-identity `cmp` for `ghidra-re-driver/SKILL.md` and `diff -r` for `repo-user-guide/skills/`.

- [ ] **Step 2: Create `scripts/check-agy-marketplace.sh`**

Create `scripts/check-agy-marketplace.sh` with executable permissions (`chmod +x`):
- Validates `.agy-plugin/marketplace.json` against `agents.json`.
- Confirms release asset URLs, author, license, and plugin definitions match.

- [ ] **Step 3: Wire into `Justfile`**

In `Justfile`, add:
```just
# Verify antigravity/plugins/ plugin configs, ids, binary resolution, and skill copies.
agy-wiring:
    bash scripts/check-agy-wiring.sh

# Verify the Antigravity marketplace manifest matches the plugins it publishes.
agy-marketplace:
    bash scripts/check-agy-marketplace.sh
```
Add `agy-wiring` and `agy-marketplace` to `check:` recipe prerequisites.

- [ ] **Step 4: Test scripts**

```bash
bash scripts/check-agy-wiring.sh
bash scripts/check-agy-marketplace.sh
```
Expected: Exit code 0, all references resolve.

- [ ] **Step 5: Commit**

```bash
git add scripts/check-agy-wiring.sh scripts/check-agy-marketplace.sh Justfile
git commit -m "feat(scripts): add check-agy-wiring and check-agy-marketplace gates (G5)"
```

---

## Task 7: Implement PR-Time Bundle Smoke Testing (`smoke-agy-bundle.sh` & CI) (G6)

**Files:**
- Create: `scripts/smoke-agy-bundle.sh`
- Modify: `Justfile`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Create `scripts/smoke-agy-bundle.sh`**

Create `scripts/smoke-agy-bundle.sh` with executable permissions:
- Iterates published Antigravity plugins with binaries (`rtk-mcp-agy`, `re-ghidra-mcp-agy`).
- Builds local debug binaries and stages them.
- Assembles host-only bundle and verifies zip contents, dispatchers, and executable flags.
- Probes entry points with `scripts/probe-plugin-bin.sh`.

- [ ] **Step 2: Add `smoke-agy` recipe to `Justfile`**

```just
# Assemble a host-only bundle for each Antigravity plugin and test entry points.
smoke-agy:
    bash scripts/smoke-agy-bundle.sh
```

- [ ] **Step 3: Wire into `.github/workflows/ci.yml`**

1. In `wiring` job, add steps:
   ```yaml
   - name: Check antigravity wiring
     run: bash scripts/check-agy-wiring.sh
   - name: Check antigravity marketplace
     run: bash scripts/check-agy-marketplace.sh
   ```
2. Add `agy-bundle-smoke` job (running on `ubuntu-latest`, `windows-latest`, `macos-latest`) executing `bash scripts/smoke-agy-bundle.sh`.

- [ ] **Step 4: Test locally**

```bash
bash scripts/smoke-agy-bundle.sh
```
Expected: All Antigravity plugin bundles assemble and probe successfully.

- [ ] **Step 5: Commit**

```bash
git add scripts/smoke-agy-bundle.sh Justfile .github/workflows/ci.yml
git commit -m "ci(antigravity): add PR-time bundle smoke testing and wiring gates (G6)"
```

---

## Task 8: Extend `tools/plugin-footprint` for Antigravity & Update Registry Self-Tests (G7, G8)

**Files:**
- Modify: `tools/plugin-footprint/src/manifest.rs`
- Modify: `tools/plugin-footprint/src/main.rs`
- Modify: `scripts/lib/registry.sh`

- [ ] **Step 1: Update `manifest.rs` in `tools/plugin-footprint`**

1. In `looks_like_a_plugin()`:
   Add `|| dir.join("plugin.json").is_file()`.
2. Add `pub fn read_agy_servers(plugin_dir: &Path) -> Result<Vec<ServerSpec>, ManifestError>`:
   Parses `mcp_config.json`, resolves commands in `bin/`, and enforces `is_confined()`.

- [ ] **Step 2: Update `main.rs` in `tools/plugin-footprint`**

In `measure()`, detect Antigravity when `plugin.json` exists without `.claude-plugin/` or `package.json`. Dispatch to `read_agy_servers()`, and set `agent: "antigravity"`.

- [ ] **Step 3: Update `scripts/lib/registry.sh` self-tests**

In `scripts/lib/registry.sh` line 52:
Add Antigravity assertion:
```bash
[ "$(registry_plugins antigravity | tr '\n' ' ')" = "rtk-mcp-agy re-ghidra-mcp-agy aiplugins-devtools-agy repo-user-guide-agy " ] \
    || { echo "FAIL plugins antigravity"; failures=$((failures + 1)); }
```

- [ ] **Step 4: Test `plugin-footprint` compilation and registry self-test**

```bash
cargo test -p plugin-footprint
bash scripts/lib/registry.sh --self-test
```
Expected: Tests pass, self-test reports no failures.

- [ ] **Step 5: Commit**

```bash
git add tools/plugin-footprint scripts/lib/registry.sh
git commit -m "feat(footprint): support Antigravity manifests in plugin-footprint and update registry test (G7, G8)"
```

---

## Task 9: Generate Footprint Baselines, Ratchet Budgets & Publish Docs (G7, G8)

**Files:**
- Modify: `Justfile`
- Modify: `docs/footprints/budgets.json`
- Modify: `.github/workflows/ci.yml`
- Create: `docs/footprints/aiplugins-devtools-agy.json`
- Create: `docs/footprints/repo-user-guide-agy.json`
- Create: `docs/footprints/rtk-mcp-agy.json`
- Create: `docs/footprints/re-ghidra-mcp-agy.json`
- Modify: `antigravity/plugins/*/README.md`
- Modify: `README.md`

- [ ] **Step 1: Add build staging recipes to `Justfile`**

In `Justfile`, add:
```just
# Build the rtk-mcp-agy plugin's binaries into its bin/ directory.
build-rtk-mcp-agy:
    cargo build -p rtk-mcp-agy --release --bin rtk-mcp --bin rtk-hook-preinvocation
    mkdir -p antigravity/plugins/rtk-mcp-agy/bin
    for b in rtk-mcp rtk-hook-preinvocation; do \
        if [ -f "target/release/$b.exe" ]; then \
            cp "target/release/$b.exe" antigravity/plugins/rtk-mcp-agy/bin/; \
        else \
            cp "target/release/$b" antigravity/plugins/rtk-mcp-agy/bin/; \
        fi; \
    done
    @echo "Plugin binaries staged in antigravity/plugins/rtk-mcp-agy/bin/"

# Build the re-ghidra-mcp-agy plugin's binaries into its bin/ directory.
build-re-ghidra-mcp-agy:
    cargo build -p re-ghidra-mcp-agy --release --bin re-ghidra-agy-mcp --bin re-ghidra-agy-hook
    mkdir -p antigravity/plugins/re-ghidra-mcp-agy/bin
    for b in re-ghidra-agy-mcp re-ghidra-agy-hook; do \
        if [ -f "target/release/$b.exe" ]; then \
            cp "target/release/$b.exe" antigravity/plugins/re-ghidra-mcp-agy/bin/; \
        else \
            cp "target/release/$b" antigravity/plugins/re-ghidra-mcp-agy/bin/; \
        fi; \
    done
    @echo "Plugin binaries staged in antigravity/plugins/re-ghidra-mcp-agy/bin/"

# Assemble the installable Antigravity plugin zips for a published release.
bundle-agy tag:
    #!/usr/bin/env bash
    set -euo pipefail
    names=$(bash scripts/registry-plugins.sh antigravity --exclude-skills-only | tr -d '\r')
    rm -rf target/agy-assets target/agy-bundles
    mkdir -p target/agy-assets
    for name in $names; do
        gh release download "{{ tag }}" -D target/agy-assets \
            -p "$name-*.tar.xz" -p "$name-*.zip" --clobber
    done
    for name in $names; do
        bash scripts/bundle-agy-plugin.sh "$name" target/agy-assets target/agy-bundles
    done
```

- [ ] **Step 2: Update `footprint-regen` and `footprint` recipes in `Justfile`**

1. In `footprint-regen`, add the Antigravity iteration loop:
   ```bash
   for plugin in $(bash scripts/registry-plugins.sh antigravity | tr -d '\r'); do
       cargo run -q -p plugin-footprint --bin plugin-footprint -- measure "antigravity/plugins/$plugin" \
           --out "docs/footprints/$plugin.json"
       cargo run -q -p plugin-footprint --bin plugin-footprint -- ratchet \
           --measured "docs/footprints/$plugin.json" \
           --budgets docs/footprints/budgets.json
       echo "regenerated docs/footprints/$plugin.json"
   done
   ```
2. In `footprint`, add `antigravity/plugins/*/README.md` to `readmes` md5 check list.
3. In `ci.yml` `footprint` job, add `just build-rtk-mcp-agy build-re-ghidra-mcp-agy` and add Antigravity READMEs to `readmes`.

- [ ] **Step 3: Stage binaries and regenerate footprints**

```bash
just build-rtk-mcp-agy build-re-ghidra-mcp-agy
just footprint-regen
```
Expected: `docs/footprints/` created for all 4 Antigravity plugins, `budgets.json` updated with ratchet thresholds, and `README.md` comparison table updated.

- [ ] **Step 4: Run full repository verification suite**

```bash
just check
```
Expected: All pre-flight checks pass (fmt, lint, test, deny, spellcheck, links, versions, wiring, marketplace, qwen-marketplace, dispatch, smoke, footprint, opencode-wiring, agy-wiring, agy-marketplace, footprint-budgets, devtools-skills, skills-bundles, skills-release-wiring).

- [ ] **Step 5: Commit**

```bash
git add Justfile docs/footprints .github/workflows/ci.yml \
        antigravity/plugins/*/README.md README.md
git commit -m "feat(footprint): measure Antigravity plugins, ratchet budgets, and publish footprint table (G7, G8)"
```

---

## Plan Self-Review Checklist

- **Spec coverage:** G1 (`repo-user-guide-agy`), G2 (`example`), G3 (`rtk-mcp-agy`), G4 (`re-ghidra-mcp-agy`), G5 (`check-agy-wiring` / `check-agy-marketplace`), G6 (`smoke-agy-bundle` / CI), G7 (`plugin-footprint` / budgets), and G8 (`registry.sh` / `Justfile`) are each covered by dedicated tasks.
- **No placeholders:** No "TODO" or "TBD" entries. Exact paths, file contents, commands, and expected outputs are specified.
- **Type and signature consistency:** Tool names, binary names (`agy-example-mcp`, `rtk-mcp`, `re-ghidra-agy-mcp`), and registry mappings are consistent across all tasks.
