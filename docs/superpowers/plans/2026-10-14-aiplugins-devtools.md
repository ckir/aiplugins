# aiplugins-devtools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a skills-only plugin published as `aiplugins-devtools` on Claude Code, OpenCode, Qwen and Antigravity, carrying one skill, `code-review-and-quality`, and its two checklists.

**Architecture:** One canonical skill under `shared/devtools/skills/`; an emit recipe copies it into four per-agent plugin folders whose registry/folder/tag names stay agent-suffixed (`aiplugins-devtools-cc|qwen|agy|opencode`) while each manifest's `name` is the plain `aiplugins-devtools`. A new `publishedNames` map in `agents.json` links the two. Skills-only plugins have no crate and no binary, so they get their own bundler and their own tag-triggered workflow instead of the dist-chained ones.

**Tech Stack:** bash + jq + zip/zipinfo (repo's script conventions), `just`, GitHub Actions YAML, release-please (generated config), the existing `plugin-footprint` Rust tool.

**Spec:** `docs/specs/2026-10-14-aiplugins-devtools-design.md`. This plan departs from it in exactly two places, both forced by code read while planning and both approved by the owner on 2026-10-14: (1) folders/tags are agent-suffixed while manifests publish the plain name (spec said unsuffixed folders); (2) the spec's "extend the bundle scripts" becomes one new bundler plus one new workflow.

## Global Constraints

- Plugin published name: `aiplugins-devtools` (every manifest `name`; CC marketplace key `aiplugins-devtools@aiplugins`). Registry/folder/tag keys: `aiplugins-devtools-cc`, `aiplugins-devtools-qwen`, `aiplugins-devtools-agy`, `aiplugins-devtools-opencode`.
- Initial version `0.1.0` in every manifest. License field `PolyForm-Noncommercial-1.0.0`; ported skill files stay MIT (upstream `github.com/addyosmani/agent-skills`, `Copyright (c) 2025 Addy Osmani`, upstream commit `28f435ea69a78531ec00779c7f3b78c64ce95896`).
- Registry schema additions (per agent object in `agents.json`): `"publishedNames": {"<key>": "aiplugins-devtools"}` and `"skillsOnly": ["<key>", …]`. "Skills-only" is an explicit registry fact, never inferred from a missing `Cargo.toml` or `plugin.ts` (an inferred exemption would silently weaken the checks for a real crate that lost its manifest).
- OpenCode skill folder is `aiplugins-devtools-code-review-and-quality` and its `name:` frontmatter equals the folder (the repo's `check-opencode-wiring.sh` already enforces name == folder).
- The skill and checklists must contain zero occurrences of `security-and-hardening`, `performance-optimization`, `constraint-driven-development`.
- Scripts follow the repo's: `set -euo pipefail`, `cd "$(dirname "$0")/.."`, `source scripts/lib/registry.sh`, refuse to run under MINGW/MSYS/CYGWIN when they build zips, `tr -d '\r'` after `jq -r`.
- Commit on a feature branch `feat/aiplugins-devtools` (never on `main`), conventional-commit messages scoped to the package, ending with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Verification gate for every task that touches scripts: `shellcheck <new script>` clean, and the repo checks named in that task green.

## Review Focus

- A tag `aiplugins-devtools-opencode-v0.1.0` matches opencode-bundles.yml's `*-opencode-v*` push filter and would run the binary-assuming bundler: pinned in Task 8 by a tag-filter exclusion check.
- Qwen/Antigravity `skills/` auto-discovery is inferred, not tested against live hosts (`re-ghidra-mcp-qwen` ships skills with no `skills` manifest key; the Antigravity builtin doc lists `skills/`): Task 10 records what could and could not be exercised.
- First-release mechanics (`.release-please-manifest.json` has no entry for the four packages) are NOT exercised by this plan: Task 9 states it plainly; owner decides tag-by-hand vs release-please bootstrap.
- A crate-less folder under a workspace glob breaks `cargo` (measured: `failed to load manifest for workspace member`): pinned in Task 3.
- `typos` / `mlc` may flag the ported prose or links: pinned in Task 1.

---

### Task 1: Canonical skill source and its content check

**Files:**
- Create: `shared/devtools/skills/code-review-and-quality/SKILL.md`
- Create: `shared/devtools/skills/code-review-and-quality/references/security-checklist.md`
- Create: `shared/devtools/skills/code-review-and-quality/references/performance-checklist.md`
- Create: `shared/devtools/skills/code-review-and-quality/THIRD-PARTY-NOTICE.md`
- Create: `scripts/check-devtools-skills.sh`
- Modify: `Justfile` (new recipe `devtools-skills`, appended to the `check` recipe's dependency list on line 218)
- Modify: `Cargo.toml` (`[workspace] exclude = ["shared/devtools"]`: the `members` glob `shared/*` matches the new directory, which has no crate)

**Interfaces:**
- Produces: `scripts/check-devtools-skills.sh` with two sections, `source` (this task) and `copies` (Task 3 extends it); exit 0 and prints `Devtools skills consistent.` when clean; prints `  FAIL  <reason>` lines to stderr and exits 1 otherwise.
- Produces: canonical skill directory `shared/devtools/skills/code-review-and-quality/` that the emit recipe (Task 3) copies.

- [ ] **Step 1: Write the check first (it must fail: no source yet).** `scripts/check-devtools-skills.sh` asserts, for the canonical dir: the four files above exist; `SKILL.md` frontmatter has `name: code-review-and-quality`; none of the three forbidden names appears in any of the three markdown files; every backticked `references/<file>` path in `SKILL.md` resolves under the skill dir; `THIRD-PARTY-NOTICE.md` contains `Copyright (c) 2025 Addy Osmani`, the 40-hex upstream commit `28f435ea69a78531ec00779c7f3b78c64ce95896`, and the words `Permission is hereby granted`.
- [ ] **Step 2: Run it.** `bash scripts/check-devtools-skills.sh` — Expected: exit 1, `FAIL` lines naming the missing files.
- [ ] **Step 3: Port the three markdown files from `/home/user/Development/Rust/agent-skills` at commit `28f435e…`** (`skills/code-review-and-quality/SKILL.md`, `references/security-checklist.md`, `references/performance-checklist.md`). Apply exactly the edits in the spec section "The ported skill", pinned to upstream lines: `SKILL.md` line 66 → "For detailed security guidance, see `references/security-checklist.md`."; line 79 → "For detailed performance checks, see `references/performance-checklist.md`."; line 164 → delete the final sentence ("For a project-wide mutation score, see `constraint-driven-development`."); line 302 → "follow the security guidance in `references/security-checklist.md`"; lines 353-354 → `references/security-checklist.md` / `references/performance-checklist.md`; add one attribution line under the `# Code Review and Quality` title naming upstream and `THIRD-PARTY-NOTICE.md`. In the checklists reword the mentions at `security-checklist.md:3` and `performance-checklist.md:3` and `:135` the same way (keep the sentence, drop the missing skill's name). Write `THIRD-PARTY-NOTICE.md` with the MIT text from `agent-skills/LICENSE`, the copyright line, upstream URL and commit.
- [ ] **Step 3b: Keep cargo working.** Creating `shared/devtools/` already broke it: run `cargo metadata --no-deps --format-version 1 >/dev/null` and quote the error (measured: `failed to load manifest for workspace member … shared/…`, `referenced via 'shared/*'`). Add `exclude = ["shared/devtools"]` under `[workspace]` in the root `Cargo.toml`; re-run — Expected: exit 0.
- [ ] **Step 4: Run the check.** Expected: `Devtools skills consistent.`
- [ ] **Step 5: Run `typos shared/devtools` and `mlc --offline shared/devtools`.** Expected: no findings. If `typos` flags upstream prose, add the word to `typos.toml` under `[default.extend-words]` mapped to itself with a one-line comment saying it is vendored upstream prose (precedent: `typos.toml` `unparseable = "unparseable"`); if `mlc` flags a link, fix the link, never delete the sentence.
- [ ] **Step 6: Add the `devtools-skills` recipe (`bash scripts/check-devtools-skills.sh`) and append it to `check:` in the Justfile.** Run `just devtools-skills` — Expected: same success line.
- [ ] **Step 7: Commit** `feat(aiplugins-devtools): add canonical code-review-and-quality skill`.

---

### Task 2: Registry published names, and marketplace checks that tolerate them

**Files:**
- Modify: `scripts/lib/registry.sh` (add two functions; update its `--self-test` block)
- Modify: `scripts/check-marketplace.sh` (entry loop starting at the `for name in $entries` loop; reverse loop at the end)
- Modify: `scripts/check-qwen-marketplace.sh` (same two loops)
- Create: `scripts/registry-plugins.sh`

**Interfaces:**
- Produces: `scripts/registry-plugins.sh <agent-id> [--exclude-skills-only]` → prints the agent's registry plugin keys one per line (CRLF stripped), omitting `skillsOnly` ones with the flag. It exists because eight call sites (Justfile recipes and `ci.yml` loops, Tasks 3, 4, 6, 9) currently loop over marketplace entry names or `opencode/*/`, which no longer equal the folder names; they call this instead of re-deriving the list.
- Produces: `agents.json` agent objects may carry `"publishedNames": { "<registry key>": "<published name>" }` (absent = identity).
- Produces: `registry_published_name <agent> <plugin>` → prints published name, or `<plugin>` when unmapped.
- Produces: `registry_plugin_for_published <agent> <name>` → prints the registry key whose published name is `<name>`; prints `<name>` unchanged when none maps (so every existing plugin keeps working).
- Produces: `registry_is_skills_only <agent> <plugin>` → exit 0 when `<plugin>` is in that agent's `skillsOnly` list, non-zero otherwise (same shape as `registry_is_excluded`).

- [ ] **Step 1: Extend the self-test first.** In `registry.sh --self-test` add assertions against the real `agents.json` (identity, since no mapping exists yet): `registry_published_name claude-code rtk-mcp-cc` = `rtk-mcp-cc`; `registry_plugin_for_published claude-code rtk-mcp-cc` = `rtk-mcp-cc`. Add a mapped case against a scratch copy of `agents.json` (in the session scratchpad) that adds `"publishedNames": {"rtk-mcp-cc": "demo"}` and `"skillsOnly": ["rtk-mcp-cc"]` to the claude-code agent, run with `REGISTRY=<scratch copy>`: `registry_published_name claude-code rtk-mcp-cc` = `demo`, `registry_plugin_for_published claude-code demo` = `rtk-mcp-cc`, `registry_is_skills_only claude-code rtk-mcp-cc` succeeds. Against the real file (no `skillsOnly` yet): `registry_is_skills_only claude-code rtk-mcp-cc` fails (non-zero), and `registry_is_skills_only claude-code no-such-plugin` fails. For `registry-plugins.sh`: against the real file `bash scripts/registry-plugins.sh claude-code | tr '\n' ' '` prints `rtk-mcp-cc re-ghidra-mcp-cc `, and with `REGISTRY=<scratch copy>` and `--exclude-skills-only` prints `re-ghidra-mcp-cc ` (the scratch copy marks `rtk-mcp-cc` skills-only).
- [ ] **Step 2: Run `bash scripts/lib/registry.sh --self-test`.** Expected: FAIL (functions missing).
- [ ] **Step 3: Implement the three functions** (`registry_published_name` via `.publishedNames[$p] // $p`; `registry_plugin_for_published` via an inverse `to_entries` lookup; `registry_is_skills_only` via `jq -e` over `.skillsOnly[]?`). Change line 6 so `REGISTRY` is overridable (`REGISTRY="${REGISTRY:-<current expression>}"`) so the self-test can point at the scratch copy.
- [ ] **Step 3b: Write `scripts/registry-plugins.sh`** to the interface (`source scripts/lib/registry.sh`; loop `registry_plugins` and filter with `registry_is_skills_only`).
- [ ] **Step 4: Make both marketplace checks resolve entries through the helpers.** In each entry loop: `key=$(registry_plugin_for_published "$agent" "$name")`; derive the manifest path, the Cargo.toml path and the URL from `$key`; keep the manifest-name assertion comparing the manifest `.name` to the entry `$name`. When `registry_is_skills_only "$agent" "$key"` succeeds, skip the crate-version equality assertion (opencode precedent in `check-package-versions.sh`: a crate-less package's version lives in its manifest alone) but still require `.version` to be present; for every other plugin a missing `Cargo.toml` stays a `FAIL`, exactly as today. In the reverse loop (`for dir in "$agent_dir"/*/`) compare `registry_published_name "$agent" "$name"` to the entries.
- [ ] **Step 5: Run `bash scripts/lib/registry.sh --self-test && bash scripts/check-marketplace.sh && bash scripts/check-qwen-marketplace.sh && bash scripts/gen-marketplaces.sh --check`.** Expected: `registry self-test ok`, `Marketplace lists 2 plugin(s)…`, `Qwen marketplace lists 2 extension(s)…`, `Marketplaces are current.` (no behaviour change for existing plugins).
- [ ] **Step 6: `shellcheck` the three scripts; commit** `refactor(scripts): resolve marketplace entries through registry published names`.

---

### Task 3: Claude Code plugin `aiplugins-devtools-cc` and the emit/drift machinery

**Files:**
- Create: `claude-code/aiplugins-devtools-cc/.claude-plugin/plugin.json`, `README.md`, `CHANGELOG.md`, and (generated) `skills/code-review-and-quality/**`
- Modify: `agents.json` (claude-code `plugins` += `aiplugins-devtools-cc`; add `publishedNames`)
- Modify: `Cargo.toml` (workspace `exclude` += `claude-code/aiplugins-devtools-cc`)
- Modify: `Justfile` (recipe `emit-devtools-skills`, CC branch only; and the three marketplace-name loops at `smoke` (line 213), `footprint-regen` (line 89) and `bundle-plugins` (line 294))
- Modify: `.github/workflows/ci.yml` (the smoke loop at line 238)
- Create (generated): `docs/footprints/aiplugins-devtools-cc.json`; modify `docs/footprints/budgets.json` and the README footprint regions
- Modify: `scripts/check-devtools-skills.sh` (`copies` section, CC row)
- Modify: `scripts/check-package-versions.sh`, `release-please-config.json` (generated), marketplace files (generated)

**Interfaces:**
- Consumes: Task 1 canonical dir; Task 2 `publishedNames`.
- Produces: recipe `emit-devtools-skills` (one block per agent; Tasks 4-6 add theirs); `copies` check compares each emitted copy to canonical (`diff -r`), failing with `FAIL  stale copy: <path>`.

- [ ] **Step 1: Add the failing drift assertion for CC** (`claude-code/aiplugins-devtools-cc/skills/code-review-and-quality` must equal canonical). Run `bash scripts/check-devtools-skills.sh` — Expected: FAIL `stale copy`/missing.
- [ ] **Step 2: Write `plugin.json`** with `name: aiplugins-devtools`, `version: 0.1.0`, `description` (one sentence: dev-tool skills container, first skill code-review-and-quality), `author`, `homepage`, `repository`, `license`, `keywords` (shapes copied from `claude-code/rtk-mcp-cc/.claude-plugin/plugin.json`). README: install line `claude plugin install aiplugins-devtools@aiplugins`, what the skill is, attribution pointer, and the `<!-- footprint:begin -->`/`<!-- footprint:end -->` marker pair (empty; Step 6b fills it). CHANGELOG: header line `# Changelog` only.
- [ ] **Step 3: Add `emit-devtools-skills` CC block** (remove then `cp -r` the canonical dir into `claude-code/aiplugins-devtools-cc/skills/`). Run `just emit-devtools-skills`.
- [ ] **Step 4: Register.** `agents.json`: append the plugin; add `"publishedNames": {"aiplugins-devtools-cc": "aiplugins-devtools"}` and `"skillsOnly": ["aiplugins-devtools-cc"]` to the claude-code agent (Tasks 4-6 do the same for their agents). Root `Cargo.toml`: append `claude-code/aiplugins-devtools-cc` to the `[workspace] exclude` list Task 1 created. Run `cargo metadata --no-deps --format-version 1 >/dev/null` — Expected: exit 0 (before the `exclude` it fails with `failed to load manifest for workspace member`; confirm by running it once without the exclude first and quoting the error).
- [ ] **Step 5: Keep the existing registry self-test true.** `registry.sh --self-test` asserts `registry_plugins claude-code` prints exactly `rtk-mcp-cc re-ghidra-mcp-cc `; update that expected string to include `aiplugins-devtools-cc` (Tasks 4-6 change the other agents' lists, which that self-test does not assert). Add `registry_published_name claude-code aiplugins-devtools-cc` = `aiplugins-devtools` and its inverse to the self-test.
- [ ] **Step 5b: Version rows.** `check-package-versions.sh`: add row `"aiplugins-devtools-cc|…/plugin.json|…/plugin.json|.version"` (manifest to itself, the opencode precedent) and change its "eight" comments/messages to say the registry packages. Regenerate: `bash scripts/gen-release-please-packages.sh` then `bash scripts/gen-marketplaces.sh`.
- [ ] **Step 6: Run the full wiring set:** `bash scripts/check-devtools-skills.sh && bash scripts/check-package-versions.sh && bash scripts/gen-release-please-packages.sh --check && bash scripts/check-marketplace.sh && bash scripts/gen-marketplaces.sh --check && bash scripts/check-plugin-wiring.sh`. Expected: all pass; `.claude-plugin/marketplace.json` now has an entry named `aiplugins-devtools` whose `source.url` is `…/aiplugins-devtools-cc-v0.1.0/aiplugins-devtools-cc-plugin.zip`.
- [ ] **Step 6b: Make every Claude Code consumer of marketplace names registry-driven.** Measured: `just smoke` and the `ci.yml` smoke job loop `jq -r '.plugins[].name' .claude-plugin/marketplace.json` and call `scripts/smoke-bundle.sh "$plugin"`, which builds `claude-code/$plugin` (a folder that no longer matches the published name) and exits 1 on a plugin with no `${CLAUDE_PLUGIN_ROOT}/bin/…` command. Replace the loop source with `bash scripts/registry-plugins.sh claude-code --exclude-skills-only` in `smoke` (Justfile:213), the `ci.yml` smoke step (line 238) and `bundle-plugins` (Justfile:294, which would otherwise try to download `aiplugins-devtools-*` binary archives that do not exist); replace `footprint-regen`'s loop source (Justfile:89) with `bash scripts/registry-plugins.sh claude-code` (devtools included, no exclusion). Then build the sibling binaries the footprint tool probes (`just build-rtk-mcp-cc build-re-ghidra-mcp-cc`), run `just footprint-regen`, and `git add docs/footprints/ README.md claude-code/*/README.md`. Expected: a new `docs/footprints/aiplugins-devtools-cc.json` (resident bytes from skill frontmatter, no MCP servers), the CC README region filled, and the other plugins' documents unchanged. Verify with `just smoke` (Expected: exit 0, devtools not probed) and `just footprint` (Expected: no staleness error; a "has no budget yet; measuring without a ceiling" line for the new plugin is normal — if the gate reports anything else, read `tools/plugin-footprint/src/gate.rs` for how budgets are seeded and follow it).
- [ ] **Step 7: Commit** `feat(aiplugins-devtools-cc): add the Claude Code plugin`.

---

### Task 4: Qwen extension `aiplugins-devtools-qwen`

**Files:**
- Create: `qwen/aiplugins-devtools-qwen/qwen-extension.json`, `README.md`, `CHANGELOG.md`, generated `skills/code-review-and-quality/**`
- Modify: `agents.json`, `Cargo.toml` (`exclude` += `qwen/aiplugins-devtools-qwen`), `Justfile` (Qwen block), `scripts/check-devtools-skills.sh` (Qwen row), `scripts/check-package-versions.sh` (row), generated marketplaces/release config

**Interfaces:** Consumes Tasks 1-3. Manifest fields: `name`, `version`, `displayName`, `description`, `license` (shape of `qwen/rtk-mcp-qwen/qwen-extension.json` without `hooks`/`mcpServers`/`contextFileName`; no `skills` key, as `re-ghidra-mcp-qwen` has none yet ships `skills/`).

- [ ] **Step 1:** Add the failing Qwen drift row; run the check — Expected: FAIL.
- [ ] **Step 2:** Create manifest (`name: aiplugins-devtools`, `version: 0.1.0`), README (install via the Qwen mechanism the sibling README documents — read `qwen/rtk-mcp-qwen/README.md` Install section and mirror it, substituting names), CHANGELOG, emit block; run `just emit-devtools-skills`.
- [ ] **Step 3:** Register in `agents.json` (`plugins` and `publishedNames`), workspace `exclude`, version row; regenerate release config and marketplaces.
- [ ] **Step 3b:** `just bundle-qwen` (Justfile:314) loops `.qwen-plugin/marketplace.json` names and would download `aiplugins-devtools-*` binary archives that do not exist: replace its `names=` source with `bash scripts/registry-plugins.sh qwen --exclude-skills-only`. Verify: `bash scripts/registry-plugins.sh qwen --exclude-skills-only | tr '\n' ' '` prints `rtk-mcp-qwen re-ghidra-mcp-qwen `.
- [ ] **Step 4:** Run `bash scripts/check-devtools-skills.sh && bash scripts/check-package-versions.sh && bash scripts/gen-release-please-packages.sh --check && bash scripts/check-qwen-marketplace.sh && bash scripts/gen-marketplaces.sh --check && cargo metadata --no-deps --format-version 1 >/dev/null`. Expected: all pass.
- [ ] **Step 5:** Commit `feat(aiplugins-devtools-qwen): add the Qwen extension`.

---

### Task 5: Antigravity plugin `aiplugins-devtools-agy`

**Files:**
- Create: `antigravity/aiplugins-devtools-agy/plugin.json`, `README.md`, `CHANGELOG.md`, generated `skills/code-review-and-quality/**`
- Modify: `agents.json`, `Cargo.toml` (`exclude` += `antigravity/aiplugins-devtools-agy`), `Justfile`, `scripts/check-devtools-skills.sh`, `scripts/check-package-versions.sh`, generated files

**Interfaces:** `plugin.json` = `{name: "aiplugins-devtools", version: "0.1.0", description, license}` (siblings carry only name/version/description; `gen-marketplaces.sh` falls back to the workspace licence when `license` is absent, so omit it to match them). No `hooks.json`/`mcp_config.json`.

- [ ] **Step 1:** Add the failing agy drift row; run — Expected: FAIL.
- [ ] **Step 2:** Create files; emit; register; regenerate (`gen-release-please-packages.sh`, `gen-marketplaces.sh`).
- [ ] **Step 3:** Run `bash scripts/check-devtools-skills.sh && bash scripts/check-package-versions.sh && bash scripts/gen-release-please-packages.sh --check && bash scripts/gen-marketplaces.sh --check && bash scripts/check-plugin-wiring.sh && cargo metadata --no-deps --format-version 1 >/dev/null`. Expected: all pass (`check-plugin-wiring.sh` skips a plugin with no `hooks.json`/`mcp_config.json`).
- [ ] **Step 4:** Do NOT touch `.agents/plugins.json` (it registers in-repo development plugins only: one entry today, `antigravity/rtk-mcp-agy`).
- [ ] **Step 5:** Commit `feat(aiplugins-devtools-agy): add the Antigravity plugin`.

---

### Task 6: OpenCode plugin `aiplugins-devtools-opencode`

**Files:**
- Create: `opencode/aiplugins-devtools-opencode/package.json`, `README.md`, `INSTALL.md`, `CHANGELOG.md`, generated `skills/aiplugins-devtools-code-review-and-quality/**`
- Modify: `agents.json`, `Justfile` (OpenCode emit block with rename; `test-opencode` line 79; `footprint-regen` OpenCode loop line 97), `.github/workflows/ci.yml` (line 81), `scripts/smoke-opencode.sh` (line 46), `scripts/check-devtools-skills.sh` (OpenCode row, rename-aware), `scripts/check-package-versions.sh`, `scripts/check-opencode-wiring.sh`, generated files

**Interfaces:**
- `package.json`: `{name: "aiplugins-devtools", version: "0.1.0", private: true, description, license}` — no `dependencies` (no `@opencode/plugin`).
- Emit rule: copy canonical to `skills/aiplugins-devtools-code-review-and-quality/` and rewrite exactly the line `name: code-review-and-quality` to `name: aiplugins-devtools-code-review-and-quality`; nothing else changes.
- `check-opencode-wiring.sh` treats a plugin for which `registry_is_skills_only opencode "$name"` succeeds as skills-only: it still runs checks 2 (semver) and 5 (skill frontmatter == folder) and skips 1, 3, 4 and the `@opencode/plugin` pin check 8 for it; every other plugin behaves exactly as before (a non-skills-only plugin with no `plugin.ts` still fails check 1).

- [ ] **Step 1: Failing tests.** (a) Add the OpenCode drift row to `check-devtools-skills.sh`: copy must equal canonical after applying that single `sed` rename (`diff <(sed … canonical/SKILL.md) copy/SKILL.md` plus `diff -r` of the rest). (b) Run `bash scripts/check-opencode-wiring.sh` after registering the plugin but before changing it — Expected: FAIL with `plugin.ts missing`; quote that output.
- [ ] **Step 2:** Create files; emit block; register in `agents.json` (`plugins` + `publishedNames`; no Cargo `exclude` needed, `opencode/*` is not a workspace glob).
- [ ] **Step 3:** Modify `check-opencode-wiring.sh` per the interface. Run it — Expected: passes, existing plugins' lines unchanged; the new plugin prints `ok` for its skill frontmatter.
- [ ] **Step 3b: Skip the skills-only dir in the four places that assume an OpenCode plugin has code.** Measured: `bun test tests/` exits 1 in a package with no `tests/` ("Tests need .test … in the filename", exit 1); `scripts/smoke-opencode.sh:46` imports `./$dir/plugin.ts`; `plugin-footprint measure` exits 2 on a directory without `plugin.ts` + `opencode.jsonc`. Add `[ -f "$dir/plugin.ts" ] || continue` (with a comment that `check-opencode-wiring.sh` check 1 is what fails loudly for a real plugin that lost its `plugin.ts`) to: the `test-opencode` loop (Justfile:79), the `opencode-test` job loop (`ci.yml:81`), `scripts/smoke-opencode.sh:46`, and the `footprint-regen` OpenCode loop (Justfile:97). The `ci.yml:270` loop only runs `bun install`, which exits 0 on the new package (measured), so it stays. Verify: `just test-opencode` — Expected: the two existing plugins' tests run, the new directory is skipped, exit 0; `bash scripts/smoke-opencode.sh` — Expected: same two plugins only (needs the mock fixture: build it as `ci.yml`'s opencode-smoke job does); `just footprint-regen` — Expected: no new OpenCode document, existing ones unchanged.
- [ ] **Step 4:** `INSTALL.md`: copy `skills/aiplugins-devtools-code-review-and-quality` to `~/.config/opencode/skills/` or `.opencode/skills/` (global/project, mirroring the sibling INSTALL's destination question), state the skill is invoked by the prefixed folder name (a colon is not valid in a folder name), and that there is no `opencode.jsonc` to merge and no binary.
- [ ] **Step 5:** Run `bash scripts/check-devtools-skills.sh && bash scripts/check-opencode-wiring.sh && bash scripts/check-package-versions.sh && bash scripts/gen-release-please-packages.sh --check && bash scripts/gen-marketplaces.sh --check`. Expected: all pass.
- [ ] **Step 6:** Commit `feat(aiplugins-devtools-opencode): add the OpenCode plugin`.

---

### Task 7: Skills-only bundler

**Files:**
- Create: `scripts/bundle-skills-plugin.sh`, `scripts/check-skills-bundles.sh`
- Modify: `Justfile` (recipe `skills-bundles`; append to `check`)

**Interfaces:**
- `scripts/bundle-skills-plugin.sh <agent-id> <plugin-key> <out-dir> [<plugin-dir>]`: refuses (exit 1) unless `registry_is_skills_only <agent-id> <plugin-key>`; the optional fourth argument overrides the source directory (default `<dir>/<plugin-key>`) so tests can bundle a throwaway copy. Reads `dir`, `manifestPattern`, `artifactSuffix` from the registry via `registry_field`/`registry_manifest_path`; stages `<dir>/<plugin-key>/` minus `tests/` and `.gitignore`, adds the repo `LICENSE`; zips the staging children by name; writes `<out-dir>/<plugin-key><artifactSuffix>`; prints `Bundled <key> -> <zip>`. Exit 2 on bad usage; exit 1 under MINGW/MSYS/CYGWIN or on any failed verification (and removes the zip).
- Verification inside it (via `zipinfo -1`): the manifest entry (manifest path relative to the plugin dir) present; `README.md` present; at least one `skills/*/SKILL.md`; `THIRD-PARTY-NOTICE.md` under that skill dir; `LICENSE`; for `opencode` also `INSTALL.md`; no entry starting `bin/`.
- `scripts/check-skills-bundles.sh`: builds all four bundles into a temp dir and asserts for each: zip exists with the registry-derived name, and the manifest entry's `.name` is `aiplugins-devtools`.

- [ ] **Step 1: Write `check-skills-bundles.sh` first** with those assertions plus negative controls: a bundle of a throwaway copy of a plugin dir with its `skills/` removed must make the bundler exit 1 and leave no zip; a copy containing `bin/x` must also exit 1; bundling a non-skills-only plugin (`claude-code rtk-mcp-cc`) must exit 1. The negative controls pass the throwaway copy as the fourth argument. Run — Expected: FAIL (bundler missing).
- [ ] **Step 2: Implement the bundler** to the interface; mirror the structure of `scripts/bundle-opencode-plugin.sh` (tool check, Windows refusal, `ls -A` child zipping, verification block).
- [ ] **Step 3:** Run `bash scripts/check-skills-bundles.sh` — Expected: passes, including both negative controls. Run `shellcheck` on both scripts.
- [ ] **Step 4:** Add recipe and `check` dependency; run `just skills-bundles`. Commit `feat(scripts): bundle skills-only plugins`.

---

### Task 8: Release plumbing for crate-less tags

**Files:**
- Create: `.github/workflows/skills-bundles.yml`
- Modify: `Justfile` (`setup` recipe: add `pyyaml`; recipe `skills-release-wiring` appended to `check`)
- Modify: `.github/workflows/release.yml` (`push.tags` at lines 58-60), `.github/workflows/opencode-bundles.yml` (`push.tags` at lines 34-35), `.github/workflows/ci.yml` (wiring job; insert after the `Check opencode wiring` step at line 119)
- Create: `scripts/check-skills-release-wiring.sh`

**Interfaces:**
- `skills-bundles.yml`: triggers `push: tags: ['aiplugins-devtools-*-v*']` and `workflow_dispatch` (input `tag`). Job `bundle`: resolve `key="${TAG%-v*}"`; find the agent whose `plugins` contains `key` via `agents.json` (fail loudly if none); run `scripts/bundle-skills-plugin.sh <agent> <key> bundles`; `gh release upload "$TAG" bundles/* --clobber`. Both jobs declare `permissions: contents: write` and `env: GH_TOKEN: ${{ github.token }}`, and tag-derived values reach `run:` scripts only through `env: TAG: ${{ inputs.tag || github.ref_name }}`, never interpolated into the script body (the pattern of `plugin-bundles.yml:37-38,53` and `opencode-bundles.yml:44-45,62`). Before uploading, the bundle job runs `gh release view "$TAG" >/dev/null 2>&1 || gh release create "$TAG" --title "$TAG" --notes "Skills-only bundle for $TAG"`, because a hand-pushed tag (`scripts/bootstrap-package-tags.sh:28-29` only runs `git tag` + `git push`) has no GitHub Release, and `gh release upload` would abort with `release not found`; release-please-created tags already have one, so the view succeeds. Job `verify` declares `needs: bundle` (all four sibling bundle workflows do, e.g. `plugin-bundles.yml:106`; without it `verify` races the upload) and runs on a matrix of ubuntu/windows/macos: download the one published asset, extract it (unzip on unix, `Expand-Archive` on Windows, as the sibling workflows do), and assert the agent's manifest file and a `skills/*/SKILL.md` exist.
- `release.yml` `tags:` gains `- '!aiplugins-devtools-*-v*'` (precedent: the hand-added `- '!*-opencode-v*'`; `allow-dirty = ["ci"]` in `dist-workspace.toml` permits it). `opencode-bundles.yml` `push.tags` gains the same exclusion, so the devtools OpenCode tag never reaches the binary-assuming bundler.
- `check-skills-release-wiring.sh` asserts (with `grep`/`jq`/`python3 -c 'import yaml'`): both exclusion lines present; `skills-bundles.yml` parsed as YAML has `jobs.verify.needs == "bundle"` (this workflow only runs on release tags, so no PR build would otherwise notice its absence); `jobs.bundle` and `jobs.verify` each set `GH_TOKEN`; `skills-bundles.yml` parses as YAML and its tag filter matches each of the four registry keys' tags (`<key>-v0.1.0`) under a bash glob test; and `aiplugins-devtools-opencode-v0.1.0` is excluded from `opencode-bundles.yml`'s filter while `rtk-mcp-opencode-v0.7.3` is still matched. The glob test emulates GitHub's tag filter (a tag is selected when the LAST pattern that matches it is a positive one; `*` does not cross `/`), and the script says in a comment that it is an emulation, not GitHub's own evaluator. The YAML parse needs PyYAML: the CI step runs `python3 -m pip install pyyaml` before it, and the script exits 1 (not skips) when `import yaml` fails, with a message naming `python3 -m pip install --user pyyaml`; add that same line to the Justfile `setup` recipe (Justfile:9-14), which installs the other local check tools.

- [ ] **Step 1: Write the wiring check first;** run it — Expected: FAIL (no workflow, no exclusions).
- [ ] **Step 2:** Write `skills-bundles.yml`; add the two exclusions; add `Check skills bundles` and `Check skills release wiring` and `Check devtools skills` steps to the `wiring` job in `ci.yml` after `Check opencode wiring`.
- [ ] **Step 3:** Run the wiring check — Expected: passes. Run `python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]" .github/workflows/*.yml` — Expected: no exception. (`actionlint` is not installed on this machine; say so in the PR.)
- [ ] **Step 4:** Commit `ci(aiplugins-devtools): bundle and release crate-less skills plugins`.

---

### Task 9: Root docs, release-please baseline, final gate

**Files:**
- Modify: `README.md` (install table row for OpenCode, documentation index entries, structure mention of `shared/devtools/`), `.release-please-manifest.json`

**Interfaces:** `.release-please-manifest.json` gains four entries `"<agent dir>/<key>": "0.1.0"`, matching the plugin manifests, so the four packages already in the generated config are not left without a baseline. (Footprint, smoke and bundle-recipe loops were fixed in Tasks 3, 4 and 6, so `just check` has stayed green since Task 3.)

- [ ] **Step 1:** Add the four manifest entries; run `bash scripts/gen-release-please-packages.sh --check` — Expected: `release-please packages match the registry.`
- [ ] **Step 2:** Run `cargo run -q -p plugin-footprint --bin plugin-footprint -- measure opencode/aiplugins-devtools-opencode` — Expected: exit 2 `is not a plugin directory` (the reason Task 6 skips it in `footprint-regen`); quote it in the PR.
- [ ] **Step 4:** README edits: documentation-index bullets under `claude-code/`, `antigravity/`, `qwen/`, `opencode/` (one line each: `aiplugins-devtools-<x>` — dev-tool skills, first skill `code-review-and-quality`); OpenCode install table row (Copies `skills/*` → `.opencode/skills/`, Binary staging `none`); repository-structure `shared/` note that `shared/devtools/` holds canonical skills, not crates. Run `mlc --offline --ignore-path "./node_modules,./target" ` as the Justfile `links` recipe does — Expected: no broken links.
- [ ] **Step 5:** Run `just check` — Expected: green except steps that need tools or hosts absent here (state which). Commit `docs(aiplugins-devtools): READMEs, index and release baseline`.

---

### Task 10: Install smoke and honest reporting

**Files:** none created; evidence goes in the PR description.

- [ ] **Step 1: Claude Code.** `bash scripts/bundle-skills-plugin.sh claude-code aiplugins-devtools-cc <tmp>`; unzip to `<tmp>/ext`; read `claude plugin --help` and `claude --help` first, then run the validate subcommand only if it exists and load the extracted dir with whichever flag `claude --help` documents for a local plugin directory (expected `--plugin-dir`; if it does not exist, say so) in a scratch session and confirm the skill `aiplugins-devtools:code-review-and-quality` is listed. Record the exact name the host printed.
- [ ] **Step 2: OpenCode.** Run the INSTALL.md copy into a scratch `.opencode/skills/` and run `opencode` if installed to list skills; else say not exercised.
- [ ] **Step 3: Qwen and Antigravity.** If `qwen` / `agy` are on PATH, install the bundle via each host's local-install command (read `qwen extensions --help` / `agy plugin --help` first; `agy` is at `/home/user/.local/bin/agy`) and list skills; Antigravity's own builtin doc is `~/.gemini/antigravity-cli/builtin/skills/agy-customizations/docs/plugins.md`. Whatever cannot run is reported as "not exercised", never as passed.
- [ ] **Step 4:** Write the PR description: what changed, the departures from the spec, hosts exercised vs not, the unexercised first-release mechanics (Task 9 seeds `.release-please-manifest.json` at `0.1.0`, so release-please will treat `0.1.0` as already released and propose the NEXT version; the marketplace entries already point at `aiplugins-devtools-<x>-v0.1.0`, so the owner must create those four tags by hand, as `scripts/bootstrap-package-tags.sh` did for the existing packages (the skills workflow creates the missing GitHub Release itself); push them in two batches of at most three, because `release.yml:44` records that GitHub drops tag-push events when more than 3 tags land on one commit — release-please's behaviour for a never-tagged seeded package was not tested), and that `actionlint` was unavailable.
- [ ] **Step 5:** Run the final repo gate once more and the agy-capstone discipline per the owner's rules before declaring complete.
