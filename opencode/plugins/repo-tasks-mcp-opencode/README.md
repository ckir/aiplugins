# repo-tasks-mcp-opencode

An OpenCode V2 plugin that gives an OpenCode session the `repo-tasks` list: the tasks of a repository, kept in the
repository and served over MCP (`task_create`, `task_list`, `task_get` and the update tool).

It is the OpenCode front of `shared/repo-tasks`; the other fronts are `repo-tasks-mcp-cc`, `repo-tasks-mcp-qwen` and
`repo-tasks-mcp-agy`. This front is a source-only bundle and ships no binary of its own: it uses the Claude Code
wrapper's `repo-tasks-cc-mcp`, started as `serve --host opencode`.

`plugin.ts` runs `repo-tasks-cc-mcp reminder --event opencode --workspace <directory>` on the first `context` event of
each session id and again on `compaction`, and pushes the line it prints onto `event.system` as
`{ type: "text", text }`. When the binary prints nothing, is missing or fails, nothing is pushed. The plugin never
reads or parses the list; the binary alone does.

Tested with `@opencode/plugin` **2.0.32**. The V2 plugin API is beta; re-test on bump.

## Installing

Follow `INSTALL.md` (written for an agent to execute).

## Settings

`REPO_TASKS_BIN` names the binary the reminder runs. Unset, the plugin uses `<config dir>/bin/repo-tasks-cc-mcp`.

## Layout

```
plugin.ts          context and compaction hooks: the reminder line
opencode.jsonc     MCP server entry (serve --host opencode)
tests/             bun tests
```

## Testing

```console
$ cd opencode/plugins/repo-tasks-mcp-opencode && bun test tests/
```
