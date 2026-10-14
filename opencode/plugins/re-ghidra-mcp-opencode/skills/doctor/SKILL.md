---
name: doctor
description: Diagnoses why the Ghidra MCP server will not start or will not attach to a project. Use when the user runs /re-ghidra-mcp-opencode:doctor, when a ghidra server tool call fails with GHIDRA_NOT_FOUND, PROJECT_NOT_FOUND, JDK_NOT_FOUND, PROJECT_LOCKED, WORKER_INCOMPATIBLE, WORKER_UNAVAILABLE or a boot timeout, when the plugin's tools are listed but every call fails with a configuration error, when the plugin's tools do not appear at all, or when the user asks why Ghidra will not connect, why the worker will not boot, or how to check their Ghidra setup.
argument-hint: "[verbose]"
---

# Diagnose a re-ghidra-mcp-opencode setup

Work through the five prerequisites in order and report which one is broken.
**Order matters** — each check is meaningless if the one before it failed, so
stop at the first hard failure rather than reporting five confusing findings
that all trace to one cause.

Report findings as you go. Do not fix anything without asking first: three of
these five involve the user's own Ghidra projects and installs.

If the invocation includes `verbose`, also print the resolved value of every
configuration key in this plugin's README table, not just the
ones that are wrong.

## Run the commands in a shell (PowerShell shown)

The commands below are PowerShell (Windows). On macOS/Linux translate them to
the POSIX shell — same checks, `/` paths. A shell failure is
indistinguishable in the report from a failed prerequisite, which is the exact
misdiagnosis this skill exists to prevent, so get the shell right first.

## 1. The plugin's binaries exist and the server is running

`opencode.jsonc` points at `<DEST>/bin/re-ghidra-cc-mcp` (`.exe` on Windows),
and that directory is not in git — a fresh copy has no binaries. OpenCode cannot
report a server it never managed to launch.

**If you can see the plugin's tools, the server started** — skip to check 2.
A missing or invalid configuration no longer stops it: the server comes up and
lists its tools regardless, and reports the problem on the first tool CALL as
GHIDRA_NOT_FOUND or PROJECT_NOT_FOUND with the missing setting named in the
message. So "tools visible, every call fails" is a CONFIGURATION fault, not a
startup one, and only a genuinely absent or unlaunchable binary produces no
tools at all.

Two different failures look identical from inside the session, and they have
different fixes, so separate them:

- **Never built.** Check whether the binaries are on disk. `<DEST>` is the
  absolute install dir from this plugin's INSTALL.md (project-local or global
  config dir) — it is not expanded by the host, so look for
  `bin/re-ghidra-cc-mcp(.exe)` under the installed plugin copy by path instead.
  If absent, the fix is `just build-re-ghidra-mcp-opencode`
  followed by a restart of OpenCode.
- **Built, but the server died on launch.** The binaries exist, yet no
  `ghidra` MCP tools are available in this session. A configuration
  problem can no longer put you here — see above — so this is a crashing or
  unloadable binary. Tell the user to
  run `/mcp` to see whether the server is registered but failing, and go to the
  log in the last section.

Either way the fix needs a **restart**: MCP servers and hooks are both loaded at
session start and cannot be hot-swapped. **Stop here** if this fails; everything
below describes a server that is not running.

## 2. Ghidra is installed, and the plugin can find it

The install root can come from the `GHIDRA_INSTALL_DIR` environment variable
**or** the `ghidra_install_dir` key in `.claude/re-ghidra-mcp-cc.local.md` in the
session directory (a fallback layer from the shared server binary, which this
plugin reuses), with the environment winning. Read both before concluding anything — reporting a hard
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

**If you search the disk for it, say how far you looked.** An unset variable
means "not configured", never "not installed", and a bounded search that comes
back empty proves only that the limit was too shallow. A real install sat at
`C:\Users\user\Development\Java\ghidra_12.1.2_PUBLIC` — five levels down, past a
`-Depth 3` sweep from `C:\` that duly reported nothing and read as *Ghidra is
missing*. Search where installs actually land, and report the bound with the
result:

```powershell
$roots = @("$env:USERPROFILE\Development", "$env:USERPROFILE", "$env:LOCALAPPDATA",
           "C:\Program Files", "C:\Tools", "C:\")
foreach ($r in $roots) {
    if (Test-Path $r) {
        Get-ChildItem $r -Filter "ghidra*" -Directory -Depth 5 -ErrorAction SilentlyContinue |
            Select-Object -ExpandProperty FullName
    }
}
```

Then ask the user rather than concluding: they know whether they have Ghidra,
and one question beats a wrong verdict.

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

Read the `GHIDRA_MCP_*` environment first, then `.claude/re-ghidra-mcp-cc.local.md`
in the session directory if it exists (fallback layer from the shared server
binary — the primary documented config on this host is the environment; see
this plugin's README table):

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

If nothing is set from either source, point the user at the configuration table in
the plugin's README and offer to set the environment for them.

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

### A stale lock is the normal cost of a session ending

Expect this rather than treating it as corruption. The worker JVM is held in a
Windows Job Object (a Unix process group elsewhere) precisely so it cannot
outlive its parent — and a session that ends, a plugin that is disabled, or a
`/reload-plugins` kills it outright. Ghidra never gets to release the lock, so
the next server to attach finds one and reports `PROJECT_LOCKED`.

Gather what can be gathered, then hand the decision over:

```powershell
Get-Content "$env:GHIDRA_MCP_PROJECT_DIR\$env:GHIDRA_MCP_PROJECT_NAME.lock"
Get-Process -Name java,javaw,ghidraRun,re-ghidra-cc-mcp -ErrorAction SilentlyContinue
```

The lock names a hostname and a timestamp. No live JVM, no live server, and a
timestamp matching a session the user remembers closing is a strong case — and
still only a case. Say which of those you checked, and let the user delete it.

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

**Check whose connection you are reading.** The log directory collects every
server that ran, including probes and other workspaces, and each connection
records the client that opened it:

```
INFO rmcp::service: Service initialized as server
     client_info: Implementation { name: "probe", version: "0" }
INFO rmcp::service: input stream terminated
INFO rmcp::service: serve finished quit_reason=Closed
```

That is a **healthy** session: a client connected, finished, and closed the
pipe. Read as evidence about a failing session it says the opposite of the
truth — and `quit_reason=Closed` invites exactly that misreading, since it looks
like the crash you came to find. Match `client_info` and the timestamp to the
session under diagnosis before drawing anything from a log line; a name like
`probe` is somebody testing the binary by hand, not the session that failed.

Two environmental causes worth naming if the log shows a boot that **hangs**
rather than fails: endpoint protection or a firewall blocking the worker's
loopback bind, and a project that was never fully auto-analyzed in the GUI.

A `WORKER_INCOMPATIBLE` error is a different thing entirely — it means the
extracted worker script does not match this binary, and its pinned advice is to
reinstall the plugin. Rebuild with `just build-re-ghidra-mcp-opencode` and restart.
