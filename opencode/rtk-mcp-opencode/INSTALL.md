# Installing rtk-mcp-opencode for OpenCode (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install
> this plugin for them. Follow this document top to bottom, in order. Do not
> skip the verification step. If any step fails, stop and tell the user
> exactly which step failed and what you saw — do not improvise workarounds
> that change what gets installed.
>
> Fetch-and-follow URL for this document:
> `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/rtk-mcp-opencode/INSTALL.md`

You will install `rtk-mcp-opencode`: a PreToolUse-style hook that rewrites
OpenCode shell commands through `rtk` (60–90% token savings) plus four MCP
analytics tools. Install is a file copy plus a config merge — no registry,
no build of the TypeScript itself. Tested with OpenCode **v2.0.20** and
`@opencode/plugin` **2.0.21**; if the user runs anything older, say so
before continuing.

This document is the executable form of `README.md`'s Installing section.
If the two ever disagree, this file wins for install mechanics and the
mismatch is a bug — tell the user.

## Step 0 — prerequisites

Confirm each one; stop and report if one fails:

1. OpenCode V2: `opencode --version` prints `opencode v2.0.20` or newer.
   (V1 is not supported by this plugin.)
2. `rtk` on `PATH`: `rtk --version` prints `rtk 0.45.0` (or newer) from the
   ckir/rtk project. If `rtk gain` fails with odd errors, the user may have
   reachingforthejack/rtk (Rust Type Kit) installed instead — different
   program, same name; report it. Without a working `rtk` the plugin still
   installs and loads, it just never rewrites anything (fail-open).
3. Download tools: `curl` and `unzip` (Git Bash on Windows has both;
   PowerShell alternative: `Invoke-WebRequest` and `Expand-Archive`).
4. `bun` for one dependency-install step (`bun --version`). OpenCode
   itself runs on Bun; any recent Bun works here.

## Step 1 — ask where to install

Ask the user: **project-local** (this project only) or **global** (every
project)? Then set `DEST` accordingly and use it for every path below:

- Project-local: `DEST` = `<project root>/.opencode`
- Global: `DEST` = `~/.config/opencode`

Do not proceed on an assumed default.

## Step 2 — fetch the plugin files

Base URL for every file (same shape as this document's own URL):

```text
https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/rtk-mcp-opencode/
```

Create the directories, then download each file to its destination
(exact names matter):

```bash
mkdir -p "$DEST/plugins" "$DEST/skills/rtk-policy" "$DEST/skills/gain"
curl -fsSL "$BASE/plugin.ts" -o "$DEST/plugins/rtk-mcp-opencode.ts"
curl -fsSL "$BASE/skills/rtk-policy/SKILL.md" -o "$DEST/skills/rtk-policy/SKILL.md"
curl -fsSL "$BASE/skills/gain/SKILL.md" -o "$DEST/skills/gain/SKILL.md"
```

with `BASE=https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/rtk-mcp-opencode`.
Every `curl` must exit 0 and produce a non-empty file — verify with
`wc -c` on each destination. (`plugin.ts` is deliberately renamed to
`rtk-mcp-opencode.ts` on install; the host loads every file in
`plugins/`, and the name keeps global installs identifiable.)

Optional but recommended: the settings template, only if the user wants
non-default config now (otherwise skip — environment variables from
README.md work without any file):

```bash
curl -fsSL "$BASE/examples/rtk-mcp-opencode.local.md" -o "$DEST/rtk-mcp-opencode.local.md"
```

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

## Step 4 — stage the MCP server binary

`rtk-cc-mcp` (the four analytics tools) ships in the Claude Code plugin
bundle on the latest release — reuse that zip, ignore everything in it
except `bin/`:

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
   curl -fsSL -o /tmp/rtk-plugin.zip \
     https://github.com/ckir/aiplugins/releases/latest/download/rtk-mcp-cc-plugin.zip
   mkdir -p "$DEST/bin"
   # Unix:
   unzip -p /tmp/rtk-plugin.zip "bin/<triple>/rtk-cc-mcp" > "$DEST/bin/rtk-cc-mcp"
   chmod +x "$DEST/bin/rtk-cc-mcp"
   # Windows (PowerShell):
   # Expand-Archive C:\tmp\rtk-plugin.zip C:\tmp\rtk-plugin
   # Copy-Item C:\tmp\rtk-plugin\bin\rtk-cc-mcp.exe $DEST\bin\rtk-cc-mcp.exe
   ```

   Do NOT extract `bin/<name>` (the extensionless dispatcher) — it is a
   shell script for the Claude Code host, and OpenCode spawns the server
   binary directly. The per-target binary is the file you want.

3. Confirm the binary executes: `$DEST/bin/rtk-cc-mcp --help` exits 0
   (any `--help` text counts; a "cannot execute binary file" error means
   the wrong triple — go back to step 1 of this section).

## Step 5 — merge the config snippet

Fetch the reference snippet and merge its two sections into the user's
config (`$DEST/opencode.json` or `$DEST/opencode.jsonc` — whichever
exists; create `opencode.jsonc` if neither does):

```bash
curl -fsSL "$BASE/opencode.jsonc" -o /tmp/rtk-opencode.jsonc
```

The reference (V2-native shapes — `mcp.servers`, ordered `permissions`):

```json
{
  "mcp": {
    "servers": {
      "rtk": {
        "type": "local",
        "command": ["./bin/rtk-cc-mcp"],
        "disabled": false,
        "timeout": { "catalog": 30000, "execution": 30000 },
        "environment": { "RUST_LOG": "info" }
      }
    }
  },
  "permissions": [{ "action": "shell", "resource": "*", "effect": "allow" }],
  "skills": []
}
```

Merge rules: add the `rtk` entry under the existing `mcp.servers
...[truncated 1587 chars]