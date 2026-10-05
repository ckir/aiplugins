# OpenCode & Antigravity First-Class Bundles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give opencode and antigravity the same versioned install-zip distribution Claude and Qwen already have, so no install reads live `main`.

**Architecture:** Two new bundler scripts mirroring the existing Claude/Qwen bundlers, two new registry files in the dot-dir convention, INSTALL/README install sections moved to tag-pinned bundle URLs, and bundle+verify CI jobs mirroring `plugin-bundles.yml`. Depends on Plan A Tasks 1–2 (registry) and Task 6 (generator) — extend those rather than duplicating them; if Plan A is unlanded, implement the registry reads inline against `agents.json` exactly as Plan A specifies and note the follow-up to deduplicate.

**Tech Stack:** bash bundle scripts, `zip`/`unzip`, GitHub Actions (`workflow_run` chaining like `plugin-bundles.yml`), `jq`, `mlc` link check.

**Spec:** `docs/specs/2026-10-05-independent-plugin-versioning.md` (Closing the Opencode Gap; Antigravity Distribution; Migration URL rules)

## Global Constraints

- Unix exec bits must survive the zips: assemble on Linux (CI) or WSL; never zip from a Windows working copy.
- Tag-pinned URLs only: `releases/download/<package>-v<version>/<file>`; the `latest` shorthand MUST NOT appear in install docs.
- Bundle contents are deterministic: same tree + same binaries = same file list (sorted entries).

---

### Task 1: Record the existing bundlers' interface

**Files:** none (read-only investigation; deliverable is the interface table in the Task 2 commit message)
- Test: no behavior change; verify by inspection

**Interfaces:**
- Consumes: `scripts/bundle-plugin.sh`, `scripts/bundle-qwen-extension.sh`, `Justfile` `bundle-plugins`/`bundle-qwen` recipes, `scripts/probe-plugin-bin.sh`.
- Produces: exact contract the new scripts mirror (arg order, staging layout, zip flags, dispatcher handling).

- [ ] **Step 1: Read and record**

Read `scripts/bundle-plugin.sh` fully and record: (1) exact CLI args and their order, (2) staging directory layout before zipping, (3) the exact `zip` invocation and flags, (4) how per-target binaries and the dispatcher shim are placed, (5) how `probe-plugin-bin.sh` verifies the result. Do the same skim for `scripts/bundle-qwen-extension.sh` noting only where it differs.

- [ ] **Step 2: No commit (investigation only)**

Proceed to Task 2 with the recorded contract; quote the arg order and zip flags in the Task 2 commit message body.

### Task 2: `bundle-opencode-plugin.sh` + local assembly test

**Files:**
- Create: `scripts/bundle-opencode-plugin.sh` (executable bit set, like the existing bundlers)
- Test: local assembly + content assertions + install dry-run

**Interfaces:**
- Consumes: Task 1 contract; plugin source tree (`plugin.ts`, `opencode.jsonc`, `package.json`, `skills/`, `agents/` when present, `README.md`, `INSTALL.md`).
- Produces: `<plugin>-opencode.zip` whose top level IS the plugin directory (extracting yields `plugin.ts`, `opencode.jsonc`, … directly, no wrapper folder).

- [ ] **Step 1: Write the script**

Mirror the Task 1 contract: usage `bundle-opencode-plugin.sh <name> <assets-dir> <out-dir>` is NOT needed (opencode bundles ship no binaries — they contain only versioned source). Final interface: `bundle-opencode-plugin.sh <plugin-dir> <out-dir>`, producing `<out-dir>/<name>-opencode.zip` from exactly: `plugin.ts`, `opencode.jsonc`, `package.json`, `skills/`, `agents/` (only if present — `rtk-mcp-opencode` has none), `README.md`, `INSTALL.md`. Exclude `node_modules/`, `bin/`, `tests/`, `examples/`, `bun.lock`. Refuse to run on non-Unix (`uname` check) with the same error wording style as the Claude bundler's platform guard.

- [ ] **Step 2: Assemble and assert contents**

Run: `bash scripts/bundle-opencode-plugin.sh opencode/rtk-mcp-opencode /tmp/oc-bundles && unzip -l /tmp/oc-bundles/rtk-mcp-opencode.zip`
Expected: lists exactly `plugin.ts opencode.jsonc package.json README.md INSTALL.md skills/gain/SKILL.md skills/rtk-policy/SKILL.md` (no `agents/`, no `bin/`, no `node_modules/`)

Run: `bash scripts/bundle-opencode-plugin.sh opencode/re-ghidra-mcp-opencode /tmp/oc-bundles && unzip -l /tmp/oc-bundles/re-ghidra-mcp-opencode.zip | grep -c "agents/re-analyst.md"`
Expected: `1` (agents included where present)

- [ ] **Step 3: Install dry-run (mirrors INSTALL.md verify logic)**

Run: `rm -rf /tmp/oc-dest && mkdir -p /tmp/oc-dest && cd /tmp/oc-dest && unzip -q /tmp/oc-bundles/rtk-mcp-opencode.zip && jq -e . opencode.jsonc > /dev/null && jq -e '.mcp.servers.rtk' opencode.jsonc > /dev/null && echo INSTALL_DRYRUN_OK`
Expected: `INSTALL_DRYRUN_OK`

- [ ] **Step 4: Commit**

```bash
git add scripts/bundle-opencode-plugin.sh
git commit -m "feat(opencode): versioned install-zip bundler

Contract mirrors scripts/bundle-plugin.sh: <plugin-dir> <out-dir> args,
top level of the zip IS the plugin directory, Linux-only platform guard."
```

### Task 3: `bundle-agy-plugin.sh` + local assembly test

**Files:**
- Create: `scripts/bundle-agy-plugin.sh` (executable bit set)
- Test: local assembly + content assertions + `probe-plugin-bin.sh` reuse

**Interfaces:**
- Consumes: Task 1 contract; agy plugin tree (`plugin.json`, `hooks.json`, `mcp_config.json`, `README.md`) + per-target binaries from dist archives. Known binary names from the wiring gate: `rtk-hook-preinvocation`, `rtk-mcp`, `re-ghidra-agy-hook`, `re-ghidra-agy-mcp`.
- Produces: `<plugin>-agy-plugin.zip` with the Claude-bundle layout convention: plugin files at top level, `bin/<target-triple>/` per-platform binaries plus top-level dispatcher shims, matching what `probe-plugin-bin.sh` expects.

- [ ] **Step 1: Write the script**

Usage `bundle-agy-plugin.sh <name> <assets-dir> <out-dir>` (same three-arg contract as the Claude bundler): stage the plugin directory, unpack each per-target dist archive's binaries into `bin/<triple>/`, place dispatcher shims at `bin/` top level exactly like the Claude layout, then zip. Same Linux-only guard.

- [ ] **Step 2: Assemble from the v0.7.2 release and assert**

Run from the repo root (`REPO` captures it so later steps work from temp dirs):

Run: `REPO="$(pwd)" && mkdir -p /tmp/agy-assets /tmp/agy-out && cd /tmp/agy-assets && gh release download v0.7.2 -D . -p "rtk-mcp-agy-*.tar.xz" -p "rtk-mcp-agy-*.zip" --clobber > /dev/null && bash "$REPO/scripts/bundle-agy-plugin.sh" rtk-mcp-agy "$PWD" /tmp/agy-out && unzip -l /tmp/agy-out/rtk-mcp-agy-agy-plugin.zip`
Expected: contains `plugin.json hooks.json mcp_config.json`, `bin/x86_64-unknown-linux-gnu/rtk-mcp`, `bin/rtk-mcp.exe`, and one entry per remaining target triple

- [ ] **Step 3: Probe the assembled bundle**

Run: `rm -rf /tmp/agy-probe && mkdir -p /tmp/agy-probe && cd /tmp/agy-probe && unzip -q /tmp/agy-out/rtk-mcp-agy-agy-plugin.zip -d rtk-mcp-agy && bash "$REPO/scripts/probe-plugin-bin.sh" rtk-mcp-agy/`
Expected: exit 0 (same probe the CI verify job uses; `REPO` is still set from Step 2 — re-export it with `REPO="$(git rev-parse --show-toplevel)"` if running in a fresh shell)

- [ ] **Step 4: Commit**

```bash
git add scripts/bundle-agy-plugin.sh
git commit -m "feat(agy): versioned install-zip bundler mirroring the Claude layout"
```

### Task 4: Registry files for both hosts

**Files:**
- Create: `.opencode-plugin/marketplace.json`, `.agy-plugin/marketplace.json`
- Test: shape equality with `.qwen-plugin/marketplace.json` + Plan A generator extension

**Interfaces:**
- Consumes: per-plugin manifests for `description`/`license`; Plan A `gen-marketplaces` (extend it; if Plan A is unlanded, hand-write the files exactly once and note the generator follow-up in the commit body).
- Produces: two registry files with the exact top-level shape of `.qwen-plugin/marketplace.json` (`$schema`, `name`, `owner`, `metadata`, `plugins[]`; per entry `name`, `source{source,url}`, `description`, `author`, `homepage`, `license`, `category`, `keywords`).

- [ ] **Step 1: Record the copy sources**

Read `qwen/rtk-mcp-qwen/qwen-extension.json` (`description`, `license`), `qwen/re-ghidra-mcp-qwen/qwen-extension.json` (same), `antigravity/rtk-mcp-agy/plugin.json` and `antigravity/re-ghidra-mcp-agy/plugin.json` (`description` if present else the `README.md` one-liner, `license`). Record the four descriptions verbatim; these are the strings the registry files must carry.

- [ ] **Step 2: Write both files**

`.opencode-plugin/marketplace.json` entries (`rtk-mcp-opencode`, `re-ghidra-mcp-opencode`): `source.source: "archive"`, `source.url` template `https://github.com/ckir/aiplugins/releases/download/<package>-v<version>/<package>-opencode.zip` with the CURRENT version filled in (regenerated per release by the Plan A generator extension, which owns these URLs afterwards — the template wording above is documentation, the file carries concrete URLs). `homepage` points at the plugin directory on `main`. Same shape for `.agy-plugin/marketplace.json` with `-agy-plugin.zip` assets. No `version` key on any entry (latest/download payload owns that — same rule as the Claude marketplace check enforces).

- [ ] **Step 3: Verify shape + extend the generator**

Run: `python3 -c "import json;q=json.load(open('.qwen-plugin/marketplace.json'));o=json.load(open('.opencode-plugin/marketplace.json'));a=json.load(open('.agy-plugin/marketplace.json'));assert set(o)==set(q)==set(a), 'top-level keys differ';print('shape ok')"`
Expected: `shape ok`

Then extend Plan A's `scripts/gen-marketplaces.sh` to render these two files from the same per-plugin sources (registry entries with non-null `marketplace`, which requires adding the two new paths to `agents.json`), and run `bash scripts/gen-marketplaces.sh --check` expecting exit 0.

- [ ] **Step 4: Commit**

```bash
git add .opencode-plugin/marketplace.json .agy-plugin/marketplace.json scripts/gen-marketplaces.sh agents.json
git commit -m "feat(marketplace): registry files for opencode and antigravity"
```

### Task 5: INSTALL.md and README install sections off versioned bundles

**Files:**
- Modify: `opencode/rtk-mcp-opencode/INSTALL.md`, `opencode/re-ghidra-mcp-opencode/INSTALL.md`, `antigravity/rtk-mcp-agy/README.md`, `antigravity/re-ghidra-mcp-agy/README.md` (install sections only; grep for `releases/latest/download` and `raw.githubusercontent.com` to enumerate every touch point first)
- Test: `mlc` offline link check + extraction dry-run against Task 2/3 zips

**Interfaces:**
- Consumes: Tasks 2–4 (bundle layout, registry URLs).
- Produces: install docs with zero `latest` and zero live-`main` file references.

- [ ] **Step 1: Enumerate every stale reference**

Run: `grep -rn "releases/latest/download\|raw.githubusercontent.com" opencode/*/INSTALL.md opencode/*/README.md antigravity/*/README.md`
Expected: the full touch list; every line below gets rewritten — no other files may reference these URL forms afterwards (re-run the grep to prove it, expecting empty output)

- [ ] **Step 2: Rewrite to versioned bundles**

Opencode INSTALL.md Step 2 (file list) becomes: download `<package>-opencode.zip` for the wanted version from `releases/download/<package>-v<version>/` and extract into `$DEST/plugins/`. Step 4 (binary from Claude `-plugin.zip`) is deleted — the opencode bundle plus the MCP binary both come from versioned release assets now (state the binary's exact asset name and triple path inside the referenced bundle). Antigravity READMEs get the same treatment with `-agy-plugin.zip`. Keep the merge-rules and verify steps byte-identical.

- [ ] **Step 3: Verify**

Run: `mlc --offline --ignore-path "./node_modules,./opencode/re-ghidra-mcp-opencode/node_modules,./opencode/rtk-mcp-opencode/node_modules,./target" > /dev/null && echo LINKS_OK` (the repo's `just links` command)
Expected: `LINKS_OK`

Run the rewritten opencode Step 2+verify against the Task 2 zip: fresh temp dir, unzip the locally built `-opencode.zip`, `jq -e . opencode.jsonc`, and from the `bun add` directory `bun -e "await import('$DEST/plugins/rtk-mcp-opencode.ts')"` per the existing verify step.
Expected: all exit 0

- [ ] **Step 4: Commit**

```bash
git add opencode/rtk-mcp-opencode/INSTALL.md opencode/re-ghidra-mcp-opencode/INSTALL.md antigravity/rtk-mcp-agy/README.md antigravity/re-ghidra-mcp-agy/README.md
git commit -m "docs: install opencode and agy plugins from versioned bundles"
```

### Task 6: Bundle + verify CI jobs for both hosts

**Files:**
- Create: `.github/workflows/opencode-bundles.yml`, `.github/workflows/agy-bundles.yml` (both modeled on `.github/workflows/plugin-bundles.yml`, which chains via `workflow_run` on `Release` completed plus `workflow_dispatch` tag input, then `bundle` + 3-OS `verify` jobs)
- Test: workflow registration + full CI run on the PR

**Interfaces:**
- Consumes: Tasks 2–3 scripts, `probe-plugin-bin.sh`, dist per-target archives on the release.
- Produces: `-opencode.zip` / `-agy-plugin.zip` assets uploaded to each package release, verified by extraction + entry-point probe on ubuntu/windows/macos.

- [ ] **Step 1: Write both workflows**

Mirror `plugin-bundles.yml` exactly: same `workflow_run`/`workflow_dispatch` triggers, same tag-guard shape (`head_branch` starts with the package tag pattern), `bundle` job (checkout at `$TAG`, download per-target archives with `gh release download`, assemble with the Task 2/3 script, `gh release upload --clobber`), `verify` matrix (download published zips, host-appropriate extract, `probe-plugin-bin.sh` per extracted dir). The ONLY intentional differences from the Claude file: script names, asset globs (`*-opencode.zip`, `*-agy-plugin.zip`), and plugin-name lists (opencode: from `.opencode-plugin/marketplace.json`; agy: from `.agy-plugin/marketplace.json`, both with `tr -d '\r'`).

- [ ] **Step 2: Register and validate**

Run: `gh workflow list --json name,state --jq '.[].name' | grep -E "Opencode|gy|Agravity|Agy" || gh workflow list | head -n 12`
Expected: both new workflows listed and enabled (adjust the grep; the point is server-side registration, verified by eye)

- [ ] **Step 3: Commit (CI run on the PR is the live test)**

```bash
git add .github/workflows/opencode-bundles.yml .github/workflows/agy-bundles.yml
git commit -m "feat(ci): bundle and verify opencode and agy install zips"
```
