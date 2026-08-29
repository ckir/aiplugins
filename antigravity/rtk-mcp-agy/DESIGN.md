# Antigravity (agy) rtk Hook Design

## Understanding Summary
- **What is being built:** A Rust-based integration (`rtk/crates/rtk-mcp-agy`) to intercept Antigravity tool calls and optimize them using `rtk rewrite`.
- **Why it exists:** To transparently rewrite shell commands through `rtk rewrite`, saving tokens by leveraging token-optimized CLI binaries.
- **Who it is for:** The Antigravity (agy) agent.
- **Key constraints:** Antigravity's `PreToolUse` hook schema lacks a way to modify tool arguments. Thus, we must use an MCP Server proxy alongside a `PreInvocation` hook injection to force the model to use the optimized tool.
- **Explicit non-goals:** Not implementing the `rtk` core logic itself, and not targeting other agents.

## Assumptions
1. The target tool to replace is `run_command`.
2. Antigravity supports configuring hooks in `~/.antigravity/config/hooks.json`.
3. The hook must execute in milliseconds to avoid latency.

## Decision Log

### 1. Hook Integration Strategy
- **Decision:** Build an MCP server (`rtk-mcp`) to expose `rtk_run`, and use a `PreInvocation` hook to inject an `ephemeralMessage` instructing the model to use it.
- **Alternatives Considered:** 
  - *PreToolUse Deny & Re-prompt:* Deny the command and return the rewritten command in the `reason`, forcing the model to re-issue the tool call. (Rejected: Adds a full round-trip token cost).
  - *PreInvocation Shell Setup:* Inject an actual `toolCall` to run shell aliases. (Rejected: Extremely fragile and breaks easily).
- **Why Chosen:** The MCP proxy approach is fast, transparent to the model's logic, and seamlessly wraps execution while maintaining reliability.

### 2. Error Handling & Fallback
- **Decision:** If `rtk rewrite` fails or yields nothing, execute the original command.
- **Alternatives Considered:** Block the command if rewrite fails.
- **Why Chosen:** `rtk rewrite` only covers specific known commands (e.g., `cat`, `grep`). We must allow all other commands (e.g., `git`, `npm`) to pass through unhindered.

### 3. Shell and rtk Executable Selection
- **Decision:** Resolve both from the environment with platform-aware defaults —
  `RTK_BIN` (default `rtk` on `PATH`) and `RTK_AGY_SHELL` (default `pwsh` on
  Windows, `sh` elsewhere).
- **Alternatives Considered:**
  - *Keep `pwsh` hardcoded:* rejected — it made `rtk_run` unable to execute
    anything on Linux or macOS, where PowerShell 7 is rarely installed. The
    failure was silent: every call returned a spawn error as a tool error.
  - *Probe for a shell at startup:* rejected — adds a launch cost and a failure
    mode of its own, to answer a question the target OS already answers.
- **Why Chosen:** Windows behaviour is unchanged, so existing installs are
  unaffected, while the other platforms get a shell that POSIX guarantees
  exists. The override covers the rest.
- **Note:** Both resolvers are pure functions in `src/lib.rs`, so the platform
  matrix is unit-tested rather than only observable on the machine that happens
  to be running the tests.

## Final Design

### Architecture & Components
1. **`rtk-mcp` Crate**: A new Rust crate in the `rtk` workspace. It acts as an MCP server over stdio and exposes a single tool: `rtk_run`. When invoked, this tool takes the command arguments, calls `rtk rewrite` internally to get the optimized command string, executes it, and returns the output to the agent.
2. **`rtk-hook-preinvocation` Crate**: A small Rust binary configured as a `PreInvocation` hook. It outputs a JSON `injectSteps` array to `stdout`.

### Data Flow & Configuration
**PreInvocation Hook Execution:**
When Antigravity starts its execution loop, the `PreInvocation` hook fires and outputs:
```json
{
  "injectSteps": [
    {
      "ephemeralMessage": "CRITICAL: Do NOT use the native `run_command` tool. You MUST use the `mcp_rtkmcp_rtk_run` tool for all shell commands to ensure token-optimized execution."
    }
  ]
}
```

**Configuration:**
In `~/.antigravity/config/hooks.json` (or workspace equivalent):
```json
{
  "rtk-agy-hook": {
    "PreInvocation": [
      {
        "type": "command",
        "command": "rtk-hook-preinvocation"
      }
    ]
  }
}
```
And the MCP server is configured in Antigravity's MCP configuration folder to run the `rtk-mcp` binary.
