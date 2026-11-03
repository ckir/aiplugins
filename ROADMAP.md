# Roadmap

Tracked work that is not in a task list yet. Entry format: the heading is the title only; the next line is `**Status:** <TOKEN> <YYYY-MM-DD> - <one line>` (OPEN, SCHEDULED, IMPLEMENTED, SHIPPED with a commit sha, CLOSED-BY-RULING, KILLED). Triaged anomalies from `.clavity/local-anomalies.md` are promoted here when they have no other home.

## opencode-ctl-mcp: execute the implementation plan

**Status:** SCHEDULED 2026-10-19 - plan `docs/superpowers/plans/2026-10-18-opencode-ctl-mcp.md` (branch `feat/opencode-ctl-mcp`, 18 panel rounds, not GREEN) waits on the owner's round-19-or-ship call and an execution method (subagent-driven recommended).

Spec: `docs/specs/2026-10-18-opencode-ctl-mcp-design.md`. After execution comes the AGY-CAPSTONE review of the committed code, then AGY-TEST-AUDIT.

## opencode-ctl-mcp: six open items for the owner

**Status:** OPEN 2026-10-19 - plan section "Open items for the owner" (plan shape, baked wrapper password, relative command in `opencode.jsonc`, `serve` surviving a log failure, tools the owner may cut, spec corrections); item 6 now also declares rule (0) precedence, the `IO_ERROR` spawn arm and `UNKNOWN_OPERATION` for a snapshot-only id.

## opencode-ctl-mcp: patch the spec's measured corrections

**Status:** OPEN 2026-10-19 - the spec still says the CLI prints `ECONNREFUSED`, names `SyntheticConflictError` as a wire tag, and describes message `parts`; the plan builds to the v2.0.32 behaviour (plan decisions 19-23). One-line cited edits, or leave the plan as the record.

## opencode-ctl-mcp: spec tension on a released parked prompt

**Status:** OPEN 2026-10-19 - spec 486-492 says the first idle marker after the anchor's stored time is the anchor's own turn "in every case", yet also that a failed busy turn lets a successor run the queued item "with its own later marker"; a wait whose parked item is released mid-slice can return `failed` for the earlier turn. Needs a live reproduction (Task 25 or the capstone) and an owner reading.

## opencode-ctl-mcp: does the Windows job object kill an auto-started service

**Status:** OPEN 2026-10-19 - POSIX escapes through `detached: true` (`service-contender.ts:19-24`); the Windows job-inheritance outcome is unmeasured. Measured in plan Task 25 Step 4, together with the other UNVERIFIED Windows and macOS items listed in the plan's Stand-downs (`cmd.exe /c` argument handling, `powershell.exe` 5.1 and the BOM-less `.ps1`, `wt -d "C:"`, macOS `date -j`).

## opencode-ctl-mcp: npm `opencode.cmd` shim cannot be launched by the wrapper

**Status:** OPEN 2026-10-19 - product gap recorded in the plan's Stand-downs (round 13): a Windows install through npm exposes `opencode.cmd`, which the launcher's binary resolution does not handle.

## aiplugins-devtools: capstone and test-audit

**Status:** OPEN 2026-10-19 - branch `feat/aiplugins-devtools` has all tasks done; the AGY-CAPSTONE review, then AGY-TEST-AUDIT, then the merge decision remain, to run in a fresh session (owner ruling).

## devtools: pin LF on the shared skill references and notices

**Status:** OPEN 2026-11-02 - `.gitattributes` pins `**/SKILL.md` and `shared/devtools/commands/*.md` but not `shared/devtools/skills/**/references/*.md` or `THIRD-PARTY-NOTICE.md`, so a Windows autocrlf checkout fails `scripts/check-devtools-skills.sh` (the `diff -r` at line 112 against the LF-pinned `opencode/**/*.md` copies). Pin every copy together (shared, cc, qwen, agy); pinning shared alone breaks the per-agent `diff -r`. CI runs on ubuntu only, so it is unseen. Verified 2026-11-02 with `git check-attr eol`.

## opencode devtools INSTALL.md: copy steps hardcode skill and command names

**Status:** OPEN 2026-11-02 - `opencode/plugins/aiplugins-devtools-opencode/INSTALL.md:70` lists the skills and the command by name with no drift check, so adding one to `shared/devtools` leaves the install steps incomplete while every gate stays green. Needs a gate that derives the list, or a loop over `skills/*`.

## Qwen installers: no release carries the extension zip

**Status:** OPEN 2026-11-02 - `scripts/install-qwen-extension.sh:37` downloads `releases/latest/download/<ext>-extension.zip`, but the repo Latest release (`aiplugins-devtools-agy-v0.3.0`) has only `aiplugins-devtools-agy-agy-plugin.zip`, so the documented one-liner 404s. `scripts/install-qwen-extension.ps1:24,34` also omits `aiplugins-devtools-qwen` from its `ValidateSet`. Per-plugin tags mean `latest` cannot work; resolve the tag per extension.

## mlc scans untracked scratch and fails local link checks

**Status:** OPEN 2026-11-02 - `just links` and the lefthook pre-push `mlc` (Justfile:51, lefthook.yml:22) do not ignore the git-ignored `.clavity/`, so `.clavity/scratch/opencode-ctl-plan-panel/round11-agy.md` and `round18-agy.md` (66 `file://` hits in round11 alone) fail the check locally. Add `./.clavity` to the `--ignore-path` list in both places.

## release-refresh: pass github.head_ref through env, not inline

**Status:** OPEN 2026-11-03 - `.github/workflows/release-refresh.yml:58` runs `git push origin HEAD:${{ github.head_ref }}` inline, which actionlint flags as potentially untrusted input. The job `if:` guard pins `head_ref` to `release-please--branches--main`, so it is likely unexploitable today, but the flagged pattern is one guard edit away from real. Pass it through an `env:` variable and quote it.

## release-please: a release PR goes stale behind main and is not rebuilt

**Status:** OPEN 2026-11-03 - release-please re-pushes its branch only when the PR body changes (v17.6.0 `src/manifest.ts:1094-1100`), so a release PR stays based on an old main and merges conflict with feature branches that touched the same generated files; a conflicted PR gets no `pull_request` workflows, so `release-refresh` cannot repair it. Measured on #129 (2026-11-02): fixed by `gh pr update-branch`, which needs the owner's say-so before a release merge. The footprint stamp removal (#133) removed the main conflict source; the merge-flow spec `docs/specs/2026-11-02-merge-flow.md` (M2/M3) covers the rest. Decide whether the leftover cases warrant automation there.
