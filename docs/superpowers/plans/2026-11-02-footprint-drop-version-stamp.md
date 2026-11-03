# Drop the version stamp from footprint documents: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A release version bump no longer changes any file under `docs/footprints/`, so feature branches stop conflicting with release PRs.

**Architecture:** Remove `pluginVersion` from the `Document` the `plugin-footprint` tool writes, regenerate the 16 committed documents, and delete the release-refresh step that existed only to re-stamp them. `schemaVersion` stays 1: the gate reads baselines as untyped JSON and ignores unknown keys (`tools/plugin-footprint/src/gate.rs:198-215`, `schema_of` at `:318`).

**Tech Stack:** Rust (`tools/plugin-footprint`, `cargo nextest`), `just`, GitHub Actions, bash.

**Spec:** `docs/specs/2026-11-02-generated-file-conflicts.md` (option A; defaults in force: no `schemaVersion` bump, separate PRs for A and E, drop the `check-site.sh` version check).

All citations below were read against `origin/main` at 1c32d39's parent on 2026-11-02, except Task 4, which cites code that is not on `main` yet and is gated (see there).

## Global Constraints

- `SCHEMA_VERSION` stays `1` (`tools/plugin-footprint/src/document.rs:31`); `tests/document.rs:51` asserts it.
- Repo test gate, as CI runs it (`.github/workflows/ci.yml:64`): `cargo nextest run --workspace --no-fail-fast`. Do not add flags.
- Committed documents are regenerated only with `just footprint-regen` after the nine `just build-*` recipes listed at `.github/workflows/ci.yml:204-212` (the Plugin Footprint job). A hand-edited document fails the freshness check.
- `.gitattributes` pins `docs/footprints/*.json` to LF; do not reformat them.
- Commit trailer: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A merge-base document that still carries `pluginVersion` must stay comparable (the transition PR reads exactly that). Pinned in Task 1.
2. A version bump must leave `docs/footprints/` and every README region byte-identical. This is the goal itself; pinned in Task 2 step 4 against a real manifest.
3. A failed or `no_server` probe document is unchanged apart from the missing field (`no_server` still carries tiers). Pinned by the existing `tests/document.rs` cases staying green.
4. An in-flight release PR whose documents still carry the stamp: the next `release-please` update rebuilds the branch from `main`, so it picks up the regenerated documents. Not testable locally; checked on the first release PR after merge (Task 3 step 4).
5. Other open branches that regenerated footprints will conflict once more after this lands. Expected, one time; resolve with the regenerate procedure.

---

### Task 1: Remove the field from the tool

**Files:**
- Modify: `tools/plugin-footprint/src/document.rs` (field at `:54-55`, `build` parameter at `:138`, assignment at `:166`)
- Modify: `tools/plugin-footprint/src/main.rs` (import at `:15`, call at `:173-180`)
- Modify: `tools/plugin-footprint/src/manifest.rs` (delete `plugin_version`, `:279-301`, with its doc comment)
- Delete: `tools/plugin-footprint/tests/version_source.rs` (it tests only `plugin_version`)
- Modify: `tools/plugin-footprint/tests/document.rs` (every `build(` call, 14 sites)
- Modify: `tools/plugin-footprint/tests/gate.rs` (one new test)

**Interfaces:**
- Produces: `pub fn build(plugin: &str, plugin_dir: &Path, tree: Tree, outcome: &Outcome, files: &FileSources) -> Document`. The `plugin_version: Option<&str>` parameter is gone. `Document` has no `plugin_version` field.

**Step 0 (state check):** open the files above; confirm `build` currently takes `plugin_version: Option<&str>` as its fourth parameter, that `plugin_version` in `manifest.rs` has no other caller (`grep -rn plugin_version tools/` shows only `main.rs`, `document.rs`, `manifest.rs`, `tests/version_source.rs`), and that `tests/document.rs` has 14 `build(` calls. If any differs, STOP and report `STATE_MISMATCH: <what>`.

- [ ] **Step 1: Write the failing tests.**
  - In `tests/document.rs`, add `a_document_carries_no_version_stamp`: build with the new five-argument signature using `ok_outcome()` and `FileSources::default()`, serialise with `serde_json::to_value`, assert `value.get("pluginVersion").is_none()`.
  - In `tests/gate.rs`, add `a_baseline_that_still_carries_a_plugin_version_is_comparable`: `let mut baseline = doc("ok", 19, 18_000); baseline["pluginVersion"] = json!("0.1.0");` and `let measured = doc("ok", 19, 18_000);` then `assert_eq!(check(&measured, Some(&baseline), &budget()), Verdict::Pass);`.
- [ ] **Step 2: Run them to see the failure.** `cargo nextest run -p plugin-footprint --test document --test gate`. Expected: compile error on the `build` arity in `tests/document.rs`; the gate test passes already (that is the point: it pins existing behaviour).
- [ ] **Step 3: Implement.** Remove the field, the parameter, the assignment, the `main.rs` argument and import, the `plugin_version` function, and `tests/version_source.rs`. In every other `build(` call in `tests/document.rs` delete the fourth argument. Shape-divergence stop: if removing the field changes any other serialised key or its order, STOP and report `[original] -> [yours] because <reason>`; the oracle is the existing assertions in `tests/document.rs` and the committed documents in Task 2.
- [ ] **Step 4: Run the suite.** `cargo nextest run -p plugin-footprint --no-fail-fast`. Expected: all pass, no `version_source` test listed.
- [ ] **Step 5: Commit.** `git add -A tools/plugin-footprint && git commit -m "feat(footprint): stop stamping pluginVersion into the document"`.

### Task 2: Regenerate the committed documents

**Files:**
- Modify: the 16 `docs/footprints/*.json` except `budgets.json` (`ls docs/footprints` lists them: aiplugins-devtools x3, opencode-ctl-mcp x3, re-ghidra-mcp x3, repo-user-guide x3, rtk-mcp x3, todov2-opencode)

**Interfaces:**
- Consumes: Task 1's tool.

- [ ] **Step 1: Build the plugin binaries.** Run the nine `just build-*` recipes exactly as `.github/workflows/ci.yml:204-212` lists them.
- [ ] **Step 2: Regenerate.** `just footprint-regen`.
- [ ] **Step 3: Check the oracle.** The only change per file must be the removed key. For each changed file: `diff <(git show HEAD:docs/footprints/F | jq -S 'del(.pluginVersion)') <(jq -S . docs/footprints/F)` prints nothing. Expected: 16 files changed, `git diff --stat -- README.md claude-code opencode antigravity` empty. Any other difference: STOP and report it; do not commit.
- [ ] **Step 4: Prove the goal.** Edit `claude-code/plugins/aiplugins-devtools-cc/.claude-plugin/plugin.json` `version` to `9.9.9`, run `just footprint-regen`, expect `git diff --stat -- docs/footprints README.md claude-code/plugins/*/README.md` to show nothing new beyond the plugin.json edit's own file; then `git checkout` that manifest.
- [ ] **Step 5: Commit.** `git add docs/footprints && git commit -m "chore(footprint): regenerate documents without the version stamp"`.

### Task 3: Remove the footprint half of release-refresh

**Files:**
- Modify: `.github/workflows/release-refresh.yml` (header comment `:3-13`; the `Build plugin binaries` step; the `Regenerate derived files` step; the `git add` list in `Commit refresh`)

**Interfaces:**
- Consumes: Task 2 (documents no longer depend on version).

- [ ] **Step 1: Edit the workflow.** Keep `bash scripts/gen-marketplaces.sh` (the tag-pinned URLs still change with versions). Delete the `Setup Rust`, `Cache dependencies`, `Install just` and `Build plugin binaries` steps and `just footprint-regen`; shrink the `git add` list to the four `marketplace.json` paths. Rewrite the header so it names only the marketplace manifests. Do not change the trigger, the `if:` guard, the checkout `token`, or the commit and push commands.
- [ ] **Step 2: Parse check.** `python3 -c 'import yaml,sys; yaml.safe_load(open(".github/workflows/release-refresh.yml"))'` exits 0, and `actionlint .github/workflows/release-refresh.yml` if `actionlint` is installed (skip, and say so, if not).
- [ ] **Step 3: Commit.** `git add .github/workflows/release-refresh.yml && git commit -m "ci(release-refresh): stop regenerating footprints on the release branch"`.
- [ ] **Step 4: After merge (owner-visible, not part of the PR):** open the next release-please PR and confirm its CI is green with no `chore: refresh derived files` commit touching `docs/footprints/`. If the freshness check fails there, restore the footprint step and report.

### Task 4: Drop the stamp check from `check-site.sh` (unblocked: #118 merged as dbfa5bd)

**Files:**
- Modify: `scripts/check-site.sh` (comment at `:511`; the block at `:587-590`)
- Modify: the five fixtures `tests/site/root/docs/footprints/{alpha-mcp-agy,alpha-mcp-cc,alpha-mcp-opencode,repo-user-guide-agy,repo-user-guide-cc}.json`

**Interfaces:**
- Consumes: Task 2's new document shape (no `pluginVersion`).

**Step 0 (state check):** `sed -n 583,591p scripts/check-site.sh` must show the `if [ -f "$fpfile" ]; then` block reading `.pluginVersion` and comparing it to `$version`; `grep -c pluginVersion tests/site/root/docs/footprints/*.json` must list exactly those five files. Otherwise STOP with `STATE_MISMATCH: <what>`.

- [ ] **Step 1: Baseline.** `just site-check` exits 0 and the selftest line reads 42 cases (`check-site.sh:140` pins `case_n` to 42).
- [ ] **Step 2: Edit.** Delete the four lines `if [ -f "$fpfile" ]; then` through its `fi` (:587-590). Keep the `-v<tag>` comparison above it. Change the comment at `:511` to `# footprint-null, tier-totals, budget`. Check `$fpfile` is still used elsewhere in the function (`grep -n fpfile scripts/check-site.sh`); if it is now unused, delete its assignment too and say so.
- [ ] **Step 3: Fixtures.** For each of the five fixtures remove the `"pluginVersion":"..."` member, keeping the file otherwise byte-identical and on one line (`jq -cS 'del(.pluginVersion)'` reproduces the committed key order; compare `jq -cS .` of the old file first).
- [ ] **Step 4: Verify.** `just site-check` exits 0 with the selftest still at 42 cases and the `mutate version` case still refusing (the retained tag check catches it). `grep -rn pluginVersion scripts tests` finds nothing.
- [ ] **Step 5: Commit.** `git add scripts/check-site.sh tests/site && git commit -m "ci(site): stop comparing the removed pluginVersion stamp"`.

### Task 5: Record the change in the footprint spec

**Files:**
- Modify: `docs/specs/2026-09-03-plugin-footprint.md` (example document `:518`, and the provenance bullet `:558-562`)

- [ ] **Step 1:** Remove `"pluginVersion": "0.6.4",` from the example at `:518`. In the bullet at `:558`, replace the `pluginVersion`/`reportedVersion` agreement claim with one sentence: the version stamp was removed on 2026-11-02 (see `docs/specs/2026-11-02-generated-file-conflicts.md`) because it made every release bump rewrite the documents; no tool compared it, and `reportedVersion` was never emitted (`grep -rn reportedVersion tools scripts .github Justfile` finds nothing).
- [ ] **Step 2: Commit.** `git add docs/specs/2026-09-03-plugin-footprint.md && git commit -m "docs(footprint): note the version stamp was removed"`.

## Dispatch notes

- Task 1 is Sonnet work (a signature change with tests written first). The 14 mechanical call-site edits in `tests/document.rs` may go to Haiku. Tasks 2-3 are run-and-report plus a bounded workflow edit: Sonnet. Task 4 is Haiku-sized (two bounded edits) but run it as Sonnet because the oracle is `just site-check`; Task 5 is Haiku.
- Every implementer dispatch needs the standing clauses: Step 0 state check, shape-divergence stop, named oracle (above), complete enumerations, the exact commands above, "tests are already written, implement until they pass", the `## Anomalies noticed` clause and the `FILES` clause listing only the paths under that task.
- After Task 5: AGY-CAPSTONE on the committed range, then AGY-TEST-AUDIT, unless the owner waives (the waiver so far covered drafting the spec only).

## Self-review

- Spec coverage: option A (Tasks 1-3), fork 2 (Task 4), verification plan items 1-3 (Task 1 gate test, Task 2 steps 3-4); item 4 (capstone) in dispatch notes. Option E is a separate PR by default and is not in this plan.
- Spec correction found while planning: the spec's fork 1 risk ("the gate may deny unknown fields") is resolved by reading `gate.rs`; the baseline is parsed as `serde_json::Value`, so no bump is needed. Its `reportedVersion` row is also moot: no code emits it.
- Open item with an owner: Task 4's line-level steps wait for #118 to merge; the first release PR after merge is the live check of Task 3.
