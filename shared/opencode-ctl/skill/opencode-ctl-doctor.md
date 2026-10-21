<!--
SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
Copyright the opencode-ctl authors. License: https://polyformproject.org/licenses/noncommercial/1.0.0
The canonical source-of-truth for the doctor skill lives at `shared/opencode-ctl/skill/opencode-ctl-doctor.md`.
This file is a TEMPLATE: a later task renders it per host by substituting the host placeholders and the
host-specific install section; the engine embeds this template and renders it per host at run time, and the
committed copies under each plugin's `skills/` are generated from that output; regenerate that plugin's copy from the shipped binary with
`emit-skill opencode-ctl-doctor --host <cc|qwen|agy|opencode>` so it can never drift. Edit ONLY that canonical
file - if the copy you are viewing is under a plugin/CLI `skills/` dir, it is generated; do NOT edit it, your
changes will be overwritten on the next emit.
-->
---
name: opencode-ctl-doctor
description: Diagnoses why opencode-ctl-mcp tools fail or do not appear. Use when the user runs {{INVOCATION}}, when a tool returns OPENCODE_NOT_FOUND, SERVICE_DOWN, AUTH_FAILED, UNKNOWN_OPERATION or REFUSED_ON_HOST, when opencode-pair is missing or stale{{INSTALL_CLAUSE}}
argument-hint: "{{ARGUMENT_HINT}}"
allowed-tools: {{ALLOWED_TOOLS}}
---

Run `{{BINARY}} doctor-json{{HOST_FLAG}}`, and append ` verbose` when the user asked for more detail
(`doctor-json` accepts `verbose` and `--host <h>` in either order). Report each check below in this order, and
treat a `skip "server_info failed"` as a check that could not run: tell the user to re-run the doctor once
`server_info` is `ok`.

1. **Binary.** That the `opencode` binary was found, and that its version is >= 2.0. Warn when
   `server.info.version` is outside the tested `2.0.x` range, since `session.diff` turn semantics and the
   experimental wait route are version behaviours.
2. **Registration.** That the service registration file is present, that its pid is alive, and whether the
   service config's `disabled` flag selects a standalone server.
3. **`server.info`.** That `opencode api server.info` answers; when the doctor had to start the service to
   get an answer, say so, and when it could not, say that.
4. **Wait route.** That the operation table contains `experimental.session.wait`.
5. **Pairing.** That any pairing resolves to a live session and that its record generation has a sentinel;
   show both liveness values with their meaning, `starting`, `unknown` and `none` included.
6. **Lock and policy source.** Which source set `permissions_locked`, and which set `permission_policy`.
7. **Protected keys.** Every protected key whose env value differs from the user-level file.
8. **Windows leftovers.** On the Windows copy, the `.source` file and any `.old` leftovers.
9. **Launcher link.** That the `opencode-pair` link resolves to the running binary; show the `--force` hint
   when the link is stale.
10. **`server_url` password.** A `server_url` without `OPENCODE_SERVER_PASSWORD`.
11. **`OPENCODE_PASSWORD`.** Whether a `OPENCODE_PASSWORD` is set; it precedes the server password.
12. **`OPENCODE_CTL_HOME`.** Whether it is set; when it is set, report it as the FIRST line.
13. **Log.** The newest log file and its last error.
14. **Orphans.** Pairs whose controller side is `dead` (not `none`) while the TUI side is `alive`, and
    sessions in `session.active` whose pair has no living side (orphaned workers).
15. **Ignored or clamped settings.** Any setting that was ignored or clamped.
16. **Resolved settings.** Print the resolved settings when invoked with `verbose`.

{{INSTALL_SECTION}}
