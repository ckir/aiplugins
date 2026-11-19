# repo-tasks-mcp-qwen

A task list kept per git repository, shared with other agents working in it. Use `task_create` to add tasks,
`task_list` to see them, `task_get` to read one and `task_update` to change its status. Tasks outlive the
session; call `task_list` when a session start says some are open.
