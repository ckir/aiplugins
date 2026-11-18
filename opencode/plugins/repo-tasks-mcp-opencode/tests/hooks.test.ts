/**
 * Copyright 2026 Costas Kirgoussios
 * SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
 *
 * Source-available software. It is NOT open source.
 * Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
 * with the additional terms in LICENSE-ADDENDUM (evaluation grant,
 * anti-circumvention). See both files at the repository root.
 */

import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import plugin from "../plugin.ts";

const root = join(import.meta.dir, "..");
const LINE = "repo-tasks: 2 open tasks in .repo-tasks/tasks.json; use task_list.";

type Handler = (event: Record<string, unknown>) => void;
let dir: string;
let savedBin: string | undefined;

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), "repo-tasks-hooks-"));
  savedBin = process.env.REPO_TASKS_BIN;
});
afterEach(() => {
  if (savedBin === undefined) delete process.env.REPO_TASKS_BIN;
  else process.env.REPO_TASKS_BIN = savedBin;
  rmSync(dir, { recursive: true, force: true });
});

// A fake binary: records its argv, one per line, to argv.txt, prints `output`.
function fakeBinary(output: string): { bin: string; argv: () => string[] } {
  const bin = join(dir, "fake-repo-tasks");
  const log = join(dir, "argv.txt");
  writeFileSync(bin, `#!/bin/sh\nfor a in "$@"; do echo "$a" >> "${log}"; done\nprintf '%s' '${output}'\n`);
  chmodSync(bin, 0o755);
  process.env.REPO_TASKS_BIN = bin;
  return { bin, argv: () => (existsSync(log) ? readFileSync(log, "utf8").split("\n").filter(Boolean) : []) };
}

async function load(directory = "/work/space") {
  const handlers: Record<string, Handler> = {};
  const ctx = {
    location: { directory },
    session: {
      hook: async (name: string, fn: Handler) => {
        handlers[name] = fn;
      },
    },
  };
  await (plugin as unknown as { setup: (c: unknown) => Promise<void> }).setup(ctx);
  return handlers;
}

describe("hooks", () => {
  test("pushes_once_per_session", async () => {
    fakeBinary(LINE);
    const h = await load();
    const a1 = { sessionID: "s1", system: [] as unknown[] };
    const a2 = { sessionID: "s1", system: [] as unknown[] };
    const b1 = { sessionID: "s2", system: [] as unknown[] };
    h.context!(a1);
    h.context!(a2);
    h.context!(b1);
    expect(a1.system).toHaveLength(1);
    expect(a2.system).toHaveLength(0);
    expect(b1.system).toHaveLength(1);
  });

  test("pushes_object_form", async () => {
    fakeBinary(LINE);
    const h = await load();
    const e = { sessionID: "s1", system: [] as unknown[] };
    h.context!(e);
    expect(e.system).toEqual([{ type: "text", text: LINE }]);
  });

  test("silent_when_no_line", async () => {
    fakeBinary("");
    const h = await load();
    const e = { sessionID: "s1", system: [] as unknown[] };
    h.context!(e);
    expect(e.system).toHaveLength(0);
    const c = { system: [] as unknown[] };
    h.compaction!(c);
    expect(c.system).toHaveLength(0);
  });

  test("silent_when_system_not_array", async () => {
    fakeBinary(LINE);
    const h = await load();
    expect(() => h.context!({ sessionID: "s1" })).not.toThrow();
    expect(() => h.context!({ sessionID: "s1", system: "text" })).not.toThrow();
    expect(() => h.compaction!({})).not.toThrow();
    expect(() => h.compaction!({ system: null })).not.toThrow();
  });

  test("compaction_pushes", async () => {
    fakeBinary(LINE);
    const h = await load();
    const first = { sessionID: "s1", system: [] as unknown[] };
    h.context!(first);
    const c = { sessionID: "s1", system: [] as unknown[] };
    h.compaction!(c);
    expect(c.system).toEqual([{ type: "text", text: LINE }]);
  });

  test("never_throws_when_binary_missing", async () => {
    process.env.REPO_TASKS_BIN = join(dir, "does-not-exist");
    const h = await load();
    const e = { sessionID: "s1", system: [] as unknown[] };
    expect(() => h.context!(e)).not.toThrow();
    expect(() => h.compaction!(e)).not.toThrow();
    expect(e.system).toHaveLength(0);
  });

  test("passes_workspace_and_event_arguments", async () => {
    const f = fakeBinary(LINE);
    const h = await load("/some/work dir");
    h.context!({ sessionID: "s1", system: [] });
    expect(f.argv()).toEqual(["reminder", "--event", "opencode", "--workspace", "/some/work dir"]);
  });

  test("contains_no_list_parsing", () => {
    const src = readFileSync(join(root, "plugin.ts"), "utf8");
    expect(src).not.toContain("JSON.parse");
    expect(src).not.toMatch(/readFileSync|readFile|Bun\.file/);
    expect(src).not.toMatch(/\.json/);
  });
});

describe("opencode.jsonc", () => {
  const cfg = JSON.parse(readFileSync(join(root, "opencode.jsonc"), "utf8"));
  test("launches serve with --host opencode", () => {
    const server = cfg.mcp.servers["repo-tasks"];
    expect(server.command).toEqual(["<DEST>/bin/repo-tasks-cc-mcp", "serve", "--host", "opencode"]);
  });
});
