import { describe, test, expect } from "bun:test";
import { defaultConfig, delegate, runRtk, type RtkConfig } from "../plugin.ts";

const EVENT = '{"tool_name":"Bash","tool_input":{"command":"cat README.md"}}';
const REWRITE =
  '{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"rtk read README.md"}}}';

describe("delegate", () => {
  test("forwards rtk JSON verbatim and hands rtk the payload byte-for-byte", () => {
    let seen = "";
    const out = delegate(EVENT, defaultConfig(), (args, payload) => {
      expect(args).toEqual(["hook", "claude"]);
      seen = payload;
      return REWRITE;
    });
    expect(out).toBe(REWRITE);
    expect(seen).toBe(EVENT);
  });

  test("disabled config never spawns rtk", () => {
    const config: RtkConfig = { ...defaultConfig(), enabled: false };
    const out = delegate(EVENT, config, () => {
      throw new Error("rtk must not be spawned when disabled");
    });
    expect(out).toBe("");
  });

  test("empty payload is a no-op without spawning", () => {
    const out = delegate("   \n", defaultConfig(), () => {
      throw new Error("rtk must not be spawned for an empty payload");
    });
    expect(out).toBe("");
  });

  test("missing rtk fails open", () => {
    expect(delegate(EVENT, defaultConfig(), () => null)).toBe("");
  });

  test("empty rtk output is a no-op (rtk's own nothing-to-rewrite signal)", () => {
    expect(delegate(EVENT, defaultConfig(), () => "")).toBe("");
    expect(delegate(EVENT, defaultConfig(), () => "  \n")).toBe("");
  });

  test("non-JSON rtk output is suppressed", () => {
    const out = delegate(EVENT, defaultConfig(), () => "rtk: something went sideways\n");
    expect(out).toBe("");
  });

  test("runRtk returns null for a binary that does not exist (no spawn, no throw)", () => {
    expect(runRtk("definitely-not-a-real-binary-xyz", ["hook", "claude"], EVENT)).toBeNull();
  });
});
