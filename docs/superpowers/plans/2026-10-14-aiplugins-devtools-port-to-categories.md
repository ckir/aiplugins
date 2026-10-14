# Port aiplugins-devtools onto the per-agent-category layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring branch `feat/aiplugins-devtools` up to date with `main` (about 60 commits ahead, including a repository restructure) so it can merge, without changing what the branch delivers.

**Architecture:** Merge `main` into the feature branch (keep the 20 reviewed commits), resolve the conflicts, move the four plugin folders to the new `<agent>/plugins/<key>` locations, make every file that names those paths follow the registry instead of literals where practical, regenerate the generated files, and re-run every gate.

**Tech Stack:** git, bash + jq, `just`, GitHub Actions YAML, Rust (`tools/plugin-footprint`).

**Spec:** `docs/specs/2026-10-14-aiplugins-devtools-design.md` and the original plan `docs/superpowers/plans/2026-10-14-aiplugins-devtools.md` (both still describe the plugin; only the PATHS changed, see below).

## Verified facts about the new `main` (read before writing this plan; commit `2c79de5`)

- Layout: each agent directory is now split into categories (`plugins/ agents/ skills/ commands/ models/ themes/ hooks/`). Plugins live at `claude-code/plugins/<key>`, `qwen/plugins/<key>`, `opencode/plugins/<key>`, `antigravity/plugins/<key>`. `claude-code/skills/` etc. hold only a README.
- `agents.json` on main: per agent `"dir": "<agent>/plugins"` and `"manifestPattern": "<agent>/plugins/{plugin}/<manifest>"` (e.g. `claude-code/plugins/{plugin}/.claude-plugin/plugin.json`); artifact suffixes unchanged.
- Root `Cargo.toml` workspace members on main: `claude-code/plugins/*`, `antigravity/plugins/*`, `qwen/plugins/*`, `shared/*`, `tools/*` (my crate-less folders must therefore be excluded at the NEW paths).
- `.release-please-manifest.json` keys on main are the new paths (`claude-code/plugins/rtk-mcp-cc`, ...), and a new `scripts/check-release-manifest.sh` (wired into `just check` and `ci.yml`) requires every registry package `<dir>/<name>` to have a manifest key and every manifest key to map to a registry package.
- Main added a ninth package `todov2-opencode` (pure TypeScript OpenCode plugin: has `plugin.ts`, `tool/`, `tui.tsx`, no `skills/`), reworked `scripts/bundle-opencode-plugin.sh` and `.github/workflows/opencode-bundles.yml` for it, changed `bun test tests/` to `bun test` in the opencode test loops, made `release.yml` create-if-missing-then-upload (the same idea as my `skills-bundles.yml`), and added `.github/workflows/release-refresh.yml`.
- Main changed `tools/plugin-footprint` (`main.rs`, `bin/footprint-gate.rs`, `manifest.rs`, tests) in the same functions the branch changed (`published_plugins()` replaced by `registry::published_plugin_keys`; ratchet keyed by file stem).
- A trial merge (aborted) conflicts in exactly these 10 files: `.github/workflows/ci.yml`, `.opencode-plugin/marketplace.json`, `.release-please-manifest.json`, `Justfile`, `README.md`, `agents.json`, `docs/footprints/budgets.json`, `release-please-config.json`, `scripts/check-package-versions.sh`, `scripts/smoke-opencode.sh`. Auto-merged without conflict but needing a semantic check: `scripts/check-marketplace.sh`, `scripts/check-qwen-marketplace.sh`, `scripts/check-opencode-wiring.sh`, `scripts/lib/registry.sh`, `tools/plugin-footprint/**`, `.github/workflows/release.yml`, `.github/workflows/opencode-bundles.yml`.
- Files on the branch that name the old paths literally (grep, excluding generated): `Cargo.toml`, `Justfile`, `README.md`, `qwen/aiplugins-devtools-qwen/README.md`, `scripts/check-devtools-skills.sh`, `scripts/check-package-versions.sh`, `scripts/check-skills-bundles.sh`, `shared/devtools/README.md`, plus the generated `.release-please-manifest.json`, `release-please-config.json` and the four marketplace files, plus the two historical docs under `docs/`.

## Global Constraints

- The merge of main INTO the branch keeps history (`git merge main`, never rebase or force-push; nothing is pushed).
- Everything main already has is preserved exactly (`todov2-opencode`, `release-refresh.yml`, the new check, the footprint tool changes). Every behaviour of the branch is preserved: published name `aiplugins-devtools`; keys `aiplugins-devtools-cc|qwen|agy|opencode`; `publishedNames` / `skillsOnly` in agents.json; the exemptions key ONLY on `skillsOnly`; the OpenCode skill folder and `name:` are `aiplugins-devtools-code-review-and-quality`; versions `0.1.0`.
- New plugin locations: `claude-code/plugins/aiplugins-devtools-cc`, `qwen/plugins/aiplugins-devtools-qwen`, `antigravity/plugins/aiplugins-devtools-agy`, `opencode/plugins/aiplugins-devtools-opencode` (move with `git mv` so history follows).
- Prefer deriving a plugin's directory from the registry (`registry_field <agent> dir`) over writing the literal path in scripts the branch added, so a future restructure does not break them again.
- Generated files (`release-please-config.json`, the four marketplace files, `docs/footprints/*`, README footprint regions) come from their generators, never by hand.
- Commits end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. Nothing is pushed or merged into `main` by this plan.

## Review Focus

- A path literal the grep misses (workflow, doc link, Justfile recipe, README install URL) that still names the old location: pinned by the final grep in Task 1 and by `mlc` link checks in Task 2.
- The skills-only exemptions weakening for the nine existing packages after the merge (todov2-opencode has `plugin.ts`; it must still be checked): pinned by `check-opencode-wiring.sh` in Task 2.
- `skills-bundles.yml` / `bundle-skills-plugin.sh` manifest-path math with the new `dir` value (`claude-code/plugins`): pinned by `check-skills-bundles.sh` and `check-skills-release-wiring.sh`.
- Footprint budgets and README footprint regions after the merge: pinned by `just footprint`.

---

### Task 1: Merge main, resolve conflicts, move the plugins, repoint everything

**Files:** every file named in "Verified facts" above (merge conflicts, literal paths), plus the four plugin directories (`git mv`).

**Interfaces:**
- Consumes: the registry shape on main (`dir` = `<agent>/plugins`).
- Produces: a branch whose registry-driven checks pass at the new paths.

- [ ] **Step 1:** On `feat/aiplugins-devtools` run `git merge main`. Resolve the 10 conflicted files by keeping BOTH sides' intent: for `agents.json` take main's file and re-add the branch's four plugin entries, `publishedNames` and `skillsOnly` per agent; for `.release-please-manifest.json` take main's keys and add `"claude-code/plugins/aiplugins-devtools-cc"`, `"qwen/plugins/aiplugins-devtools-qwen"`, `"opencode/plugins/aiplugins-devtools-opencode"`, `"antigravity/plugins/aiplugins-devtools-agy"` each `"0.1.0"`; for `release-please-config.json`, the four marketplace files and `docs/footprints/budgets.json` take main's version first and regenerate/re-add (Task 1 steps below, Task 2); for `Justfile`, `ci.yml`, `README.md`, `scripts/check-package-versions.sh`, `scripts/smoke-opencode.sh` combine: main's path updates and todov2/`bun test` changes plus the branch's additions (the `[ -f "$dir/plugin.ts" ] || continue` guards, `emit-devtools-skills`, the new recipes and CI steps, the new rows, the README entries) with all branch paths rewritten to the new layout.
- [ ] **Step 2:** `git mv` the four plugin folders to their new locations (see Global Constraints). Repoint every literal old path (Cargo.toml `exclude`: members are now `<agent>/plugins/*`, so the excludes become the new paths plus the unchanged `shared/devtools`; `scripts/check-devtools-skills.sh`; `scripts/check-skills-bundles.sh`; `scripts/check-package-versions.sh` rows; the Justfile `emit-devtools-skills` recipe; `qwen/plugins/aiplugins-devtools-qwen/README.md`; `shared/devtools/README.md`; root README links and the INSTALL.md raw URL). Where a branch-added script can take the directory from `registry_field <agent> dir` instead of a literal, do that.
- [ ] **Step 3:** Regenerate: `bash scripts/gen-release-please-packages.sh` then `bash scripts/gen-marketplaces.sh`.
- [ ] **Step 4:** Run, all must pass: `bash scripts/check-devtools-skills.sh && bash scripts/check-skills-bundles.sh && bash scripts/check-skills-release-wiring.sh && bash scripts/lib/registry.sh --self-test && bash scripts/gen-release-please-packages.sh --check && bash scripts/gen-marketplaces.sh --check && bash scripts/check-marketplace.sh && bash scripts/check-qwen-marketplace.sh && bash scripts/check-release-manifest.sh && bash scripts/check-package-versions.sh && bash scripts/check-opencode-wiring.sh && bash scripts/check-plugin-wiring.sh && cargo metadata --no-deps --format-version 1 >/dev/null`.
- [ ] **Step 5:** Final grep, expected empty: `grep -rIn "claude-code/aiplugins-devtools\|qwen/aiplugins-devtools\|antigravity/aiplugins-devtools\|opencode/aiplugins-devtools" --exclude-dir=.git --exclude-dir=target --exclude-dir=node_modules --exclude-dir=.superpowers --exclude-dir=.clavity .` except the two historical docs under `docs/` (leave their text; add one line at the top of the design doc: "Layout note: after the repository restructure the plugins live under `<agent>/plugins/`; paths below are the original ones").
- [ ] **Step 6:** Commit the merge (merge commit message `Merge main into feat/aiplugins-devtools` with the trailer) and any follow-up fix commits.

---

### Task 2: Build-dependent gates (footprint, smoke, full check)

**Files:** `docs/footprints/*`, `docs/footprints/budgets.json`, README footprint regions (generated), `tools/plugin-footprint/**` only if a semantic conflict with main's changes requires it, and any file the gates show to be stale.

- [ ] **Step 1:** Review the auto-merged footprint tool: confirm `registry::published_plugin_keys` is still the only source of plugin keys in `main.rs` and `bin/footprint-gate.rs`, that main's new `manifest.rs`/test changes coexist with the branch's ratchet-by-file-stem change, and run `cargo nextest run -p plugin-footprint`, `cargo clippy -p plugin-footprint --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [ ] **Step 2:** Build the sibling binaries (`just build-rtk-mcp-cc build-re-ghidra-mcp-cc build-rtk-mcp-opencode build-re-ghidra-mcp-opencode` as the Justfile defines them at the new paths), run `just footprint-regen`, `git add docs/footprints README.md` (the generated regions) and confirm only expected documents changed (the new `aiplugins-devtools-cc` document and budget key; main's `todov2-opencode` document must be unchanged or regenerated identically).
- [ ] **Step 3:** `just smoke`, `just test-opencode`, and the opencode smoke (`cargo build -p rtk-mcp-cc --bin mock-rtk-cc` then `env "CARGO_BIN_EXE_mock-rtk-cc=$(pwd)/target/debug/mock-rtk-cc" bash scripts/smoke-opencode.sh`): the skills-only OpenCode plugin must be skipped, `todov2-opencode`, `rtk-mcp-opencode` and `re-ghidra-mcp-opencode` must run.
- [ ] **Step 4:** `just check` exits 0; `mlc` link check on the changed READMEs has no new warnings; `claude plugin validate --strict` on a freshly built Claude Code bundle (built with `bash scripts/bundle-skills-plugin.sh claude-code aiplugins-devtools-cc <tmpdir>` and extracted) and on the repo's `.claude-plugin/marketplace.json` passes.
- [ ] **Step 5:** Commit generated updates and fixes; write `shared/devtools/README.md` updates if any path or command in it changed.
