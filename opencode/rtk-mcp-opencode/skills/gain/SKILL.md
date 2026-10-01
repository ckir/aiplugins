---
name: gain
description: Reports rtk's token savings for this project or all projects, and the optimization opportunities being missed. Use when the user runs /rtk-mcp-cc:gain or asks how many tokens rtk has saved, what rtk savings look like, whether rtk is actually working, or which commands could be optimized but are not.
argument-hint: "[history] [discover] [project|all]"
allowed-tools: Bash
---

# Report rtk token savings

Show what rtk has saved, and what it is still leaving on the table.

## Arguments

`$ARGUMENTS` may contain, in any order:

- `history` — include the recent per-command breakdown, not just the summary.
- `discover` — also report missed optimization opportunities.
- `project` — restrict to the current project. **This is the default.**
- `all` — cover every project instead of just this one.

## Steps

1. **Summary.** Call the `rtk_gain` tool from this plugin's `rtk-mcp-cc` MCP
   server. Pass `history: true` when the user asked for it, and
   `project_only: true` unless they asked for `all`.

   If that tool is unavailable — the plugin's binaries have not been built —
   say so, tell the user to run `just build-rtk-mcp-cc`, and fall back to
   running `rtk gain` through `Bash` so the request is still answered.

2. **Missed savings.** Only when the user asked for `discover`, call
   `rtk_discover`. Pass `all_projects: true` for `all`.

   Skip this step otherwise. It scans session history and is much slower than
   the summary; running it unasked turns a quick question into a long wait.

3. **Report.** Lead with the headline number — tokens saved — in one sentence.
   Then, if `history` was requested, a table of the top commands by savings:
   command, invocations, tokens saved.

4. **Missed opportunities**, when gathered: list the commands rtk could have
   optimized, highest-value first, and name the tool each should become
   (`grep` → `rg`, `find` → `fd`). Keep it to the top handful; the full dump is
   rarely what the user wants.

5. **Do not change anything.** This skill reports. If the user wants to act on
   a missed opportunity, that is a separate request.

## Reporting notes

- If rtk is not installed, the tool call fails with a message saying so. Report
  that plainly — "rtk is not on PATH, so there are no statistics to show" — and
  do not present zero savings as though it were a measurement.
- Zero or near-zero savings on a fresh install is normal, not a fault. Say so
  rather than implying something is misconfigured.
- Savings figures are rtk's own accounting, not a measured benchmark of this
  session. Report them as what rtk claims, and do not extrapolate them into
  wall-clock or cost figures the user did not ask for.
- When the user asks whether rtk is "working", the honest check is
  `rtk_check` on a concrete command — it shows the rewrite directly. A savings
  total is weaker evidence, since it accumulates across sessions.
