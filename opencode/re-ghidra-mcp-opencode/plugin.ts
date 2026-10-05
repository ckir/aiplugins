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
  // Live keys are `<server>_<tool>` (opencode.jsonc `mcp.servers.ghidra`),
  // so exact-prefix match; `mcp__ghidra__*` is Claude Code's form only.
  return toolNames.some((name) => name.startsWith("ghidra"));
}

export function driverPointer(): string {
  return "Ghidra tools are available: follow the ghidra-re-driver skill for navigate/decompile/search before falling back to raw tool calls.";
}

// No `context: string[]` on SessionContext (@opencode/plugin@2.0.21):
// push `{ type: "text", text }` onto `event.system`; tool names via
// `Object.keys(event.tools)`.
export default Plugin.define({
  id: "re-ghidra-mcp-opencode",
  async setup(ctx) {
    await ctx.session.hook("compaction", (event) => {
      // Never throw inside the host: push only when `system` is an array.
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
