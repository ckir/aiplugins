# GitHub Pages site design

Date: 2026-10-22. Status: draft for owner review. Branch: `feat/pages-site`.

## Goal

A modern public site on GitHub Pages for this monorepo. A visitor lands, sees what
the plugins are, filters by agent (Claude Code, Antigravity, Qwen, OpenCode), copies
the right install command and sees each plugin's context-window cost. The catalog is
generated from repo data on every push to `main`, so it cannot be hand-maintained
into staleness. The existing `cargo doc` deployment keeps working.

## Decisions

- **Purpose:** marketplace storefront (landing page plus filterable catalog). Chosen
  by the owner. A docs hub and a single-page showcase were rejected.
- **Generator:** repo-native `jq` script plus hand-written HTML, CSS and an ES
  module. No Node, no npm, no SSG binary. Chosen by the owner after the AGY-FIRST
  consult (peer and driver agreed; Zola and Astro/VitePress ranked lower).
- **Hosting:** one Pages deployment from the existing `docs.yml`, site at the root,
  rustdoc under `/api/`. Two workflows must never both call `deploy-pages`: a Pages
  deployment replaces the whole site, so the last workflow to finish would erase the
  other's content.
- **Qwen footprint:** the four Qwen plugins have no measurement. The catalog shows a
  "Not measured" badge and the generator emits `footprint: null`. Measuring Qwen is
  a separate task.

## Facts this design depends on

Measured at `e32170a` before writing:

- `agents.json` lists 20 plugin entries: 5 Claude Code, 4 Qwen, 6 OpenCode,
  5 Antigravity.
- `docs/footprints/` holds 16 plugin files plus `budgets.json`. None are Qwen.
- `docs/footprints/todov2-opencode.json` has `resident` 0, `invocation` 0 and a
  `setup` tier of 1603 bytes. Only OpenCode plugins carry a `setup` tier.
- `.github/workflows/docs.yml` triggers on every push to `main` (no `paths:`
  filter), builds `cargo doc --workspace --no-deps --document-private-items` and
  publishes `target/doc` at the site root with a root redirect to
  `rtk_mcp_qwen/index.html`.
- `ci.yml` runs on push and pull request to `main`; `docs.yml` runs only on push to
  `main`.
- Install commands live in no manifest. `publishedNames` remaps directory keys for
  four agents (Claude Code, Qwen, OpenCode, Antigravity; for example
  `aiplugins-devtools-cc` is published as `aiplugins-devtools`), and the
  normalized marketplace files already carry the published name. Qwen's native
  install is broken upstream (QwenLM/qwen-code
  issue 10741), so Qwen uses `scripts/install-qwen-extension.sh`. OpenCode installs
  by file copy or an agent prompt that fetches the plugin's `INSTALL.md`.
  Antigravity unzips a release asset and runs `agy plugin install`, plus a `PATH`
  step for plugins that are not `skillsOnly`.
- `repo-user-guide` has no Qwen build and `todov2` exists only on OpenCode, so the
  agent-by-family matrix is sparse.
- The repo root `package.json` is a stub kept for the OpenCode plugin pin; the site
  must not touch it.

Not yet verified (the plan must check before relying on them, as its step 0): the
exact crate directory names under `target/doc`, that the Pages source is already
set to GitHub Actions (a read-only `gh api repos/ckir/aiplugins/pages`), and that
GitHub Pages serves a root `404.html` for missing paths on this project site.

## Architecture

### Source layout

- `docs/site/index.html`, `docs/site/styles.css`, `docs/site/app.js`: hand-written,
  unminified, no external requests. `index.html` holds one placeholder comment
  where the staging step injects the generated `<noscript>` plugin list.
- `scripts/gen-site.sh`: builds `catalog.json` with `jq`.
- `scripts/stage-site.sh`: assembles `_site/` (steps under Deploy). Both `docs.yml`
  and a pull-request CI job call it, so the staging logic exists once.
- `scripts/check-site.sh` (behind `just site-check`): the invariants below.
- `tests/site/` fixtures: a small fake `agents.json`, marketplace files and
  footprint set, plus expected `catalog.json`.

### Data flow

`gen-site.sh` joins three inputs:

1. `agents.json` and the four generated marketplace files
   (`.claude-plugin`, `.qwen-plugin`, `.opencode-plugin`, `.agy-plugin`) for the
   plugin list, descriptions, versions, `publishedNames` and `skillsOnly`. The
   normalized marketplace files are read, not the raw per-agent manifests, because
   `scripts/gen-marketplaces.sh` already absorbs their differences.
2. `docs/footprints/*.json` and `budgets.json` for the `resident`, `invocation` and
   optional `setup` tiers. A plugin with no footprint file gets `footprint: null`.
3. A per-agent install-template table inside the script, keyed by agent, plugin,
   published name and `skillsOnly`.

The output is `catalog.json`, grouped by plugin family, with agent availability as
a sparse map.

**Family key.** A plugin's key in `agents.json` with its trailing agent suffix
(`-cc`, `-qwen`, `-opencode`, `-agy`) removed. Today that gives six families:
`rtk-mcp`, `re-ghidra-mcp`, `aiplugins-devtools`, `opencode-ctl-mcp`,
`repo-user-guide`, `todov2`. The check fails if a plugin key has no recognized
suffix or maps to a different family than its siblings' descriptions imply.

**`catalog.json` schema** (no timestamp, so the output is reproducible):

```json
{
  "agents": [{ "id": "claude-code", "label": "Claude Code" }],
  "families": [{
    "key": "aiplugins-devtools",
    "description": "string",
    "plugins": {
      "claude-code": {
        "dirKey": "aiplugins-devtools-cc",
        "name": "aiplugins-devtools",
        "version": "string",
        "description": "string",
        "skillsOnly": true,
        "readme": "claude-code/plugins/aiplugins-devtools-cc/README.md",
        "install": [{ "label": "string", "command": "string" }],
        "footprint": { "resident": 0, "invocation": 0, "setup": null,
                       "budget": { } }
      }
    }
  }]
}
```

`name` is the marketplace entry name (already the published name). The family
`description` comes from the first agent that has the family, in the order
claude-code, opencode, antigravity, qwen. `setup` is `null` for tiers an agent
does not measure. `footprint` is `null`, and the UI shows "Not measured", only for
a plugin with no footprint file. `budget` copies the matching `budgets.json` entry
verbatim.

**Install templates.** One table in `gen-site.sh`, one shape per agent:

| Agent | Shape |
|---|---|
| Claude Code | `claude plugin install <name>@aiplugins` after the marketplace-add command |
| Qwen | the `install-qwen-extension.sh` one-liner for the plugin (and the `.ps1` form for Windows) |
| OpenCode | the agent prompt that fetches the plugin's `INSTALL.md` |
| Antigravity | download and unzip the release asset, `agy plugin install <dir>`, plus the `PATH` step unless `skillsOnly` |

The exact commands and the argument each one takes (directory key or published
name) are copied from each plugin's README or `INSTALL.md`; the plan cites the
`path:line` for every one before the table is written.

### Deploy (`docs.yml`, the only Pages workflow)

`docs.yml` builds `cargo doc` as today, then calls `scripts/stage-site.sh`, which:

1. Stages `_site/`: site files, `catalog.json`, the generated `<noscript>` list
   injected at the placeholder, and `target/doc` moved to `_site/api/`.
2. Writes a redirect stub at each old `/<crate>/index.html` pointing to
   `./api/<crate>/index.html`, plus a root `404.html` that maps any other old
   `/<crate>/...` path to the same path under `/api/<crate>/`, so deep rustdoc
   links keep working. Crate names are read from `target/doc` at build time, not
   hardcoded. The old root redirect to `rtk_mcp_qwen` is replaced by the landing
   page.
3. Copies `docs/USER_GUIDE.pdf` to `_site/USER_GUIDE.pdf`.
4. Runs the staged-tree gate.

`docs.yml` then uploads `_site/` as one artifact. If the gate fails, nothing is
uploaded and the previously deployed site stays live. A pull-request CI job runs
the same script (with `cargo doc` cached), so a staging failure is seen before the
merge and not after it.

The built site is never committed. **Rollback:** revert the merge commit; `docs.yml`
redeploys the previous layout.

## Page design

One scrollable page, no router.

1. **Hero:** one-line pitch, the `claude plugin marketplace add ckir/aiplugins`
   command with a copy button, and four agent chips that double as the filter.
2. **Catalog:** one card per plugin family. Each card has the description, a
   version, and an agent tab strip. Agents without the family are greyed out, not
   hidden. Picking a tab swaps the install command, version and footprint bar.
3. **Footprint bar:** stacked resident, invocation and setup bytes against the
   budget. A measured zero reads "0 B". A missing measurement shows a "Not
   measured" badge and no bar, so the two cases are never confused.
4. **Why it matters:** a short explanation of resident versus invocation cost, in
   the wording of the README footprint section.
5. **Footer:** links to `./api/`, the User Guide PDF, GitHub, the changelog and the
   license.

Look and feel: system font stack, colors as tokens on `:root`, dark theme through
`prefers-color-scheme` with a manual toggle remembered in `localStorage` (wrapped
in try/catch so the page works without it), responsive down to phone width,
all URLs relative because the site lives under `/aiplugins/`.

Accessibility: keyboard navigation, ARIA roles on the tabs, visible focus, a live
region announcing "Copied". Without JavaScript, a `<noscript>` list gives plugin
names and README links.

## Verification gates

### `just site-check` (generator output)

The check fails if any of these is false:

- Every plugin entry in `agents.json` appears exactly once.
- Each footprint file's tier totals match the catalog byte for byte, including the
  `setup` tier.
- A plugin with no footprint file is flagged "Not measured" and carries
  `footprint: null`; a plugin without one that is not flagged fails.
- For every agent, `name` equals the marketplace entry name (the published name
  where `publishedNames` remaps it), never the directory key.
- Every plugin entry has exactly one install template, and no install command
  is empty.
- In `catalog.json`, `null` appears only as `footprint` of a plugin flagged
  "Not measured", or as `setup` of a plugin whose agent has no setup tier; no
  string field is empty, and none contains `undefined` or `NaN`.
- The family key rule yields the expected six families and every plugin maps to
  exactly one.
- Each failure names the invariant and the offending plugin.

### Staged-tree gate (in `docs.yml` and runnable locally)

`_site/` must contain `index.html` (with the `<noscript>` placeholder replaced),
`catalog.json`, `404.html`, `USER_GUIDE.pdf`, an `api/<crate>/index.html` and a root
stub for every crate built, and no root-absolute `href="/..."` or `src="/..."`.

The client-side rendering in `app.js` (cards, tabs, footprint bars, copy button)
is not machine-gated, because there is no Node or headless browser in the
toolchain. It is checked by the manual preview below, and that limit is stated
here so a green gate is not read as proof the page renders correctly.

### Repo gates

- `typos` and `mlc` pass on the new files; add exclusions only if a check proves
  one is needed.
- `site-check` joins `just check` and the CI job that already guards generated
  marketplaces and footprints; a second CI job runs `stage-site.sh` on pull
  requests.
- Before any push, preview `_site/` under a `/aiplugins/` prefix locally at phone
  and desktop widths in light and dark. No remote CI iteration to find problems a
  local run would show.

### Tests

Fixture-based, written first: fake inputs in, expected `catalog.json` out. Fixtures
cover a sparse family, a missing-footprint plugin, a `setup`-only plugin and a
`publishedNames` remap.

## Rollout

Steps, one commit each, execution index in memory refreshed after each:

1. Fixtures and `gen-site.sh` until `just site-check` passes.
2. Static page and local preview.
3. `docs.yml` rewrite with the staged-tree gate.
4. `just check` and CI wiring, plus a README link to the live site.

Finishing: AGY-CAPSTONE on the committed implementation, then AGY-TEST-AUDIT,
before declaring complete (in a fresh session if this one is long).

## Risks

- The first deploy replaces the live root with a landing page. Mitigation: crate
  stubs keep old links alive, and the staged tree is verified locally first.
- Install templates can drift from what each plugin's README says. Mitigation: one
  table and a one-template-per-plugin assertion. Comparing the templates against
  the README text is out of scope.
- Crate names under `target/doc` are unverified. Mitigation: read at build time.

## Out of scope

Qwen footprint measurement, per-plugin sub-pages (cards link to each plugin's
README on GitHub), a custom domain, analytics, and the two untriaged anomalies in
`.clavity/local-anomalies.md` (devtools skill-sync checks), which get their own
triage.
