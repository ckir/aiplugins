# repo-tasks-mcp-qwen

A Qwen Code extension that gives Qwen a task list kept per git repository: four MCP tools
(`task_create`, `task_list`, `task_get`, `task_update`) whose tasks outlive the session and are shared with
any other agent working in the same repository. A SessionStart hook adds one line saying how many tasks are
open, so a new or resumed session knows to call `task_list`; it never prints the tasks themselves.

It is the Qwen Code front of `shared/repo-tasks`; the other fronts are `repo-tasks-mcp-cc`,
`repo-tasks-mcp-agy` and `repo-tasks-mcp-opencode`.

## Install

Install once the extension is registered, or build the binary locally:

```console
$ bash scripts/install-qwen-extension.sh repo-tasks-mcp-qwen
$ just build-repo-tasks-mcp-qwen
```

## Layout

```
qwen-extension.json   manifest, MCP server registration and SessionStart hooks (startup, resume, clear)
QWEN.md               context file (kept minimal: it is resident in every session)
src/main.rs           thin main: all behaviour is in shared/repo-tasks
tests/e2e.rs          the built binary, driven over stdio, in throwaway git repositories
```

## Testing

```console
$ cargo nextest run -p repo-tasks-mcp-qwen
```
