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
- **Hosting:** one Pages deployment from the existing `docs.yml`. The rustdoc output
  stays at the site root, where it is today, so every existing rustdoc URL keeps
  working with no redirects; the landing page takes over only the root
  `index.html`. Chosen by the owner after the AGY-FIRST consult and the panel's
  round 1 (peer and driver both recommended it over moving rustdoc to `/api/`,
  which had four concrete defects: wrong relative stubs, no `api/index.html`,
  non-crate directories in the crate list, and a depth-sensitive `404.html`).
  Two workflows must never both call `deploy-pages`: a Pages deployment replaces
  the whole site, so the last workflow to finish would erase the other's content.
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
- `docs/footprints/budgets.json` budgets only `residentBytes` (and `setupBytes` on
  four OpenCode plugins); it has no `invocation` budget. The README says the
  footprint figure is "not a context budget".
- The marketplace entries carry no `version` field. The version is the
  `-v<semver>` release tag inside `source.url`, present for every entry of all four
  marketplaces, and every entry has a `homepage` (a GitHub tree URL).
- The two OpenCode `skillsOnly` plugins (`aiplugins-devtools-opencode`,
  `repo-user-guide-opencode`) have no `setup` tier in their footprint file.
- Marketplace descriptions are agent-branded: the Claude Code entries for
  `rtk-mcp`, `re-ghidra-mcp` and `opencode-ctl-mcp` say "Claude Code".
- Plugin READMEs hardcode release versions that lag the marketplace (for example
  `rtk-mcp-agy-v0.7.2` in the README against `v0.9.0` in `.agy-plugin`), and
  `repo-user-guide-agy`'s `INSTALL.md` is a skill-copy procedure with no
  `agy plugin install`.
- `scripts/gen-marketplaces.sh` strips the carriage returns that a Windows `jq`
  emits (its `jqr` helper); `gen-marketplaces.sh --check` runs in CI but not in
  `just check`.
- `.gitignore` and the `mlc` ignore list in the `Justfile` both cover `target/`
  but not any other build-output directory.
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

Not yet verified (the plan must check before relying on them, as its step 0, by
running `cargo doc` once): the crate directories under `target/doc`; that the
non-crate directories rustdoc writes there (`src`, `static.files`, `trait.impl`,
`implementors`) have no top-level `index.html`, which is how the crate list tells
them apart; that no rustdoc output path equals a site file name (`index.html`,
`styles.css`, `app.js`, `catalog.json`, `USER_GUIDE.pdf`, `api`); and that the
Pages source is already set to GitHub Actions (a read-only
`gh api repos/ckir/aiplugins/pages`).

## Architecture

### Source layout

- `docs/site/index.html`, `docs/site/styles.css`, `docs/site/app.js`: hand-written,
  unminified, no external requests. `index.html` holds one placeholder comment
  where the staging step injects the generated `<noscript>` plugin list.
- `scripts/gen-site.sh`: builds `catalog.json` with `jq`.
- `scripts/stage-site.sh`: assembles `target/site/` (steps under Deploy; `target/`
  is already ignored by git and by the `mlc` link check). Both `docs.yml`
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
`repo-user-guide`, `todov2`. The check strips `-(cc|qwen|opencode|agy)$` from every
plugin key in `agents.json` and fails unless the result is exactly that set.
`gen-site.sh` and `check-site.sh` read `jq` output through the same carriage-return
stripping helper as `gen-marketplaces.sh`, so they work on a Windows `jq`.

**`catalog.json` schema** (no timestamp, so the output is reproducible):

```json
{
  "agents": [{ "id": "claude-code", "label": "Claude Code" }],
  "families": [{
    "key": "aiplugins-devtools",
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

Joins use the directory key, because the marketplace files and the footprint
files' internal `plugin` field carry the published name, not the directory key:
`expectedName = publishedNames[dirKey] // dirKey` finds the marketplace entry, and
the footprint file and `budgets.json` entry are looked up by `dirKey`.

- `name` is the marketplace entry name (already the published name).
- `version` is the `-v<semver>` tag from the entry's `source.url`.
- `description` is the entry's own description. There is no family-level
  description, so a card never shows one agent's branding on another agent's tab.
- `readme` is the entry's `homepage`, a full GitHub URL.
- `footprint` is `null`, and the UI shows "Not measured", only for a plugin with no
  footprint file.
- `setup` is `null` exactly when that plugin's footprint file has no `tiers.setup`
  (so Qwen, non-OpenCode plugins and the OpenCode `skillsOnly` plugins).
- `budget` copies the matching `budgets.json` entry verbatim; it can lack
  `setupBytes`, and it never has an invocation figure.

**Install templates.** One table in `gen-site.sh`, one shape per agent:

| Agent | Shape |
|---|---|
| Claude Code | `claude plugin install <name>@aiplugins` after the marketplace-add command |
| Qwen | the `install-qwen-extension.sh` one-liner for the plugin (and the `.ps1` form for Windows) |
| OpenCode | the agent prompt that fetches the plugin's `INSTALL.md` |
| Antigravity | download and unzip the release asset, `agy plugin install <dir>`, plus the `PATH` step unless `skillsOnly` |

Qwen's `install` has two labeled entries (shell and PowerShell); every other
agent has at least one. Release URLs and versions are never copied from README
text, which lags the marketplace; they come from the entry's `source.url`. Only the
command shape and argument (directory key or published name) are taken from each
plugin's README or `INSTALL.md`, and the plan cites the `path:line` for every one
before the table is written. A plugin whose own documentation differs from its
agent's shape (`repo-user-guide-agy` copies a skill folder instead of running
`agy plugin install`) gets a per-plugin override in the table, and the check
requires every plugin to resolve to one shape or one override.

### Deploy (`docs.yml`, the only Pages workflow)

`docs.yml` builds `cargo doc` as today, then calls `scripts/stage-site.sh`, which:

1. Starts `target/site/` clean and copies `target/doc` into it (a copy, so
   re-running the script locally works without rebuilding rustdoc).
2. Fails if any site file name already exists in the copy: `index.html`,
   `styles.css`, `app.js`, `catalog.json`, `USER_GUIDE.pdf` or `api`. The old
   `docs.yml` step that wrote a root redirect to `rtk_mcp_qwen` is removed, so
   rustdoc never supplies a root `index.html`.
3. Copies in the files from `docs/site/` and `catalog.json`, and injects the
   generated `<noscript>` list at the placeholder.
4. Writes `api/index.html`, a plain list linking each crate's
   `../<crate>/index.html`. The crates are the top-level directories of the copy
   that contain an `index.html`, so rustdoc's non-crate directories are skipped.
5. Copies `docs/USER_GUIDE.pdf` to the root.
6. Runs the staged-tree gate.

`docs.yml` then uploads `target/site/` as one artifact. If the gate fails, nothing
is uploaded and the previously deployed site stays live. A pull-request CI job runs
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
   hidden. Picking a tab swaps the description, install command, version and
   footprint bar, because each agent's description is its own. With the "All"
   filter a card opens on its first available agent in the order claude-code,
   opencode, antigravity, qwen.
3. **Footprint bar:** three labeled figures, because the budgets cover only two of
   the tiers. Resident is drawn against `budget.residentBytes`, and setup (when
   present) against `budget.setupBytes`. Invocation is shown as a plain number
   with no ceiling, since it loads only when a skill runs and `budgets.json` has no
   figure for it. Nothing is stacked against a budget, and a note repeats the
   README's "compare between plugins, not a context budget". A measured zero reads
   "0 B". A missing measurement shows a "Not measured" badge and no bar, so the two
   cases are never confused.
4. **Why it matters:** a short explanation of resident versus invocation cost, in
   the wording of the README footprint section.
5. **Footer:** links to the API docs, the User Guide PDF, GitHub, and the changelog
   and license as GitHub blob URLs (those two files are not staged into the site).
   Each card's README link is the entry's `homepage`, also a full GitHub URL.

Look and feel: system font stack, colors as tokens on `:root`, dark theme through
`prefers-color-scheme` with a manual toggle remembered in `localStorage` (wrapped
in try/catch so the page works without it), responsive down to phone width,
all URLs relative because the site lives under `/aiplugins/`.

Accessibility: keyboard navigation, ARIA roles on the tabs, visible focus, a live
region announcing "Copied". Without JavaScript, a `<noscript>` list gives plugin
names and README links.

## Verification gates

### `just site-check` (generator output)

The check first runs `bash scripts/gen-marketplaces.sh --check`, so it never
validates against stale marketplace files. It then fails if any of these is false:

- Every plugin entry in `agents.json` appears exactly once.
- Each footprint file's tier totals match the catalog byte for byte, including the
  `setup` tier.
- A plugin with no footprint file is flagged "Not measured" and carries
  `footprint: null`; a plugin without one that is not flagged fails.
- For every agent, `name` equals the marketplace entry name (the published name
  where `publishedNames` remaps it), never the directory key.
- Every plugin resolves to exactly one install shape or override, has at least one
  `install` entry, and no install command is empty.
- In `catalog.json`, `null` appears only as `footprint` of a plugin with no
  footprint file, or as `setup` of a plugin whose footprint file has no
  `tiers.setup`; no string field is empty, and none contains `undefined` or `NaN`.
- Each `version` matches the `-v<semver>` tag of its entry's `source.url`.
- The family key rule yields exactly the six expected families and every plugin
  maps to exactly one.
- Each failure names the invariant and the offending plugin.

### Staged-tree gate (in `docs.yml` and runnable locally)

`target/site/` must contain `index.html` (with the `<noscript>` placeholder
replaced), `catalog.json`, `USER_GUIDE.pdf`, and `api/index.html` linking every
crate directory that has an `index.html`, with each of those links resolving to an
existing file. The footer's API link must point at `api/index.html`. The files
this script writes or copies from `docs/site/` contain no root-absolute
`href="/..."` or `src="/..."` (rustdoc's own files are not checked).

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
- Before any push, preview `target/site/` under a `/aiplugins/` prefix locally (a
  static server over a directory holding an `aiplugins` link to it) at phone and
  desktop widths in light and dark. No remote CI iteration to find problems a
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

- The first deploy replaces the live root redirect with a landing page. Rustdoc
  paths do not move, so existing links keep working. Mitigation: the staged tree is
  verified locally first.
- A future crate or rustdoc output file could collide with a site file name.
  Mitigation: the staging script fails on any collision, and the PR job runs it.
- Install templates can drift from what each plugin's README says. Mitigation: one
  table and a one-template-per-plugin assertion. Comparing the templates against
  the README text is out of scope.
- How rustdoc lays out `target/doc` is unverified here. Mitigation: plan step 0
  runs `cargo doc` and checks it before the staging script is written; the crate
  list is derived at build time, never hardcoded.

## Out of scope

Qwen footprint measurement, per-plugin sub-pages (cards link to each plugin's
README on GitHub), a custom domain, analytics, and the two untriaged anomalies in
`.clavity/local-anomalies.md` (devtools skill-sync checks), which get their own
triage.
