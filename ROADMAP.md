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
