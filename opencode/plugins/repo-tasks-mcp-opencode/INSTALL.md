# Installing repo-tasks-mcp-opencode for OpenCode (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install this plugin for them. Follow this
> document top to bottom, in order. Do not skip the verification step. If any step fails, stop and tell the user
> exactly which step failed and what you saw. Do not improvise workarounds that change what gets installed.
>
> Agree the versions with the user first (plugin and binary version independently; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use them for every URL below.

You will install `repo-tasks-mcp-opencode`: a plugin that gives an OpenCode session the `repo-tasks` MCP server and a
once-per-session reminder. Install is a file copy plus a config merge. Tested with `@opencode/plugin` **2.0.32**.

## Step 0 - prerequisites

`opencode --version` works; `curl` and `unzip` (PowerShell: `Invoke-WebRequest`, `Expand-Archive`); `bun`.

## Step 1 - ask where to install

Ask: **project-local** (`DEST` = `<project root>/.opencode`) or **global** (`DEST` = `~/.config/opencode`)? Do not
proceed on an assumed default.

## Step 2 - fetch the plugin and the binary

```bash
VERSION=0.1.0
CC_VERSION=0.1.0
mkdir -p "$DEST/plugins" "$DEST/bin"
curl -fsSL -o /tmp/repo-tasks-opencode.zip \
  "https://github.com/ckir/aiplugins/releases/download/repo-tasks-mcp-opencode-v$VERSION/repo-tasks-mcp-opencode-opencode.zip"
rm -rf /tmp/repo-tasks-opencode-stage && mkdir -p /tmp/repo-tasks-opencode-stage
unzip -q -o /tmp/repo-tasks-opencode.zip -d /tmp/repo-tasks-opencode-stage
cp /tmp/repo-tasks-opencode-stage/plugin.ts "$DEST/plugins/repo-tasks-mcp-opencode.ts"
```

This plugin has no binary of its own. `repo-tasks-cc-mcp` ships in the Claude Code plugin bundle: asset
`repo-tasks-mcp-cc-plugin.zip` on release `repo-tasks-mcp-cc-v$CC_VERSION`. Map the machine to a target triple
(`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`,
`x86_64-pc-windows-msvc`; anything else: stop and report) and take only that binary:

```bash
curl -fsSL -o /tmp/repo-tasks-cc.zip \
  "https://github.com/ckir/aiplugins/releases/download/repo-tasks-mcp-cc-v$CC_VERSION/repo-tasks-mcp-cc-plugin.zip"
unzip -p /tmp/repo-tasks-cc.zip "bin/<triple>/repo-tasks-cc-mcp" > "$DEST/bin/repo-tasks-cc-mcp"
chmod +x "$DEST/bin/repo-tasks-cc-mcp"
"$DEST/bin/repo-tasks-cc-mcp" --version
```

Do NOT extract `bin/repo-tasks-cc-mcp` without the triple directory (the extensionless dispatcher is a shell script
for the Claude Code host). On Windows use the `.exe` name. `--version` must exit 0.

## Step 3 - install the runtime dependency

```bash
cd "$DEST/.." && bun add @opencode/plugin@2.0.32
```

If it hangs or fails partway, kill it, delete the partial state and retry once.

## Step 4 - merge the config snippet

```json
{
  "mcp": {
    "servers": {
      "repo-tasks": {
        "type": "local",
        "command": ["<DEST>/bin/repo-tasks-cc-mcp", "serve", "--host", "opencode"],
        "disabled": false
      }
    }
  }
}
```

Add the `repo-tasks` entry under the existing `mcp.servers` (creating it if absent) in `$DEST/opencode.json` or
`$DEST/opencode.jsonc` (create `opencode.jsonc` if neither exists). Write `command[0]` as an ABSOLUTE path: replace
`<DEST>` with the absolute config dir. A relative `./bin/...` does not resolve against the config dir. Keep `serve`,
`--host`, `opencode` exactly. On Windows name the `.exe`. Do not paste the snippet over the user's config.

The reminder in `plugin.ts` looks for the binary at `$DEST/bin/repo-tasks-cc-mcp`. To use another location, set the
environment variable `REPO_TASKS_BIN` to its absolute path before starting OpenCode.

## Step 5 - verify

1. `$DEST/bin/repo-tasks-cc-mcp` and `$DEST/plugins/repo-tasks-mcp-opencode.ts` exist and are non-empty.
2. The merged config parses and the `repo-tasks` entry is under `mcp.servers`.
3. From the directory where you ran `bun add`:
   `bun -e "await import('$DEST/plugins/repo-tasks-mcp-opencode.ts')"` exits 0 with no output.
4. Restart OpenCode and confirm the `repo-tasks` server is connected (`/mcp`).

## Uninstall

Delete `$DEST/plugins/repo-tasks-mcp-opencode.ts` and `$DEST/bin/repo-tasks-cc-mcp`, and remove the `repo-tasks`
entry from `mcp.servers`. The task list lives in the repository and is not touched.
