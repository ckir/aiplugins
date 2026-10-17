---
name: gain
description: Reports rtk's token savings for this project or all projects, and the optimization opportunities being missed. Use when the user asks how many tokens rtk has saved, what rtk savings look like, whether rtk is actually working, or which commands could be optimized but are not.
argument-hint: "[history] [discover] [project|all]"
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

1. **Summary.** Call the `rtk_gain` tool from this plugin's `rtkmcp` MCP server. Pass `history: true` when the user asked for it, and `project_only: true` unless they asked for `all`.
   If that tool is unavailable, fall back to running `rtk gain` through the shell.

2. **Missed savings.** Only when the user asked for `discover`, call `rtk_discover`. Pass `all_projects: true` for `all`.

3. **Report.** Lead with the headline number — tokens saved — in one sentence. Then, if `history` was requested, a table of the top commands by savings.
