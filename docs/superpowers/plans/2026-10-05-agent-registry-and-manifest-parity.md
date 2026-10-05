# Agent Registry & Manifest Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace per-host hardcoded wiring checks and hand-copied marketplace files with a single `agents.json` registry plus generated marketplaces, with zero behavior change.

**Architecture:** A small bash core (`scripts/lib/registry.sh`) resolves plugin dirs, manifest paths, and exclusions from `agents.json`; the existing check scripts keep only their host-specific comparisons. A `gen-marketplaces` generator renders both marketplace files from plugin manifests, and CI fails when committed copies drift.

**Tech Stack:** bash (Git Bash on Windows, `bash` on CI), `jq`, existing `scripts/check-*.sh`, `.github/workflows/ci.yml` wiring job.

**Spec:** `docs/specs/2026-10-05-independent-plugin-versioning.md` (Agent Registry; Single-Source Manifests & Check Parity sections)

## Global Constraints

- No per-agent special-casing outside `agents.json` — checks iterate the registry.
- Unregistered paths are ignored by every gate; registered ones must pass all of them (best-effort contract).
- Byte-identical output: regenerated marketplaces must `git diff --quiet` clean on the current tree.
- Windows-safe: strip `\r` from `jq`/file input exactly like the current `jqr()` helpers; keep `eol=lf` behavior unchanged.

---

### Task 1: `agents.json` registry

**Files:**
- Create: `agents.json`
- Test: none (validated in Step 2 by the core lib's loader)

**Interfaces:**
- Consumes: repo layout (`claude-code/`, `qwen/`, `opencode/`, `antigravity/`, `.claude-plugin/`, `.qwen-plugin/`), bundle filename conventions (`-plugin.zip`, `-extension.zip`).
- Produces: registry schema consumed by Tasks 2–6. Agent entry fields: `id`, `dir`, `plugins[]`, `marketplace` (path or `null`), `manifestPattern` (`{plugin}` placeholder for the per-plugin manifest), `artifactSuffix`, `sourceUrlBase`, `notPublished[]`.

- [ ] **Step 1: Write the registry file**

```json
{
  "agents": [
    {
      "id": "claude-code",
      "dir": "claude-code",
      "plugins": ["rtk-mcp-cc", "re-ghidra-mcp-cc"],
      "marketplace": ".claude-plugin/marketplace.json",
      "manifestPattern": "claude-code/{plugin}/.claude-plugin/plugin.json",
      "artifactSuffix": "-plugin.zip",
      "sourceUrlBase": "https://github.com/ckir/aiplugins/releases/latest/download",
      "notPublished": ["example"]
    },
    {
      "id": "qwen",
      "dir": "qwen",
      "plugins": ["rtk-mcp-qwen", "re-ghidra-mcp-qwen"],
      "marketplace": ".qwen-plugin/marketplace.json",
      "manifestPattern": "qwen/{plugin}/qwen-extension.json",
      "artifactSuffix": "-extension.zip",
      "sourceUrlBase": "https://github.com/ckir/aiplugins/releases/latest/download",
      "notPublished": ["example"]
    },
    {
      "id": "opencode",
      "dir": "opencode",
      "plugins": ["rtk-mcp-opencode", "re-ghidra-mcp-opencode"],
      "marketplace": null,
      "manifestPattern": "opencode/{plugin}/package.json",
      "artifactSuffix": "",
      "sourceUrlBase": "",
      "notPublished": []
    },
    {
      "id": "antigravity",
      "dir": "antigravity",
      "plugins": ["rtk-mcp-agy", "re-ghidra-mcp-agy"],
      "marketplace": null,
      "manifestPattern": "antigravity/{plugin}/plugin.json",
      "artifactSuffix": "",
      "sourceUrlBase": "",
      "notPublished": []
    }
  ]
}
```

- [ ] **Step 2: Validate it parses and covers the tree**

Run: `jq -e '.agents | length == 4' agents.json`
Expected: exit 0 (prints `true`)

Run: `for d in claude-code qwen opencode antigravity; do jq -e --arg d "$d" '.agents[] | select(.id == $d) | .plugins | length > 0' agents.json > /dev/null && echo "$d ok"; done`
Expected: all four `ok`

- [ ] **Step 3: Commit**

```bash
git add agents.json
git commit -m "feat(agents): add registry of supported agent fronts"
```

### Task 2: Shared registry core lib

**Files:**
- Create: `scripts/lib/registry.sh`
- Test: `scripts/lib/registry self-test` (new `--self-test` flag on the lib itself, run in CI wiring job)

**Interfaces:**
- Consumes: `agents.json` (Task 1).
- Produces: `registry_plugins <agent-id>` (prints plugin names, one per line), `registry_manifest_path <agent-id> <plugin>` (prints manifest path with `{plugin}` expanded), `registry_is_excluded <agent-id> <name>` (exit 0 if listed in `notPublished`), `registry_field <agent-id> <field>` (raw field value). All stdout values pass through `tr -d '\r'`.

- [ ] **Step 1: Write the failing probe (red)**

Run: `bash scripts/lib/registry.sh --self-test`
Expected: FAIL — file does not exist (`No such file or directory`)

- [ ] **Step 2: Write the lib with a self-test**

```bash
#!/usr/bin/env bash
# Shared core for per-host wiring checks: every agent/plugin enumeration
# comes from agents.json. No host names are hardcoded below this line.
set -euo pipefail

REGISTRY="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/agents.json"

rq() { jq -r "$@" "$REGISTRY" | tr -d '\r'; }

registry_plugins() {
    rq --arg a "$1" '.agents[] | select(.id == $a) | .plugins[]'
}

registry_manifest_path() {
    local pattern
    pattern=$(rq --arg a "$1" '.agents[] | select(.id == $a) | .manifestPattern')
    printf '%s\n' "${pattern//\{plugin\}/$2}"
}

registry_is_excluded() {
    local name=$2
    rq --arg a "$1" '.agents[] | select(.id == $a) | .notPublished[]' \
        | grep -qxF "$name"
}

registry_field() {
    rq --arg a "$1" --arg f "$2" '.agents[] | select(.id == $a) | .[$f] // ""'
}

if [ "${1:-}" = "--self-test" ]; then
    failures=0
    [ "$(registry_plugins claude-code | tr '\n' ' ')" = "rtk-mcp-cc re-ghidra-mcp-cc " ] \
        || { echo "FAIL plugins claude-code"; failures=$((failures + 1)); }
    [ "$(registry_manifest_path qwen rtk-mcp-qwen)" = "qwen/rtk-mcp-qwen/qwen-extension.json" ] \
        || { echo "FAIL manifest path"; failures=$((failures + 1)); }
    registry_is_excluded qwen example \
        || { echo "FAIL exclusion"; failures=$((failures + 1)); }
    ! registry_is_excluded qwen rtk-mcp-qwen \
        || { echo "FAIL non-exclusion"; failures=$((failures + 1)); }
    [ "$(registry_field claude-code artifactSuffix)" = "-plugin.zip" ] \
        || { echo "FAIL field"; failures=$((failures + 1)); }
    [ "$failures" -eq 0 ] && echo "registry self-test ok"
    exit "$failures"
fi
```

- [ ] **Step 3: Run the self-test (green)**

Run: `bash scripts/lib/registry.sh --self-test`
Expected: `registry self-test ok`, exit 0

- [ ] **Step 4: Commit**

```bash
git add scripts/lib/registry.sh
git commit -m "feat(agents): add shared registry core for wiring checks"
```

### Task 3: Claude marketplace check onto the core (behavior-preserving)

**Files:**
- Modify: `scripts/check-marketplace.sh` (replace hardcoded `not_published="example"`, `claude-code/$name/...` paths, source-url construction with registry calls; keep every comparison identical)
- Test: the script itself (output diff before/after), plus `ci.yml` wiring job still green

**Interfaces:**
- Consumes: `registry_plugins`, `registry_manifest_path`, `registry_is_excluded`, `registry_field` (Task 2).
- Produces: identical stdout/exit on the current tree (verified in Step 2).

- [ ] **Step 1: Capture baseline output (red reference)**

Run: `bash scripts/check-marketplace.sh > /tmp/mkt-before.txt 2>&1; echo "exit=$?"`
Expected: exit 0, ends with `all consistent with their manifests` (record the line count: `wc -l /tmp/mkt-before.txt`)

- [ ] **Step 2: Rewrite loops over the registry**

Replace the `not_published` block and the `for name in $entries` / reverse-direction `for dir in claude-code/*/` loops so plugin names, `plugin_json` paths, the exclusion, `artifactSuffix`, and `sourceUrlBase` all come from `registry_*` calls with agent id `claude-code`. Keep the `description`/`license` comparison, source-url/type checks, and no-`version` rule byte-for-byte.

- [ ] **Step 3: Verify identical behavior (green)**

Run: `bash scripts/check-marketplace.sh > /tmp/mkt-after.txt 2>&1; echo "exit=$?"; diff /tmp/mkt-before.txt /tmp/mkt-after.txt && echo IDENTICAL`
Expected: exit 0 and `IDENTICAL`

- [ ] **Step 4: Negative test (proves the check still catches drift)**

Run: `cp .claude-plugin/marketplace.json /tmp/mkt-backup.json && python3 -c "import json;p='.claude-plugin/marketplace.json';d=json.load(open(p));d['plugins'][0]['license']='WRONG';json.dump(d,open(p,'w'),indent=2)" && bash scripts/check-marketplace.sh 2>&1 | head -n 3; echo "exit=$?"; cp /tmp/mkt-backup.json .claude-plugin/marketplace.json`
Expected: a `FAIL ... license differs` line and non-zero exit; file restored afterwards (`git status --porcelain -- .claude-plugin/` empty)

- [ ] **Step 5: Commit**

```bash
git add scripts/check-marketplace.sh
git commit -m "refactor(agents): claude marketplace check reads the registry"
```

### Task 4: Qwen marketplace check onto the core

**Files:**
- Modify: `scripts/check-qwen-marketplace.sh` (same treatment, agent id `qwen`; the `example` exclusion now comes from the registry instead of its own `not_published` variable)
- Test: script output diff before/after + negative test

**Interfaces:**
- Consumes: Task 2 lib.
- Produces: identical stdout/exit on the current tree.

- [ ] **Step 1: Capture baseline output**

Run: `bash scripts/check-qwen-marketplace.sh > /tmp/qmkt-before.txt 2>&1; echo "exit=$?"`
Expected: exit 0, `all consistent with their manifests`

- [ ] **Step 2: Rewrite loops over the registry**

Same as Task 3 with agent id `qwen`. Delete the local `not_published="example"` block and route the reverse-direction loop through `registry_is_excluded`.

- [ ] **Step 3: Verify identical behavior**

Run: `bash scripts/check-qwen-marketplace.sh > /tmp/qmkt-after.txt 2>&1; echo "exit=$?"; diff /tmp/qmkt-before.txt /tmp/qmkt-after.txt && echo IDENTICAL`
Expected: exit 0 and `IDENTICAL`

- [ ] **Step 4: Negative test**

Run: `cp qwen/rtk-mcp-qwen/qwen-extension.json /tmp/qext-backup.json && python3 -c "import json;p='qwen/rtk-mcp-qwen/qwen-extension.json';d=json.load(open(p));d['description']='WRONG';json.dump(d,open(p,'w'),indent=2)" && bash scripts/check-qwen-marketplace.sh 2>&1 | head -n 3; echo "exit=$?"; cp /tmp/qext-backup.json qwen/rtk-mcp-qwen/qwen-extension.json`
Expected: `FAIL ... description differs` and non-zero exit; `git status --porcelain -- qwen/` empty afterwards

- [ ] **Step 5: Commit**

```bash
git add scripts/check-qwen-marketplace.sh
git commit -m "refactor(agents): qwen marketplace check reads the registry"
```

### Task 5: Opencode + binary wiring checks onto the core

**Files:**
- Modify: `scripts/check-opencode-wiring.sh` (plugin list, `plugin.ts` id check, `opencode.jsonc` command check, skill-copy pairs stay; enumerate plugins via `registry_plugins opencode`), `scripts/check-plugin-wiring.sh` (binary-name lists for claude-code + antigravity via registry where it currently hardcodes globs)
- Test: output diff before/after for both scripts

**Interfaces:**
- Consumes: Task 2 lib.
- Produces: identical stdout/exit on the current tree for both scripts.

- [ ] **Step 1: Capture baselines**

Run: `bash scripts/check-opencode-wiring.sh > /tmp/oc-before.txt 2>&1; echo "exit=$?"; bash scripts/check-plugin-wiring.sh > /tmp/pw-before.txt 2>&1; echo "exit=$?"`
Expected: both exit 0

- [ ] **Step 2: Rewrite enumeration over the registry**

`check-opencode-wiring.sh`: `for dir in opencode/*/` becomes `for name in $(registry_plugins opencode)` with `dir="opencode/$name/"`. `check-plugin-wiring.sh`: replace hardcoded directory globs with registry plugin lists for agent ids `claude-code` and `antigravity`. No comparison logic changes.

- [ ] **Step 3: Verify identical behavior**

Run: `bash scripts/check-opencode-wiring.sh > /tmp/oc-after.txt 2>&1; diff /tmp/oc-before.txt /tmp/oc-after.txt && echo IDENTICAL; bash scripts/check-plugin-wiring.sh > /tmp/pw-after.txt 2>&1; diff /tmp/pw-before.txt /tmp/pw-after.txt && echo IDENTICAL`
Expected: both `IDENTICAL`, both exit 0

- [ ] **Step 4: Commit**

```bash
git add scripts/check-opencode-wiring.sh scripts/check-plugin-wiring.sh
git commit -m "refactor(agents): opencode and binary wiring checks read the registry"
```

### Task 6: Generated marketplaces with CI verification

**Files:**
- Create: `scripts/gen-marketplaces.sh` (reads each registered agent with non-null `marketplace`, renders entries from per-plugin manifests + `sourceUrlBase`/`artifactSuffix`; `--check` mode exits non-zero on any diff without writing)
- Modify: `.github/workflows/ci.yml` wiring job (add a `gen-marketplaces --check` step), `Justfile` (add `gen-marketplaces` recipe)
- Test: byte-identical regeneration; negative drift test; CI step

**Interfaces:**
- Consumes: Task 1 registry; per-plugin manifests (`description`, `license`, `name`, `homepage`, `category`, `keywords`, `author`); repo URL from `Cargo.toml` (same `sed` extraction the checks use).
- Produces: `.claude-plugin/marketplace.json`, `.qwen-plugin/marketplace.json` byte-identical to committed copies.

- [ ] **Step 1: Write the generator, prove byte-identical output (red→green in one move is allowed here only because the test pre-exists: the committed files ARE the oracle)**

Write `scripts/gen-marketplaces.sh` so that for each agent with non-null `marketplace`, it emits the plugin array with fields in the exact current key order and 2-space indent (`jq` with `--sort-keys` off, explicit field construction), preserving the top-level `$schema`/`name`/`owner`/`metadata` blocks verbatim. Include `--check`: render to temp files and `cmp -s` against committed copies, failing with the differing agent name.

Run: `bash scripts/gen-marketplaces.sh && git status --porcelain -- .claude-plugin/ .qwen-plugin/`
Expected: empty output (no changes — byte-identical). If it differs, fix field order/whitespace in the generator until empty. Do NOT edit the committed files to match the generator; the committed files are the oracle.

- [ ] **Step 2: Negative test (proves generation is live, not echo)**

Run: `cp .qwen-plugin/marketplace.json /tmp/qmkt-gen-backup.json && python3 -c "import json;p='.qwen-plugin/marketplace.json';d=json.load(open(p));d['plugins'][0]['description']='STALE';json.dump(d,open(p,'w'),indent=2)" && bash scripts/gen-marketplaces.sh --check; echo "exit=$?"; cp /tmp/qmkt-gen-backup.json .qwen-plugin/marketplace.json`
Expected: non-zero exit naming the qwen marketplace; restore leaves `git status` clean

- [ ] **Step 3: Wire into Justfile and CI**

Add to `Justfile` next to the other verification recipes:

```make
# Regenerate the host marketplace manifests from the plugin manifests.
# CI runs --check: committed copies must match, so a stale copy fails
# the same way a stale footprint does.
gen-marketplaces:
    bash scripts/gen-marketplaces.sh
```

Add to the `wiring` job in `.github/workflows/ci.yml` after the marketplace checks:

```yaml
      - name: Check generated marketplaces are current
        run: bash scripts/gen-marketplaces.sh --check
```

- [ ] **Step 4: Run the full local gate for this plan**

Run: `bash scripts/lib/registry.sh --self-test && bash scripts/check-marketplace.sh > /dev/null && bash scripts/check-qwen-marketplace.sh > /dev/null && bash scripts/check-opencode-wiring.sh > /dev/null && bash scripts/check-plugin-wiring.sh > /dev/null && bash scripts/gen-marketplaces.sh --check && echo ALL_GREEN`
Expected: `ALL_GREEN`, every command exit 0

- [ ] **Step 5: Commit**

```bash
git add scripts/gen-marketplaces.sh Justfile .github/workflows/ci.yml
git commit -m "feat(agents): generate host marketplaces from plugin manifests"
```

### Task 7: Document the add/remove-agent procedure

**Files:**
- Modify: `HACK.md` (append an "Adding or removing a supported agent" section) or `README.md` repository-structure area — whichever documents contributor process; check which exists and follow it
- Test: `mlc --offline` link check if docs touched (repo's `just links` gate), plus a dry-run read-through

**Interfaces:**
- Consumes: Tasks 1–6 (the procedure must name the real files).
- Produces: contributor docs listing exact steps.

- [ ] **Step 1: Write the procedure**

Content, verbatim in spirit: to add an agent, (1) copy the template front directory, (2) add one entry to `agents.json` with `id`, `dir`, `plugins`, `marketplace` (or `null`), `manifestPattern`, `artifactSuffix`, `sourceUrlBase`, `notPublished`, (3) run `bash scripts/gen-marketplaces.sh` if the agent has a marketplace, (4) run the Task 6 gate command; every check picks the new agent up with no script edits. To remove: delete the directory, drop the registry entry, regenerate. Best-effort support means an agent nobody registers is invisible to all gates.

- [ ] **Step 2: Verify docs gate still passes**

Run: `mlc --offline --ignore-path "./node_modules,./opencode/re-ghidra-mcp-opencode/node_modules,./opencode/rtk-mcp-opencode/node_modules,./target"` (the repo's `just links` command)
Expected: exit 0, no errors

- [ ] **Step 3: Commit**

```bash
git add HACK.md
git commit -m "docs(agents): document adding and removing a supported agent"
```
