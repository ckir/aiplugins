# Release-Please Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut releases per agent plugin from conventional commits via release-please, retire the lockstep scheme, and migrate install docs to pinned URLs — in a strict order that can never trigger the old pipeline with new tags.

**Architecture:** Per-crate explicit versions replace workspace inheritance; release-please (one workflow, manifest config generated from `agents.json`) opens per-package bump PRs; dist stays for binary builds scoped by the new tag glob; migration rewrites old URLs and retires the global version check. Requires Plan A Tasks 1–2 (registry) landed; Plan C SHOULD land first for opencode/agy bundlers, else their packages stay out of the release table per the spec.

**Tech Stack:** `release-please` (`googleapis/release-please-action@v4`), cargo-dist (existing pin), GitHub Actions + tag protection rules, bash, `Justfile`.

**Spec:** `docs/specs/2026-10-05-independent-plugin-versioning.md` (Release-Please Monorepo Setup, Version Sources, Release-PR Flow, Bootstrap, CI/CD, Tag Protection, Migration)

## Global Constraints

- Cutover order is load-bearing (Task 5 before Task 4's push step): the legacy tag glob MUST be retired before the first new-style tag exists.
- `latest` MUST NOT appear in any install doc afterwards (pinned `releases/download/<tag>/<file>` only).
- `cargo fmt` / `clippy -D warnings` green; `bun test` for touched opencode plugins unchanged in behavior (no TS logic changes in this plan).

---

### Task 1: Per-crate explicit versions + per-package version check

**Files:**
- Modify: 6 crates' `Cargo.toml` (`claude-code/rtk-mcp-cc`, `claude-code/re-ghidra-mcp-cc`, `qwen/rtk-mcp-qwen`, `qwen/re-ghidra-mcp-qwen`, `antigravity/rtk-mcp-agy`, `antigravity/re-ghidra-mcp-agy`): `version.workspace = true` → `version = "0.7.2"` (values unchanged, inheritance only)
- Create: `scripts/check-package-versions.sh`
- Test: new check (red→green), `cargo metadata` still resolves, full workspace builds

**Interfaces:**
- Consumes: current `0.7.2` everywhere (equality preserved; this task changes ownership, not numbers).
- Produces: rule "shippable crates carry explicit versions; `shared/*`, `tools/*`, examples keep inheritance" enforced in CI.

- [ ] **Step 1: Write the failing check (red)**

```bash
#!/usr/bin/env bash
# Per-package version agreement. Replaces check-manifest-versions.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
failures=0
check_pair() { # $1 = label, $2 = crate Cargo.toml, $3 = host manifest, $4 = jq field
    local crate_v manifest_v
    crate_v=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$2" | head -n 1)
    manifest_v=$(jq -r "$4 // \"\"" "$3" | tr -d '\r')
    if [ "$crate_v" != "$manifest_v" ]; then
        echo "  FAIL  $1: crate says $crate_v, manifest says $manifest_v" >&2
        failures=$((failures + 1))
    else
        printf '  ok       %-24s %s\n' "$1" "$crate_v"
    fi
}
for spec in \
    "rtk-mcp-cc|claude-code/rtk-mcp-cc/Cargo.toml|claude-code/rtk-mcp-cc/.claude-plugin/plugin.json|.version" \
    "re-ghidra-mcp-cc|claude-code/re-ghidra-mcp-cc/Cargo.toml|claude-code/re-ghidra-mcp-cc/.claude-plugin/plugin.json|.version" \
    "rtk-mcp-qwen|qwen/rtk-mcp-qwen/Cargo.toml|qwen/rtk-mcp-qwen/qwen-extension.json|.version" \
    "re-ghidra-mcp-qwen|qwen/re-ghidra-mcp-qwen/Cargo.toml|qwen/re-ghidra-mcp-qwen/qwen-extension.json|.version" \
    "rtk-mcp-opencode|opencode/rtk-mcp-opencode/package.json|opencode/rtk-mcp-opencode/package.json|.version" \
    "re-ghidra-mcp-opencode|opencode/re-ghidra-mcp-opencode/package.json|opencode/re-ghidra-mcp-opencode/package.json|.version" \
    "rtk-mcp-agy|antigravity/rtk-mcp-agy/Cargo.toml|antigravity/rtk-mcp-agy/plugin.json|.version" \
    "re-ghidra-mcp-agy|antigravity/re-ghidra-mcp-agy/Cargo.toml|antigravity/re-ghidra-mcp-agy/plugin.json|.version" \
; do
    IFS='|' read -r label crate manifest field <<< "$spec"
    check_pair "$label" "$crate" "$manifest" "$field"
done
# Shippable crates must NOT inherit the workspace version anymore.
for crate in claude-code/rtk-mcp-cc/Cargo.toml claude-code/re-ghidra-mcp-cc/Cargo.toml \
    qwen/rtk-mcp-qwen/Cargo.toml qwen/re-ghidra-mcp-qwen/Cargo.toml \
    antigravity/rtk-mcp-agy/Cargo.toml antigravity/re-ghidra-mcp-agy/Cargo.toml; do
    if grep -q '^version\.workspace' "$crate"; then
        echo "  FAIL  $crate still inherits the workspace version" >&2
        failures=$((failures + 1))
    fi
done
[ "$failures" -eq 0 ] && echo "All package versions agree."
exit "$failures"
```

(Note: opencode rows compare `package.json` to itself — they assert presence and form, keeping all eight packages in one table; the real opencode agreement check is crate-independent by design since opencode ships no crates.)

Run: `bash scripts/check-package-versions.sh`
Expected: FAIL listing all six crates still inheriting (red achieved, nothing else touched)

- [ ] **Step 2: Break the inheritance (values unchanged)**

In each of the six `Cargo.toml` files replace line 3 `version.workspace = true` with `version = "0.7.2"`. Touch nothing else. `shared/*`, `tools/*`, examples keep inheritance (workspace `version` stays as their internal identity).

- [ ] **Step 3: Verify green + workspace intact**

Run: `bash scripts/check-package-versions.sh`
Expected: 8 `ok` lines + `All package versions agree`, exit 0

Run: `cargo metadata --no-deps --format-version 1 > /dev/null && echo METADATA_OK && cargo build --workspace --exclude plugin-footprint 2>&1 | tail -n 2`
Expected: `METADATA_OK` and a successful build (proves the version lines are all still valid manifests; full build may take minutes — the metadata check is the fast gate, the build is the thorough one)

- [ ] **Step 4: Commit**

```bash
git add scripts/check-package-versions.sh claude-code/rtk-mcp-cc/Cargo.toml claude-code/re-ghidra-mcp-cc/Cargo.toml qwen/rtk-mcp-qwen/Cargo.toml qwen/re-ghidra-mcp-qwen/Cargo.toml antigravity/rtk-mcp-agy/Cargo.toml antigravity/re-ghidra-mcp-agy/Cargo.toml
git commit -m "feat(release): per-crate explicit versions with per-package check"
```

### Task 2: release-please config, manifest, and generator

**Files:**
- Create: `release-please-config.json`, `.release-please-manifest.json`, `scripts/gen-release-please-packages.sh`
- Modify: `.github/workflows/ci.yml` wiring job (add the config-drift check step)
- Test: drift check red→green + negative test

**Interfaces:**
- Consumes: Plan A `agents.json` (plugin list); Task 1 (per-crate versions exist to bump).
- Produces: release-please input files; `packages` table owned by the generator.

- [ ] **Step 1: Write the config with all eight packages**

`release-please-config.json` (top-level flags plus per-path entries; `changelog-path` keeps changelogs out of plugin dirs that ship in zips — wait, no: per spec, per-package `CHANGELOG.md` lives IN the package directory. release-please default writes `CHANGELOG.md` in the package path, which is exactly right; leave defaults, do not set `changelog-path`):

```json
{
  "$schema": "https://raw.githubusercontent.com/googleapis/release-please/main/schemas/config.json",
  "packages": {
    "claude-code/rtk-mcp-cc": {
      "release-type": "rust",
      "extra-files": ["claude-code/rtk-mcp-cc/.claude-plugin/plugin.json"]
    },
    "claude-code/re-ghidra-mcp-cc": {
      "release-type": "rust",
      "extra-files": ["claude-code/re-ghidra-mcp-cc/.claude-plugin/plugin.json"]
    },
    "qwen/rtk-mcp-qwen": {
      "release-type": "rust",
      "extra-files": ["qwen/rtk-mcp-qwen/qwen-extension.json"]
    },
    "qwen/re-ghidra-mcp-qwen": {
      "release-type": "rust",
      "extra-files": ["qwen/re-ghidra-mcp-qwen/qwen-extension.json"]
    },
    "opencode/rtk-mcp-opencode": {
      "release-type": "simple",
      "extra-files": ["opencode/rtk-mcp-opencode/package.json"]
    },
    "opencode/re-ghidra-mcp-opencode": {
      "release-type": "simple",
      "extra-files": ["opencode/re-ghidra-mcp-opencode/package.json"]
    },
    "antigravity/rtk-mcp-agy": {
      "release-type": "rust",
      "extra-files": ["antigravity/rtk-mcp-agy/plugin.json"]
    },
    "antigravity/re-ghidra-mcp-agy": {
      "release-type": "rust",
      "extra-files": ["antigravity/re-ghidra-mcp-agy/plugin.json"]
    }
  }
}
```

(`extra-files` entries bump the `version` key by default in JSON files. No `bootstrap-sha` is configured: Task 4's manual pre-tags are the seed, so there is nothing for release-please to bootstrap from.)

`.release-please-manifest.json`:

```json
{
  "claude-code/rtk-mcp-cc": "0.7.2",
  "claude-code/re-ghidra-mcp-cc": "0.7.2",
  "qwen/rtk-mcp-qwen": "0.7.2",
  "qwen/re-ghidra-mcp-qwen": "0.7.2",
  "opencode/rtk-mcp-opencode": "0.7.2",
  "opencode/re-ghidra-mcp-opencode": "0.7.2",
  "antigravity/rtk-mcp-agy": "0.7.2",
  "antigravity/re-ghidra-mcp-agy": "0.7.2"
}
```

`scripts/gen-release-please-packages.sh`: reads plugin dirs from `agents.json`, rewrites ONLY the top-level `packages` key of `release-please-config.json` (formatting of the rest preserved), supports `--check` (exit non-zero naming the drift). Requires Plan A Tasks 1–2 landed; if unlanded, hardcode the eight paths with a `# TODO(plan-a): read from agents.json` — no: Plan A is prerequisite, state it and stop if missing (check `[ -f agents.json ]` with a clear error naming Plan A).

- [ ] **Step 2: Drift check red→green + negative**

Run: `bash scripts/gen-release-please-packages.sh --check; echo "exit=$?"`
Expected: exit 0 on first run (generator just wrote what is committed — commit both together in Step 3, so run the script first without `--check` to normalize, then `--check` must pass)

Run: `cp release-please-config.json /tmp/rp-backup.json && python3 -c "import json;p='release-please-config.json';d=json.load(open(p));del d['packages']['qwen/rtk-mcp-qwen'];json.dump(d,open(p,'w'),indent=2)" && bash scripts/gen-release-please-packages.sh --check; echo "exit=$?"; cp /tmp/rp-backup.json release-please-config.json`
Expected: non-zero exit (missing package detected); restore leaves `git status` clean

- [ ] **Step 3: CI wiring**

Append to the `wiring` job in `.github/workflows/ci.yml`:

```yaml
      - name: Check release-please packages match the registry
        run: bash scripts/gen-release-please-packages.sh --check
```

- [ ] **Step 4: Commit**

```bash
git add release-please-config.json .release-please-manifest.json scripts/gen-release-please-packages.sh .github/workflows/ci.yml
git commit -m "feat(release): release-please manifest config generated from the registry"
```

### Task 3: The release-please workflow

**Files:**
- Create: `.github/workflows/release-please.yml`
- Test: structural key check + server-side registration + first live run observation after merge

**Interfaces:**
- Consumes: Task 2 config files.
- Produces: per-package release PRs on `main` pushes; tags + GitHub releases on PR merge.

- [ ] **Step 1: Write the workflow**

```yaml
name: Release Please
on:
  push:
    branches: [main]
  workflow_dispatch:
permissions:
  contents: write
  pull-requests: write
concurrency:
  group: release-please
  cancel-in-progress: false
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: googleapis/release-please-action@v4
        with:
          config-file: release-please-config.json
          manifest-file: .release-please-manifest.json
```

- [ ] **Step 2: Validate structure locally**

Run: `python3 -c "t=open('.github/workflows/release-please.yml').read(); assert 'concurrency:' in t and 'contents: write' in t and 'pull-requests: write' in t and 'release-please-action@v4' in t and 'release-please-config.json' in t, 'missing load-bearing keys'; print('keys ok')"`
Expected: `keys ok`

- [ ] **Step 3: Commit (first live run after merge is the functional test; observe it opens no bogus PRs — manifest versions equal tree versions, so expect release PRs only for subsequently merged conventional commits)**

```bash
git add .github/workflows/release-please.yml
git commit -m "feat(release): release-please workflow for per-package releases"
```

### Task 4: Bootstrap tagging script

**Files:**
- Create: `scripts/bootstrap-package-tags.sh` (executable)
- Test: dry-run output correctness; real push happens only in the Task 7 cutover

**Interfaces:**
- Consumes: Task 1 (all version files read `0.7.2` at the base commit), Task 2 manifest.
- Produces: eight annotated tags, pushed sequentially with per-tag verification.

- [ ] **Step 1: Write the script (dry-run by default)**

```bash
#!/usr/bin/env bash
# One-time bootstrap: tag every package <name>-v0.7.2 so release-please has a
# base. Dry-run unless --execute. Base = the v0.7.2 tag's commit.
set -euo pipefail
cd "$(dirname "$0")/.."
BASE=$(git rev-list -n 1 v0.7.2)
declare -A VERSION_FILE=(
  [rtk-mcp-cc]=claude-code/rtk-mcp-cc/.claude-plugin/plugin.json
  [re-ghidra-mcp-cc]=claude-code/re-ghidra-mcp-cc/.claude-plugin/plugin.json
  [rtk-mcp-qwen]=qwen/rtk-mcp-qwen/qwen-extension.json
  [re-ghidra-mcp-qwen]=qwen/re-ghidra-mcp-qwen/qwen-extension.json
  [rtk-mcp-opencode]=opencode/rtk-mcp-opencode/package.json
  [re-ghidra-mcp-opencode]=opencode/re-ghidra-mcp-opencode/package.json
  [rtk-mcp-agy]=antigravity/rtk-mcp-agy/plugin.json
  [re-ghidra-mcp-agy]=antigravity/re-ghidra-mcp-agy/plugin.json
)
for name in "${!VERSION_FILE[@]}"; do
  v=$(git show "$BASE:${VERSION_FILE[$name]}" | jq -r .version | tr -d '\r')
  [ "$v" = "0.7.2" ] || { echo "ABORT: $name is $v at base $BASE" >&2; exit 1; }
done
echo "base $BASE verified: all eight packages read 0.7.2"
for name in "${!VERSION_FILE[@]}"; do
  if git ls-remote origin "refs/tags/$name-v0.7.2" | grep -q .; then
    echo "exists, skipping $name-v0.7.2 (resume-safe)"
    continue
  fi
  if [ "${1:-}" = "--execute" ]; then
    git tag -a "$name-v0.7.2" "$BASE" -m "Bootstrap $name at 0.7.2"
    git push origin "$name-v0.7.2"
    git ls-remote origin "refs/tags/$name-v0.7.2" | grep -q . \
      || { echo "ABORT: tag $name-v0.7.2 did not land" >&2; exit 1; }
    echo "pushed $name-v0.7.2"
  else
    echo "would tag $name-v0.7.2 -> $BASE"
  fi
done
```

- [ ] **Step 2: Dry-run test**

Run: `bash scripts/bootstrap-package-tags.sh`
Expected: `base <sha> verified: all eight packages read 0.7.2` plus eight `would tag` lines; `git tag -l "*-v*"` still empty afterwards

- [ ] **Step 3: Commit (do NOT pass --execute here; execution belongs to the Task 7 cutover)**

```bash
git add scripts/bootstrap-package-tags.sh
git commit -m "feat(release): one-time bootstrap tagging script (dry-run)"
```

### Task 5: Dist coexistence, tag protection, run naming

**Files:**
- Modify: `.github/workflows/release.yml` (tag glob + run name)
- Test: glob match table (shell proof), server-side rule listing

**Interfaces:**
- Consumes: current trigger `tags: - '**[0-9]+.[0-9]+.[0-9]+*'`; new trigger `tags: - '*-v[0-9]*'`.
- Produces: old workflow blind to new tags and vice versa; releases attributable per package.

- [ ] **Step 1: Prove the globs partition the tag space**

Run: `for t in v0.7.2 rtk-mcp-cc-v1.2.0 re-ghidra-mcp-opencode-v0.7.2 chore-test; do case "$t" in *-v[0-9]*) new=yes;; *) new=no;; esac; echo "$t new=$new"; done`
Expected: `v0.7.2 new=no`, both package tags `new=yes`, `chore-test new=no`. (The legacy-glob side was already proven to match new-style tags via its `1.2.0` substring — that is why it is being retired, not kept.)

- [ ] **Step 2: Edit the workflow**

In `.github/workflows/release.yml`: replace the `tags:` entry with `- '*-v[0-9]*'` and set the run name to include the tag (`run-name: "Release ${{ github.ref_name }}"` at the top level of the workflow).

- [ ] **Step 3: Tag protection rule**

Run: `gh api repos/ckir/aiplugins/tags/protection -f pattern='*-v*' 2>&1 | head -n 5; gh api repos/ckir/aiplugins/tags/protection --jq '.[].pattern' 2>&1 | head -n 5`
Expected: rule listed for `*-v*` (if the API path differs on this GHES/cloud version, create it via Settings → Tags → Add rule with pattern `*-v*` restricted to the Actions identity, and record the UI path in the commit body instead)

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "feat(release): scope dist to per-package tags with run naming"
```

### Task 6: Migration edits

**Files:**
- Modify: every `INSTALL.md`/`README.md` with `releases/latest/download` (claude + qwen docs only — opencode/agy docs belong to Plan C), delete `scripts/check-manifest-versions.sh`, edit `.github/workflows/ci.yml` (drop its step) + `Justfile` (drop the `versions:` recipe), append HACK.md tag-prohibition note, add `Justfile` `doctor` recipe
- Test: grep-empty proofs + link check + recipe run

**Interfaces:**
- Consumes: Tasks 1–5 (nothing here may reference the retired global version).
- Produces: no `latest` in install docs; no global version check anywhere.

- [ ] **Step 1: Enumerate and rewrite old URLs (claude/qwen docs only)**

Run: `grep -rln "releases/latest/download" --include="*.md" claude-code qwen antigravity README.md`
Expected: the touch list (opencode/agy INSTALL overhauls are Plan C — if they appear here, leave them; Plan C owns those files)

For each listed non-Plan-C file: replace `https://github.com/ckir/aiplugins/releases/latest/download/<asset>` with `https://github.com/ckir/aiplugins/releases/download/v0.7.2/<asset>` (frozen line, tag-pinned). Re-run the grep over those files expecting empty output.

- [ ] **Step 2: Retire the global version check**

Run: `git rm scripts/check-manifest-versions.sh`
Remove its step from the `wiring` job in `.github/workflows/ci.yml` and delete the `versions:` recipe at the end of the `Justfile` (lines ~412-413: `versions:` + `bash scripts/check-manifest-versions.sh`). Verify: `grep -rn "check-manifest-versions" --include="*.yml" --include="Justfile" --include="*.sh" . | grep -v ".git/"`
Expected: empty (no references remain)

- [ ] **Step 3: Onboarding notes (tag prohibition)**

Append to `HACK.md`:

```markdown
## Tags and releases

Never push tags by hand — per-package `*-v*` tags are created only by
release-please after its bump PR merges, and tag protection enforces it.
In particular never run `git push --tags`: it pushes every local tag,
including stale or experimental ones, and each matching tag mints a real
release. Footprint gate overrides use `footprint-override: <reason>` in the
merge message (see the `footprint` recipe notes in the Justfile).
```

- [ ] **Step 4: `just doctor` recipe**

Append to `Justfile`:

```make
# Report working-copy files checked out as CRLF against an `eol=lf` pin.
# A stale Windows checkout fails byte-identity gates (wiring `cmp`, footprint
# freshness) on files whose blobs are already correct; renormalize instead of
# editing content: `git rm --cached` is NOT needed, `rm <file> && git checkout
# -- <file>` restores the pinned endings.
doctor:
    @git ls-files --eol | grep "w/crlf" | grep "eol=lf" || echo "Working copy matches all eol pins."
```

Run: `just doctor`
Expected: exit 0 (either a short drift list or the all-clear line)

Run: `mlc --offline --ignore-path "./node_modules,./opencode/re-ghidra-mcp-opencode/node_modules,./opencode/rtk-mcp-opencode/node_modules,./target" > /dev/null && echo LINKS_OK` (the repo's `just links` command, since HACK.md changed)
Expected: `LINKS_OK`

- [ ] **Step 5: Commit**

```bash
git add -A && git status --porcelain | head -n 20
git commit -m "chore(release): pin old install URLs, retire global version check, add doctor"
```
(Inspect the `add -A` list before committing — it must contain only the files from Steps 1–4.)

### Task 7: Ordered cutover checklist

**Files:** none (operations, in strict order)
- Test: each step's verification command

**Interfaces:**
- Consumes: Tasks 1–6 merged to `main`; Plan C landed for opencode/agy bundlers (else their packages stay out of the release table per the spec — if C is unlanded, remove opencode/agy entries from `release-please-config.json` and `.release-please-manifest.json` first, in a separate commit on this branch).
- Produces: live per-package releases.

- [ ] **Step 1: Confirm preconditions on clean main**

Run: `git checkout main && git pull --ff-only && git tag -l "*-v*" | head; echo "---"; gh workflow list --json name --jq '.[].name' | grep -i "release please"`
Expected: no `*-v*` tags exist yet; a "Release Please" workflow is listed and enabled

- [ ] **Step 2: Retire-then-tag order verification**

Confirm the Task 5 workflow edit is on `main` (merged) BEFORE any bootstrap tag exists: `git log --oneline -3 -- .github/workflows/release.yml && git tag -l "*-v*" | wc -l`
Expected: workflow edit present, tag count `0` — only then proceed to Step 3

- [ ] **Step 3: Execute bootstrap**

Run: `bash scripts/bootstrap-package-tags.sh --execute`
Expected: eight `pushed <name>-v0.7.2` (or `exists, skipping`, on resume) lines; abort on first failure, re-run to resume

- [ ] **Step 4: Observe the first release-please run**

Run: `gh run list --workflow release-please.yml --limit 3`
Expected: a completed run that opens no bogus release PRs (manifest versions equal tree versions everywhere, so the correct outcome is zero new PRs — any opened PR is investigated, not merged blindly)

- [ ] **Step 5: Merge the next conventional-commit PR and watch one full per-package cycle**

Expected: exactly one package's bump PR opens, merging it creates exactly one `<pkg>-v*` tag and one GitHub release with that package's dist binaries, and the host bundle workflow attaches that host's zip. Record any deviation as a follow-up task — do not hot-fix the pipeline mid-cycle.
