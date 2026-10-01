import { Plugin } from "@opencode/plugin";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

export interface RtkConfig {
  enabled: boolean;
  rtkBin: string;
  ultraCompact: boolean;
  skipEnv: boolean;
}

export function defaultConfig(): RtkConfig {
  return { enabled: true, rtkBin: "rtk", ultraCompact: false, skipEnv: false };
}

export function parseBool(value: string): boolean | undefined {
  switch (value.trim().toLowerCase()) {
    case "1":
    case "true":
    case "yes":
    case "on":
      return true;
    case "0":
    case "false":
    case "no":
    case "off":
      return false;
    default:
      return undefined;
  }
}

function extractFrontmatter(source: string): string | undefined {
  if (!source.startsWith("---")) return undefined;
  const rest = source.slice(3).replace(/^\r?\n/, "");
  const end = rest.indexOf("\n---");
  if (end === -1) return undefined;
  return rest.slice(0, end);
}

export function configFromMarkdown(source: string): RtkConfig {
  const config = defaultConfig();
  const front = extractFrontmatter(source);
  if (!front) return config;
  for (const line of front.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const sep = trimmed.indexOf(":");
    if (sep === -1) continue;
    const key = trimmed.slice(0, sep).trim();
    const value = trimmed
      .slice(sep + 1)
      .trim()
      .replace(/^["']|["']$/g, "");
    switch (key) {
      case "enabled": {
        const b = parseBool(value);
        if (b !== undefined) config.enabled = b;
        break;
      }
      case "rtk_bin":
        if (value) config.rtkBin = value;
        break;
      case "ultra_compact": {
        const b = parseBool(value);
        if (b !== undefined) config.ultraCompact = b;
        break;
      }
      case "skip_env": {
        const b = parseBool(value);
        if (b !== undefined) config.skipEnv = b;
        break;
      }
      default:
        break;
    }
  }
  return config;
}

export function applyEnv(
  config: RtkConfig,
  get: (key: string) => string | undefined
): RtkConfig {
  const rtkBin = get("RTK_BIN");
  if (rtkBin !== undefined && rtkBin.trim() !== "") config.rtkBin = rtkBin.trim();
  const disable = get("RTK_OPENCODE_DISABLE");
  if (disable !== undefined && (parseBool(disable) ?? false)) config.enabled = false;
  const ultra = get("RTK_OPENCODE_ULTRA_COMPACT");
  if (ultra !== undefined) {
    const b = parseBool(ultra);
    if (b !== undefined) config.ultraCompact = b;
  }
  const skip = get("RTK_OPENCODE_SKIP_ENV");
  if (skip !== undefined) {
    const b = parseBool(skip);
    if (b !== undefined) config.skipEnv = b;
  }
  return config;
}

export function loadConfig(
  projectDir: string,
  get: (key: string) => string | undefined
): RtkConfig {
  const path = join(projectDir, ".opencode", "rtk-mcp-opencode.local.md");
  let config = defaultConfig();
  try {
    if (existsSync(path)) config = configFromMarkdown(readFileSync(path, "utf8"));
  } catch {
    config = defaultConfig();
  }
  return applyEnv(config, get);
}

export function hookArgs(config: RtkConfig): string[] {
  const args = ["hook", "claude"];
  if (config.ultraCompact) args.push("--ultra-compact");
  if (config.skipEnv) args.push("--skip-env");
  return args;
}

export function delegate(
  payload: string,
  config: RtkConfig,
  run: (args: string[], payload: string) => string | null
): string {
  if (!config.enabled) return "";
  if (payload.trim() === "") return "";
  const stdout = run(hookArgs(config), payload);
  if (stdout === null || stdout.trim() === "") return "";
  try {
    JSON.parse(stdout.trim());
  } catch {
    return "";
  }
  return stdout;
}

export function runRtk(rtkBin: string, args: string[], payload: string): string | null {
  try {
    const out = spawnSync(rtkBin, args, {
      input: payload,
      encoding: "utf8",
      timeout: 10000,
    });
    if (out.status !== 0) return null;
    return typeof out.stdout === "string" ? out.stdout : null;
  } catch {
    return null;
  }
}

const SHELL_TOOL = "shell"; // MUST equal shell-shape.json "tool"; Task 1 owns this value
const COMMAND_FIELD = "command"; // MUST equal the command key in shell-shape.json "input"

interface Verdict {
  hookSpecificOutput?: { updatedInput?: { command?: string } };
}

export function applyVerdict(
  input: Record<string, unknown>,
  commandField: string,
  verdict: string
): void {
  let parsed: Verdict;
  try {
    parsed = JSON.parse(verdict) as Verdict;
  } catch {
    return;
  }
  const rewritten = parsed.hookSpecificOutput?.updatedInput?.command;
  if (typeof rewritten === "string" && rewritten !== "" && rewritten !== input[commandField]) {
    input[commandField] = rewritten;
  }
}

export default Plugin.define({
  id: "rtk-mcp-opencode",
  async setup(ctx) {
    await ctx.tool.hook("execute.before", (event) => {
      const tool = (event as unknown as { tool?: unknown }).tool;
      if (tool !== SHELL_TOOL) return;
      const input = (event as unknown as { input?: unknown }).input as
        | Record<string, unknown>
        | undefined;
      if (!input || typeof input[COMMAND_FIELD] !== "string") return;
      const command = input[COMMAND_FIELD] as string;
      if (command.trim() === "") return;
      const config = loadConfig(process.cwd(), (key) => process.env[key]);
      if (!config.enabled) return;
      const envelope = JSON.stringify({ tool_name: "Bash", tool_input: { command } });
      const verdict = delegate(envelope, config, (args, payload) =>
        runRtk(config.rtkBin, args, payload)
      );
      if (!verdict) return;
      applyVerdict(input, COMMAND_FIELD, verdict);
    });
  },
});
