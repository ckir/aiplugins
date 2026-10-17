# Installing todov2-opencode for OpenCode (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install
> this plugin for them. Follow this document top to bottom, in order. Do not
> skip the verification step. If any step fails, stop and tell the user
> exactly which step failed and what you saw — do not improvise workarounds
> that change what gets installed.
>
> Everything below comes from versioned release assets — no branch files,
> no unpinned URLs. Agree the version with the user first (`0.3.0` is
> current as of this writing; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use it
> for every URL below.

You will install `todov2-opencode`: the `todowrite` session tool that V2
removed, plus an optional live todo list in the session sidebar. Install is
a file copy plus two config entries — no registry, no build of the
TypeScript itself (opencode transpiles it at runtime). Tested against
`@opencode/plugin` **2.0.26**; it targets OpenCode **V2** (V1 is not
supported by this plugin).

This document is the executable form of `README.md`'s Installing section.
If the two ever disagree, this file wins for install mechanics and the
mismatch is a bug — tell the user.

## Step 0 — prerequisites

Confirm each one; stop and report if one fails:

1. OpenCode V2: `opencode --version` prints a `v2.x` version.
   (V1 is not supported by this plugin.)
2. Download tools: `curl` and `unzip` (Git Bash on Windows has both;
   PowerShell alternative: `Invoke-WebRequest` and `Expand-Archive`).
3. `bun` for one dependency-install step (`bun --version`). OpenCode
   itself runs on Bun; any recent Bun works here.

## Step 1 — ask where to install

Ask the user: **project-local** (this project only) or **global** (every
project)? Then set `DEST` accordingly and use it for every path below:

- Project-local: `DEST` = `<project root>/.opencode`
- Global: `DEST` = `~/.config/opencode`

Do not proceed on an assumed default. Use the **plural `plugins`** key in
the V2 config (`$DEST/opencode.json` or `$DEST/opencode.jsonc` — whichever
exists; create `opencode.jsonc` if neither does). The singular `plugin`
key is legacy and will **not** load this plugin.

## Step 2 — fetch the plugin bundle

Set the agreed version once:

```bash
VERSION=0.3.0
```

Download the versioned plugin bundle — asset
`todov2-opencode-opencode.zip` on release
`todov2-opencode-v$VERSION` (the doubled name is `<package>-opencode.zip`
for package `todov2-opencode`) — and extract it as a directory (unlike the
single-file plugins, this one ships several TypeScript files that import
each other, so the whole folder must stay together):

```bash
mkdir -p "$DEST/plugins"
curl -fsSL -o /tmp/todov2-opencode.zip \
  "https://github.com/ckir/aiplugins/releases/download/todov2-opencode-v$VERSION/todov2-opencode-opencode.zip"
rm -rf "$DEST/plugins/todov2-opencode"
mkdir -p /tmp/todov2-opencode-stage
unzip -q -o /tmp/todov2-opencode.zip -d /tmp/todov2-opencode-stage
cp -r /tmp/todov2-opencode-stage "$DEST/plugins/todov2-opencode"
```

The zip's top level IS the plugin directory (`plugin.ts`, `tui.tsx`,
`tsconfig.json`, `tool/`, `package.json`, `opencode.jsonc` — no wrapper folder), so the
copy lands the files exactly where the config entries in Step 4 point.
Every download and copy must exit 0; verify afterwards that
`$DEST/plugins/todov2-opencode/plugin.ts`,
`$DEST/plugins/todov2-opencode/tui.tsx`,
`$DEST/plugins/todov2-opencode/tsconfig.json`, and
`$DEST/plugins/todov2-opencode/tool/index.ts` all exist and are non-empty
(`wc -c` on each).

There is no MCP server binary and nothing to merge from `opencode.jsonc`
(the shipped one carries no servers, permissions, or skills) — skip both.
The next step is the runtime dependency.

## Step 3 — install the plugin runtime dependency

`plugin.ts` imports `@opencode/plugin` as a value (`Plugin.define`), so it
must resolve from the plugin directory. Install it there now rather than
relying on a later restart:

```bash
cd "$DEST/plugins/todov2-opencode" && bun install
```

Verify `node_modules/@opencode/plugin/package.json` exists in that
directory afterwards. If `bun install` hangs (no output for minutes) or
fails partway: kill it, delete the partial state in that directory
(`node_modules/`, `bun.lock`), and retry once — an interrupted first
attempt poisons the second. If it still fails, the problem is registry
access (proxy, offline mirror), not this plugin: OpenCode itself runs
`bun install` at startup and will hit the same wall, so fix network
access first and then retry.

## Step 4 — register the plugin entries

Add two absolute-path entries to the user's config under the top-level
`plugins` list (creating the list if absent). JSON has no variables —
expand `$DEST` yourself now:

```json
{
  "plugins": [
    "<DEST>/plugins/todov2-opencode/plugin.ts",
    "<DEST>/plugins/todov2-opencode/tui.tsx"
  ]
}
```

The first entry is the server-side `todowrite` tool (required). The second
is the optional TUI sidebar list — omit it to run tool-only. Do not paste
either path over the user's existing `plugins` list; append. Confirm the
merged config parses (`jq -e .` on the file exits 0).

Optional behavior flags (defaults shown — omit the block to accept them):

```json
{
  "plugins": [
    {
      "package": "<DEST>/plugins/todov2-opencode/plugin.ts",
      "options": { "enabled": true, "injectEveryRound": true }
    }
  ]
}
```

`enabled: false` disables everything; `injectEveryRound: false` keeps the
tool but skips the per-round system-prompt injection. (The object form
above replaces the plain path entry for `plugin.ts` when you need flags;
`tui.tsx` stays a plain path.)

## Step 5 — verify the install

All three, in order:

1. Every destination file exists and is non-empty (the directory Step 2
   copied plus `node_modules/@opencode/plugin` from Step 3).
2. The plugin modules load: from `$DEST/plugins/todov2-opencode` (so
   `node_modules` resolves),
   `bun -e "await import('./plugin.ts')"` exits 0 with no output, and
   `bun -e "await import('./tui.tsx')"` exits 0 with no output.
   Importing only defines the plugin — nothing executes until the host
   loads it.
3. Restart OpenCode (config and plugins load at startup), then confirm:
   - the plugin health check lists `"id": "todov2-opencode"` with
     `status: "active"` (see `README.md`'s health-check section for the
     exact request);
   - asking the agent to track work with `todowrite` invokes the tool and
     the list persists across rounds in the session;
   - (if `tui.tsx` was registered) the session sidebar shows the Todos
     list. If the sidebar stays empty while the tool itself works, the
     TUI half did not attach on this OpenCode build — the tool half is
     still a complete install; report it as a partial and keep going.
