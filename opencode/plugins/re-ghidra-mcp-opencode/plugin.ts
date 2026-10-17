import { Plugin } from "@opencode/plugin";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

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

// ── Preflight ──────────────────────────────────────────────────────────
// A pure mirror of claude-code/plugins/re-ghidra-mcp-cc/src/lib.rs::preflight:
// same settings keys, same env overlay, same thresholds, same finding texts
// (with the OpenCode settings-file name). Silent when healthy; every path
// fails open so a broken preflight can never block a session.

export const MIN_JAVA_MAJOR = 21;
export const SETTINGS_REL_PATH = join(".opencode", "re-ghidra-mcp-opencode.local.md");

export interface RawSettings {
  ghidra_install_dir?: string;
  project_dir?: string;
  project_name?: string;
  bootstrap_program?: string;
  bootstrap_program_path?: string;
  max_heap?: string;
}

const SETTING_KEYS = [
  "ghidra_install_dir", "project_dir", "project_name",
  "bootstrap_program", "bootstrap_program_path", "max_heap",
] as const;

const ENV_KEYS: Array<[keyof RawSettings, string]> = [
  ["ghidra_install_dir", "GHIDRA_INSTALL_DIR"],
  ["project_dir", "GHIDRA_MCP_PROJECT_DIR"],
  ["project_name", "GHIDRA_MCP_PROJECT_NAME"],
  ["bootstrap_program", "GHIDRA_MCP_BOOTSTRAP_PROGRAM"],
  ["bootstrap_program_path", "GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH"],
  ["max_heap", "GHIDRA_MCP_MAX_HEAP"],
];

export function settingsFromMarkdown(source: string): RawSettings {
  const out: RawSettings = {};
  const body = source.startsWith("\uFEFF") ? source.slice(1) : source;
  if (!body.startsWith("---")) return out;
  const rest = body.slice(3).replace(/^\r/, "");
  if (!rest.startsWith("\n")) return out;
  const end = rest.indexOf("\n---");
  if (end < 0) return out;
  for (const rawLine of rest.slice(1, end).split("\n")) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const colon = line.indexOf(":");
    if (colon < 0) continue;
    const key = line.slice(0, colon).trim();
    let value = line.slice(colon + 1).trim();
    if (value.startsWith('"') || value.startsWith("'")) value = value.slice(1);
    if (value.endsWith('"') || value.endsWith("'")) value = value.slice(0, -1);
    if (!value) continue; // blank = unset, same as the Rust side
    if ((SETTING_KEYS as readonly string[]).includes(key)) {
      (out as Record<string, string>)[key] = value;
    }
  }
  return out;
}

export function overlayEnv(
  cfg: RawSettings,
  env: Record<string, string | undefined> = process.env,
): RawSettings {
  const out: RawSettings = { ...cfg };
  for (const [slot, key] of ENV_KEYS) {
    const v = env[key];
    if (v) (out as Record<string, string>)[slot] = v; // empty counts as unset
  }
  return out;
}

export function parseJavaMajor(versionOutput: string): number | null {
  const quoted = versionOutput.split('"')[1];
  if (quoted === undefined) return null;
  const parts = quoted.split(/[.\-_]/);
  const first = Number.parseInt(parts[0] ?? "", 10);
  if (Number.isNaN(first)) return null;
  if (first === 1) {
    const second = Number.parseInt(parts[1] ?? "", 10);
    return Number.isNaN(second) ? null : second;
  }
  return first;
}

export interface Probe { launcherPresent: boolean; javaMajor: number | null }
export type ProbeFn = (ghidraDir: string | undefined) => Probe;

export function collectProbe(ghidraDir: string | undefined): Probe {
  const launcherPresent =
    ghidraDir !== undefined &&
    (existsSync(join(ghidraDir, "support", "analyzeHeadless")) ||
      existsSync(join(ghidraDir, "support", "analyzeHeadless.bat")));
  let javaMajor: number | null = null;
  try {
    const res = spawnSync("java", ["-version"], { encoding: "utf8" });
    if (!res.error) {
      javaMajor =
        parseJavaMajor(String(res.stderr ?? "")) ?? parseJavaMajor(String(res.stdout ?? ""));
    }
  } catch {
    // no java (or spawn unsupported): stays null -> a finding, never a throw
  }
  return { launcherPresent, javaMajor };
}

export function preflight(cfg: RawSettings, probe: Probe): string[] {
  const findings: string[] = [];
  if (cfg.ghidra_install_dir === undefined) {
    findings.push(
      "GHIDRA_INSTALL_DIR is not set. Point it at your Ghidra install root (the directory containing `support/`).",
    );
  } else if (!probe.launcherPresent) {
    findings.push(
      `GHIDRA_INSTALL_DIR is set to \`${cfg.ghidra_install_dir}\` but there is no \`support/analyzeHeadless\` there, so it is not a Ghidra install root.`,
    );
  }
  if (probe.javaMajor === null) {
    findings.push(
      "No `java` on PATH. Ghidra 12.1.2 requires JDK 21 (`application.java.min=21`).",
    );
  } else if (probe.javaMajor < MIN_JAVA_MAJOR) {
    findings.push(
      `\`java\` on PATH reports version ${probe.javaMajor}; Ghidra 12.1.2 requires JDK ${MIN_JAVA_MAJOR} or newer.`,
    );
  }
  const missing = (["project_dir", "project_name", "bootstrap_program"] as const).filter(
    (name) => cfg[name] === undefined,
  );
  if (missing.length > 0) {
    findings.push(
      `Unset: ${missing.join(", ")}. Set them in \`.opencode/re-ghidra-mcp-opencode.local.md\` (see the plugin's examples/ directory) or as GHIDRA_MCP_PROJECT_DIR / GHIDRA_MCP_PROJECT_NAME / GHIDRA_MCP_BOOTSTRAP_PROGRAM.`,
    );
  }
  return findings;
}

export function preflightFindings(
  projectDir: string,
  env: Record<string, string | undefined> = process.env,
  probeFn: ProbeFn = collectProbe,
): string[] {
  try {
    let text = "";
    try {
      text = readFileSync(join(projectDir, SETTINGS_REL_PATH), "utf8");
    } catch {
      // missing/unreadable contributes nothing, like the Rust side
    }
    const cfg = overlayEnv(settingsFromMarkdown(text), env);
    return preflight(cfg, probeFn(cfg.ghidra_install_dir));
  } catch {
    return []; // fail-open: the preflight may never break a session
  }
}

export function preflightReport(findings: string[]): string {
  return [
    "re-ghidra-mcp-opencode is not ready to attach to a Ghidra project:",
    ...findings.map((f) => `  - ${f}`),
  ].join("\n");
}

// No `context: string[]` on SessionContext (@opencode/plugin@2.0.21):
// push `{ type: "text", text }` onto `event.system`; tool names via
// `Object.keys(event.tools)`.
export default Plugin.define({
  id: "re-ghidra-mcp-opencode",
  // `probeFn` is a test seam only; real hosts call setup(ctx) and collectProbe
  // runs. Optional, so the shape still fits Plugin.define's `setup(context)`.
  async setup(ctx, probeFn: ProbeFn = collectProbe) {
    // Silent when healthy, never blocks: any failure degrades to no report.
    let pending: string[] = [];
    try {
      const findings = preflightFindings(ctx.location.directory, process.env, probeFn);
      if (findings.length > 0) pending = [preflightReport(findings)];
    } catch {
      pending = [];
    }
    await ctx.session.hook("compaction", (event) => {
      // Never throw inside the host: push only when `system` is an array.
      if (Array.isArray(event.system)) {
        event.system.push({ type: "text", text: compactionReminder() });
      }
    });
    await ctx.session.hook("context", (event) => {
      // `tools` may be absent on API drift; default to no tools (silent).
      if (Array.isArray(event.system)) {
        if (pending.length > 0) {
          event.system.push({ type: "text", text: pending[0]! });
          pending = [];
        }
        if (needsDriverPointer(Object.keys(event.tools ?? {}))) {
          event.system.push({ type: "text", text: driverPointer() });
        }
      }
    });
  },
});
