# Merge flow: stop PRs queueing behind each other

Status: DRAFT 2026-11-02. Author: Claude, solo (the owner waived AGY-FIRST for this draft). Owner decides the forks in section 5.

## 1. Problem

On 2026-11-02 four PRs (#118, #126, #127, #128) had to land. Every merge left the next one BEHIND `main`, so each needed an update, a fresh CI run of about 27 checks, and in two cases a manual rerun. #118 additionally conflicted on three hand-edited files. Elapsed time was dominated by waiting and rerunning, not by the changes.

## 2. Measured facts

| Fact | Evidence |
|---|---|
| `main` requires branches to be up to date, with 7 required contexts, no required reviews, admins enforced | `gh api repos/ckir/aiplugins/branches/main/protection`: `strict: true`; contexts `Format & Lint`, `Test (ubuntu-latest)`, `Test (windows-latest)`, `Test (macos-latest)`, `Plugin Wiring`, `Dependency Audit`, `Spell Check`; `reviews: null`; `enforce_admins: true` |
| All 7 required contexts come from one workflow | job names at `.github/workflows/ci.yml:17,43,89,366,376`; its triggers are `push` to `main` and `pull_request` (`ci.yml:4-8`); no workflow has a `merge_group` trigger |
| macOS cancellations were runner capacity, not our config | run 41495365746 attempt 1: `Test (macos-latest)` ran 17:53-18:18 and was cancelled; check-run annotation: "The job was not acquired by Runner of type hosted even after multiple attempts", plus the notice "Due to capacity constraints, jobs targeting macOS arm64 runners may experience longer queue times" |
| `ci.yml` has no `concurrency` block and no `timeout-minutes` | `grep -n 'concurrency\|timeout-minutes' .github/workflows/ci.yml` finds nothing; matrices use `fail-fast: false` (`:48,:70,:267,:298`) |
| The two ledgers are hardcoded in the clavity plugin, which gates on them | `agy-capstone/SKILL.md:393`, `agy-test-audit/SKILL.md:30,380`, hooks `agy-ledger-lib.sh` and `agy-test-audit-reminder.sh:117` read and append `docs/agy-capstone-ledger.md` and `docs/agy-test-audit-ledger.md` by exact path and sha |
| The `check:` recipe is a single dependency line | `Justfile:342` on `feat/pages-site` after the rebase (line 334 on `main`); #118 and `main` each edited it |
| The repo merges with merge commits | `git log --merges origin/main` shows "Merge pull request #N" |

## 3. Goals and non-goals

Goals: merging N green PRs costs about one CI run each and no manual updating; a cancelled runner never needs a human; hand-edited append-only files stop producing trivial conflicts.

Non-goals: moving the ledgers (the plugin owns their path, so a per-entry layout is unsound here); changing which checks are required (an observation, not a ruling: `Plugin Footprint`, `OpenCode TS Tests` and the bundle smokes run on every PR but are not required); the footprint version-stamp conflict (spec 2026-11-02-generated-file-conflicts).

## 4. Design

**M1. Merge queue on `main`.** Configure it as a repository ruleset (Settings, Rules; the classic branch-protection API has no merge-queue field, so this is an owner action I cannot script). Add `merge_group:` to the `on:` block of `ci.yml`, the only workflow that supplies required contexts. Method: merge commit, matching history. With the queue on, "require branches to be up to date" is dropped: the queue builds each entry on top of the queue ahead of it. Cost: each entry still runs the full required set once; concurrent entries share nothing.
- Limits: a textual conflict ejects the entry, so M1 does not cure conflicts, only the update-and-wait loop. `release-refresh.yml` and `release.yml` trigger on `pull_request` and are not required, so they need no `merge_group` trigger.

**M2. Bounded automatic rerun of runner-acquisition failures.** A `workflow_run` workflow on `CI` (completed, conclusion failure or cancelled) that, for run attempt 1 only, reruns failed jobs when a failed job's annotation contains "not acquired by Runner". Permissions `actions: write` only. One retry; a second failure stays red for a human. This removes the 25-28 minute stall plus manual rerun seen on #127 and #128.

**M3. Union-merge the ledgers.** Add `docs/agy-capstone-ledger.md merge=union` and `docs/agy-test-audit-ledger.md merge=union` to `.gitattributes`. Both files are append-only tables, so keeping both sides is always correct, and `union` is built into git (no per-clone config) so local rebases honour it. UNVERIFIED: whether GitHub's server-side merge and `update-branch` honour it; test with a throwaway pair of PRs before relying on it.

**M4. One entry per line in `check`.** Replace the long dependency line at `Justfile:342` with a recipe body of one `just <name>` per line, so two branches adding different gates stop editing the same line. Cost: gates then run sequentially and `just` no longer dedupes them; confirm no gate is also a dependency of another recipe before changing.

## 5. Forks for the owner

1. Enable the merge queue (M1)? Recommended yes. It is a settings change only you can make; I will supply the exact ruleset values.
2. Build M2, or accept manual reruns? Recommended build; roughly 30 lines of workflow. Alternative: none, GitHub's macOS capacity is outside our control.
3. Try M3 on a throwaway PR pair first, then keep or drop it?
4. M4 (a `Justfile` change that serialises gates): yes, or skip as low value?

## 6. Verification plan (for the build)

1. M1: after enabling, open two trivial PRs, queue both, confirm both merge with one CI run each and no update step; confirm a `merge_group` run appears for `ci.yml` with all 7 contexts reporting.
2. M2: not reproducible on demand. Verify by reading the workflow against a saved cancelled run (41495365746 attempt 1 annotation text) with `act` or a dry-run step that only echoes the decision; then watch the next real occurrence.
3. M3: two PRs each appending a different row to the same ledger; merge one, then run `gh pr update-branch` on the other. Pass: no conflict and both rows present. Fail: drop M3.
4. M4: `just --dry-run check` lists the same gates before and after.
5. Capstone and test audit per the usual gates before declaring complete.
