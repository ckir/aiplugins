import { describe, test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import plugin, { driverPointer } from "../plugin.ts";

const root = join(import.meta.dir, "..");

describe("driverPointer", () => {
  test("fires once, then stays silent", () => {
    const sent = { value: false };
    const first = driverPointer(sent);
    expect(first).toBe(
      "opencode-ctl-mcp: use the opencode-ctl-driver skill to delegate work to a worker session; service_stop and service_restart are refused on this host."
    );
    expect(sent.value).toBe(true);
    expect(driverPointer(sent)).toBeUndefined();
    expect(driverPointer(sent)).toBeUndefined();
  });
});

describe("plugin", () => {
  test("id matches the directory", () => {
    expect((plugin as unknown as { id: string }).id).toBe("opencode-ctl-mcp-opencode");
    const src = readFileSync(join(root, "plugin.ts"), "utf8");
    expect(src).toContain('\n  id: "opencode-ctl-mcp-opencode",');
  });

  test("context hook pushes the pointer once onto event.system", async () => {
    let handler: ((event: { system?: unknown[] }) => void) | undefined;
    const ctx = {
      session: {
        hook: async (name: string, fn: typeof handler) => {
          if (name === "context") handler = fn;
        },
      },
    };
    await (plugin as unknown as { setup: (c: unknown) => Promise<void> }).setup(ctx);
    expect(handler).toBeDefined();
    const event = { system: [] as unknown[] };
    handler!(event);
    handler!(event);
    expect(event.system).toHaveLength(1);
    expect((event.system[0] as { type: string }).type).toBe("text");
    handler!({});
  });
});

describe("opencode.jsonc", () => {
  const cfg = JSON.parse(readFileSync(join(root, "opencode.jsonc"), "utf8"));
  const server = cfg.mcp.servers["opencode-ctl"];

  test("launches serve with --host opencode and sets OPENCODE_CTL_HOST", () => {
    expect(server.command).toEqual(["./bin/opencode-ctl-cc-mcp", "serve", "--host", "opencode"]);
    expect(server.environment).toEqual({ OPENCODE_CTL_HOST: "opencode" });
    expect(server.timeout).toEqual({ catalog: 30000, execution: 330000 });
  });
});
