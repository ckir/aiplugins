# Spec: `plugin-footprint` — static token cost of a shipped plugin

**Status:** SPEC, reviewed and decided; unimplemented. All eight design forks
are settled (§8), so a line-level plan is now writable — but only against code
that exists, which means the prerequisite in §4.1.1 lands first.

**Date:** 2026-09-03
**Author:** drafted with Claude Opus 5
**Review:** adversarial panel, 4 rounds + 2 bounded consults, all folded
2026-09-03 (see §11). Not carried to a GREEN round: the panel was stopped by
the owner once round 4 changed §4's central contract, on the grounds that
further rounds would be reviewing a design about to change shape.

**Sequencing — read this before planning.** §4.1.1 is a prerequisite that lives
in a DIFFERENT crate: `shared/ghidra-mcp` must defer config resolution so
`serve` starts unconditionally. Until that merges, the gate cannot measure
`re-ghidra-mcp-cc` at all, and any line-level plan for this tool would be
written against a probe that does not work. Order: (1) defer config resolution
in `shared/ghidra-mcp`; (2) scaffold `tools/plugin-footprint`; (3) the gate;
(4) the published figure.

---

## 1. Goal

Measure, gate, and publish the token cost that installing a plugin from this
marketplace imposes on a user's context window.

Two deliverables, one measurement:

- **A. Maintainer gate.** CI fails when a plugin's always-on footprint grows
  past its budget, so the repo cannot ship *new* bloat by accident.
- **B. Installer disclosure.** Each plugin README, and the root marketplace
  README, publish the footprint so a prospective user sees the cost *before*
  they install.

**What A does not do.** Initial budgets are seeded from today's measured values
(§6), so the gate ratifies the cost that already ships and objects only to
growth. It is a ratchet, not an audit. Reducing existing footprint is a separate
piece of work and no gate here will ask for it.

## 2. Why — the measured case

Driven live against the built binaries on 2026-09-03 (`tools/list` over a real
MCP stdio handshake, not source parsing):

| Plugin | MCP tools | Schema bytes | Resident (est. tokens) | Invocation (est. tokens) |
|---|---|---|---|---|
| `rtk-mcp-cc` | 4 | 2 831 | ~1 000 | ~1 920 |
| `re-ghidra-mcp-cc` | 19 | 18 379 | ~5 180 | ~8 460 |

Largest single tool schema: `set_prototype` at 2 293 B — one tool costing most
of what the entire `rtk-mcp-cc` plugin costs.

> **Superseded by §4.5.1.** This table counts only the MCP half, by hand, before
> `canonical_json` pinned the serialisation. §4.5.1 carries the generated figures
> for both tiers — and the largest resident source turns out not to be a tool
> schema at all.

Installing `re-ghidra-mcp-cc` therefore spends roughly 5k tokens of every
context window, on every request, before the user types anything. Nothing in
this repo measures that today, and no plugin marketplace publishes it.

**The measured set is `.claude-plugin/marketplace.json`'s `plugins[]` array**,
not every directory under `claude-code/`. That is the same iteration source
`just smoke` and the `bundle-smoke` CI job already use, and it is why
`claude-code/example` is absent from the table above: it is a demo crate, not a
published plugin. A tool that measured it would gate something nobody installs.

## 3. The cost model

A single blended "cost" number would be dishonest, and would make the published
figure untrustworthy. Footprint is reported in **three tiers**, always
separately:

| Tier | Paid | Sources |
|---|---|---|
| **Resident** | every request, for the whole session | MCP `tools/list` schemas; MCP `prompts/list` entries (§4.1); skill frontmatter (name + description, used for discovery); agent context files (`CLAUDE.md`, and Qwen's `contextFileName`); command and agent descriptions |
| **Invocation** | only when the thing is triggered | `SKILL.md` bodies; command bodies; hook stdout |
| **Session** | once per session | `SessionStart` hook output |

**The installer-facing headline is Resident**, because it is the tax a user
cannot avoid. Publishing a merged figure would hide exactly the distinction
that should drive the install decision.

### 3.1 Report it as context budget, not money

With prompt caching a stable tool schema becomes a `cache_read` (~0.1x) after
the first request, so the dollar impact is far smaller than the token count
implies. The **context-window** impact is full price, always. All published
figures are framed as context budget consumed. Cost in currency is out of
scope (see §9).

## 4. Measurement contracts

### 4.1 MCP payloads come from a live probe, never from source

The measured payload MUST be obtained by launching the server and issuing the
MCP handshake:

1. `initialize`
2. `notifications/initialized`
3. `tools/list`, **following `result.nextCursor` until it is absent**, and
   measuring the concatenation of every page's `result.tools`
4. `prompts/list`, same pagination rule

Rationale: source parsing would miss drift from `rmcp` macro changes,
`schemars` output changes, and doc-comment edits — all of which change what
actually ships. The probe is the only measurement that cannot silently
disagree with reality.

**Pagination is not optional.** A single-page read of a server that paginates
undercounts, and an undercount presents as a footprint *improvement* — the gate
would reward the defect. The prober MUST follow cursors even if today's servers
return one page.

**Pagination is also bounded.** Cursor-following is an unbounded loop driven by
the thing being measured: a server returning a circular cursor, or large pages
without end, exhausts memory long before §4.4's wall-clock budget fires. The
prober caps a list method at **64 pages and 8 MiB of accumulated response
bytes**; exceeding either is `failed` (§4.4), never a partial measurement. A
truncated page set would understate the footprint, which is the same
wrong-direction error pagination-following exists to prevent. The failure names
**which** limit tripped and the method it tripped on, because "cap exceeded" and
"the server crashed" both arrive as `failed` and want opposite responses from
the reader.

**`prompts/list` is measured because Claude Code surfaces MCP prompts as slash
commands**, which sit alongside the file-declared commands §3 already counts.
`resources/list` is explicitly EXCLUDED: resources are fetched on demand, not
enumerated into every request. If that changes, this exclusion is what needs
revisiting.

**A method the server does not implement is not an error.** A server answering
`prompts/list` with a method-not-found error contributes zero prompt tokens and
the probe continues. Distinguish that from a failed probe (§4.4).

**Verified:** the `tools/list` half works today against both
`bin/rtk-cc-mcp.exe` and `bin/re-ghidra-cc-mcp.exe` — **on a developer machine
with Ghidra installed.** That qualifier is load-bearing; see §4.1.1.

### 4.1.1 The probe must not require a working backend — LANDED 2026-09-03

This was the spec's most serious defect, found in round 4: it invalidated §4.1
as written for the plugin the whole spec was motivated by.

**Fixed and committed** on branch `feat/ghidra-defer-config`, six commits from
`3384b58` to `456c1c4`, reviewed to a clean round under AGY-CAPSTONE (five
rounds; severity fell from production defects to test hygiene, and the final
round's one named uncertainty was settled by mutation rather than accepted).
Not merged; no PR. Measured
before and after from a clean working directory with every `GHIDRA_*` variable
cleared, driving a real MCP handshake against the built binary:

| | `tools/list` | schema bytes | exit |
|---|---|---|---|
| before | no response | — | 2 |
| after | 19 tools | 18 588 | 0 |

Note the 18 588 against §2's recorded 18 379: the two probes serialise
differently, which is exactly why §5 pins the serialisation. Do not treat either
number as the published figure until an oracle produces it under that pinned
form.

**Two places needed the short-circuit, not one** — the correction recorded in
`## Stand-downs` was what surfaced the second. `server::serve` kicked
`start_warmup()` unconditionally, and `execute::wait_until_ready` kicks a
single-flight boot whenever the slot is `Empty`. Guarding only the first would
have left an unconfigured server spinning the boot loop — fail, reset to
`Empty`, retry — until `warming_deadline` expired and answered `WORKER_WARMING`,
a timeout saying nothing about the real fault. Both now decline up front, pinned
by `boot_count() == 0` assertions in
`shared/ghidra-mcp/tests/config_deferral.rs`.

The description of the original defect follows, kept because the gate depends on
understanding why the probe contract is shaped this way.

`re-ghidra-cc-mcp serve` cannot answer `tools/list` without a complete, valid
Ghidra configuration. Measured:

- `shared/ghidra-mcp/src/config.rs:56-88` — `resolve()` returns
  `ConfigError::Missing` unless `ghidra_install_dir`, `project_dir`,
  `project_name` and `bootstrap_program` are all supplied, and
  `ConfigError::NotGhidra` if the install directory is not a real Ghidra
  install.
- `shared/ghidra-mcp/src/cli.rs:83-88` — `run_serve` turns that into exit code 2
  **before** `server::serve` is ever reached. No handshake, no `tools/list`.
- `.github/workflows/ci.yml` installs no Ghidra, and
  `.github/workflows/e2e-ghidra.yml:14-21` is `workflow_dispatch` + `schedule`
  only, deliberately not `pull_request`, on the stated grounds that it
  "downloads ~400 MB of Ghidra" and "a break shows up as a red scheduled run,
  not a blocked merge."

So on a pull request the probe exits 2, `probe.status` is `failed`, and §6's
layer 1 — which is fatal by design — makes CI permanently red for
`re-ghidra-mcp-cc`. The repo has already decided, explicitly, that PR CI must not
depend on a Ghidra install; a footprint gate cannot quietly reverse that
decision.

**The contract this forces: listing tools must not depend on runtime config.**
A plugin measured by this tool MUST be able to enumerate its tools and prompts
without a working backend. The intended mechanism is a dedicated listing mode —
a `--dump-tools` subcommand, or a `tools/list`-only path that skips
`resolve_config` — that serialises the same `rmcp`/`schemars`-generated
definitions the server would serve.

This preserves §4.1's whole rationale: the payload still comes from the real
compiled binary, so `rmcp` macro drift, `schemars` output changes and
doc-comment edits are still caught. What it drops is the requirement that the
plugin's *backend* be reachable, which was never what was being measured. It is
not a retreat to source parsing.

**DECIDED (Fork 8): deferral, not a second code path.** `run_serve` resolves
config lazily, in the same place boot already is (`execute.rs:76`), so `serve`
starts unconditionally and `tools/list` is answered by the one real handler —
no fixture, no `--dump-tools`, nothing to drift. This is a change to
`shared/ghidra-mcp`, not to the footprint tool, and it is a precondition for
Deliverable A rather than a nice-to-have. It also improves the plugin on its own
terms: before `3384b58` a misconfigured user got exit 2 and an MCP server that
never appeared; now they get visible tools and a first-call error naming what is
missing, with a `suggested_action` pointing at the settings rather than at
reinstalling Ghidra.

### 4.2 The launch command is manifest-driven — no per-plugin special-casing

`.mcp.json` already carries everything needed:

```json
{ "mcpServers": { "re-ghidra-mcp-cc": {
    "command": "${CLAUDE_PLUGIN_ROOT}/bin/re-ghidra-cc-mcp",
    "args": ["serve"] } } }
```

The prober reads `command`, `args` and `env`, substitutes
`${CLAUDE_PLUGIN_ROOT}` with the plugin directory, and launches. A bare
invocation of `re-ghidra-cc-mcp` prints usage and exits — the `args` field is
what makes the difference, and it is already declared. Any new plugin is
therefore measurable with zero changes to the tool.

**Confinement.** The prober MUST refuse a `command` that resolves outside the
plugin root after substitution. The manifest is repo content, so on a pull
request it is PR-authored input; this repo already builds and runs PR-authored
code in `bundle-smoke`, so the incremental exposure is small, but a prober that
will launch any absolute path is a gratuitous widening of it.

**Environment allowlisting — load-bearing at release time.** The prober launches
the child with **a minimal platform allowlist plus exactly the keys `.mcp.json`
declares in `env`**, and nothing else. The allowlist is `PATH`, `HOME` /
`USERPROFILE`, `TMPDIR` / `TEMP` / `TMP`, `LANG`, and on Windows `SystemRoot`,
`SystemDrive` and `WINDIR`.

Why the allowlist and not an empty environment: a truly empty environment breaks
the measurement rather than securing it. A process with no `PATH` cannot resolve
anything it shells out to, and on Windows a process without `SystemRoot` fails
before `main` in most runtimes — the prober would report `failed` for every
plugin and §6's layer 1 would turn that into a permanently red build.

Why it matters at all: §7 puts the exact-oracle regeneration in a release-time
job *because that is where `ANTHROPIC_API_KEY` exists*, and §4.1 has that same
job launch the plugin binary. An inherited environment hands the API key to
every plugin server the prober starts, during a handshake the plugin controls.
The measurement job and the secret must not share an environment, and an
allowlist is what makes that true without making the probe unusable.

Allowlist keys are matched **case-insensitively on Windows**, where the
environment block is itself case-insensitive and a runner may expose
`SYSTEMROOT` rather than `SystemRoot`. A case-sensitive allowlist would strip
the variable it was written to keep.

**The allowlist is only safe because of §4.1.1.** `.mcp.json` for
`re-ghidra-mcp-cc` declares no `env` at all, so an allowlist strips
`GHIDRA_INSTALL_DIR` and friends — which, under the current `serve` path, is
fatal (`config.rs:56-88`). Once listing no longer resolves runtime config, the
allowlist costs nothing. Until then, the two folds are in direct conflict, and
§4.1.1 is the one that has to land first.

**Qwen uses different placeholders, and this repo has already been bitten by
them.** `qwen-extension.json` does carry an `mcpServers` block of the same shape
— `command` plus `args` — so the probe itself transfers. What does not transfer
is the substitution: Qwen writes
`"${extensionPath}${/}bin${/}re-ghidra-qwen-mcp"`, using `${extensionPath}`
rather than `${CLAUDE_PLUGIN_ROOT}` and a `${/}` **path-separator token** rather
than a literal `/`. Commit `306c7be` is a fix for precisely this
("extract binary name from `${/}` path separator, not `/bin/`"), and
`scripts/bundle-qwen-extension.sh:103` strips it with `sed 's#.*\${/}##'`. A
Qwen reader that substitutes only `${CLAUDE_PLUGIN_ROOT}` resolves nothing. It
also declares hooks inline with per-hook `timeout` values and names its context
file via `contextFileName`; the Qwen reader targets that manifest instead.

#### 4.2.1 Which tree is authoritative

Two trees can be probed and they are different objects:

- the **dev tree** (`claude-code/<plugin>/bin/<name>`), where `bin/<name>` is
  the real binary that `just build-<plugin>` staged;
- the **shipped bundle**, where `bin/<name>` is the platform **dispatcher
  script** — the thing `scripts/check-bundle-dispatch.sh` and
  `scripts/probe-plugin-bin.sh` exist to police.

**The gate (A) probes the dev tree**, because that is what a PR can produce.
**The published figure (B) is authoritative only if it describes the bundle**,
because the bundle is what a user installs. The two MUST NOT be conflated in
one document; `agent`/`tree` is recorded in the output (§5) so a reader can
tell which was measured.

**In the dev tree, `bin/<name>` is already the real binary — no special
resolution is needed.** `just build-<plugin>` copies `target/release/<name>`
straight into `<plugin>/bin/` (`Justfile`, the `build-*` recipes). The
dispatcher is written only by `scripts/bundle-plugin.sh` when it stages a
release bundle. So the prober applies §4.2's manifest substitution unchanged in
both trees, and what differs is only which artifact sits at the resolved path.

An earlier draft of this section had the prober resolve dev-tree probes against
`target/<profile>/<name>` instead. That was wrong twice over: it solved a
dispatcher problem the dev tree does not have, and it silently assumed every
plugin is a Rust crate whose `[[bin]]` name matches its manifest command —
which would have broken §4.2's own promise that any new plugin is measurable
with zero changes to the tool. The rule is: **substitute and launch what the
manifest says, in whichever tree is being probed.**

### 4.3 One oracle, cached — DECIDED 2026-09-03 (Fork 6, option b)

There is no API-reported ground truth for a static artifact — nothing has been
sent to a model — so tokens must be counted. **The exact counter is
authoritative for both deliverables**, and its results are cached into the
committed footprint documents (§5) so that CI reads the cache rather than the
network. The estimator survives only as an offline convenience, never as a
published or asserted figure.

This kills the split-brain the two-oracle design carried: the gate and the
README now report the same number, so the mix-shift hole — where a change could
lower an estimate while raising the true count — cannot arise.

#### 4.3.1 How a hermetic gate uses a cached exact figure

The apparent contradiction ("exact counts need the network; the gate must not")
dissolves once bytes and tokens are separated:

- **Bytes need no oracle.** §4.1's probe yields the exact serialised payload, so
  the per-source `bytes` figures are computed hermetically, offline, on every
  run, on any machine.
- **Tokens come from the cache.** The committed footprint document carries the
  token counts the exact counter produced when it was last regenerated.

So the gate's logic is:

1. Probe, and compute `bytes` per source. No network.
2. Compare those bytes against the **merge-base** footprint document (§6.2).
3. **Bytes unchanged** → the cached token counts are still valid. Assert the
   budget against them. This is the common case and it is fully hermetic.
4. **Bytes changed** → the cached tokens are stale and the document must be
   regenerated with the exact counter. If the PR already regenerated it, CI
   verifies the committed document's bytes match the probe exactly and proceeds.
   If it did not, the gate fails with "footprint document stale for
   `<plugin>`; regenerate with `<command>`".

**The cost of this choice, stated plainly.** Regeneration needs an API key, so a
contributor without one cannot produce the document for a schema change they
make. Their PR fails at step 4 with a clear message, and a maintainer
regenerates before merge. That is a real friction on outside contributions, and
it is the price of the published figure being exact rather than estimated. The
alternative was Fork 6 option (c), which removed the friction by making every
figure an estimate.

**The estimator's remaining job.** It exists so a developer without a key can
see roughly what a change costs before pushing. Its output is never committed,
never published, and never asserted on. Its `charsPerToken` constant is still
calibrated once against the exact counter and still read from the merge base
(§6.2), because a local estimate that silently drifts is worse than none.

For Claude Code the exact counter is Anthropic's `count_tokens` endpoint, called
with the real tool definitions. It is free and consumes no tokens, but it is
**authenticated and model-indexed**: the same payload counts differently across
models because tool-use overhead differs, so the model id is part of the
measurement and is recorded in the output (§5).

For Qwen the tokenizer differs, so its exact counter belongs in the
agent-specific reader, not the shared core — deferred with the rest of Qwen
support by Fork 3.

**Calibration is a pinned constant, not a per-run computation.** The estimator's
ratio MUST be calibrated once against the exact counter on the same payload and
committed as a constant with its derivation date. Re-calibrate when a new source
*kind* is added to a tier (§3), not on schema changes. The `3.7` chars/token
used in §2 is an unvalidated placeholder and MUST NOT ship in any committed
document.

**The estimator's known limit, now confined.** It is delta-faithful only at
constant content mix: the Resident tier mixes JSON schemas with English prose,
which tokenize at materially different chars/token, so a change that shifts the
*mix* can move the estimate the wrong way. Under Fork 6(b) this no longer
reaches any asserted or published number — the gate compares bytes and reads
cached exact tokens (§4.3.1) — but it is why the estimator's output must never
be presented as more than a local preview.

### 4.4 A failed probe is never zero

The prober distinguishes three outcomes and records which in `probe.status`
(§5):

- `ok` — handshake completed and every list method answered or declined
  cleanly;
- `failed` — the process exited non-zero, the handshake did not complete, or a
  response did not parse;
- `timed_out` — the process did not answer within the probe budget.

**The probe budget is 30 s per method, 120 s per plugin**, and a timeout is
`timed_out`, never a zero measurement.

**Every probe must reap its child — including a successful one.** After
measuring, on *every* outcome and not only the failures, the prober closes the
child's stdin, waits a short grace period for a voluntary exit, and then
terminates the whole process tree — a Windows Job Object with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, or `setsid` plus a process-group
`SIGKILL` on Unix.

The success path is the one that matters most, because it is the one that runs
every time. Nothing in MCP obliges a server to exit on stdin EOF, and the spec
mandates no `exit` notification, so a server that ignores EOF survives a
perfectly successful probe. A gate that leaks one process per plugin per run,
only when everything works, is a slow leak nobody attributes to the gate. This
repo already implements exactly that containment for a different child in
`shared/ghidra-worker-ctl/src/job_object.rs`; the prober needs its own, because
it is the parent here.

## 4.5 Non-MCP sources need an acquisition contract too

§3 sources two of the three tiers from things the MCP handshake cannot return,
and §4.1 defines a mechanism only for the MCP half. Without this section the
Invocation and Session tiers would silently measure zero — the same
failure §4.4 exists to prevent, arriving through a different door.

- **File-backed sources** (skill frontmatter and `SKILL.md` bodies, command and
  agent files, `CLAUDE.md` / Qwen's `contextFileName`) are read from the plugin
  directory. Frontmatter and body are split at the closing `---`; the split is
  what the Resident/Invocation distinction rests on, and it is the assumption
  §10 already flags as unverified.
- **Hook stdout is NOT measured at all.** Running a plugin's hooks would execute
  PR-authored code on a schedule the prober chooses, against a synthetic event
  payload the hook was never written for, and the result would be neither stable
  nor safe. But not running them means there is nothing to measure: a hook
  declares a *command*, not an output, so it has no static size to read. An
  earlier draft said the figure was "the static size of the hook's declared
  output where one exists" — that describes a property `hooks.json` does not
  have, and dressing an unmeasurable thing up as a conditional measurement is
  worse than admitting the gap. The source is reported as
  `{ "kind": "hook_stdout", "measured": false, "reason": "requires execution" }`,
  `probe.status` stays `ok`, and it is never folded into a total as zero.

**An unmeasured source must be disclosed, not just omitted.** §3 assigns hook
output to the Session tier and part of Invocation, so a published figure that
silently drops it understates what the installer pays. Wherever a tier contains
an unmeasured source, Deliverable B (§7) MUST render the figure as a lower bound
and name what is excluded — "≥ N tokens; excludes hook output, not measurable
without executing the hook" — rather than printing N as if it were the total.
The gate (§6) may assert on the measured subset, because it compares like with
like across runs; the *disclosure* may not, because its reader is deciding
whether to install.

### 4.5.1 Implemented 2026-09-03 — the measured figures

File-backed acquisition is implemented (`tools/plugin-footprint/src/sources.rs`).
Frontmatter goes to Resident, the body to Invocation, split at the closing `---`;
`skills/<name>/SKILL.md`, `agents/<name>.md` and `commands/<name>.md` are read,
and a mis-structured skill is an error rather than a silent skip. Measured against
the `dev` tree at plugin version 0.6.4, both plugins probing `ok`:

| Plugin | Resident bytes | of which MCP | of which files | Invocation bytes |
|---|---|---|---|---|
| `rtk-mcp-cc` | 3 696 | 2 832 | 864 | 6 227 |
| `re-ghidra-mcp-cc` | 21 670 | 18 419 | 3 251 | 37 513 |

Per source, for `re-ghidra-mcp-cc`:

| Source | Resident (frontmatter) | Invocation (body) |
|---|---|---|
| `agents/re-analyst.md` | 2 359 | 6 421 |
| `skills/doctor` | 642 | 11 607 |
| `skills/ghidra-re-driver` | 250 | 19 485 |

**This settles the sequencing question §8 raised.** `agents/re-analyst.md`'s
frontmatter, at 2 359 B, is the single largest resident source in the plugin —
larger than `set_prototype` (2 305 B), the largest tool schema. Had the gate been
built before this section, the biggest thing the plugin charges to every request
would have sat outside the measurement entirely, and a ceiling fitted to the MCP
half alone would have been 15% low from the day it was set.

The Resident tier is therefore **no longer a lower bound, with one exception**:
hook output, which stays unmeasured permanently for the reason given above. §7's
disclosure obligation is unchanged and still applies to it.

The §2 table's Invocation column predates this contract and was produced by hand;
it also predates `canonical_json`, so its schema-byte figures run a few bytes
below the ones this section reports. Read the table for the argument and this
section for the numbers.

## 5. Output contract

One JSON document per plugin. This is the stable interface: the gate, the
snapshot test, and the README generator all consume it, and nothing else parses
plugin internals.

```json
{
  "schemaVersion": 1,
  "plugin": "re-ghidra-mcp-cc",
  "agent": "claude-code",
  "tree": "dev",
  "pluginVersion": "0.6.4",
  "measuredAt": "2026-09-03T12:00:00Z",
  "probe": {
    "status": "ok",
    "binary": "claude-code/re-ghidra-mcp-cc/bin/re-ghidra-cc-mcp.exe",
    "reportedVersion": "0.6.4",
    "toolCount": 19,
    "promptCount": 0
  },
  "oracle": { "kind": "exact", "id": "anthropic-count-tokens", "model": "claude-opus-5" },
  "tiers": {
    "resident": {
      "bytes": 19176,
      "tokens": 5182,
      "sources": [
        { "kind": "mcp_tool_schema",   "id": "set_prototype", "bytes": 2293, "tokens": 619 },
        { "kind": "skill_frontmatter", "id": "doctor",        "bytes":  542, "tokens": 146 }
      ]
    },
    "invocation": { "bytes": 31293, "tokens": 8457, "sources": [] },
    "session":    { "bytes": 0,     "tokens": 0,    "sources": [] }
  }
}
```

Requirements:

- `sources` is always itemised, never only a total. The per-item breakdown is
  what makes a failing gate actionable ("`set_prototype` grew 400 B") rather
  than merely red.
- `oracle` is recorded in every document. A number without its oracle is not a
  measurement, and estimator-derived figures must never be presentable as exact.
  It is a **tagged union on `kind`**: `estimator` carries `id` and
  `charsPerToken`; `exact` carries `id` and `model`, and no `charsPerToken`.
  `charsPerToken` is meaningless for an exact count and an `exact` figure
  without its model is not reproducible.
- `probe` is recorded in every document, including a failed one. **`status`
  other than `ok` means the tier figures are absent, not zero** — the document
  omits `tiers` entirely rather than serialising zeros that a ceiling assertion
  would happily pass.
- `probe.binary` and `probe.reportedVersion` pin provenance. `pluginVersion`
  comes from the manifest and `reportedVersion` from the binary itself; a
  document where they disagree measured a stale binary. This repo already ships
  `scripts/probe-plugin-bin.sh` to catch that exact mismatch in bundles, and the
  footprint document inherits the same failure mode.
  **`probe.binary` is normalised and excluded from the snapshot**: recorded
  repo-relative, with forward slashes and any `.exe` suffix stripped. Unnormalised
  it differs by platform — the gate may run on Linux while the published figure
  is regenerated on Windows — so snapshotting it would flake on the OS rather
  than on the footprint, which is the one thing layer 2 exists to show.
- `bytes` is **the UTF-8 length of the JSON text as the prober serialises it**,
  using a single pinned serialisation (sorted keys, no insignificant
  whitespace, non-ASCII emitted literally rather than `\u`-escaped). JSON is not
  canonical by default, and an unpinned serialisation makes the `insta` snapshot
  in §6 flake on key order alone.
- `schemaVersion` is bumped on any breaking change; consumers reject unknown
  versions rather than guessing. **"Unknown" means newer, never older.** The
  consumers are the gate and the README generator, and §6.2 has the gate read
  its baseline from the merge base — which necessarily holds the *previous*
  schema version on the very PR that bumps it. A strict equality check therefore
  deadlocks: the bump cannot merge, because the gate cannot read `main`'s
  document to compute the ratchet it is bumping. The gate MUST read any version
  up to and including its own, and refuse only versions above it.

## 6. Deliverable A — the maintainer gate

Five layers, because they fail differently and a reviewer needs all of them:

1. **Probe assertion** — `probe.status == "ok"` and `probe.toolCount > 0` for
   every plugin in `marketplace.json`. This runs *first* and its failure is
   fatal. Without it, a plugin whose binary is missing or refuses to start
   measures as absent, and every ceiling below it passes. §4.4 makes the
   distinction representable; this is what enforces it.
2. **Snapshot test** (`insta`, already a workspace dev-dependency at
   `Cargo.toml:52`) over the per-source breakdown. A schema change shows up in
   review as a readable diff naming the tool that grew.
3. **Freshness assertion** — the per-source `bytes` measured by the probe match
   the merge-base footprint document. If they differ, its cached token counts
   are stale and the document must be regenerated with the exact oracle
   (§4.3.1); the failure says so and names the command.
4. **Budget assertion** — a per-plugin ceiling on `tiers.resident.tokens`,
   using the cached exact counts. Fails the build when exceeded.
5. **Per-PR delta cap** (Fork 2, decided) — a PR may not add more than `D`
   resident tokens in one step, where `D < H`. The ratchet alone permits any
   growth up to the headroom `H` above the low-water mark, so a single PR adding
   300 tokens passes silently while still under budget. `D` catches the jump the
   ratchet is blind to; the ratchet catches the creep that never trips `D`.

Initial budgets are set from today's measured values plus headroom, so the gate
is live and meaningful from the first commit rather than aspirational — with the
ratchet caveat in §1.

**A static ceiling is not a ratchet, and §1 must not claim it is.** With a fixed
budget, a plugin that drops from 5 000 to 3 000 tokens leaves 2 500 tokens of
headroom under a 5 500 ceiling that a later PR can refill without ever tripping
the gate. The cost then returns to its old level while every build stays green —
the gate's own slack becomes the bypass. Two honest resolutions, and this spec
must pick one rather than describe a ratchet and implement a ceiling:
(a) keep the fixed ceiling and describe it as a cap, dropping the word ratchet;
(b) make it a real ratchet — the budget follows the measured low-water mark
plus a **fixed headroom `H`**, so recovered savings are partly banked rather
than left wholly available. **(b) is the recommendation**, because it is what §1
already promises the reader, and because it is the same mechanism as Fork 2's
delta option seen from the other end.

`H` is not optional and it is not zero. A ratchet that snaps the budget to the
exact last measurement leaves no headroom at all: a PR that saves 50 tokens
lowers the bar by 50, and the next PR fails on a single added token. That is a
repo hostile to ordinary iteration, and it is the shape a "tighten
automatically" rule takes if nobody writes the tolerance down. `H` is the same
headroom used to seed the initial budgets, so the gate's strictness is one
number a maintainer can reason about rather than an emergent property.

**The ratchet also needs hysteresis, or a revert fails.** Lowering on every
improvement means a PR that saves `H + 5` tokens tightens the budget by 5, and
reverting that PR — restoring a footprint the repo was happy with a day
earlier — then fails the gate. The budget lowers only when the measurement comes
in below `budget - H` by a further margin, so ordinary fluctuation does not
re-tighten and a revert of a recent improvement stays green.

### 6.2 The baseline must not come from the tree being measured

(b) makes the budget a generated value, and the obvious place to keep it is the
committed footprint document (§5). **That, done naively, destroys the gate.** CI
checks out the PR's branch; if the budget is read from that checkout, the author
whose growth the gate exists to catch is also the author of the threshold it is
checked against. Editing one number in a committed JSON file turns any failing
PR green, and the diff looks like a routine regeneration.

So: **the baseline is read from the merge base (or `main`), never from the PR's
own working tree.** The measurement is taken from the PR; the threshold it is
compared against comes from the branch being merged into. A PR may propose a new
budget, but the proposal is reviewed as a diff against the base value rather
than silently consumed as input.

**Every input to the verdict follows the same rule, not just the budget.** The
gate's answer depends on more numbers than the ceiling, and each one is equally
authorable by the party being gated. From the merge base, therefore: the budget,
the headroom `H` (§6), and the pinned `charsPerToken` calibration constant
(§4.3) — a PR that raises `charsPerToken` from 3.7 to 4.2 shrinks every
estimate it is judged by, and the diff looks like a recalibration. From the PR:
only the measurement itself.

**The set of plugins is an input too.** §6's layer 1 iterates
`marketplace.json`, so a PR that adds bloat *and deletes its plugin's entry from
`marketplace.json`* is not measured at all and passes. The gate therefore
cross-checks the manifest's plugin list against the plugin directories present,
and a plugin that disappears from the manifest while its directory remains is a
failure, not an empty result. Removing a plugin from the marketplace stays
possible; it just cannot happen as a side effect of a footprint-bearing PR.

**Threat model, stated plainly.** All of the above defends against *accident and
convenience* — the failing PR that takes the easy way out — not against a
hostile plugin author. A plugin that detects the probe and reports a small
`tools/list` cannot be caught by measuring what it reports, and this tool does
not try. That is acceptable here because the plugins are authored by the same
maintainers as the gate; it would not be acceptable for a marketplace accepting
third-party submissions, and this line is what would need revisiting first.

This is the same mechanism Fork 2's delta option needs, which is worth noting
before Fork 2 is decided: choosing the ratchet and choosing merge-base deltas are
largely the same piece of work, and choosing a committed-baseline delta without
this rule reintroduces the hole.

**Failure output is part of the contract.** The JSON is the interface for tools;
a red CI log is the interface for a person. A failing budget assertion MUST
print, to stderr: the plugin, the tier, budget vs actual, and the top 3 sources
by size, plus their growth against the previous measurement **where a baseline
is loaded**, which under the decided design it always is: §6.2 has the runner
read the merge-base footprint document for its thresholds, and §4.3.1 has it
read the same document for cached token counts, so the per-source baseline is
already in hand. The round-1 draft specified this message before either of those
existed, when the only per-source baseline lived in layer 2's `insta` snapshot
that the budget runner never opens — a message the runner had no data to
produce. It is only payable now because two later decisions happened to put the
data there; had they gone the other way, the message would have had to shrink to
absolute sizes only.

### 6.1 Where it runs — measured, not assumed

**`just check` is not CI.** Verified 2026-09-03: `.github/workflows/ci.yml`
never invokes `just` — it hand-enumerates `cargo fmt`, `cargo clippy`,
`cargo nextest`, three `scripts/check-*.sh`, the bundle-smoke matrix,
`cargo deny` and `typos`. And `lefthook.yml` runs only fmt, clippy and typos.
So extending the `check:` aggregate gates a developer who remembers to run it
and nothing else. **Deliverable A requires a change to `ci.yml` as well as to
the `Justfile`**, and the spec that omitted that omitted the deliverable.

**No CI job currently holds plugin binaries.** Verified: `ci.yml` contains no
`cargo build` and no `just build-*`. The only job with binaries is
`bundle-smoke`, which builds **debug** binaries via `scripts/smoke-bundle.sh`
into a staged bundle. Because §4.1 requires probing a real binary, the gate must
either carry its own build step or attach to a job that already has one — and
`bundle-smoke` is a three-OS matrix, which would run the gate three times and
make "the footprint" an OS-indexed value. This is a genuine dependency, not an
optimisation: a footprint check that runs without binaries can only silently
measure nothing.

The Justfile aggregate today reads:

```
check: fmt lint test deny spellcheck wiring marketplace dispatch smoke
```

Note `qwen-marketplace` is a Justfile target that is **not** in that list —
relevant to Fork 3.

## 7. Deliverable B — the published figure

Generated, never hand-written, and verified fresh in CI.

**This pattern already exists in the repo.** `scripts/check-qwen-marketplace.sh`
verifies a generated manifest against the things it copies, on the stated
reasoning that "nothing fails when a copy goes stale — the marketplace keeps
installing, it just advertises the wrong thing." A stale footprint number is
the same failure mode, and gets the same treatment.

Surfaces:

- **Per-plugin README** — the three tiers, with the resident figure leading.
- **Root marketplace README** — a comparison table across plugins, since that is
  where an install decision is actually made. **The root README carries no
  per-plugin table today** (verified — it has a `shared/` crate table and
  nothing else), so the region has to be introduced before it can be
  regenerated. **The `<!-- footprint:begin -->` / `<!-- footprint:end -->`
  marker pair is committed by hand, once, in the PR that lands this tool** — a
  generator cannot write "between markers" that do not exist yet, and inferring
  a placement from surrounding prose is exactly the kind of guess that makes a
  generated region land somewhere different on the next run. Thereafter the
  generator only ever rewrites what is between them, and **a missing or unpaired
  marker is a hard error naming the file**, never a silent no-op: a checker that
  quietly passes on a README it could not find its region in is the stale-copy
  failure §7 exists to prevent. The cited precedent verifies a *manifest*, where
  every field has an owner; a README region needs the markers to have one at all.

  **The root region is generated from ALL committed footprint documents, never
  from the one plugin being changed.** §5 emits one document per plugin, so a
  generator invoked per-plugin against a shared aggregate region would rewrite
  the whole table with a single row and silently delete every other plugin's
  line. The per-plugin README regions are written per plugin; the root
  comparison table is written once, from the full set, and regenerating it
  requires every plugin's document to be present — a missing one is an error,
  not an omitted row.

**The verification cannot use the exact oracle on a pull request.** GitHub
withholds secrets from fork pull requests, so an `ANTHROPIC_API_KEY`-dependent
check hard-fails every outside contributor for a reason unrelated to their
change. The freshness check on a PR therefore verifies that the README region
matches the *committed* footprint documents; regenerating those documents with
the exact oracle is a release-time step that runs where secrets exist. §4.3's
"no network in CI" and this section's "verified fresh in CI" are only compatible
under that split.

Any new script follows the existing `scripts/check-*.sh` house style:
`set -euo pipefail`, `cd "$(dirname "$0")/.."`, and `jq` output piped through
`tr -d '\r'` — the Windows `jq` emits CRLF, and a stray carriage return turns
every comparison into a mismatch.

## 8. Design forks — ALL DECIDED 2026-09-03

All eight are settled. Each entry states the decision and keeps the options and
reasoning, because the rejected branches are what make a later reversal cheap.
Nothing here now blocks a line-level implementation plan.

**Fork 1 — where the crate lives. DECIDED: (b) a new top-level `tools/`.**
Add `"tools/*"` to workspace `members`. Rejected (c) xtask: the peer recommended
it on the stated ground that it needs no `members` change, which is false — this
repo has no `.cargo/` and an explicit glob list, so xtask needs both a `members`
entry and a new `.cargo/config.toml` alias, strictly more setup than `tools/`.
It would also be a third automation idiom beside `just` and `scripts/*.sh`.
The options as they stood: The root README defines `shared/` as
engines that *several agents' plugins front*. This tool is fronted by no
plugin: it is maintainer tooling and must never appear in the marketplace.
Options: (a) `shared/plugin-footprint` and widen the README's definition of
`shared/`; (b) a new top-level `tools/`; (c) an `xtask`-style dev binary.
Recommendation: (b) — it keeps `shared/` honest, and the distinction between
"engine we ship" and "tool we maintain with" is worth a directory. Note (b) also
requires adding `"tools/*"` to the workspace `members` list, which today is
`claude-code/*`, `antigravity/*`, `qwen/*`, `shared/*`; (a) is picked up by the
existing glob for free. Do not place it under `claude-code/`, where the glob
would make it look like a plugin to `scripts/check-plugin-wiring.sh`.

**Fork 2 — gate strictness. DECIDED: ratchet PLUS a per-PR delta cap `D`**
(§6 layers 4-5). The peer called the fork badly posed because §6's ratchet
already moves; half right — it was written before the ratchet existed. But the
two are not equivalent: the ratchet permits any growth up to `H`, so a single
300-token PR passes while under budget. `D < H` closes that. The options as they
stood: Absolute per-plugin ceiling, delta-only ("no
single PR may add more than N tokens"), or both. Recommendation: both — the
ceiling caps total cost, the delta catches slow creep that never trips it.
**The delta option needs a baseline mechanism this spec does not yet have**, and
the two candidates cost differently: a committed baseline document per plugin
(cheap, but is itself a copy that can go stale) versus a merge-base measurement
(always current, but requires building the base commit's binaries too — doubling
the build cost §6.1 already flags). Choosing "both" is choosing one of those.

**Fork 3 — v1 agent coverage. DECIDED: Claude Code only.** Qwen support waits
until Qwen has a CI job to run a gate in; building that job is unrelated
infrastructure work and should not block this tool. §5's `agent` field already
makes the output contract multi-agent, so this is cheap to reverse. The options
as they stood: Claude Code only, or Claude Code + Qwen. Qwen's
manifest is a different shape but the probe is identical. Antigravity has no
equivalent studied here and is deferred. **If Qwen is in scope, this fork must
also decide where the Qwen gate runs:** verified 2026-09-03, `qwen-marketplace`
is absent from the `check:` aggregate and `ci.yml` mentions Qwen nowhere, so a
Qwen footprint gate written today would never fire. Adding Qwen means adding its
CI home, which is work this fork's "the probe is identical" framing hides.

**Fork 4 — publication form. DECIDED: table, not badge.** Follows directly from
§3: a badge compresses to one blended number, which is the form §3 argues hides
the distinction that should drive the install decision. The options as they
stood: Table, badge, or both. A badge compresses to one
number, which §3 argues is the dishonest form; a table costs README space.

**Fork 5 — block or warn. DECIDED: hard-fail.** The fork-PR concern does not
apply: §7 keeps the exact oracle off pull requests entirely, so no PR check
needs a secret. (The peer reached the same answer via a premise it invented —
it treated its own Fork 6 recommendation as already chosen — but the conclusion
holds on §7's actual split.) The options as they stood: A hard failure on a footprint budget
could block an otherwise-good PR. Recommendation: hard-fail, since a budget
that only warns is the thing that lets bloat ship. Note the interaction with
§7: whatever this fork decides, the *exact-oracle* half must not hard-fail on
fork PRs, or the strictness lands on contributors instead of on footprint.

**Fork 6 — one oracle or two. DECIDED: (b) exact oracle for both, cached.**
The exact counter is authoritative for the gate and the published figure alike;
its results are cached into the committed footprint documents so CI reads the
cache, not the network, and the estimator survives only as an offline preview.
§4.3.1 works out how a hermetic gate uses a cached exact figure — bytes are
computed fresh and offline on every run, tokens come from the merge-base
document, and a bytes change means the document is stale and must be
regenerated. The accepted cost is on outside contributions: a contributor
without an API key cannot regenerate the document for a schema change they make,
so their PR fails with a "regenerate" message and a maintainer does it before
merge. That friction is the price of the published figure being exact.

The options and reasoning as they stood, including the peer's dissent:

Raised by review; not in the original draft.
§4.3 assumes two oracles are free because they answer different questions, but
they produce two numbers for the same field: the gate asserts on estimated
`tiers.resident.tokens` while the README publishes the exact one, and nothing
says which is authoritative when they disagree. Combined with the mix-shift
limit in §4.3, a change can pass the gate and worsen the published figure.
Options: (a) keep both, and state in §5 that the gate's number is advisory and
the published one authoritative; (b) exact oracle for both, with results cached
into committed footprint documents so CI reads the cache rather than the network
(the estimator survives only as an offline fallback); (c) estimator for both,
and publish a figure honestly labelled as an estimate with its ratio.

**The second reviewer's call was (c)**, asked with no lean disclosed. Its
reasoning: (c) is the only option where the gate and the published figure are
mathematically the same number, so the split-brain in (a) cannot arise, and
every party — CI, maintainer, outside contributor — computes it offline with the
same heuristic. It defends the mix-shift hole (§4.3) with layer 2 rather than
with the oracle: the `insta` snapshot forces a human to see which strings
changed density.

**One caveat on its rejection of (b), measured.** It rejected (b) on the grounds
that an outside contributor cannot run the exact counter and so "cannot update
the baseline, meaning any schema change from an outside contributor will
persistently fail CI." Under §7 as written that does not follow: the PR-time
check compares the README region against the *committed* footprint documents,
not against a fresh measurement, so a fork PR that changes a schema leaves both
stale and mutually consistent, and release-time regeneration reconciles them.
The objection lands only if the PR-time check is also made to verify documents
against reality with the exact oracle — which §7 explicitly does not do. So (b)
survives its stated objection; whether it survives on its own merits (a
generated baseline is one more copy that can go stale, per §6) was the question
the owner actually weighed. §6.2's merge-base rule is what makes that staleness
tractable: the cached document is read from the base branch, never from the tree
being measured, and §6 layer 3 fails loudly when it no longer matches the probe.

**Fork 7 — where in CI the gate runs. DECIDED: (a) a new single-OS job** that
runs `just build-<plugin>` and then the gate. Footprint is a property of the
schema, not the OS, so a 3-OS matrix would triple the cost to answer the same
question — and both this reviewer and the peer independently named this the
fork most expensive to reverse, because wedging the gate into `bundle-smoke`
would couple a static measurement to the packaging lifecycle. The options as
they stood: Raised by review; §6.1 diagnoses that no
CI job holds plugin binaries and then stops, which leaves the implementer
guessing. Options: (a) a new single-OS job that runs `cargo build` for the
plugin binaries and then the gate — clean, one authoritative number, costs a
build that no existing job shares; (b) attach the gate to one designated leg of
the existing three-OS `bundle-smoke` matrix (say `ubuntu-latest`), reusing a
build that already happens — cheap, but couples an unrelated job to the gate and
makes the other two legs silently not run it; (c) run it on all three legs and
assert the three agree — most honest about the OS question, three times the
cost, and it turns any genuine cross-platform schema difference into a build
failure rather than a measurement. Recommendation: (a), on the grounds that the
published figure should not depend on which matrix leg produced it, and that §7
needs a job it can attach the release-time regeneration to anyway. This fork
must be decided before §6 can be implemented at all.

**Whichever option is chosen, it must be chosen twice.** Deliverable A's gate
runs on pull requests and Deliverable B's exact-oracle regeneration runs at
release time — two different jobs, and *both* launch plugin binaries. Solving
the binary problem for the gate's job leaves the release-time job with nothing
to probe. The release job additionally holds `ANTHROPIC_API_KEY`, which is what
§4.2's environment allowlist exists to keep away from the binaries it launches.

**The gate's job runs `just build-<plugin>`, not bare `cargo build`.** §4.2.1
has the prober substitute the manifest's `${CLAUDE_PLUGIN_ROOT}/bin/<name>`, and
`cargo build` writes to `target/<profile>/`, not to `<plugin>/bin/` — it is the
`just build-*` recipes that stage the binaries where the manifest points.

**The release job measures the release artifacts, not a fresh compile.** §4.2.1
already says the published figure is authoritative only if it describes the
bundle; recompiling from source at release time would measure a binary that is
not the one being shipped, and `probe.reportedVersion` (§5) exists precisely to
catch that class of mismatch. Deliverable B probes the assembled bundles.

**Fork 8 — how a plugin becomes listable without its backend. DECIDED
2026-09-03: option (a), implemented as deferral.** `run_serve` resolves config
lazily, in the same place boot already is; `serve` starts unconditionally and
`tools/list` is answered by the real handler with no fixture and no second
serialisation path. This is a prerequisite for Deliverable A and is a change to
`shared/ghidra-mcp` (`run_serve` / `ServerState`), not to the footprint tool —
so it is its own piece of work, sequenced before the gate. The options as they
stood, kept because the rejected ones carry the reasoning:

Raised by round 4; §4.1.1 shows the live-probe contract cannot measure
`re-ghidra-mcp-cc` on a pull request at all. Four options:

- **(a) A config-free listing mode per MCP server** — a `--dump-tools`
  subcommand. Makes the gate work everywhere, but puts a recurring requirement
  on every plugin, and carries a real failure mode: a second serialisation path
  can drift from the one the server actually serves, which defeats §4.1's whole
  rationale for probing a live binary.
- **(b) Measure only plugins that already list without a backend** (today:
  `rtk-mcp-cc` alone) and exempt the rest. Costs nothing and guts Deliverable A,
  since it exempts the largest footprint in the marketplace.
- **(c) Give the PR gate its own Ghidra install.** Reverses the cost decision
  `e2e-ghidra.yml:14-21` made explicitly.
- **(d) Point the prober at a fixture directory.** Verified viable:
  `config.rs:78-88` performs exactly two filesystem checks, both pure
  `.exists()` — that `<install>/support/analyzeHeadless[.bat]` is present and
  that `project_dir` is present — with no content validation, and the JVM boot
  is lazy (`execute.rs:76`). So `mkdir -p fixture/support && touch
  fixture/support/analyzeHeadless` plus a project dir satisfies resolution and
  the unmodified server serves `tools/list`. Zero plugin changes, zero
  downloads.

**(d)'s cost is where it conflicts with §4.2.** The fixture is per-plugin
knowledge that lives in no manifest: the prober would have to know that *this*
plugin needs *these four* environment variables pointing at *that* directory
shape — which is exactly the per-plugin special-casing §4.2 promises there will
be none of. It is also brittle in a way that fails toward red: if the plugin
later validates the launcher's contents or a version file, the fixture stops
satisfying it and the gate fails for a reason having nothing to do with
footprint. And it needs the prober to inject environment variables the manifest
does not declare, which cuts against §4.2's allowlist.

**Recommendation: (a), implemented as deferral rather than as a second path.**
Both this reviewer and the independent peer chose (a) on the same reasoning —
a server's ability to declare its API should not depend on its backend being
present. The peer's objection to (a), that a `--dump-tools` serializer can drift
from the real one, is answered by not building a second serializer: **make
`run_serve` defer config resolution instead of resolving it eagerly.** Boot is
already lazy (`execute.rs:76` boots from `wait_until_ready`, on the tool-
execution path); config resolution can be lazy in the same place. Then `serve`
starts unconditionally, `tools/list` is answered by the one real handler with no
fixture and no second code path, and a missing config surfaces on the first tool
call — where the user can act on it.

That is a genuine improvement to the plugin independent of this tool. Today a
misconfigured user gets exit 2 and an MCP server that never appears; after, they
get a server whose tools are visible and whose first call explains what is
missing. It is nonetheless a real change to `run_serve` and `ServerState`, not a
one-line edit, and it is the maintainer's call.

## 9. Out of scope

- **Runtime token usage.** Reading agent transcripts to report what a session
  actually spent is a separate, larger tool. Claude Code and Qwen both record
  per-request ground truth and Antigravity does not — that asymmetry drives a
  different design and should not be entangled with static measurement.
- **Currency.** Price tables rot; a stale price is a wrong number. Tokens only.
- **Runtime enforcement.** This never changes what a plugin loads. It measures
  and reports.
- **Reducing existing footprint.** See §1: the gate is a ratchet.

## 10. Facts register

**Verified on 2026-09-03** (re-verify before relying on any of these):

- `tools/list` payloads: `rtk-mcp-cc` 4 tools / 2 831 B; `re-ghidra-mcp-cc`
  19 tools / 18 379 B, the latter requiring the `serve` subcommand.
- `.mcp.json` carries `command`, `args`, `env`; re-ghidra's `args` is `["serve"]`.
- `qwen-extension.json` carries `contextFileName` and declares hooks inline.
- `just check` currently aggregates: fmt, lint, test, deny, spellcheck, wiring,
  marketplace, dispatch, smoke. `qwen-marketplace` is a target but is NOT in it.
- `insta` is a workspace dev-dependency (`Cargo.toml:52`).

**Verified on 2026-09-03 during panel review** (corrects the round-1 draft):

- `.github/workflows/ci.yml` never invokes `just`, and contains no `cargo
  build` / `just build-*`. CI hand-enumerates its steps. Editing the `check:`
  aggregate does not change what CI runs.
- `lefthook.yml` runs fmt, clippy and typos only — pre-commit and pre-push.
- The only CI job holding plugin binaries is `bundle-smoke`, which builds
  **debug** binaries through `scripts/smoke-bundle.sh`.
- `.claude-plugin/marketplace.json` publishes exactly two plugins:
  `rtk-mcp-cc` and `re-ghidra-mcp-cc`. `claude-code/example` is not published.
- The root `README.md` has no per-plugin table; its only table lists `shared/`
  crates.
- Workspace `members` is `claude-code/*`, `antigravity/*`, `qwen/*`,
  `shared/*` — a top-level `tools/` would need adding.
- `ci.yml` mentions Qwen nowhere.

**Verified on 2026-09-03 during round 4** (the facts behind §4.1.1). These
describe the code AS IT WAS BEFORE commit `3384b58`, which fixed it — they are
kept because they are why the probe contract is shaped this way, not because
they still hold:

- `shared/ghidra-mcp/src/config.rs:56-88` — `resolve()` requires
  `ghidra_install_dir`, `project_dir`, `project_name` and `bootstrap_program`,
  and rejects an install directory that is not a real Ghidra install.
- `shared/ghidra-mcp/src/cli.rs:83-88` — a config error exits 2 before
  `server::serve`, so no MCP handshake occurs. **No longer true after `3384b58`:**
  `run_serve` hands the unresolved inputs to `serve`, and `dispatch`'s exit-code
  contract no longer lists 2 for a configuration problem at all.
- `.github/workflows/ci.yml` installs no Ghidra;
  `.github/workflows/e2e-ghidra.yml:14-21` is `workflow_dispatch` + `schedule`
  only and says in comment why it is not on `pull_request`.
- `qwen-extension.json` DOES carry an `mcpServers` block with `command` and
  `args`, but uses `${extensionPath}` and a `${/}` separator token; commit
  `306c7be` fixed a bug in exactly that parsing, and
  `scripts/bundle-qwen-extension.sh:103` strips `${/}` by hand.
- `Justfile`'s `build-*` recipes copy `target/release/<name>` into
  `<plugin>/bin/`; `scripts/bundle-plugin.sh:187` writes the dispatcher, and
  only into a staged bundle.
- `scripts/smoke-bundle.sh:122` runs `scripts/probe-plugin-bin.sh`, which spawns
  plugin binaries directly via node `spawnSync` (lines 134-142) on every PR.
- A `docs/` tree now exists (created by this spec). The round-1 draft claimed it
  did not. Unrelated to `.github/workflows/docs.yml`, which publishes
  `target/doc` and never reads `docs/`.

**Assumed, NOT verified — confirm during implementation:**

- That every skill's frontmatter is resident for discovery while its body is
  loaded only on invocation. The whole Resident/Invocation split rests on this;
  it is the first thing to check, and the cost model changes if it is wrong.
- The `3.7` chars/token ratio (§4.3).
- `count_tokens` behaviour when passed plugin tool definitions, and how much of
  its result is model-specific tool-use overhead rather than the schemas.
- Whether `rmcp` ever paginates `tools/list` for these servers. §4.1 requires
  cursor-following regardless, so this is a curiosity, not a dependency.
- Whether Claude Code counts MCP `prompts/list` entries as resident context.
  §3 assumes yes; if no, prompts move to the Invocation tier.

## 11. Review record

**Round 1 (solo panel), 2026-09-03.** Persona: relentless-adversarial-auditor.
Seats: Axiom Breaker, Cascade Analyst, Protocol Pedant, Mechanism Gamer,
Literal Implementer, Blindspot Auditor, Dependency Cynic, Resource Vampire,
Boundary Smuggler, Activation Auditor. State Corruptor dropped — no
concurrency, ordering or shared mutable state in the artifact.

21 findings; 19 folded, 2 stood down (below). The load-bearing two: §6's stated
integration point could not deliver Deliverable A because CI does not run
`just`, and §5's contract could not distinguish a failed probe from a zero
measurement, making the gate false-green on precisely the failure §6 had
already named in prose.

**Round 2 (agy escalation), 2026-09-03.** Independent second-model panel, sent
the artifact by path with the round-1 do-not-re-raise ledger inlined and no lean
disclosed on Fork 6. Ten seats; State Corruptor dropped for the same reason.
Eight findings folded, one rejected on measurement, one stood down.

Three of the folded findings were defects **introduced by round 1's own fixes** —
the unbounded cursor loop created by adding pagination (§4.1), the impossible
stderr message created by demanding growth data the budget runner cannot reach
(§6), and the markers that could not be written between markers that did not
exist (§7). That is the expected shape: a fix spawns its own edges, and it is
the argument for the round after this one.

The sharpest genuinely new finding was the API-key one (§4.2): §7 had put the
exact-oracle regeneration in a secret-bearing job and §4.1 had that same job
launch a plugin binary, and nothing connected the two.

**Round 3 (agy escalation), 2026-09-03.** Seats derived from round 2's defect
class rather than rotated generically: a Fold Auditor aimed only at round 2's
eight edits (enumerated in the brief), State Corruptor (whose use-when fired for
the first time once §6 introduced a generated baseline), a Disposition
Challenger aimed at my own three stand-downs, and a Coverage Interrogator.
Ten findings: seven folded, two rejected on measurement, one partially upheld.

**The pattern held a third time — five of round 3's findings were defects
created by round 2's fixes.** Environment scrubbing to an empty environment
would have broken every probe on Windows (no `SystemRoot`) and everywhere
without `PATH`. The `target/<profile>/` dev-tree rule solved a dispatcher
problem the dev tree does not have and assumed every plugin is a Rust crate. The
process-tree kill was specified only for failures, leaking a process on every
*successful* run. The ratchet had no headroom band, so one saved token would
have made the next PR fail. §4.5's "static size of the hook's declared output"
described a property `hooks.json` does not have.

**The best finding of the whole review came from this round**, and it is not one
any of the earlier seats was positioned to see: §6(b) had put the budget in the
committed footprint document, so CI would have read the threshold out of the
same PR tree it was measuring. The regulated party would have authored its own
limit, and inflating it would have looked like a routine regeneration. §6.2 now
requires the baseline to come from the merge base.

**Round 4 (agy escalation), 2026-09-03.** Seats rotated by *coverage gap* rather
than by defect class, using what the previous round's Coverage Interrogator had
named as its own thinnest reading: a Coverage-Gap seat aimed at the Qwen
manifest rules and `schemaVersion`, a Fold Auditor on round 3's eight edits, a
Threshold Adversary derived from round 3's best finding, and a Portability
Realist. Twelve findings: five folded, six rejected on measurement, one folded
in reduced form.

**The rejection rate doubled, and the reason is worth recording:** most of
round 4's misses argued from what a mechanism could do in principle without
checking what the artifact says or what the repo does. Reaping "breaks
pagination" — §4.4 reaps *after* measuring. A `failed` probe "passes the ceiling
because there is no value to compare" — §6 layer 1 asserts `status == "ok"`
first and is fatal. The lower-bound rule "breaks the integer ceiling" — hook
output is Session/Invocation, and the ceiling is on `tiers.resident.tokens`.

**But the round's one *false* finding is what produced the review's most
important one.** The Coverage-Gap seat claimed `qwen-extension.json` has no
`mcpServers` block, "as verified in the repo". It does have one. Checking that
claim led to the Qwen placeholder defect the seat had missed (`${extensionPath}`
and the `${/}` separator token — already the subject of commit `306c7be`), and
then, via the Fold Auditor's *true* finding that the environment allowlist
strips config the plugin needs, to §4.1.1: `re-ghidra-cc-mcp serve` cannot
answer `tools/list` without a validated Ghidra install, PR CI deliberately has
none, and so the spec's central measurement contract cannot measure the plugin
the spec was written for. No seat in four rounds stated that. It fell out of
refusing to take a confident claim at face value.

**Panel stopped after round 4, by the owner.** Round 4 was not clean, but it
changed §4's central contract (§4.1.1) and opened Fork 8, so a round 5 would
have been reviewing a design about to change shape. This is a deliberate stop
with findings open, not a GREEN verdict, and the spec should get a fresh round
once the §4.1.1 prerequisite has landed and the text has settled.

**Two bounded consults followed, not panel rounds.** The first put Fork 8 to the
peer neutrally; it chose the same option this reviewer had, and volunteered a
fourth — a fixture directory — which measurement confirmed was viable
(`config.rs:78-88` does only `.exists()` checks) and which the owner weighed and
rejected as per-plugin special-casing. The second put the six remaining forks to
it. Both its calls that this reviewer disagreed with rested on checkable claims
that turned out false: that an xtask needs no workspace `members` change (this
repo has no `.cargo/` and an explicit glob list), and that Fork 6 option (c) was
already chosen (it was not — the peer reasoned from its own recommendation as
though it were settled).

**All eight forks were decided by the owner on 2026-09-03.** §8 records each
decision with its rejected branches.

## Stand-downs

- `DISCARDED-BELOW-FLOOR: fork-PR code execution via a manifest-declared
  command is not an incremental risk, because CI already builds and runs
  PR-authored code — .github/workflows/ci.yml bundle-smoke job invokes
  scripts/smoke-bundle.sh:63 (`cargo build -p "$plugin"`), which executes
  PR-authored build scripts and binaries before any footprint prober would.`
  The cheap hygiene half (confine `command` to the plugin root) was folded into
  §4.2 anyway.
- `DISCARDED-BELOW-FLOOR: a naming collision between the new docs/ tree and the
  repo's docs pipeline is unreachable, because .github/workflows/docs.yml
  uploads only 'target/doc' (its upload-pages-artifact path) and never reads
  docs/.`
- `DISCARDED-BELOW-FLOOR: a tools/list probe leaking a zombie Ghidra JVM is
  unreachable, because the worker boots lazily on the tool-EXECUTION path —
  shared/ghidra-mcp/src/execute.rs:76 calls spawn_boot only from
  wait_until_ready, and shared/ghidra-mcp/src/cli.rs:102 run_serve goes straight
  to server::serve with no boot. initialize + tools/list never starts a JVM.`
  The generic half of that finding — kill the child's process tree rather than
  dropping its pipes — was folded into §4.4. **Round 3 upheld half the
  challenge:** the round-2 fold was scoped to *failures only*, leaving the
  plugin's own host process to leak on every successful probe. That gap is closed
  in §4.4.

  **CORRECTED 2026-09-03, after the stand-down was written: this disposition was
  WRONG, and the original finding was right.** The reasoning cited only the lazy
  path. `shared/ghidra-mcp/src/server.rs:135` also calls
  `state.start_warmup().await` during `serve()`, and `state.rs`'s `start_warmup`
  spawns a background `boot_and_probe` immediately whenever the slot is `Empty`.
  So a probe that reaches `serve()` at all — which, today, means a machine with
  a valid Ghidra config — **does** boot a JVM at startup, before any tool call.
  `execute.rs:76` is the *second* way a boot starts, not the only one.

  The practical risk was already covered by §4.4's reap-on-every-outcome rule,
  which is why this correction changes no requirement. What it changes is the
  reasoning a future reader would rely on. Recorded rather than quietly edited,
  because a stand-down that cites a guard is only as good as the guard, and this
  one cited an incomplete reading of the call graph. It also raises the priority
  of §4.1.1's prerequisite: deferral must gate the *warmup*, not merely the
  config resolution, or a probed server still launches a JVM it will never use.
- `REJECTED: the spec introduces no new Unix-toolchain dependency — jq is
  already used by 7 scripts under scripts/, Justfile:45,51,55,61,70 shell out to
  bash for every existing check, and .github/workflows/ci.yml runs those same
  scripts with 'shell: bash' on windows-latest. The suggested replacement (a
  Rust xtask for the README check) would make this one check the only one not
  following the house style §7 explicitly adopts.` Re-challenged in round 3 on
  the ground that multi-file text surgery in bash is brittler than the existing
  `jq` reads. The rejection stands as to the *claim made* — no new dependency is
  introduced — but the challenge has a fair residue: Fork 1 puts the tool in
  Rust anyway, so which half is shell and which is Rust is a live implementation
  choice, and it is recorded under Fork 1 rather than settled here.
- `REJECTED: the prober would NOT be the first job to natively execute a
  compiled plugin daemon from PR-authored code — scripts/smoke-bundle.sh:122
  invokes scripts/probe-plugin-bin.sh, which at lines 134-142 spawns the plugin
  binary directly via node spawnSync ("Route 2: spawned directly, no shell — how
  an MCP server is started"), on every pull request across the three-OS
  bundle-smoke matrix.` Raised in round 3 as a challenge to the fork-PR
  stand-down above; measured and did not hold.
- `REJECTED: the §4.1 page/byte cap does not ban legitimately large plugins —
  the 8 MiB ceiling is ~435x re-ghidra-mcp-cc's entire 18 379 B tools/list
  payload and corresponds to roughly 2.2 million tokens, which exceeds any
  context window this tool measures against. A payload at that size is the
  runaway the cap exists to catch, not a plugin the marketplace could ship.`
  The narrower half was folded: the failure must name which limit tripped.
- `REJECTED: qwen-extension.json DOES declare an mcpServers block with command
  and args — qwen/re-ghidra-mcp-qwen/qwen-extension.json, "mcpServers":
  {"re-ghidra-mcp-qwen": {"command": "${extensionPath}${/}bin${/}re-ghidra-qwen-mcp",
  "args": ["serve"]}}. The claim that it has none was asserted as verified and
  is false.` The real adjacent defect it missed — the differing placeholder and
  separator tokens — was folded into §4.2.
- `REJECTED: reaping the child does not truncate pagination — §4.4 reaps "After
  measuring, on every outcome", i.e. after §4.1's cursor loop has finished.`
- `REJECTED: a deliberately failed probe does not slip past the ceiling — §6
  layer 1 asserts probe.status == "ok" and probe.toolCount > 0 before any
  budget is evaluated, and states "This runs first and its failure is fatal."`
- `REJECTED: the lower-bound disclosure rule does not break the budget ceiling —
  the ceiling is on tiers.resident.tokens (§6 layer 4) and hook stdout belongs
  to the Session and Invocation tiers (§3), so it never enters the compared
  value; §4.5 already states the gate asserts on the measured subset while the
  disclosure may not.`
- `REJECTED: merge-base baselines do not make a stacked PR "suddenly fail" on
  rebase — a PR based on an unmerged PR is measured against main and therefore
  carries its ancestor's growth as its own, which is stricter; rebasing after
  the ancestor merges moves the merge base forward and makes the check more
  lenient, not less.`
- `REJECTED: generating the root table from all committed footprint documents
  does not force every PR to re-measure every plugin — §7's PR-time check
  compares the README region against committed documents already in the tree.
  Reading a committed document is not probing a binary.`
- `DISCARDED-BELOW-FLOOR: native line endings cannot vary the `bytes` count,
  because §5 pins the serialisation to "no insignificant whitespace" — compact
  JSON contains no line breaks for a platform to render differently.`
