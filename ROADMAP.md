# Roadmap

Tracked work that is not in a task list yet. Entry format: the heading is the title only; the next line is `**Status:** <TOKEN> <YYYY-MM-DD> - <one line>` (OPEN, SCHEDULED, IMPLEMENTED, SHIPPED with a commit sha, CLOSED-BY-RULING, KILLED). Triaged anomalies from `.clavity/local-anomalies.md` are promoted here when they have no other home.

## opencode-ctl-mcp: execute the implementation plan

**Status:** SHIPPED 2026-11-18 - merged as PR #103 (9f315e5); the plan was executed, and `opencode-ctl-mcp-*` v0.2.x is released. The open items below are tracked separately.

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

**Status:** SHIPPED 2026-11-18 - `feat/aiplugins-devtools` merged as PR #80 (260d49c) and `aiplugins-devtools-*` v0.5.0 is released. The repo holds no record that the capstone and test-audit named here ran for that branch; whether they were waived is the owner's to say.

## devtools: pin LF on the shared skill references and notices

**Status:** SHIPPED 2026-11-18 - merged as PR #150 (e1d10bb); `.gitattributes` now pins the whole devtools skill tree (shared and the cc, qwen, agy and opencode copies) to LF, so `references/`, `THIRD-PARTY-NOTICE.md` and `assets/` are covered; `git check-attr eol` reports `lf` on all 50 files. Windows autocrlf checkout itself not exercised.

## opencode devtools INSTALL.md: copy steps hardcode skill and command names

**Status:** SHIPPED 2026-11-18 - merged as PR #150 (e1d10bb); INSTALL.md copies by loop over the bundle (`"$STAGE"/skills/*/`, `"$STAGE"/commands/*.md`) and names no skill; `scripts/check-devtools-skills.sh` fails if it names a shipped skill or command or loses the globs (four new mutation cases).

## Qwen installers: no release carries the extension zip

**Status:** SHIPPED 2026-11-18 - merged as PR #150 (e1d10bb); both installers resolve the newest `<extension>-v*` release through the releases API and download from that tag; `aiplugins-devtools-qwen` added to the `.ps1` `ValidateSet` and both usage lists. Measured live with bash and `pwsh` 7 on Linux for all four extensions; Windows PowerShell 5.1 not run.

## mlc scans untracked scratch and fails local link checks

**Status:** SHIPPED 2026-11-18 - merged as PR #150 (e1d10bb); the Justfile already ignored `./.clavity`; the lefthook pre-push `mlc` line now does too.

## release-refresh: pass github.head_ref through env, not inline

**Status:** SHIPPED 2026-11-18 - merged as PR #150 (e1d10bb); the push step reads `HEAD_REF` from `env:` and quotes it; actionlint clean.

## release-please: a release PR goes stale behind main and is not rebuilt

**Status:** OPEN 2026-11-03 - release-please re-pushes its branch only when the PR body changes (v17.6.0 `src/manifest.ts:1094-1100`), so a release PR stays based on an old main and merges conflict with feature branches that touched the same generated files; a conflicted PR gets no `pull_request` workflows, so `release-refresh` cannot repair it. Measured on #129 (2026-11-02): fixed by `gh pr update-branch`, which needs the owner's say-so before a release merge. The footprint stamp removal (#133) removed the main conflict source; the merge-flow spec `docs/specs/2026-11-02-merge-flow.md` (M2/M3) covers the rest. Decide whether the leftover cases warrant automation there.

## repo-tasks-mcp: overlap of todov2-opencode with OpenCode's built-in todowrite

**Status:** OPEN 2026-11-18 - OpenCode 2.3.0 ships a built-in `todowrite` again as an opt-in (`{ "experimental": { "todo": true } }`, `packages/core/src/tool/plugin/todo.ts`, `CHANGELOG.md:1244` at the installed tag; spec `docs/superpowers/specs/2026-11-18-repo-tasks-design.md` 15.4), under the same tool name `opencode/plugins/todov2-opencode` registers. The repo-tasks work leaves the plugin untouched (spec 12); the owner rules whether to keep it, make it yield when the built-in is on, or retire it.
