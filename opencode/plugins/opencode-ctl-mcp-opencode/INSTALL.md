# Installing opencode-ctl-mcp-opencode for OpenCode (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install
> this plugin for them. Follow this document top to bottom, in order. Do not
> skip the verification step. If any step fails, stop and tell the user
> exactly which step failed and what you saw. Do not improvise workarounds
> that change what gets installed.
>
> Everything below comes from versioned release assets. Agree the versions
> with the user first (plugin and MCP server binary version independently;
> `0.1.0` / `0.1.0` are current as of this writing; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use
> them for every URL below.

You will install `opencode-ctl-mcp-opencode`: a plugin that lets an OpenCode
session delegate work to a worker OpenCode session through the
`opencode-ctl` MCP server. Install is a file copy plus a config merge: no
registry, no build of the TypeScript itself. Tested with OpenCode **v2.0.20**
and `@opencode/plugin` **2.0.32**; if the user runs anything older, say so
before continuing.

## Step 0 - prerequisites

1. OpenCode V2: `opencode --version` prints `opencode v2.0.20` or newer.
2. Download tools: `curl` and `unzip` (PowerShell alternative:
   `Invoke-WebRequest` and `Expand-Archive`).
3. `bun` for one dependency-install step (`bun --version`).

## Step 1 - ask where to install

Ask the user: **project-local** (this project only) or **global** (every
project)? Then set `DEST` and use it for every path below:

- Project-local: `DEST` = `<project root>/.opencode`
- Global: `DEST` = `~/.config/opencode`

Do not proceed on an assumed default.

## Step 2 - fetch the plugin bundle and the MCP server binary

```bash
VERSION=0.1.0
CC_VERSION=0.1.0
```

### The plugin files

Download asset `opencode-ctl-mcp-opencode-opencode.zip` from release
`opencode-ctl-mcp-opencode-v$VERSION` and stage it outside `$DEST`:

```bash
mkdir -p "$DEST/plugins" "$DEST/skills/opencode-ctl-driver" "$DEST/skills/opencode-ctl-doctor"
curl -fsSL -o /tmp/opencode-ctl-opencode.zip \
  "https://github.com/ckir/aiplugins/releases/download/opencode-ctl-mcp-opencode-v$VERSION/opencode-ctl-mcp-opencode-opencode.zip"
rm -rf /tmp/opencode-ctl-opencode-stage && mkdir -p /tmp/opencode-ctl-opencode-stage
unzip -q -o /tmp/opencode-ctl-opencode.zip -d /tmp/opencode-ctl-opencode-stage
cp /tmp/opencode-ctl-opencode-stage/plugin.ts "$DEST/plugins/opencode-ctl-mcp-opencode.ts"
cp /tmp/opencode-ctl-opencode-stage/skills/opencode-ctl-driver/SKILL.md "$DEST/skills/opencode-ctl-driver/SKILL.md"
cp /tmp/opencode-ctl-opencode-stage/skills/opencode-ctl-doctor/SKILL.md "$DEST/skills/opencode-ctl-doctor/SKILL.md"
```

`plugin.ts` is deliberately renamed on install. Every download and copy must
exit 0 and every destination must be non-empty (`wc -c`). Leave the stage in
place: Step 4 merges the staged `opencode.jsonc`.

### The MCP server binary

`opencode-ctl-cc-mcp` ships in the Claude Code plugin bundle: asset
`opencode-ctl-mcp-cc-plugin.zip` on release `opencode-ctl-mcp-cc-v$CC_VERSION`.
Reuse that zip and take only the binary for this machine:

1. Map the machine to a target triple (`uname -s` + `uname -m`):

   | System | Triple |
   |---|---|
   | Linux x86_64 | `x86_64-unknown-linux-gnu` |
   | Linux aarch64 | `aarch64-unknown-linux-gnu` |
   | macOS x86_64 | `x86_64-apple-darwin` |
   | macOS arm64 | `aarch64-apple-darwin` |
   | Windows x86_64 | `x86_64-pc-windows-msvc` |

   Anything else: stop and report.

2. Download and extract that one binary:

   ```bash
   curl -fsSL -o /tmp/opencode-ctl-cc.zip \
     "https://github.com/ckir/aiplugins/releases/download/opencode-ctl-mcp-cc-v$CC_VERSION/opencode-ctl-mcp-cc-plugin.zip"
   mkdir -p "$DEST/bin"
   # Unix:
   unzip -p /tmp/opencode-ctl-cc.zip "bin/<triple>/opencode-ctl-cc-mcp" > "$DEST/bin/opencode-ctl-cc-mcp"
   chmod +x "$DEST/bin/opencode-ctl-cc-mcp"
   # Windows (PowerShell):
   # Expand-Archive C:\tmp\opencode-ctl-cc.zip C:\tmp\opencode-ctl-cc
   # Copy-Item C:\tmp\opencode-ctl-cc\bin\opencode-ctl-cc-mcp.exe $DEST\bin\opencode-ctl-cc-mcp.exe
   ```

   Do NOT extract `bin/<name>` (the extensionless dispatcher): it is a shell
   script for the Claude Code host.

3. Confirm it runs: `$DEST/bin/opencode-ctl-cc-mcp --version` exits 0.

## Step 3 - install the plugin runtime dependency

`plugin.ts` imports `@opencode/plugin` as a value (`Plugin.define`), so the
config dir needs that package resolvable:

```bash
cd "$DEST/.." && bun add @opencode/plugin@2.0.32
```

Run it in `.opencode/`'s parent for a project-local install, or in
`~/.config/opencode/` for a global install. If the README names a newer
`@opencode/plugin`, use the README's version and say so. If `bun add` hangs
or fails partway, kill it, delete the partial state (`node_modules/`,
`bun.lock`, and a `package.json` it just created) and retry once.

## Step 4 - merge the config snippet

```bash
cp /tmp/opencode-ctl-opencode-stage/opencode.jsonc /tmp/opencode-ctl-opencode.jsonc
```

The reference (V2-native shapes):

```json
{
  "mcp": {
    "servers": {
      "opencode-ctl": {
        "type": "local",
        "command": ["<DEST>/bin/opencode-ctl-cc-mcp", "serve", "--host", "opencode"],
        "environment": { "OPENCODE_CTL_HOST": "opencode" },
        "disabled": false,
        "timeout": { "catalog": 30000, "execution": 330000 }
      }
    }
  },
  "permissions": [],
  "skills": []
}
```

Merge rules: add the `opencode-ctl` entry under the existing `mcp.servers`
(creating it if absent) into `$DEST/opencode.json` or `$DEST/opencode.jsonc`
(create `opencode.jsonc` if neither exists). Write `command[0]` as an
ABSOLUTE path: replace `<DEST>` with the absolute config dir. A relative
`./bin/...` does not resolve against the config dir. Keep `serve`, `--host`,
`opencode` and the `OPENCODE_CTL_HOST` environment entry exactly; keep the
`330000` ms execution timeout (a `session_wait` slice must end before the host
cuts the call off). On Windows name the `.exe`. Leave existing `permissions`
and `skills` alone. Do not paste the whole snippet over the user's config.

## Step 5 - verify the install

1. Every destination file exists and is non-empty.
2. The merged config parses: `jq -e . $DEST/opencode.jsonc` exits 0 and the
   `opencode-ctl` entry is under `mcp.servers`.
3. The plugin module loads: from the directory where you ran `bun add`,
   `bun -e "await import('$DEST/plugins/opencode-ctl-mcp-opencode.ts')"`
   exits 0 with no output.
4. Restart OpenCode, then run `/opencode-ctl-mcp-opencode:opencode-ctl-doctor`
   and confirm the `opencode-ctl` server is connected (`/mcp`).
