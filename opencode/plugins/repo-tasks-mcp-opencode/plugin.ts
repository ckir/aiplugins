import { Plugin } from "@opencode/plugin";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const BIN_NAME = process.platform === "win32" ? "repo-tasks-cc-mcp.exe" : "repo-tasks-cc-mcp";

// The binary: REPO_TASKS_BIN when set, else the Claude Code wrapper's binary
// installed beside the plugin (`<config dir>/bin`, or `./bin` in a source
// checkout), else the bare name on PATH.
export function binaryPath(get: (key: string) => string | undefined = (k) => process.env[k]): string {
  const configured = get("REPO_TASKS_BIN");
  if (configured !== undefined && configured.trim() !== "") return configured.trim();
  try {
    const here = dirname(fileURLToPath(import.meta.url));
    for (const candidate of [join(here, "..", "bin", BIN_NAME), join(here, "bin", BIN_NAME)]) {
      if (existsSync(candidate)) return candidate;
    }
  } catch {
    // fall through to PATH
  }
  return BIN_NAME;
}

// Runs `reminder --event opencode --workspace <dir>` and returns its line, or
// undefined when nothing comes back. Never throws.
export function reminderLine(directory: string, bin: string = binaryPath()): string | undefined {
  try {
    const r = spawnSync(bin, ["reminder", "--event", "opencode", "--workspace", directory], {
      encoding: "utf8",
      timeout: 5000,
      windowsHide: true,
    });
    if (r.error || r.status !== 0 || typeof r.stdout !== "string") return undefined;
    const line = r.stdout.trim();
    return line === "" ? undefined : line;
  } catch {
    return undefined;
  }
}

// Push `{ type: "text", text }` onto `event.system` only when it is an array
// (same pattern as re-ghidra-mcp-opencode). Never throw inside the host.
function push(event: unknown, directory: string): void {
  try {
    const system = (event as { system?: unknown } | undefined)?.system;
    if (!Array.isArray(system)) return;
    const text = reminderLine(directory);
    if (text) system.push({ type: "text", text });
  } catch {
    return;
  }
}

export default Plugin.define({
  id: "repo-tasks-mcp-opencode",
  async setup(ctx) {
    const directory = ctx.location.directory;
    const reminded = new Set<string>();
    await ctx.session.hook("compaction", (event) => {
      push(event, directory);
    });
    await ctx.session.hook("context", (event) => {
      try {
        const id = String((event as unknown as { sessionID?: unknown }).sessionID ?? "");
        if (reminded.has(id)) return;
        reminded.add(id);
      } catch {
        return;
      }
      push(event, directory);
    });
  },
});
