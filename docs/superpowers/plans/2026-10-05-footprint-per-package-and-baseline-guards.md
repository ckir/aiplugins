# Footprint Per-Package Version & Baseline Guards Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the footprint tool read each package's own version, stamp all four documents, and add the scheduled freshness guard plus the recorded override procedure — with gate comparison logic deliberately untouched.

**Architecture:** A `plugin_version()` function moves into the `plugin_footprint` library crate reading host manifests in priority order; `main.rs` calls it instead of its private helper. A new scheduled workflow regenerates and upserts one correction branch. The override is documentation plus a merge-message token, not code. Gate delta/baseline logic is explicitly out of scope (verified unchanged by test, not by editing).

**Tech Stack:** Rust (`tools/plugin-footprint`, `cargo test` / `cargo nextest`), GitHub Actions (scheduled workflow), `Justfile`, `HACK.md`.

**Spec:** `docs/specs/2026-10-05-independent-plugin-versioning.md` (Tooling Adaptations → The Footprint Tool)

## Global Constraints

- Line endings are normalized to LF before anything is counted (existing `sources.rs` rule); new fixtures must be LF-only.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` stay green.
- No per-agent special-casing: version lookup is an ordered manifest list, not host `if/else` chains.

---

### Task 1: Per-package `plugin_version` in the library crate

**Files:**
- Modify: `tools/plugin-footprint/src/manifest.rs` (add `pub fn plugin_version`), `tools/plugin-footprint/src/main.rs:543-554` (delete private `plugin_version` + `plugin_json`, call the library function)
- Create: `tools/plugin-footprint/tests/version_source.rs`
- Test: `cargo test -p plugin-footprint --test version_source`

**Interfaces:**
- Consumes: `std::path::Path`; host manifests already in tree (`.claude-plugin/plugin.json`, `qwen-extension.json`, `package.json`, `antigravity` `plugin.json`).
- Produces: `pub fn plugin_version(plugin_dir: &Path) -> Option<String>` — first hit in order: `.claude-plugin/plugin.json`, `qwen-extension.json`, `package.json`, `antigravity/<plugin>/plugin.json` (literal filename, resolved against `plugin_dir`), each read for top-level `"version"` as string. Returns `None` only when no manifest carries a version.

- [ ] **Step 1: Write the failing test**

```rust
//! pluginVersion must come from each package's own manifest, never from a
//! global. Uses real in-tree plugins (real_plugin.rs precedent), so no
//! fixtures can drift from reality.
use plugin_footprint::manifest::plugin_version;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

#[test]
fn each_host_reports_its_own_manifest_version() {
    let root = repo_root();
    let cases = [
        ("claude-code/rtk-mcp-cc", ".claude-plugin/plugin.json"),
        ("qwen/rtk-mcp-qwen", "qwen-extension.json"),
        ("opencode/rtk-mcp-opencode", "package.json"),
        ("antigravity/rtk-mcp-agy", "plugin.json"),
    ];
    for (dir, manifest) in cases {
        let plugin = root.join(dir);
        let text = std::fs::read_to_string(plugin.join(manifest))
            .unwrap_or_else(|_| panic!("fixture manifest missing: {dir}/{manifest}"));
        let want: serde_json::Value = serde_json::from_str(&text).unwrap();
        let want = want.get("version").and_then(|v| v.as_str()).unwrap();
        assert_eq!(
            plugin_version(&plugin).as_deref(),
            Some(want),
            "pluginVersion for {dir} must equal its {manifest}"
        );
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p plugin-footprint --test version_source`
Expected: FAIL — `plugin_footprint::manifest::plugin_version` does not exist (compile error). If the opencode/antigravity cases instead return `None`, that is the second half of the failure: record it.

- [ ] **Step 3: Implement in `manifest.rs`, rewire `main.rs`**

Add to `manifest.rs`:

```rust
/// The version stamped into a footprint document. Each package owns its
/// number in its own manifest; lookup tries each candidate path in order and
/// the first manifest carrying a string `version` wins. A missing file moves
/// on to the next candidate — only the absence of every manifest yields `None`.
pub fn plugin_version(plugin_dir: &Path) -> Option<String> {
    for rel in [
        ".claude-plugin/plugin.json",
        "qwen-extension.json",
        "package.json",
        "plugin.json",
    ] {
        let Ok(text) = std::fs::read_to_string(plugin_dir.join(rel)) else {
            continue;
        };
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(s) = v.get("version").and_then(|v| v.as_str()) {
                return Some(s.to_string());
            }
        }
    }
    None
}
```

In `main.rs`: replace the call site to use `manifest::plugin_version` (add to the existing `use plugin_footprint::manifest::...` import), and delete the now-dead private `plugin_version` (line 543) and `plugin_json` (line 550) helpers. The build must have zero dead-code warnings (clippy `-D warnings` enforces it).

- [ ] **Step 4: Run tests + lints**

Run: `cargo test -p plugin-footprint --test version_source`
Expected: PASS (4/4 cases)

Run: `cargo fmt --all -- --check && cargo clippy -p plugin-footprint --all-targets --all-features -- -D warnings`
Expected: both exit 0

- [ ] **Step 5: Regenerate stamps and commit**

Run: `just footprint-regen` (needs staged plugin binaries per the `Justfile` recipes; CI builds them, locally run the four `just build-*` recipes first)
Expected: `docs/footprints/*.json` gain correct `pluginVersion` (opencode documents gain the field for the first time); `git status` shows only the four JSON files plus README regions if numbers moved (numbers must NOT move — same sources, same binaries)

Run: `cargo run -q -p plugin-footprint --bin footprint-gate -- origin/main`
Expected: all four plugins `ok`

```bash
git add tools/plugin-footprint/src/manifest.rs tools/plugin-footprint/src/main.rs tools/plugin-footprint/tests/version_source.rs docs/footprints/
git commit -m "feat(footprint): stamp pluginVersion from each package manifest"
```

### Task 2: Scheduled main-branch freshness job

**Files:**
- Create: `.github/workflows/footprint-freshness.yml`
- Test: `workflow_dispatch` dry observation + `gh workflow view`; branch-upsert logic tested by running the script body locally

**Interfaces:**
- Consumes: `just footprint-regen`, `footprint-gate -- origin/main`, `gh` CLI on the runner (`GH_TOKEN`).
- Produces: branch `chore/footprint-freshness` updated (never duplicated) + at most one open PR from it.

- [ ] **Step 1: Write the workflow**

```yaml
name: Footprint Freshness
on:
  schedule:
    - cron: "0 6 * * 1"
  workflow_dispatch:
permissions:
  contents: write
  pull-requests: write
concurrency:
  group: footprint-freshness
  cancel-in-progress: true
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - name: Early exit when measured inputs are untouched
        run: |
          if [ -z "$(git log --since='7 days' --oneline -- claude-code/ qwen/ opencode/ antigravity/ shared/ tools/plugin-footprint/ docs/footprints/)" ]; then
            echo "No measured inputs changed in 7 days; nothing to regenerate."
            echo "SKIP=true" >> "$GITHUB_ENV"
          fi
      - uses: dtolnay/rust-toolchain@stable
        if: env.SKIP != 'true'
      - uses: Swatinem/rust-cache@v2
        if: env.SKIP != 'true'
      - uses: taiki-e/install-action@just
        if: env.SKIP != 'true'
      - name: Regenerate and upsert correction branch
        if: env.SKIP != 'true'
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          just build-rtk-mcp-cc
          just build-re-ghidra-mcp-cc
          just build-rtk-mcp-opencode
          just build-re-ghidra-mcp-opencode
          just footprint-regen
          if git diff --quiet -- docs/footprints/ README.md claude-code/*/README.md opencode/*/README.md; then
            echo "Fresh. Nothing to do."
            exit 0
          fi
          git checkout -B chore/footprint-freshness
          git add docs/footprints/ README.md claude-code/*/README.md opencode/*/README.md
          git -c user.name="github-actions[bot]" -c user.email="github-actions[bot]@users.noreply.github.com" \
            commit -m "chore(footprint): regenerate stale documents (scheduled)"
          git push --force-with-lease origin chore/footprint-freshness
          if [ -z "$(gh pr list --head chore/footprint-freshness --json number --jq '.[].number')" ]; then
            gh pr create --head chore/footprint-freshness --base main \
              --title "chore(footprint): regenerate stale documents (scheduled)" \
              --body "Scheduled freshness job found main's committed footprints stale. Review the measured deltas, then merge via the recorded override procedure if the gate trips on the correction."
          fi
```

- [ ] **Step 2: Validate structure locally**

Run: `python3 -c "import sys; t=open('.github/workflows/footprint-freshness.yml').read(); assert 'concurrency:' in t and 'force-with-lease' in t and 'pull-requests: write' in t, 'workflow missing load-bearing keys'; print('keys ok')"`
Expected: `keys ok`

Run: `bash -n <(sed -n '/run: |/,/^      - /p' .github/workflows/footprint-freshness.yml | sed '1d;$d') 2>/dev/null || echo "manual review only"`
Expected: either silent pass or `manual review only` (then review the embedded script by eye against the `ci.yml` footprint job it mirrors)

- [ ] **Step 3: Commit (CI validates on merge; first scheduled run is the live test)**

```bash
git add .github/workflows/footprint-freshness.yml
git commit -m "feat(footprint): scheduled main-branch freshness job"
```

### Task 3: Recorded override procedure

**Files:**
- Modify: `Justfile` (comment block above the `footprint:` recipe), `HACK.md` (append short section; file exists at repo root)
- Test: `grep` for the token + `mlc` link check if docs touched

**Interfaces:**
- Consumes: gate failure output format (`footprint-gate: <plugin> FAILED`), branch protection reality (red checks cannot merge without admin).
- Produces: a procedure a maintainer can execute at 2am without inventing it.

- [ ] **Step 1: Add the Justfile comment**

Above the `footprint:` recipe, insert:

```make
# OVERRIDE PROCEDURE (correction PRs that trip the delta cap on stale-base
# cleanup, never for new growth): a maintainer reviews the measured delta,
# confirms it is a measurement correction (zero new source bytes vs base,
# shown by `git diff <base>...HEAD -- <plugin dir>`), then admin-merges with
# `footprint-override: <reason>` in the merge message. New growth that trips
# the cap is never overridden — shrink the change or earn the budget first.
```

- [ ] **Step 2: Append the HACK.md section**

```markdown
## Footprint gate override (stale-base corrections only)

The delta cap compares fresh measurements against the merge-base document.
If the base document itself is stale, the correction PR trips the cap with
zero new source bytes. Procedure: verify with
`git diff origin/main...HEAD -- <measured dirs>` (must be empty of measured
inputs), admin-merge with `footprint-override: <reason>` in the merge
message. Each use is greppable via `git log --grep=footprint-override`.
```

- [ ] **Step 3: Verify**

Run: `grep -c "footprint-override" Justfile HACK.md && mlc --offline --ignore-path "./node_modules,./opencode/re-ghidra-mcp-opencode/node_modules,./opencode/rtk-mcp-opencode/node_modules,./target" > /dev/null && echo DOCS_OK`
Expected: count `2`, then `DOCS_OK`

- [ ] **Step 4: Commit**

```bash
git add Justfile HACK.md
git commit -m "docs(footprint): record the stale-base override procedure"
```
