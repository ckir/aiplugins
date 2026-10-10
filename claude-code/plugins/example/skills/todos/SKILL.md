---
name: todos
description: Lists the TODO/FIXME/HACK markers in the current project, optionally filtered to unowned ones. Use when the user runs /claude-example:todos or asks what markers, TODOs, or deferred work exist in the codebase.
argument-hint: "[unowned] [path]"
allowed-tools: Read, Glob, Grep
---

# List code markers

Report the TODO/FIXME/HACK markers in this project.

## Arguments

`$ARGUMENTS` may contain, in any order:

- `unowned` — restrict the report to markers with no `(owner)`.
- a path — scan only that directory. Defaults to the project root.

## Steps

1. **Scan.** Call the `scan_todos` tool from this plugin's `claude-example` MCP
   server. Pass `path` when the user gave one, and `unowned_only: true` when the
   user asked for unowned markers.

   If that tool is unavailable — the plugin's binaries have not been built —
   say so, tell the user to run `just build-claude-example`, and fall back to
   `Grep` for `TODO|FIXME|HACK` so the request is still answered.

2. **Group** the results by file, in descending order of marker count. Within a
   file, keep line order.

3. **Report** as a table: file, line, kind, owner (`—` when absent), note.

4. **Summarise** in one line: total markers, how many are unowned, and how many
   files they span.

5. **Do not edit anything.** This skill reports; it does not fix. If the user
   wants markers triaged into a plan of action, hand off to the `todo-triager`
   agent. If they want the conventions themselves, that is the
   `todo-conventions` skill.

## Reporting notes

- Lead with unowned markers when the user asked for them — that is the finding.
- When the scan is empty, say so plainly in one line rather than printing an
  empty table.
- When results hit the configured `max_results` cap, say the list is truncated
  and name the cap; a silently short list reads as good news.
