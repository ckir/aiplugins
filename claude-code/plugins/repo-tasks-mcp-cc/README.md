# repo-tasks-mcp-cc

A Claude Code plugin that gives Claude a task list kept per git repository: four MCP tools
(`task_create`, `task_list`, `task_get`, `task_update`) whose tasks outlive the session and are shared with
any other agent working in the same repository. A SessionStart hook adds one line saying how many tasks are
open, so a new or compacted session knows to call `task_list`; it never prints the tasks themselves.

It is the Claude Code front of `shared/repo-tasks`; the other fronts are `repo-tasks-mcp-qwen`,
`repo-tasks-mcp-agy` and `repo-tasks-mcp-opencode`.

## Install

Install from the marketplace once it is registered, or build the binary locally:

```console
$ just build-repo-tasks-mcp-cc
```

## Layout

```
.claude-plugin/plugin.json   manifest
.mcp.json                    registers `repo-tasks-cc-mcp serve`
hooks/hooks.json             SessionStart (startup, resume, clear, compact) runs `repo-tasks-cc-mcp reminder --event claude-code`
src/bin/mcp.rs               thin main: all behaviour is in shared/repo-tasks
tests/e2e.rs                 the built binary, driven over stdio, in throwaway git repositories
```

## Testing

```console
$ cargo nextest run -p repo-tasks-mcp-cc
```
