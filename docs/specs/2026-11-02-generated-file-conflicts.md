# Generated-file conflicts on rebase: spec

Status: APPROVED 2026-11-02 (owner approved the recommendation: options A and E; fork 2 follows the recommendation, forks 1, 3, 4 take the stated defaults below unless the owner says otherwise). Author: Claude, solo (the owner waived AGY-FIRST for this draft). 

## 1. Problem

Four open PRs were rebased onto `main` on 2026-11-02. Three rebased cleanly. #126 conflicted, and the only conflicting files were `docs/footprints/aiplugins-devtools-{agy,cc,opencode}.json`. `main` had merged release PRs that bumped those plugins (cc 0.2.0; agy and opencode 0.3.0), and each footprint document stamps the plugin version. Resolving meant taking `main`'s file, re-measuring with `plugin-footprint measure`, re-running `publish` (which rewrites the footprint region of 17 READMEs) and continuing the rebase for every commit on the branch.

The conflict class is therefore: **a release bump changes a generated file that a feature branch also regenerated.** The cost is paid by the branch, by hand, once per release that lands while it is open.

## 2. Measured facts

| Fact | Evidence |
|---|---|
| Each footprint document embeds the plugin version | `docs/footprints/aiplugins-devtools-cc.json` carries `"pluginVersion":"0.2.0"`; field is `plugin_version: Option<String>` at `tools/plugin-footprint/src/document.rs:55`, filled from the package manifest at `tools/plugin-footprint/src/main.rs:177` |
| Release PRs rewrite that stamp | `.github/workflows/release-refresh.yml:7` says so; commit `05407de chore: refresh derived files on release branch` is the last change to the cc document |
| One consumer reads the stamp | `scripts/check-site.sh:588` (on `feat/pages-site`, PR #118; not on `main`) requires it to equal the manifest version. Test `tools/plugin-footprint/tests/version_source.rs` pins where it comes from. No other consumer found by grep over `*.rs *.sh *.yml Justfile *.md` |
| CI requires the committed documents to be fresh | `.github/workflows/ci.yml:226-246`: runs `just footprint-regen`, fails on any README or `docs/footprints/` diff |
| The gate reads the base branch's document as its baseline | `.github/workflows/ci.yml:250-253` (`footprint-gate origin/<base>`), spec `docs/specs/2026-09-03-plugin-footprint.md` section 6.2 |
| `main` requires branches to be up to date | `gh api repos/ckir/aiplugins/branches/main/protection`: `strict: true`, 7 required contexts |
| No merge queue support exists today | no `merge_group` trigger anywhere in `.github/` |
| Probe-ok documents carry a second version field | the spec's example document (`2026-09-03-plugin-footprint.md:523`) has `reportedVersion`; `grep -l reportedVersion docs/footprints/*.json` matches no committed document, so only `pluginVersion` needs removing today |

## 3. Goals and non-goals

Goals: a release bump never conflicts with a feature branch's footprint files; keep the freshness layer and the base-threshold ratchet intact; no new always-on infrastructure.

Non-goals: removing the committed documents (they are the ratchet baseline, so this is unsound, see option C); changing branch protection beyond what an option needs; changing how footprints are measured.

## 4. Options

**A. Remove version stamps from the footprint document.** A footprint is a claim about bytes, not releases. Drop `pluginVersion` (`reportedVersion` appears in no committed document), bump `schemaVersion`, drop the `check-site.sh:588` comparison (the manifest version is already checked against the marketplace elsewhere), delete the version-refresh half of `release-refresh.yml`. Release bumps then no longer touch `docs/footprints/`.
- Sound: freshness still regenerates and diffs; the gate compares bytes only.
- Risks: (1) RESOLVED while planning: the gate parses baselines as `serde_json::Value` and ignores unknown keys (`tools/plugin-footprint/src/gate.rs:198-215`, `:318`), so no `schemaVersion` bump is needed; (2) any README footprint region that prints a version (grep of `publish.rs` found none, not exhaustively read).
- Residual: a branch that genuinely changes a plugin's bytes still conflicts with another branch doing the same. That is a real conflict, not churn.

**B. GitHub merge queue, drop `strict`.** Add `merge_group` to every required workflow. This removes the manual "update branch" step only. It does NOT fix this problem: the queue builds a merge commit, and a textual conflict in a footprint file ejects the PR from the queue. Worth doing for other reasons, but not a fix.

**C. Stop committing generated footprints; regenerate in CI.** Unsound. The merge-base document is the ratchet baseline (section 6.2); without a committed baseline a change could raise the ceiling it is judged against, which is the property section 6.2 exists to protect.

**D. Merge driver or conflict-resolving bot.** A `.gitattributes` merge driver needs per-clone `git config` and never runs on GitHub-side merges. A bot cannot help because GitHub does not run `pull_request` workflows on a PR that has merge conflicts (documented GitHub behaviour; confirm before relying on this). Not recommended.

**E. Commit the rebase helper.** Add `just rebase-regen`: resolve footprint and README conflicts to the incoming side, run `footprint-regen`, `git add`, `git rebase --continue`, looping per commit. Turns the half-hour of manual work into one command. Does not remove the conflicts; it is the fallback for A's residual and for any future generated file.

## 5. Recommendation and forks for the owner

Recommend **A, then E**. A removes the conflict at the source; E covers what A cannot. B is a separate, optional improvement.

Forks:
1. Approve A? If yes, is a `schemaVersion` bump acceptable, or should the field simply stop being written while old readers still accept it (no bump)?
2. Keep or drop the `check-site.sh:588` version check? Recommendation: drop it; the site check already compares manifest to marketplace.
3. E: land with A in one PR, or separately?
4. B: do you want it tracked as its own ROADMAP item?

Defaults in force: fork 1 = stop writing the field with no `schemaVersion` bump if the verification in section 6 step 1 shows old readers accept the change, otherwise bump; fork 3 = separate PRs (A first, then E); fork 4 = not tracked unless asked.

## 6. Verification plan (for the build, not for this draft)

1. Confirm the gate and `ratchet` accept a document with and without the removed fields (add a test with an old-shape fixture as merge base).
2. After A: bump a plugin version on a scratch branch and confirm `git diff --stat -- docs/footprints README.md` is empty and `just footprint-regen` produces no diff.
3. For E: rebuild the 2026-11-02 situation on a scratch branch (feature commit that regenerates a footprint, then a version bump on `main`) and confirm the recipe finishes the rebase with CI-equivalent `just footprint` green.
4. Capstone and test audit per the usual gates before declaring complete.
