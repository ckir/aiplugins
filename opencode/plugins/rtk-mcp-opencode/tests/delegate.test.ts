import { describe, test, expect } from "bun:test";
import { applyVerdict, defaultConfig, delegate, runRtk, type RtkConfig } from "../plugin.ts";

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

  test("a throwing run fails open (returns empty, no throw)", () => {
    expect(
      delegate(EVENT, defaultConfig(), () => {
        throw new Error("boom");
      })
    ).toBe("");
  });
});

describe("applyVerdict", () => {
  test("happy-path rewrite is applied onto the input", () => {
    const input: Record<string, unknown> = { command: "cat README.md" };
    applyVerdict(input, "command", REWRITE);
    expect(input.command).toBe("rtk read README.md");
  });

  test("same-command verdict leaves the input unchanged", () => {
    const input: Record<string, unknown> = { command: "rtk read README.md" };
    const verdict = JSON.stringify({
      hookSpecificOutput: { updatedInput: { command: "rtk read README.md" } },
    });
    applyVerdict(input, "command", verdict);
    expect(input).toEqual({ command: "rtk read README.md" });
  });

  test("empty rewritten command is ignored", () => {
    const input: Record<string, unknown> = { command: "cat README.md" };
    const verdict = JSON.stringify({ hookSpecificOutput: { updatedInput: { command: "" } } });
    applyVerdict(input, "command", verdict);
    expect(input).toEqual({ command: "cat README.md" });
  });

  test("missing hookSpecificOutput/updatedInput leaves the input unchanged", () => {
    const bare: Record<string, unknown> = { command: "cat README.md" };
    applyVerdict(bare, "command", JSON.stringify({ hookSpecificOutput: {} }));
    expect(bare).toEqual({ command: "cat README.md" });
    const empty: Record<string, unknown> = { command: "cat README.md" };
    applyVerdict(empty, "command", JSON.stringify({}));
    expect(empty).toEqual({ command: "cat README.md" });
  });

  test("non-JSON verdict is ignored", () => {
    const input: Record<string, unknown> = { command: "cat README.md" };
    applyVerdict(input, "command", "this is not json{{{");
    expect(input).toEqual({ command: "cat README.md" });
  });
});
