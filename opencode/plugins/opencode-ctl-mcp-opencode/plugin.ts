import { Plugin } from "@opencode/plugin";

const POINTER =
  "opencode-ctl-mcp: use the opencode-ctl-driver skill to delegate work to a worker session; service_stop and service_restart are refused on this host.";

// Returns the pointer text the first time, then undefined: the context hook
// runs on every request and the reminder must not repeat.
export function driverPointer(sent: { value: boolean }): string | undefined {
  if (sent.value) return undefined;
  sent.value = true;
  return POINTER;
}

// Push `{ type: "text", text }` onto `event.system` (same pattern as
// re-ghidra-mcp-opencode). Never throw inside the host.
export default Plugin.define({
  id: "opencode-ctl-mcp-opencode",
  async setup(ctx) {
    const sent = { value: false };
    await ctx.session.hook("context", (event) => {
      if (Array.isArray(event.system)) {
        const text = driverPointer(sent);
        if (text) event.system.push({ type: "text", text });
      }
    });
  },
});
