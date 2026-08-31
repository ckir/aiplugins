---
name: doctor
description: Diagnoses why the Ghidra MCP server will not start or will not attach to a project. Use when the user runs /re-ghidra-mcp-cc:doctor, when a re-ghidra-mcp-cc tool call fails with GHIDRA_NOT_FOUND, JDK_NOT_FOUND, PROJECT_LOCKED, WORKER_INCOMPATIBLE, WORKER_UNAVAILABLE or a boot timeout, when the plugin's tools do not appear at all, or when the user asks why Ghidra will not connect, why the worker will not boot, or how to check their Ghidra setup.
argument-hint: "[verbose]"
allowed-tools: Bash, Read, Write
---

# Diagnose a re-ghidra-mcp-cc setup

Work through the five prerequisites in order and report which one is broken.
**Order matters** — each check is meaningless if the one before it failed, so
stop at the first hard failure rather than reporting five confusing findings
that all trace to one cause.

Report findings as you go. Do not fix anything without asking first: three of
these five involve the user's own Ghidra projects and installs.

If the invocation includes `verbose`, also print the resolved value of every
configuration key in `examples/re-ghidra-mcp-cc.local.md`'s table, not just the
ones that are wrong.

## Run the commands in PowerShell

This plugin runs on Windows only, and the paths in these variables are
backslash paths. Use `pwsh` for every command below. A POSIX shell here fails
for shell reasons — path translation, glob semantics — and a shell failure is
indistinguishable in the report from a failed prerequisite, which is the exact
misdiagnosis this skill exists to prevent.

## 1. The plugin's binaries exist and the server is running

`.mcp.json` points at `${CLAUDE_PLUGIN_ROOT}/bin/re-ghidra-cc-mcp`, and that
directory is gitignored — a fresh clone has no binaries. Claude Code cannot
report a server it never managed to launch.

Two different failures look identical from inside the session, and they have
different fixes, so separate them:

- **Never built.** Check whether the binaries are on disk. `${CLAUDE_PLUGIN_ROOT}`
  is expanded by Claude Code in config files but **not** in a shell command, so
  it will be empty in Bash — look for `bin/re-ghidra-cc-mcp.exe` under the
  plugin directory by path instead. If absent, the fix is `just build-re-ghidra-mcp-cc`
  followed by a restart of Claude Code.
- **Built, but the server died on launch.** The binaries exist, yet no
  `re-ghidra-mcp-cc` MCP tools are available in this session. Tell the user to
  run `/mcp` to see whether the server is registered but failing, and go to the
  log in the last section.

Either way the fix needs a **restart**: MCP servers and hooks are both loaded at
session start and cannot be hot-swapped. **Stop here** if this fails; everything
below describes a server that is not running.

## 2. Ghidra is installed, and the plugin can find it

The install root can come from either `GHIDRA_INSTALL_DIR` **or** the
`ghidra_install_dir` key in `.claude/re-ghidra-mcp-cc.local.md`, with the
environment winning. Read both before concluding anything — reporting a hard
failure here when the settings file supplied a valid path would stop the
diagnosis three checks early.

```powershell
$env:GHIDRA_INSTALL_DIR
Get-ChildItem "$env:GHIDRA_INSTALL_DIR\support\analyzeHeadless*" -ErrorAction SilentlyContinue
```

The value must name the Ghidra **install root** — the directory containing
`support\`, `Ghidra\` and `ghidraRun`. Pointing it at `support\` itself, or at a
project directory, is the usual mistake.

If `analyzeHeadless` is missing, the directory is not a Ghidra install root. If
neither source supplies a value, setting `GHIDRA_INSTALL_DIR` machine-wide is
usually right, since it is the same for every project on the machine.

**Version:** this plugin targets Ghidra **12.1.2**.

```powershell
Select-String '^application.version' "$env:GHIDRA_INSTALL_DIR\Ghidra\application.properties"
```

Treat a different version as a **warning, not a hard failure** — report it and
keep going. Only a missing `analyzeHeadless` stops the run here. A version skew
is worth naming because it is a plausible cause of a later boot failure, but it
is not proof of one.

## 3. A JDK 21 or newer is available

```powershell
java -version
```

`java -version` writes to **stderr**, not stdout — capture both if you pipe it.
Ghidra 12.1.2 declares `application.java.min=21`; an older JDK fails at JVM
start with a message that does not mention the version, which is why this check
exists. Both `21.0.x` and a bare `25` are fine; `1.8.0_x` is Java 8 and is not.

## 4. The project is configured

Read `.claude/re-ghidra-mcp-cc.local.md` in the project root if it exists, and
check the environment:

```powershell
Get-ChildItem Env: | Where-Object Name -like 'GHIDRA_MCP_*'
```

Three values must resolve, from either source:

| Value | Settings key | Environment variable |
|---|---|---|
| Directory holding the `.gpr`/`.rep` | `project_dir` | `GHIDRA_MCP_PROJECT_DIR` |
| Ghidra project name | `project_name` | `GHIDRA_MCP_PROJECT_NAME` |
| A bare program filename in the project | `bootstrap_program` | `GHIDRA_MCP_BOOTSTRAP_PROGRAM` |

The environment wins over the file — but an **empty** variable counts as unset
and falls through to the file. `GHIDRA_MCP_PROJECT_NAME=` is not a configured
empty name; do not report it as one.

If nothing is set from either source, point the user at the template in the
plugin's `examples/` directory and offer to write
`.claude/re-ghidra-mcp-cc.local.md` for them.

Then confirm the project actually exists where it claims to:

```powershell
Get-ChildItem "$env:GHIDRA_MCP_PROJECT_DIR\*.gpr", "$env:GHIDRA_MCP_PROJECT_DIR\*.rep" -ErrorAction SilentlyContinue
```

`project_dir` is the directory **containing** the `.gpr` and `.rep` — not the
`.rep` itself. `project_name` is the `.gpr` filename without its extension.

## 5. Nothing else holds the project

One Ghidra project supports **one** live server, with the GUI closed. Two
servers, or one server and an open GUI, collide on Ghidra's lock file. This is
the `PROJECT_LOCKED` error code, whose pinned advice is "close this project in
the Ghidra GUI, then retry".

The lock sits **beside** the `.gpr`/`.rep`, in the project directory — not
inside the `.rep`. The trailing `*` matters: Ghidra also writes a `.lock~`.

```powershell
Get-ChildItem "$env:GHIDRA_MCP_PROJECT_DIR\$env:GHIDRA_MCP_PROJECT_NAME.lock*" -ErrorAction SilentlyContinue
```

No output means the project is free.

A lock file present usually means the Ghidra GUI still has the project open, or
another workspace is running its own server against it. Ask the user to close
the GUI.

**Never delete a lock file on the user's behalf.** Deleting a live one corrupts
the project. A lock can also be genuinely stale after a crash — but it carries
no PID and no owner token, so *nothing* can prove it stale, which is exactly why
the server itself never parses, reclaims, or deletes it either. If the user
confirms nothing is holding it, removing it is their call to make, not yours.

## Reporting

Finish with a short verdict, not a transcript:

- **Everything passed** — say so in one line, and add that the first tool call
  after a cold start returns `WORKER_WARMING` while the JVM boots. That is
  normal; wait at least 10 seconds rather than retrying in a tight loop.
- **Something failed** — name the single check that failed, what the value
  actually was, and the one action that fixes it. Mention the later checks you
  skipped so the user knows the report is not exhaustive.

## When the setup is fine but tools still fail

Point the user at the worker log, which records what the JVM did. It lives under
`%USERPROFILE%\.ghidra-mcp\logs\` (or under `GHIDRA_MCP_HOME` if they relocated
the data directory). The files rotate daily with five kept, so the name carries
a date suffix — `worker-<pid>.log.<YYYY-MM-DD>`, not a bare `worker-<pid>.log`:

```powershell
Get-ChildItem "$env:USERPROFILE\.ghidra-mcp\logs\" | Sort-Object LastWriteTime -Descending | Select-Object -First 5
```

Log verbosity is fixed — there is no `RUST_LOG` for it.

Two environmental causes worth naming if the log shows a boot that **hangs**
rather than fails: endpoint protection or a firewall blocking the worker's
loopback bind, and a project that was never fully auto-analyzed in the GUI.

A `WORKER_INCOMPATIBLE` error is a different thing entirely — it means the
extracted worker script does not match this binary, and its pinned advice is to
reinstall the plugin. Rebuild with `just build-re-ghidra-mcp-cc` and restart.
