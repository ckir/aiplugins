import { describe, test, expect } from "bun:test";
import {
  defaultConfig,
  configFromMarkdown,
  applyEnv,
  hookArgs,
  loadConfig,
  loadFileConfig,
  clearConfigCache,
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

  test("loadConfig reads the project file once, then serves it from cache", () => {
    const dir = mkdtempSync(join(tmpdir(), "rtk-cfg-"));
    try {
      mkdirSync(join(dir, ".opencode"));
      writeFileSync(
        join(dir, ".opencode", "rtk-mcp-opencode.local.md"),
        "---\nenabled: false\nrtk_bin: /cached/rtk\n---\n"
      );
      clearConfigCache();
      const first = loadConfig(dir, get({}));
      expect(first.enabled).toBe(false);
      expect(first.rtkBin).toBe("/cached/rtk");
      // Second call hits the mtime cache: equal value, fresh object.
      const second = loadConfig(dir, get({}));
      expect(second).toEqual(first);
      // Env still layers per call on top of the cached file.
      const over = loadConfig(dir, get({ RTK_BIN: "/env/rtk" }));
      expect(over.rtkBin).toBe("/env/rtk");
      expect(over.enabled).toBe(false);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("loadConfig falls back to defaults when the file is missing or unreadable", () => {
    const dir = mkdtempSync(join(tmpdir(), "rtk-cfg-"));
    try {
      clearConfigCache();
      expect(loadFileConfig(dir)).toEqual(defaultConfig());
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
