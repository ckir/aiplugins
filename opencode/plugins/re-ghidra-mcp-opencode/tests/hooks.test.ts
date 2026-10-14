import { describe, test, expect } from "bun:test";
import plugin from "../plugin.ts";
import { compactionReminder, needsDriverPointer } from "../plugin.ts";

function fakeCtx() {
  const hooks: Record<string, Array<(event: unknown) => void>> = {};
  return {
    hooks,
    ctx: {
      tool: {
        hook: async (name: string, fn: (event: unknown) => void) => {
          (hooks[`tool:${name}`] ??= []).push(fn);
        },
      },
      session: {
        hook: async (name: string, fn: (event: unknown) => void) => {
          (hooks[`session:${name}`] ??= []).push(fn);
        },
      },
    },
  };
}

type SystemEntry = { type: string; text: string };

function toolEntry() {
  return { description: "test tool", input: {} };
}

describe("setup", () => {
  test("registers exactly compaction and context session hooks, nothing else", async () => {
    const { hooks, ctx } = fakeCtx();
    await (plugin as unknown as { setup: (ctx: unknown) => Promise<void> }).setup(ctx);
    expect(Object.keys(hooks).sort()).toEqual(["session:compaction", "session:context"]);
  });

  test("compaction handler injects the RE-state reminder", async () => {
    const { hooks, ctx } = fakeCtx();
    await (plugin as unknown as { setup: (ctx: unknown) => Promise<void> }).setup(ctx);
    // Shape per @opencode/plugin@2.0.21 dist/promise/session.d.ts:
    // SessionCompaction carries system: SystemPart[], not context: string[].
    const event = { system: [] as SystemEntry[] };
    for (const fn of hooks["session:compaction"]) fn(event);
    expect(event.system).toHaveLength(1);
    expect(event.system[0].text).toContain("Ghidra");
    expect(event.system[0].text).toBe(compactionReminder());
  });

  test("context handler stays silent without Ghidra tools, points at the driver with them", async () => {
    const { hooks, ctx } = fakeCtx();
    await (plugin as unknown as { setup: (ctx: unknown) => Promise<void> }).setup(ctx);
    // Shape per @opencode/plugin@2.0.21 dist/promise/session.d.ts:
    // SessionContext.tools is a Record keyed by tool name, not a string[].
    const idle = { system: [] as SystemEntry[], tools: { bash: toolEntry(), read: toolEntry() } };
    for (const fn of hooks["session:context"]) fn(idle);
    expect(idle.system).toHaveLength(0);
    const active = {
      system: [] as SystemEntry[],
      tools: { bash: toolEntry(), ghidra_decompile: toolEntry() },
    };
    for (const fn of hooks["session:context"]) fn(active);
    expect(active.system).toHaveLength(1);
    expect(active.system[0].text).toContain("ghidra-re-driver");
  });

  test("needsDriverPointer matches any ghidra-prefixed tool", () => {
    expect(needsDriverPointer(["bash", "read"])).toBe(false);
    expect(needsDriverPointer(["ghidra_decompile"])).toBe(true);
    expect(needsDriverPointer([])).toBe(false);
  });

  test("needsDriverPointer matches live OpenCode-registered ghidra_* names", () => {
    // OpenCode prefixes MCP tools with the server name (`ghidra` here), so
    // these are the exact keys SessionContext.tools carries live.
    expect(needsDriverPointer(["bash", "ghidra_attach_program"])).toBe(true);
    expect(needsDriverPointer(["ghidra_list_project_programs", "ghidra_rename"])).toBe(true);
  });

  test("needsDriverPointer rejects non-prefixed and Claude-style namespaced names", () => {
    // `mcp__ghidra__*` is Claude Code's namespacing; it never occurs in
    // OpenCode (docs: `<server>_<tool>`), so it must not match here.
    expect(needsDriverPointer(["mcp__ghidra__attach_program"])).toBe(false);
    expect(needsDriverPointer(["notghidra_foo", "myghidra_attach"])).toBe(false);
  });

  test("compaction handler tolerates a malformed event (no fields)", async () => {
    const { hooks, ctx } = fakeCtx();
    await (plugin as unknown as { setup: (ctx: unknown) => Promise<void> }).setup(ctx);
    const event = {};
    expect(() => {
      for (const fn of hooks["session:compaction"]) fn(event);
    }).not.toThrow();
    expect(event).toEqual({});
  });

  test("context handler tolerates a malformed event (no fields)", async () => {
    const { hooks, ctx } = fakeCtx();
    await (plugin as unknown as { setup: (ctx: unknown) => Promise<void> }).setup(ctx);
    const event = {};
    expect(() => {
      for (const fn of hooks["session:context"]) fn(event);
    }).not.toThrow();
    expect(event).toEqual({});
  });
});
