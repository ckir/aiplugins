# Merge flow: stop PRs queueing behind each other

Status: DECIDED 2026-11-03. Drafted solo on 2026-11-02 (the owner waived AGY-FIRST for the draft); the forks were then consulted with agy (AGY-FIRST, 2026-11-03) and the owner ruled in section 5. Built: M2 and the `concurrency` block. Not built: M1 (unavailable), M3 (not pursued), M4 (dropped).

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
| The `check:` recipe is a single dependency line | `Justfile:356` on `main` at 3c57661; #118 and `main` each edited it |
| The repository is owned by a User account | `gh api repos/ckir/aiplugins --jq .owner.type` prints `User`; GitHub's "Managing a merge queue" page: "Pull request merge queues are available in any public repository owned by an organization, or in private repositories owned by organizations using GitHub Enterprise Cloud" |
| Auto-merge is on and merged branches are deleted | `gh api repos/ckir/aiplugins --jq '{auto:.allow_auto_merge,del:.delete_branch_on_merge}'` prints `true`, `true` |
| The repo merges with merge commits | `git log --merges origin/main` shows "Merge pull request #N" |

## 3. Goals and non-goals

Goals: merging N green PRs costs about one CI run each and no manual updating; a cancelled runner never needs a human; hand-edited append-only files stop producing trivial conflicts.

Non-goals: moving the ledgers (the plugin owns their path, so a per-entry layout is unsound here); changing which checks are required (an observation, not a ruling: `Plugin Footprint`, `OpenCode TS Tests` and the bundle smokes run on every PR but are not required); the footprint version-stamp conflict (spec 2026-11-02-generated-file-conflicts).

## 4. Design

**M1. Merge queue on `main`. UNAVAILABLE: not buildable on a User-owned repository (see the measured facts); the options that remain are listed in section 5.** Configure it as a repository ruleset (Settings, Rules; the classic branch-protection API has no merge-queue field, so this is an owner action I cannot script). Add `merge_group:` to the `on:` block of `ci.yml`, the only workflow that supplies required contexts. Method: merge commit, matching history. With the queue on, "require branches to be up to date" is dropped: the queue builds each entry on top of the queue ahead of it. Cost: each entry still runs the full required set once; concurrent entries share nothing.
- Limits: a textual conflict ejects the entry, so M1 does not cure conflicts, only the update-and-wait loop. `release-refresh.yml` and `release.yml` trigger on `pull_request` and are not required, so they need no `merge_group` trigger.

**M2. Bounded automatic rerun of runner-acquisition failures. BUILT.** `.github/workflows/ci-rerun-runner-failures.yml` is a `workflow_run` workflow on `CI` (completed, conclusion failure or cancelled, run attempt 1 only) that calls `scripts/rerun-runner-failures.sh`. The script re-runs the failed jobs only when EVERY failed or cancelled job in the run carries the annotation "not acquired by Runner"; one real red job means nothing is re-run, so a flaky test cannot be retried to green by this. Permissions `actions: write`, `checks: read`, `contents: read`. One retry; a second failure stays red for a human. It runs from the default branch, checks out only that branch, and takes only the numeric run id from the event, through `env`. This removes the 25-28 minute stall plus manual rerun seen on #127 and #128. Companion change in `ci.yml`: a `concurrency` group per pull request with `cancel-in-progress: true`, so a superseded push stops holding runners; non-PR runs get a group of their own (`github.run_id`), so a run on `main` is never cancelled. `timeout-minutes` was NOT added: it counts from job start, and the failure here is a job that never started (reasoned, not measured).

**M3. Union-merge the ledgers. NOT PURSUED (owner ruling 2026-11-03).** Add `docs/agy-capstone-ledger.md merge=union` and `docs/agy-test-audit-ledger.md merge=union` to `.gitattributes`. Both files are append-only tables, so keeping both sides is always correct, and `union` is built into git (no per-clone config) so local rebases honour it. UNVERIFIED: whether GitHub's server-side merge and `update-branch` honour it; test with a throwaway pair of PRs before relying on it.

**M4. One entry per line in `check`. DROPPED (owner ruling 2026-11-03; agy and the author both recommended skipping).** Replace the long dependency line at `Justfile:356` with a recipe body of one `just <name>` per line, so two branches adding different gates stop editing the same line. Cost: gates then run sequentially and `just` no longer dedupes them; confirm no gate is also a dependency of another recipe before changing.

## 5. Decisions

| Fork | Ruling (2026-11-03) |
|---|---|
| 1. Merge queue (M1) | Skipped. Not available on a User-owned repository. Alternatives named in the consult and NOT built: transfer the repository to a free organization (moves the Pages URL `ckir.github.io/aiplugins` and every `ckir/aiplugins` URL in the marketplaces and installers, relying on GitHub redirects); a workflow that updates the next auto-merge PR after each merge (would need `RELEASE_PLEASE_TOKEN`, since pushes made with the default token start no CI; unmeasured); a staging branch (breaks the merge-commit history and per-PR required checks); job-level path filters (a workflow-level filter would leave the required contexts pending on filtered PRs). |
| 2. Auto-rerun (M2) | Build, plus the `concurrency` block. Built in this change. |
| 3. `merge=union` trial (M3) | Skipped. |
| 4. One gate per line (M4) | Dropped. |

Open consequence of skipping 1: merging N green PRs still costs one update and one CI run per PR. M2 removes the manual rerun, not the update-and-wait loop.

## 6. Verification

1. M2 decision logic: `bash scripts/rerun-runner-failures.sh --selftest` (also `just rerun-runner-failures-test`, in `check`, and a step in `ci.yml`) runs six scenarios against a stubbed `gh`. Logic mutants of the script each turn their named scenario red: any-failed instead of all-failed, a wrong marker, dropping `cancelled` from the job filter, swallowing the annotation fetch error, and re-running an empty run.
2. M2 wiring: `actionlint` is clean on the new workflow. The `workflow_run` trigger cannot be exercised before it is on the default branch; confirm on the next real "not acquired by Runner" cancellation, or by watching the first `CI rerun on runner acquisition failure` run after merge (expected: `leave` for a run that failed for a real reason).
3. `concurrency`: push twice in quick succession to a throwaway PR branch and confirm the first run is cancelled and the second completes; confirm a push to `main` is not cancelled by a following push.
