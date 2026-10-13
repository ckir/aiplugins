# aiplugins-devtools: a skills container plugin, first skill `code-review-and-quality`

Status: draft for owner approval. Date: 2026-10-14.

## Intent and why

The owner wants one plugin that acts as a container for dev-tool skills (mostly skills, no
code). It is named `aiplugins-devtools` so that a skill invoked in a host reads
`aiplugins-devtools:<skill>` and the user can always tell the skill came from this repository.

The first content is a port of `code-review-and-quality` from
`/home/user/Development/Rust/agent-skills/skills/code-review-and-quality` (upstream:
`github.com/addyosmani/agent-skills`, MIT, copyright 2025 Addy Osmani, 398-line `SKILL.md`). The
owner treats the port as a BASE and will rework the skill text later ("we use this as base and I
will rule later"). Reworking it is out of scope here.

## Goals

1. A plugin named `aiplugins-devtools` ships to all four supported agents: Claude Code, OpenCode,
   Qwen Code, Antigravity.
2. On each agent the skill is reachable as `aiplugins-devtools:code-review-and-quality` (or the
   host's native namespacing of plugin name + skill name).
3. The skill carries the full upstream five-axis review plus the two upstream checklists as
   bundled reference files, so its security and performance axes can actually read them.
4. Adding the SECOND skill later is cheap: one canonical source, one recipe run, one drift check.
5. Installing it uses the same user-facing mechanism as the sibling plugin on that agent (Claude
   Code marketplace, OpenCode file copy + INSTALL.md, Qwen extension, Antigravity plugin).

## Non-goals

- The adversarial-panel "personas" (palette of seats). The owner will ship those AFTER this, as
  their own piece of work. Not designed here. (A consult on that fork was run and is recorded
  below under "Parked work" so it is not lost.)
- Any other skill from `agent-skills`.
- Rewriting or restyling the skill's text beyond the three mechanical edits listed under "The
  ported skill".
- A Rust binary, MCP server, hook, or agent definition in this plugin.

## Anti-goals (a failure even if it technically works)

- The copies for the four agents drift apart and nobody notices.
- The skill points at files or skills that do not exist in the installed plugin (the upstream
  version does exactly that today).
- The plugin installs but the skill is not discoverable on one of the four agents.
- The upstream copyright and permission notice is lost from the copies.

## Constraints

- Monorepo conventions apply: per-agent folder under `claude-code/`, `opencode/`, `qwen/`,
  `antigravity/`; independent versioning via release-please; marketplaces generated from
  `agents.json` by `scripts/gen-marketplaces.sh`; plugin licence PolyForm-Noncommercial-1.0.0 like
  the siblings.
- Upstream licence is MIT: the copyright line and permission notice must accompany the ported
  files.
- Verified facts about hosts (measured this session):
  - Claude Code, OpenCode and Qwen plugins in this repo already carry `skills/<name>/SKILL.md`
    (e.g. `qwen/re-ghidra-mcp-qwen/skills/ghidra-re-driver/SKILL.md`).
  - Antigravity plugin = directory with `plugin.json` and optional `mcp_config.json`,
    `hooks.json`, `rules/`, `skills/<name>/SKILL.md`. It does NOT define an `agents/` folder
    (source: `~/.gemini/antigravity-cli/builtin/skills/agy-customizations/docs/plugins.md`).
  - The repo has a canonical-source + committed-copies pattern: `shared/ghidra-mcp/skill/SKILL.md`
    and the Justfile recipe `emit-ghidra-skill`.

## Decisions made by the owner

- Plugin name is `aiplugins-devtools`, on every agent, with no per-agent suffix (the agents are
  already separated by their parent folder and their own marketplace file).
- All four agents, like the other plugins.
- The first skill is a port of `code-review-and-quality`, with its two checklists bundled.
- The skill text is "my own version" in the long run, but for now the upstream text is the base.
- Panel personas are a later, separate deliverable.

## Design (calls made by the author; owner can override)

### Layout

Canonical source, the only place the skill is edited:

```
shared/devtools/skills/code-review-and-quality/
    SKILL.md
    references/security-checklist.md
    references/performance-checklist.md
    THIRD-PARTY-NOTICE.md        # MIT notice for the upstream material
```

Per-agent plugins, each containing a committed copy of `skills/` plus that agent's manifest:

```
claude-code/aiplugins-devtools/.claude-plugin/plugin.json   + skills/ + README.md + CHANGELOG.md
opencode/aiplugins-devtools/package.json                    + skills/ + README.md + INSTALL.md + CHANGELOG.md
qwen/aiplugins-devtools/qwen-extension.json                 + skills/ + README.md + CHANGELOG.md
antigravity/aiplugins-devtools/plugin.json                  + skills/ + README.md + CHANGELOG.md
```

Plugin manifests carry `name: aiplugins-devtools`, an initial version `0.1.0` (author's call), the
licence `PolyForm-Noncommercial-1.0.0`, and a one-line description. The Qwen manifest names no
`skills` key unless the build step shows one is needed (see "To confirm while building").

### Keeping copies identical

- Recipe `emit-devtools-skills` in the Justfile copies `shared/devtools/skills/` into the four
  plugin folders, byte for byte, except OpenCode: its skill folder is renamed
  `aiplugins-devtools-code-review-and-quality` and its `name:` frontmatter line changed to match
  (see the OpenCode decision below). Recipe name and location are the author's call.
- Script `scripts/check-devtools-skills.sh` fails if any of the four copies differs from the
  canonical source (`diff -r`), and runs wherever the repo's other `scripts/check-*.sh` run (the
  `just check` path and CI). It also fails if the canonical `SKILL.md` references a file that
  does not exist under the skill folder. The author's call: wire it next to the existing checks
  rather than invent a new gate.
- Honest limit: this proves the four copies agree with each other and with the source. It does
  not prove the skill loads on a host. Loading is checked by the install smoke (below).

### The ported skill

Start from the upstream `SKILL.md` verbatim, then make exactly these edits:

1. Frontmatter: keep `name: code-review-and-quality` and the upstream `description` unchanged.
2. The "See Also" section links `../../references/security-checklist.md` and
   `../../references/performance-checklist.md`. Change both to `references/security-checklist.md`
   and `references/performance-checklist.md` (the files are bundled with the skill).
3. Three sibling-skill mentions point at skills that will not exist in this plugin. Reword each so
   it no longer references a missing skill, keeping the surrounding advice:
   - Section 4 Security opens "For detailed security guidance, see `security-and-hardening`."
     Replace the pointer with the bundled checklist: "For detailed security guidance, see
     `references/security-checklist.md`."
   - Section 5 Performance opens "For detailed profiling and optimization, see
     `performance-optimization`." Replace with "For detailed performance checks, see
     `references/performance-checklist.md`."
   - Step 2 ends "For a project-wide mutation score, see `constraint-driven-development`."
     Delete that sentence.
   - Also in "Dependency Discipline": the last line says "follow the `security-and-hardening`
     skill". Reword to "follow the security guidance in `references/security-checklist.md`".
   (That is four occurrences of the three skill names; the first three above are the headline
   edits, the fourth is the same skill name appearing a second time.)
4. Add a short attribution line under the title naming the upstream source and pointing at
   `THIRD-PARTY-NOTICE.md`.

Everything else, including tone, the five axes, severity labels, checklists and "Common
Rationalizations", is left exactly as upstream. The bundled checklists are copied from
`/home/user/Development/Rust/agent-skills/references/` with one mechanical exception: they name
sibling skills too (measured: `security-checklist.md` line 3 names `security-and-hardening`;
`performance-checklist.md` lines 3 and 135 name `performance-optimization`). Reword or drop
those mentions the same way, keeping the surrounding sentence. The upstream line numbers cited
here (`SKILL.md` 66, 79, 164, 302, 353-354) were read from upstream commit
`28f435ea69a78531ec00779c7f3b78c64ce95896`, the last commit touching these three files; record
that commit in `THIRD-PARTY-NOTICE.md`. After the edits, grep all three files for the names
`security-and-hardening`, `performance-optimization` and `constraint-driven-development` and
expect zero hits.

`THIRD-PARTY-NOTICE.md` contains the MIT permission notice and the line "Copyright (c) 2025 Addy
Osmani", plus the upstream URL and the upstream commit the port was taken from.

### Release and registry wiring

- Add `aiplugins-devtools` to each agent's `plugins` list in `agents.json`; marketplaces are then
  regenerated by `scripts/gen-marketplaces.sh` (never hand-edited).
- Add one entry per agent to `release-please-config.json` and `.release-please-manifest.json`,
  release type `simple`, `extra-files` pointing at that agent's manifest (as the OpenCode
  plugins do with `package.json`). Package paths: `claude-code/aiplugins-devtools`,
  `opencode/aiplugins-devtools`, `qwen/aiplugins-devtools`, `antigravity/aiplugins-devtools`.
- Each plugin gets a README with its install line and a CHANGELOG, like the siblings.
- Add the plugin to the root README install section (OpenCode copy table row, and the per-agent
  install pointers) and to `.agents/plugins.json` only if the Antigravity registration there
  requires it (see below).

### Install and verification

Success is shown by installing each of the four plugins into a throwaway location and observing
the skill present, not by file existence alone:

- Claude Code: `claude plugin install aiplugins-devtools@aiplugins` from a locally built bundle,
  then the skill `aiplugins-devtools:code-review-and-quality` appears in the available-skills
  list.
- OpenCode: copy `skills/*` to `.opencode/skills/` per the sibling INSTALL pattern, confirm the
  skill is listed.
- Qwen: install the extension, confirm the skill is listed.
- Antigravity: install the plugin (`agy plugin install` or the registered path), confirm the
  skill is listed.

If a host cannot be exercised on the build machine, say so in the plan and in the PR, never claim
it verified.

## To confirm while building (resolved in the plan, not here)

Each is a question about code and hosts that exist now; the plan reads and measures them:

1. How `scripts/bundle-plugin.sh`, `bundle-qwen-extension.sh`, `bundle-agy-plugin.sh` and
   `bundle-opencode-plugin.sh` behave for a plugin with no Rust binary (they may assume a built
   executable). Add a skills-only path if they do.
2. Whether `tools/plugin-footprint` (README footprint table, `just footprint-regen`) and
   `scripts/check-*.sh`, `scripts/lib/registry.sh` tolerate a plugin with no MCP server and no
   binary. If not, extend them minimally or exempt this plugin explicitly.
3. Whether the Qwen manifest needs a `skills` key, and the exact Qwen and Antigravity install
   commands (Antigravity docs fetched this session were a redirect page; the local builtin doc
   above is the source used).
4. Whether Antigravity registration in `.agents/plugins.json` is needed for a published plugin
   or only for in-repo development.
5. Whether `claude-code/example`-style `notPublished` conventions apply (not expected).

## Parked work (do not lose)

The owner's NEXT deliverable after this is the adversarial-panel personas (from the clavity skill
`adversarial-panel-review`, section "Seat & persona palette": 2 stances, 2 core seats, 9
specialist seats, a selection rule). A consult of the agy peer on how to ship them was run and
measured on 2026-10-14. Verified results to start from:

- Antigravity plugins cannot carry agent definitions (verified in the local builtin doc above), so
  per-seat `agents/*.md` cannot be the only delivery.
- Agy claimed Qwen extensions cannot carry agents; Qwen's published docs say they can
  (`agents/`). Unverified on this machine; do not rely on either.
- Agy and the author both lean toward: one canonical palette markdown under `shared/`, copied into
  a skill's `references/` on all four agents, seats convened inline, with the same drift check.
  The owner has NOT decided this; it is a later brainstorming session.

## Calls made after the builder check (author's calls, owner can override)

- Distribution pipeline: a skills-only plugin goes through the same zip-and-release pipeline as
  its siblings. If a bundle script or workflow assumes a built binary, extend it minimally with a
  skills-only path rather than exempt the plugin. Measured: the root `Cargo.toml` workspace
  members glob `claude-code/*`, `antigravity/*` and `qwen/*`; a folder there without a
  `Cargo.toml` may break `cargo`. The plan measures this and, if it does, adds an `exclude` entry
  for each `aiplugins-devtools` folder instead of adding a dummy crate. `opencode/*` is not in
  that glob.
- Unverifiable host: if a host (Qwen, Antigravity) cannot be installed on the build machine, the
  PR says so plainly and the plugin still ships for the hosts that were verified; the unverified
  host is released flagged as untested, not dropped and not blocked. The Antigravity install
  command is taken from the local builtin doc, not the redirecting web page.
- Licences: the plugin manifest and README say PolyForm-Noncommercial-1.0.0 like the siblings.
  The ported skill and checklist files stay MIT as upstream and carry `THIRD-PARTY-NOTICE.md`
  inside the skill folder, which the per-agent copies include. A repo-level `LICENSE` file is
  shipped in each plugin only if the siblings ship one.
- Extra dangling references: the port is the listed edits plus any further reference to a missing
  skill or file that the builder finds. The builder FLAGS each extra one in the PR and fixes it
  only if the fix is the same mechanical reword; anything that would change the advice is left
  and flagged. The check script's dangling-name grep (the three sibling names, expecting zero
  hits in the canonical source) runs in CI, not only once, so a later edit cannot reintroduce
  them. Wording fix: the upstream text names the three sibling skills in four places (lines 66,
  79, 164, 302); the edits above cover all four.

## Decision: OpenCode skill name (owner, 2026-10-14)

Goal 2 and the owner's intent ("so I know which repo it comes from") need the prefix visible. The
OpenCode install copies skills into `.opencode/skills/<skill>/`, which would give the bare name
`code-review-and-quality`. Claude Code namespaces by plugin name automatically; Qwen and
Antigravity are to be confirmed. **The owner chose: on OpenCode, use the prefixed folder name
`aiplugins-devtools-code-review-and-quality`.**

Consequences the plan must handle (author's analysis; the first is unmeasured):

1. OpenCode typically requires the skill's `name:` frontmatter to match its folder name. If so,
   the OpenCode copy must set `name: aiplugins-devtools-code-review-and-quality`, so that copy is
   NOT byte-identical to the canonical source. The plan reads OpenCode's skill loader/docs to
   confirm, and if the match is required, the emit recipe rewrites exactly that one frontmatter
   line (and the folder name) for OpenCode and copies everything else unchanged.
2. The drift check (`scripts/check-devtools-skills.sh`) compares the three non-OpenCode copies
   byte for byte, and compares the OpenCode copy after applying the same single-line rename, so
   it still fails on any other difference.
3. The skill is invoked on OpenCode by the prefixed name, which differs from the form
   `aiplugins-devtools:code-review-and-quality` used elsewhere (a colon is not valid in a folder
   name). The OpenCode README and INSTALL.md say so explicitly.
4. The `references/` and `THIRD-PARTY-NOTICE.md` files sit inside the renamed folder and are
   referenced by relative path, so they need no change.

## Not decided, and why it is fine

- Initial version `0.1.0` and recipe/script names: author's calls, trivially renamed.
- Whether to also expose the skill under a bare name: not needed. Claude Code namespacing gives
  `aiplugins-devtools:<skill>`; OpenCode uses the prefixed folder name (see the decision above);
  Qwen and Antigravity are confirmed while building.

## Self-audit of this document

Gaps closed in the document: skill edits enumerated exactly; layout, recipe, check script and
release wiring named; licence handling stated; verification defined per host; every non-goal
stated. Gaps left open ON PURPOSE and where they resolve: the five items under "To confirm while
building" resolve in the implementation plan, after reading the bundle scripts and tools that
exist now. Nothing here cites a line number in code that has not been read.
