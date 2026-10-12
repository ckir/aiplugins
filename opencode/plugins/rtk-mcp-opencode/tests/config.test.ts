import { describe, test, expect } from "bun:test";
import {
  defaultConfig,
  configFromMarkdown,
  applyEnv,
  hookArgs,
  loadConfig,
  type RtkConfig,
} from "../plugin.ts";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

const get =
  (vars: Record<string, string>) =>
  (key: string): string | undefined =>
    vars[key];

describe("config", () => {
  test("defaults are enabled and use rtk on PATH", () => {
    expect(defaultConfig()).toEqual({
      enabled: true,
      rtkBin: "rtk",
      ultraCompact: false,
      skipEnv: false,
    });
  });

  test("frontmatter overrides defaults; unknown keys and bad values are ignored", () => {
    const config = configFromMarkdown(
      "---\nenabled: false\nrtk_bin: /opt/rtk/bin/rtk\nultra_compact: true\nnonsense: yes\n---\n\nprose\n"
    );
    expect(config).toEqual({
      enabled: false,
      rtkBin: "/opt/rtk/bin/rtk",
      ultraCompact: true,
      skipEnv: false,
    });
    expect(configFromMarkdown("no frontmatter here")).toEqual(defaultConfig());
    expect(configFromMarkdown("---\nenabled: banana\n---\n")).toEqual(defaultConfig());
  });

  test("env overrides file; blank RTK_BIN keeps the default", () => {
    const config = configFromMarkdown("---\nenabled: true\nrtk_bin: from-file\n---\n");
    applyEnv(config, get({ RTK_BIN: "from-env", RTK_OPENCODE_DISABLE: "1" }));
    expect(config.rtkBin).toBe("from-env");
    expect(config.enabled).toBe(false);
    const kept = defaultConfig();
    applyEnv(kept, get({ RTK_BIN: "   " }));
    expect(kept.rtkBin).toBe("rtk");
  });

  test("RTK_OPENCODE_DISABLE=0 does not disable", () => {
    const config = defaultConfig();
    applyEnv(config, get({ RTK_OPENCODE_DISABLE: "0" }));
    expect(config.enabled).toBe(true);
  });

  test("RTK_CC_DISABLE is ignored (prefix split is load-bearing)", () => {
    const config = defaultConfig();
    applyEnv(config, get({ RTK_CC_DISABLE: "1" }));
    expect(config.enabled).toBe(true);
  });

  test("hook args are bare by default and carry flags in stable order", () => {
    expect(hookArgs(defaultConfig())).toEqual(["hook", "claude"]);
    const config: RtkConfig = { ...defaultConfig(), ultraCompact: true, skipEnv: true };
    expect(hookArgs(config)).toEqual(["hook", "claude", "--ultra-compact", "--skip-env"]);
  });

  test("setup snapshot plus per-call env: file loads once, env stays live", () => {
    const dir = mkdtempSync(join(tmpdir(), "rtk-cfg-"));
    try {
      mkdirSync(join(dir, ".opencode"));
      writeFileSync(
        join(dir, ".opencode", "rtk-mcp-opencode.local.md"),
        "---\nenabled: false\nrtk_bin: /cached/rtk\n---\n"
      );
      // What setup() holds: the file layer with no env applied.
      const snapshot = loadConfig(dir, () => undefined);
      expect(snapshot.enabled).toBe(false);
      expect(snapshot.rtkBin).toBe("/cached/rtk");
      // What each hook call derives: env layers on a clone, snapshot unmutated.
      const live = applyEnv({ ...snapshot }, get({ RTK_BIN: "/env/rtk" }));
      expect(live.rtkBin).toBe("/env/rtk");
      expect(live.enabled).toBe(false);
      expect(snapshot.rtkBin).toBe("/cached/rtk");
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("loadConfig falls back to defaults when the file is missing or unreadable", () => {
    const dir = mkdtempSync(join(tmpdir(), "rtk-cfg-"));
    try {
      expect(loadConfig(dir, get({}))).toEqual(defaultConfig());
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
