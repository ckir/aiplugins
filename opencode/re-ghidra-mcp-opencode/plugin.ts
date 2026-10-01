import { Plugin } from "@opencode/plugin";

export function compactionReminder(): string {
  return [
    "## Ghidra RE state (preserve across compaction)",
    "- Attached program and analysis freshness: which binary is loaded, whether it is analyzed.",
    "- Pending writes: renames, comments, datatype/prototype/local changes not yet saved to the project.",
    "- Current investigation target: function or address under analysis and the question being answered.",
  ].join("\n");
}

export function needsDriverPointer(toolNames: string[]): boolean {
  // OpenCode registers MCP tools with the server name as prefix
  // (https://opencode.ai/docs/mcp-servers: "registered with server name as
  // prefix", e.g. `mymcpservername_*`; agent globs like `my-mcp*` match
  // `my-mcp_search`). Our server is named `ghidra` (opencode.jsonc
  // `mcp.servers.ghidra`), so live tool keys are `ghidra_<tool>` (e.g.
  // `ghidra_attach_program` — bare names come from rmcp `#[tool_router]`
  // method names in shared/ghidra-mcp/src/server.rs). Exact-prefix matching
  // is therefore correct here; there is no `mcp__ghidra__*` namespacing in
  // OpenCode (that is Claude Code's form). Installed
  // @opencode/plugin@2.0.21 types are silent on naming (SessionContext.tools
  // is just Record<string, ...>), so the docs above are the authority.
  return toolNames.some((name) => name.startsWith("ghidra"));
}

export function driverPointer(): string {
  return "Ghidra tools are available: follow the ghidra-re-driver skill for navigate/decompile/search before falling back to raw tool calls.";
}

// Step 1 deviation, verified against the installed types
// (node_modules/@opencode/plugin/dist/promise/session.d.ts @2.0.21,
// SystemPart in node_modules/@opencode/ai/dist/schema/messages.d.ts @2.0.21):
// SessionCompaction and SessionContext carry `system: SystemPart[]` and
// `tools: Record<string, ...>` — there is no `context: string[]`, and tools
// are not a string array. Both handlers therefore push
// `{ type: "text", text }` parts onto `event.system`, and the context handler
// derives tool names via `Object.keys(event.tools)`.
export default Plugin.define({
  id: "re-ghidra-mcp-opencode",
  async setup(ctx) {
    await ctx.session.hook("compaction", (event) => {
      // Shape drift must never throw inside the host (spec: never a session
      // block): only push when `system` is actually an array.
      if (Array.isArray(event.system)) {
        event.system.push({ type: "text", text: compactionReminder() });
      }
    });
    await ctx.session.hook("context", (event) => {
      // `tools` may be absent on API drift; default to no tools (silent).
      if (Array.isArray(event.system) && needsDriverPointer(Object.keys(event.tools ?? {}))) {
        event.system.push({ type: "text", text: driverPointer() });
      }
    });
  },
});
