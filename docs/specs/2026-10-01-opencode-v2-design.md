# Spec: OpenCode V2 support — `opencode/` marketplace dir + port of both plugins

**Status:** SPEC, brainstormed 2026-10-01; unimplemented. All five design
sections were presented and approved in chat before writing. Approach A
(mirror the agent dirs 1:1) was chosen over a shared TS library (B) and
npm publishing (C, deferred to a later phase).

**Date:** 2026-10-01
**Scope:** both real plugins (`rtk-mcp`, `re-ghidra-mcp`); the `example`
scaffold is out of scope. V2-native API only — no V1 compatibility shim.
Thin TypeScript over existing binaries — no new Rust crates.

---

## 1. Goal

Add OpenCode V2 support to this marketplace at parity with the three
agents already served (`claude-code/`, `antigravity/`, `qwen/`):

- **A. Two ported plugins** under a new `opencode/` dir, implemented as
  V2-native TypeScript (`Plugin.define` + `setup(ctx)`, `@opencode/plugin`),
  delegating to the binaries this workspace already builds.
- **B. A "marketplace" without a registry.** OpenCode has no official
  marketplace/registry (upstream tracks the desire as a master issue only),
  so the repo itself is the catalog: the two `opencode/*/` dirs plus the
  root README's agent table. No manifest file, no npm publish in this phase.
- **C. Full repo-gate parity.** Wiring, staleness, footprint, smoke, and
  `just check` / CI integration — the same gates that keep the Claude and
  Qwen fronts honest, extended to `opencode/`.

**Non-goals:** the `example` scaffold port; npm packages; auto-update,
provenance, or search (everything a real registry would do); V1
(`@opencode-ai/plugin`) support; any rewrite of `shared/` engines.

## 2. Layout

```text
opencode/
├── rtk-mcp-opencode/
│   ├── plugin.ts            # default export Plugin.define({ id, setup })
│   ├── opencode.jsonc       # example snippet: plugins + mcp.servers + permissions
│   ├── skills/
│   │   ├── rtk-policy/SKILL.md
│   │   └── gain/SKILL.md    # verbatim copies of the Claude originals
│   ├── README.md            # copy-based install (global + project-local)
│   └── tests/               # Bun hook unit tests (fail-open matrix)
└── re-ghidra-mcp-opencode/
    ├── plugin.ts
    ├── opencode.jsonc       # mcp.servers → existing re-ghidra MCP binary
    ├── skills/
    │   ├── ghidra-re-driver/SKILL.md  # third emitted copy of the canonical skill
    │   └── doctor/SKILL.md             # verbatim copy of the Claude original
    ├── agents/re-analyst.md   # port of claude-code/re-ghidra-mcp-cc/agents/
    ├── README.md
    └── tests/
```

Component dirs sit at the plugin root (like Claude's `skills/`, `hooks/`
at root, not inside a manifest dir). Install is a documented copy — no
build step for the TS itself. Binaries stay where cargo-dist already
builds them; `mcp.servers` entries point at `bin/`-staged paths the same
way `.mcp.json` does today. No new `shared/` TS crate (YAGNI: Approach B
rejected until duplication is proven).

## 3. `rtk-mcp-opencode` behavior

- **Entrypoint:** `plugin.ts` default-exports `Plugin.define({
  id: "rtk-mcp-opencode", setup(ctx) })`, importing from
  `@opencode/plugin`. No V1 export.
- **Rewrite hook:** `ctx.tool.hook("execute.before", …)` on the shell
  tool. It pipes the incoming command through `$RTK_BIN hook claude`
  over stdin (the exact envelope `rtk` already speaks) and, on a valid
  verdict, replaces the mutable `event.input` command in place. Anything
  else — missing binary, non-zero exit, non-JSON, empty output, or
  `RTK_OPENCODE_DISABLE=1` — leaves the event untouched and silent
  (fail-open, same contract as the `-cc` hook; hook stderr ignored).
- **Env:** `RTK_BIN` shared with the other agents;
  `RTK_OPENCODE_DISABLE / _ULTRA_COMPACT / _SKIP_ENV` mirror the
  `RTK_CC_*` set under a new prefix so Claude + OpenCode installs on one
  machine don't cross-talk.
- **MCP:** `opencode.jsonc → mcp.servers.rtk` reuses the existing
  `rtk-cc-mcp` binary as `type: "local"`, exposing the same four tools
  (`rtk_gain`, `rtk_discover`, `rtk_check`, `rtk_proxy`). V2 shape:
  `disabled: false`, split `timeout: { catalog, execution }`.
- **Skills/permissions:** `rtk-policy` + `gain` SKILL.md copies verbatim
  (OpenCode skill frontmatter `name`/`description` is already compatible);
  the `permissions` snippet allows the shell action the hook rewrites (V2
  action name `shell`, not `bash`) and leaves the rest to the user.
- **Load:** a copied `plugin.ts` in `.opencode/plugins/` auto-loads — no
  `plugins.package` entry needed for the standard path. The `package:`
  file-URL form is documented only for working-copy dev.
- **Risk:** the exact shell tool id/args field OpenCode V2 exposes
  (`bash` vs `shell`, `command` vs alternatives) must be pinned by a spike
  test before finalizing the mutation line. If `execute.before` cannot
  mutate shell input, the design falls back to the Antigravity-style MCP
  proxy — that fallback is NOT approved here and would need re-brainstorming.

## 4. `re-ghidra-mcp-opencode` behavior

- **Entrypoint:** `plugin.ts` as `Plugin.define({
  id: "re-ghidra-mcp-opencode", setup(ctx) })`. It owns no RE logic; the
  engine stays in `shared/ghidra-mcp` behind the existing MCP binary.
- **MCP:** `mcp.servers.ghidra` (`type: "local"`) points at the
  already-built `re-ghidra-cc-mcp` binary — same 19 tools, same env
  passthrough (Ghidra home, project path, worker controls). V2 shape only.
- **Session hooks:** `ctx.session.hook("compaction", …)` injects the
  durable RE state that must survive summarization (target program,
  analysis freshness, pending rename/comment writes) — the V2 successor to
  the `-cc` hook binary's lifecycle role. A `context` hook adds the driver
  pointer only when Ghidra tools are in play, so idle sessions pay
  nothing. Registrations are plugin-scoped and dispose on unload; durable
  plugin state goes in `ctx.storage`, not sidecar files.
- **Skills/agents:** `skills/ghidra-re-driver/SKILL.md` becomes a third
  emitted copy of the canonical `shared/ghidra-mcp/skill/SKILL.md`
  (`just emit-ghidra-skill` extended, same emit test guards it — never a
  forked edit). `skills/doctor/SKILL.md` copies verbatim (diagnostics
  skill, no canonical source). `agents/` ports
  `claude-code/re-ghidra-mcp-cc/agents/re-analyst.md` to
  `.opencode/agents/` naming, adding `mode: primary` only where the source
  was a mode, joining `model#variant`, `permission` → `permissions`;
  bodies unchanged.
- **Fail behavior:** MCP spawn failure or missing Ghidra surfaces as tool
  errors, never a session block; the plugin itself always loads.

## 5. Distribution ("marketplace" without a registry)

- **No new manifest.** No root `marketplace.json`, no npm publish. The
  two `opencode/*/` dirs plus the root README's agent table are the
  listing. Stays compatible-by-construction with the community
  `opencode-market` CLI later without committing to its schema now.
- **Install = copy, documented per plugin.** Each README gives both
  destinations: global (`~/.config/opencode/plugins/rtk-mcp-opencode.ts`
  resp. `re-ghidra-mcp-opencode.ts`, `~/.config/opencode/skills/…`, merge into
  `~/.config/opencode/opencode.jsonc`) and project-local
  (`.opencode/plugins/`, `.opencode/skills/`,
  `.opencode/opencode.jsonc`). Skills/agents auto-discover once copied;
  the TS file auto-loads with no `plugins.package` entry.
- **Root README** gains OpenCode in Supported Agents + per-plugin install
  pointers, mirroring the Claude/Qwen rows. IDs are stable
  (`rtk-mcp-opencode`, `re-ghidra-mcp-opencode`); each README pins the
  tested OpenCode release + `@opencode/plugin` version, since the V2 API
  is explicitly beta and will churn.

## 6. Gates and verification

New checks mirror the existing ones, extended — not reinvented:

- **Wiring** (`scripts/check-opencode-wiring.sh`, in `just check`):
  plugin `id` matches dir; `opencode.jsonc` parses as JSONC with V2-native
  shapes only (`plugins`, `mcp.servers`, `permissions[]`, `skills[]`);
  every `mcp.servers.*.command` resolves to a binary this workspace
  builds; every `SKILL.md` frontmatter `name` equals its dir; agent
  frontmatter uses native V2 keys.
- **Staleness:** skill copies must be byte-identical to their sources
  (rtk skills vs Claude originals; Ghidra skill vs canonical via the
  extended emit test) — the same "copies go stale silently" reasoning
  behind `check-qwen-marketplace.sh`); the re-ghidra `doctor` skill is a
  verbatim copy checked the same way as the rtk skills.
- **Footprint:** extend `tools/plugin-footprint` + `just
  footprint-regen` to measure `opencode/*/` the same way (MCP schemas +
  skill/agent frontmatter bytes, published into root + per-plugin
  READMEs). TS plugin source bytes reported separately as a third tier,
  since V2 loads it in-process — comparable, not conflated.
- **Smoke/test:** `bun test` per plugin for the fail-open matrix
  (missing/non-zero/non-JSON/empty/disabled, mock-rtk-via-`PATH`
  pattern); smoke assembles a temp HOME + fixture project, copies the
  plugin in, and asserts the id appears (`opencode2 api get /api/plugin`),
  the rewrite fires through mock `rtk`, and MCP tools list. Portable
  `node:child_process` spawning only — no Bun `$` — so one harness covers
  Win/Linux/mac.
- **CI:** `just check` gains the opencode recipes; CI runs them in the
  same matrix. V2 API beta churn is handled by pinning the tested OpenCode
  + `@opencode/plugin` versions in each README and re-running smoke on
  bump.

## 7. Sequencing for the plan

1. Spike: pin the V2 shell tool id/args shape and confirm
   `execute.before` mutation rewrites the command (§3 risk). If it
   cannot, stop — the fallback needs its own design.
2. Scaffold `opencode/rtk-mcp-opencode/` (plugin.ts, jsonc, skill copies,
   README, bun tests).
3. Scaffold `opencode/re-ghidra-mcp-opencode/` (plugin.ts, jsonc, agent
   ports, emit third copy, README).
4. Wiring script + `just` recipes + root README rows.
5. Footprint extension + regen + budgets.
6. Smoke harness + CI wiring.

## 8. Open serving notes (not design forks)

- `rtk hook claude` is reused as-is; a future upstream `rtk hook
  opencode` mode can be adopted with a one-line change and stays
  fail-open either way.
- Approach C (npm packages installable via `"plugins": […]`) layers onto
  this spec without rework: the TS sources are already package-shaped.
