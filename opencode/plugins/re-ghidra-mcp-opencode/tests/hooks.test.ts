import { describe, test, expect } from "bun:test";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import plugin from "../plugin.ts";
import { compactionReminder, needsDriverPointer } from "../plugin.ts";
import {
  preflight,
  overlayEnv,
  parseJavaMajor,
  settingsFromMarkdown,
  preflightFindings,
  preflightReport,
  type Probe,
  type ProbeFn,
} from "../plugin.ts";

const healthyProbe: Probe = { launcherPresent: true, javaMajor: 21 };

function healthyProjectDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "re-ghidra-opencode-"));
  mkdirSync(join(dir, ".opencode"), { recursive: true });
  writeFileSync(
    join(dir, ".opencode", "re-ghidra-mcp-opencode.local.md"),
    "---\nghidra_install_dir: /opt/ghidra\nproject_dir: /re\nproject_name: p\nbootstrap_program: add.exe\n---\n",
  );
  return dir;
}

function fakeCtx(directory: string = healthyProjectDir()) {
  const hooks: Record<string, Array<(event: unknown) => void>> = {};
  return {
    hooks,
    ctx: {
      location: { directory },
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

type SetupFn = { setup: (ctx: unknown, p?: ProbeFn) => Promise<void> };

type SystemEntry = { type: string; text: string };

function toolEntry() {
  return { description: "test tool", input: {} };
}

describe("setup", () => {
  test("registers exactly compaction and context session hooks, nothing else", async () => {
    const { hooks, ctx } = fakeCtx();
    await (plugin as unknown as SetupFn).setup(ctx, () => healthyProbe);
    expect(Object.keys(hooks).sort()).toEqual(["session:compaction", "session:context"]);
  });

  test("compaction handler injects the RE-state reminder", async () => {
    const { hooks, ctx } = fakeCtx();
    await (plugin as unknown as SetupFn).setup(ctx, () => healthyProbe);
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
    await (plugin as unknown as SetupFn).setup(ctx, () => healthyProbe);
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
    await (plugin as unknown as SetupFn).setup(ctx, () => healthyProbe);
    const event = {};
    expect(() => {
      for (const fn of hooks["session:compaction"]) fn(event);
    }).not.toThrow();
    expect(event).toEqual({});
  });

  test("context handler tolerates a malformed event (no fields)", async () => {
    const { hooks, ctx } = fakeCtx();
    await (plugin as unknown as SetupFn).setup(ctx, () => healthyProbe);
    const event = {};
    expect(() => {
      for (const fn of hooks["session:context"]) fn(event);
    }).not.toThrow();
    expect(event).toEqual({});
  });

  test("setup appends exactly one preflight report on the first context event", async () => {
    const { hooks, ctx } = fakeCtx(mkdtempSync(join(tmpdir(), "re-ghidra-broken-")));
    await (plugin as unknown as SetupFn).setup(ctx, () => ({ launcherPresent: false, javaMajor: null }));
    const calls = hooks["session:context"]!;
    const mk = () => ({ system: [] as Array<{ type: string; text: string }>, tools: {} });
    const e1 = mk();
    await calls[0]!(e1);
    expect(e1.system).toHaveLength(1);
    expect(e1.system[0]!.text).toContain("re-ghidra-mcp-opencode is not ready");
    expect(e1.system[0]!.text).toContain("No `java` on PATH"); // forced by the probe, env-independent
    const e2 = mk();
    await calls[0]!(e2);
    expect(e2.system).toHaveLength(0); // pushed once, then silent
  });
});

describe("preflight", () => {
  test("settingsFromMarkdown reads the documented keys", () => {
    const s = settingsFromMarkdown(
      "\uFEFF---\r\nproject_dir: C:\\re\r\nproject_name: \"demo\"\r\nunknown_key: x\r\n---\r\nbody",
    );
    expect(s.project_dir).toBe("C:\\re");
    expect(s.project_name).toBe("demo");
    expect((s as Record<string, unknown>).unknown_key).toBeUndefined();
  });

  test("settingsFromMarkdown without frontmatter returns {}", () => {
    expect(settingsFromMarkdown("# just a file")).toEqual({});
    expect(settingsFromMarkdown("---\nno closing fence")).toEqual({});
    expect(settingsFromMarkdown("---\nproject_dir:\n---\n")).toEqual({}); // blank = unset
  });

  test("overlayEnv overrides and treats empty as unset", () => {
    const cfg = overlayEnv(
      { project_dir: "file-dir" },
      { GHIDRA_MCP_PROJECT_DIR: "env-dir", GHIDRA_MCP_PROJECT_NAME: "" },
    );
    expect(cfg.project_dir).toBe("env-dir");
    expect(cfg.project_name).toBeUndefined(); // absent or empty -> falls back to file
    expect(cfg.bootstrap_program).toBeUndefined();
  });

  test("parseJavaMajor handles modern, legacy and junk output", () => {
    expect(parseJavaMajor('openjdk version "21.0.4" 2024-07-16')).toBe(21);
    expect(parseJavaMajor('java version "1.8.0_402"')).toBe(8);
    expect(parseJavaMajor('garbage')).toBeNull();
    expect(parseJavaMajor("")).toBeNull();
  });

  test("preflight mirrors the Rust finding matrix", () => {
    const good = { launcherPresent: true, javaMajor: 21 } satisfies Probe;
    const complete = {
      ghidra_install_dir: "/opt/ghidra", project_dir: "/re",
      project_name: "p", bootstrap_program: "add.exe",
    };
    expect(preflight(complete, good)).toEqual([]);
    expect(preflight({}, { launcherPresent: false, javaMajor: null }).length).toBe(3);
    // Rust test matrix (`install_dir_set_but_not_a_ghidra_root_is_reported`):
    // the finding fires only when the probe says the launcher is absent.
    expect(preflight({ ghidra_install_dir: "/bad" }, { launcherPresent: false, javaMajor: 21 })[0])
      .toContain("not a Ghidra install root");
    expect(preflight(complete, { launcherPresent: true, javaMajor: 17 })[0])
      .toContain("version 17");
    const missing = preflight(
      { ghidra_install_dir: "/opt/ghidra" }, good,
    );
    expect(missing).toHaveLength(1);
    expect(missing[0]).toContain("Unset: project_dir, project_name, bootstrap_program");
    expect(missing[0]).toContain(".opencode/re-ghidra-mcp-opencode.local.md");
  });

  test("preflightFindings: healthy project reports nothing", () => {
    expect(preflightFindings(healthyProjectDir(), {}, () => healthyProbe)).toEqual([]);
  });

  test("preflightFindings: missing file yields the project-values finding only", () => {
    const dir = mkdtempSync(join(tmpdir(), "re-ghidra-empty-"));
    const f = preflightFindings(
      dir, { GHIDRA_INSTALL_DIR: "/opt/ghidra" }, () => healthyProbe,
    );
    expect(f).toHaveLength(1);
    expect(f[0]).toContain("Unset: project_dir, project_name, bootstrap_program");
  });

  test("preflightFindings fails open on any throw", () => {
    expect(preflightFindings("/nonexistent", {}, () => {
      throw new Error("probe exploded");
    })).toEqual([]);
  });

  test("preflightReport renders a header plus bulleted findings", () => {
    expect(preflightReport(["one", "two"])).toBe(
      "re-ghidra-mcp-opencode is not ready to attach to a Ghidra project:\n" +
      "  - one\n" +
      "  - two",
    );
  });
});
