# rtk-mcp-qwen — RTK Command Rewriter

This extension installs a **PreToolUse hook** that intercepts `run_shell_command`
tool calls and rewrites the command via the external `rtk` CLI before execution.

## How it works

1. Qwen Code is about to execute `run_shell_command`
2. The hook binary reads the tool call JSON from stdin
3. It shells out to `rtk rewrite <command>` to get a safer/better version
4. If `rtk` produces a rewrite, the hook returns an `updated_input` with the new command
5. If `rtk` is missing, fails, or returns no output, the hook passes through (fail-open)

## Configuration

The `rtk` binary must be on your `PATH`. Override the path by setting the
`RTK_BIN` environment variable:

```json
{
  "env": {
    "RTK_BIN": "/path/to/rtk"
  }
}
```

## Hook contract

**Input** (stdin): `{ "tool_name": "run_shell_command", "tool_input": { "command": "..." } }`

**Output** (stdout): `{ "hook_specific_output": { "hook_event_name": "PreToolUse", "permission_decision": "allow", "permission_decision_reason": "...", "updated_input": { "command": "..." } } }`
