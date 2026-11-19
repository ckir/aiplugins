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

## Your lists

- Inside a git repository the list is under the git common directory, in `repo-tasks/` (for an ordinary
  clone, `<repo>/.git/repo-tasks/`). Git does not track that folder, so a list is never committed, pushed or
  cloned: every clone has its own.
- Outside git it is in the per-user data directory (`$XDG_DATA_HOME` or `~/.local/share` on Linux,
  `~/Library/Application Support` on macOS, `%LOCALAPPDATA%` on Windows), under `repo-tasks/<key>/`.
- To remove a list, delete its folder. Uninstalling this plugin leaves the lists in place.
- `/repo-tasks` (or `repo-tasks-qwen-mcp list` from the extension's `bin` folder) prints the list. `repo-tasks-qwen-mcp list --archived` prints the archived tasks, and `repo-tasks-qwen-mcp archive`
  moves every completed task to the archive, whatever its age.
- Windows and macOS are `not measured`: CI runs the engine's own tests there, but how this host starts the
  server and the hook on those systems has not been measured.
- "One to 50 tasks" per create call and the 2-second wait for the client's roots are proposed values and
  unmeasured.

## Layout

```
qwen-extension.json   manifest, MCP server registration and SessionStart hooks (startup, resume, clear)
QWEN.md               context file (kept minimal: it is resident in every session)
src/main.rs           thin main: all behaviour is in shared/repo-tasks
tests/e2e.rs          the built binary, driven over stdio, in throwaway git repositories
```

## The hook command is one string

Qwen Code's command hook has no `args` field: `CommandHookConfig` is `type`, `command`, `name`,
`timeout` and a few more (`packages/core/src/hooks/types.ts:91-104`, Qwen Code v0.28.0), and the
runner starts `<shell> -c <command>` (`packages/core/src/hooks/hookRunner.ts:1284-1285`). The
manifest therefore carries the arguments inside `command`
(`${extensionPath}${/}bin${/}repo-tasks-qwen-mcp reminder --event qwen`), the form the spike measured.
The `args` of an MCP server entry are a different, honoured field.

## Testing

```console
$ cargo nextest run -p repo-tasks-mcp-qwen
```
