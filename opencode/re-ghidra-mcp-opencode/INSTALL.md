# Installing re-ghidra-mcp-opencode for OpenCode (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install
> this plugin for them. Follow this document top to bottom, in order. Do not
> skip the verification step. If any step fails, stop and tell the user
> exactly which step failed and what you saw — do not improvise workarounds
> that change what gets installed.
>
> Everything below comes from versioned release assets — no branch files,
> no unpinned URLs. Agree the version with the user first (`0.7.2`
> is current as of this writing) and use it for every URL.

You will install `re-ghidra-mcp-opencode`: a persistent headless Ghidra JVM
exposed as 19 reverse-engineering tools over MCP, plus two session hooks
(RE state across compaction, driver-skill pointer). Install is a file copy
plus a config merge — no registry, no build of the TypeScript itself.
Tested with OpenCode **v2.0.20** and `@opencode/plugin` **2.0.21**; if the
user runs anything older, say so before continuing.

This document is the executable form of `README.md`'s Installing section.
If the two ever disagree, this file wins for install mechanics and the
mismatch is a bug — tell the user.

## Step 0 — prerequisites

Confirm each one; stop and report if one fails. Nothing here bundles
Ghidra — the user supplies it:

1. OpenCode V2: `opencode --version` prints `opencode v2.0.20` or newer.
   (V1 is not supported by this plugin.)
2. **Ghidra 12.1.2** with `GHIDRA_INSTALL_DIR` set to its install root
   (the directory containing `support/`, `Ghidra/` and `ghidraRun`).
3. **JDK 21** or newer on `PATH` (Ghidra 12.1.2 declares
   `application.java.min=21`).
4. A Ghidra project already created, imported, **fully analyzed, and
   closed in the GUI**. Note `GHIDRA_MCP_PROJECT_DIR` (directory holding
   the `.gpr`/`.rep`) and `GHIDRA_MCP_PROJECT_NAME` — Step 4 needs them.
   Warn the user: **one server = one Ghidra project.** The install target
   must not point at a project an open GUI (or another server) holds —
   they collide on Ghidra's `project.lock`. And the writes are durable
   with no undo: suggest a project backup before agents write to it.
5. Download tools: `curl` and `unzip` (Git Bash on Windows has both;
   PowerShell alternative: `Invoke-WebRequest` and `Expand-Archive`).
6. `bun` for one dependency-install step (`bun --version`).

If Ghidra setup is broken, install the plugin anyway (it loads fine
without Ghidra) and point the user at the `doctor` skill afterwards — it
walks these checks in order.

## Step 1 — ask where to install

Ask the user: **project-local** (this project only) or **global** (every
project)? Then set `DEST` accordingly and use it for every path below:

- Project-local: `DEST` = `<project root>/.opencode`
- Global: `DEST` = `~/.config/opencode`

Do not proceed on an assumed default.

## Step 2 — fetch the plugin bundle and the MCP server binary

Set the agreed version once; every URL below uses it (shown with
`0.7.2` — substitute the agreed version everywhere `VERSION` appears):

```bash
VERSION=0.7.2
```

### The plugin files

Download the versioned plugin bundle — asset
`re-ghidra-mcp-opencode-opencode.zip` on release
`re-ghidra-mcp-opencode-v$VERSION` (the doubled name is
`<package>-opencode.zip` for package `re-ghidra-mcp-opencode`) — and stage
it outside `$DEST`:

```bash
mkdir -p "$DEST/plugins" "$DEST/skills/ghidra-re-driver" "$DEST/skills/doctor" "$DEST/agents"
curl -fsSL -o /tmp/ghidra-opencode.zip \
  "https://github.com/ckir/aiplugins/releases/download/re-ghidra-mcp-opencode-v$VERSION/re-ghidra-mcp-opencode-opencode.zip"
rm -rf /tmp/ghidra-opencode-stage && mkdir -p /tmp/ghidra-opencode-stage
unzip -q -o /tmp/ghidra-opencode.zip -d /tmp/ghidra-opencode-stage
```

The zip's top level IS the plugin directory (`plugin.ts`,
`opencode.jsonc`, `skills/`, `agents/`, … — no wrapper folder), but the
destinations keep the host layout, so copy each staged file to its place
(exact names matter):

```bash
cp /tmp/ghidra-opencode-stage/plugin.ts "$DEST/plugins/re-ghidra-mcp-opencode.ts"
cp /tmp/ghidra-opencode-stage/skills/ghidra-re-driver/SKILL.md "$DEST/skills/ghidra-re-driver/SKILL.md"
cp /tmp/ghidra-opencode-stage/skills/doctor/SKILL.md "$DEST/skills/doctor/SKILL.md"
cp /tmp/ghidra-opencode-stage/agents/re-analyst.md "$DEST/agents/re-analyst.md"
```

(`plugin.ts` is deliberately renamed on install; the host loads every
file in `plugins/`, and the name keeps global installs identifiable.)
Every download and copy must exit 0 and every destination must be
non-empty — verify with `wc -c` on each destination.

Leave the stage in place: Step 4 merges the staged `opencode.jsonc`.

### The MCP server binary

`re-ghidra-cc-mcp` (the 19 tools) ships in the versioned Claude Code
plugin bundle — asset `re-ghidra-mcp-cc-plugin.zip` on release
`re-ghidra-mcp-cc-v$VERSION` — reuse that zip, ignore everything in it except `bin/`:

1. Map the machine to a target triple (`uname -s` + `uname -m`):

   | System | Triple |
   |---|---|
   | Linux x86_64 | `x86_64-unknown-linux-gnu` |
   | Linux aarch64 | `aarch64-unknown-linux-gnu` |
   | macOS x86_64 | `x86_64-apple-darwin` |
   | macOS arm64 (`uname -m` says `arm64`) | `aarch64-apple-darwin` |
   | Windows x86_64 | `x86_64-pc-windows-msvc` |

   Anything else: stop and report — there is no build for it.

2. Download and extract only the one binary for that triple:

   ```bash
   curl -fsSL -o /tmp/ghidra-plugin.zip \
     "https://github.com/ckir/aiplugins/releases/download/re-ghidra-mcp-cc-v$VERSION/re-ghidra-mcp-cc-plugin.zip"
   mkdir -p "$DEST/bin"
   # Unix:
   unzip -p /tmp/ghidra-plugin.zip "bin/<triple>/re-ghidra-cc-mcp" > "$DEST/bin/re-ghidra-cc-mcp"
   chmod +x "$DEST/bin/re-ghidra-cc-mcp"
   # Windows (PowerShell):
   # Expand-Archive C:\tmp\ghidra-plugin.zip C:\tmp\ghidra-plugin
   # Copy-Item C:\tmp\ghidra-plugin\bin\re-ghidra-cc-mcp.exe $DEST\bin\re-ghidra-cc-mcp.exe
   ```

   Do NOT extract `bin/<name>` (the extensionless dispatcher) — it is a
   shell script for the Claude Code host, and OpenCode spawns the server
   binary directly. The per-target binary is the file you want.

3. Confirm the binary executes: `$DEST/bin/re-ghidra-cc-mcp --help`
   exits 0 (any `--help` text counts; "cannot execute binary file" means
   the wrong triple — go back to step 1 of this section). This proves
   the binary runs; it does not prove Ghidra connects — the `doctor`
   skill does that.

## Step 3 — install the plugin runtime dependency

`plugin.ts` imports `@opencode/plugin` as a value (`Plugin.define`), so
the destination config dir needs that package resolvable. OpenCode runs
`bun install` at startup for a `package.json` in the config dir, but do
not rely on a later restart — install it now:

```bash
cd "$DEST/.." && bun add @opencode/plugin@2.0.21
```

That is: run it in `.opencode/`'s parent for a project-local install
(creates/updates `<project>/.opencode/package.json`), or in
`~/.config/opencode/` for a global install. If the README's "Tested with"
pins name a newer `@opencode/plugin` than `2.0.21`, use the README's
version instead of the one above and say so. Verify
`$DEST/../node_modules/@opencode/plugin/package.json` exists afterwards
(`$DEST/node_modules/...` when `DEST` itself is the config dir — i.e. the
global case resolves to `~/.config/opencode/node_modules/...`).

If `bun add` hangs (no output for minutes) or fails partway: kill it,
delete the partial state in that directory (`node_modules/`,
`bun.lock`, and the `package.json` it just created, if you had none
before), and retry once — an interrupted first attempt poisons the
second. If it still fails, the problem is registry access (proxy,
offline mirror), not this plugin: OpenCode itself runs `bun install` at
startup and will hit the same wall, so fix network access first and
then retry.


## Step 4 — merge the config snippet

Fetch the reference snippet and merge its section into the user's config
(`$DEST/opencode.json` or `$DEST/opencode.jsonc` — whichever exists;
create `opencode.jsonc` if neither does). Also set the project variables
from Step 0 (`GHIDRA_MCP_PROJECT_DIR`, `GHIDRA_MCP_PROJECT_NAME`, plus
`GHIDRA_INSTALL_DIR` if the user wants it in config rather than
environment):

```bash
cp /tmp/ghidra-opencode-stage/opencode.jsonc /tmp/ghidra-opencode.jsonc
```

The reference (V2-native shape — `mcp.servers`):

```json
{
  "mcp": {
    "servers": {
      "ghidra": {
        "type": "local",
        "command": ["./bin/re-ghidra-cc-mcp", "serve"],
        "disabled": false,
        "timeout": { "catalog": 30000, "execution": 30000 }
      }
    }
  },
  "permissions": [],
  "skills": []
}
```

Merge rules: add the `ghidra` entry under the existing `mcp.servers`
(creating `mcp.servers` if absent). There are no `permissions` to merge
(the reference carries none — the server needs no host-tool grants
beyond what the user already allows). Do not paste the whole snippet over
the user's config. On Windows the command must name the `.exe`:
`["./bin/re-ghidra-cc-mcp.exe", "serve"]` — adjust that one field when
merging. These shapes assume a V2-native config; if the user's file uses
V1 keys (`mcpServers`, top-level `"plugin"`), say so and merge into the
equivalent V1 locations instead of mixing shapes.

## Step 5 — verify the install

All four, in order:

1. Every destination file exists and is non-empty (the four plugin files
   Step 2 copied plus the MCP binary it staged).
2. The merged config parses: `jq -e . $DEST/opencode.jsonc` (or
   `.../opencode.json`) exits 0, and the `ghidra` server entry is present
   under `mcp.servers`.
3. The plugin module loads: from any directory with the config dir's
   `node_modules` resolvable,
   `bun -e "await import('$DEST/plugins/re-ghidra-mcp-opencode.ts')"` exits 0
   with no output. Importing only defines the hooks — nothing executes
   until the host loads it.
4. Restart OpenCode (config and plugins load at startup), then confirm
   the `ghidra` server is connected (`/mcp`) and its 19 tools are listed.
   Run the `doctor` skill if anything here is wrong — it walks the Ghidra
   setup checks from Step 0 in order.
