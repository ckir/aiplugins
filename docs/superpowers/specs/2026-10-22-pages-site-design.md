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
- Install commands live in no manifest. Claude Code remaps directory keys through
  `publishedNames`. Qwen's native install is broken upstream (QwenLM/qwen-code
  issue 10741), so Qwen uses `scripts/install-qwen-extension.sh`. OpenCode installs
  by file copy or an agent prompt that fetches the plugin's `INSTALL.md`.
  Antigravity unzips a release asset and runs `agy plugin install`, plus a `PATH`
  step for plugins that are not `skillsOnly`.
- `repo-user-guide` has no Qwen build and `todov2` exists only on OpenCode, so the
  agent-by-family matrix is sparse.
- The repo root `package.json` is a stub kept for the OpenCode plugin pin; the site
  must not touch it.

Not yet verified (the plan must check before relying on them): the exact crate
directory names under `target/doc`, and that the Pages source is already set to
GitHub Actions.

## Architecture

### Source layout

- `docs/site/index.html`, `docs/site/styles.css`, `docs/site/app.js`: hand-written,
  unminified, no external requests.
- `scripts/gen-site.sh`: builds `catalog.json` with `jq`.
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

### Deploy (`docs.yml`, the only Pages workflow)

1. Build `cargo doc` as today.
2. Stage `_site/`: site files, `catalog.json`, `target/doc` moved to `_site/api/`.
3. Write a redirect stub at each old `/<crate>/index.html` pointing to
   `./api/<crate>/index.html`, so existing links do not 404. Crate names are read
   from `target/doc` at build time, not hardcoded. The old root redirect to
   `rtk_mcp_qwen` is replaced by the landing page.
4. Copy `docs/USER_GUIDE.pdf` to `_site/USER_GUIDE.pdf`.
5. Run the staged-tree gate, then upload `_site/` as one artifact.

The built site is never committed.

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
- The Claude Code install command uses the published name.
- Every plugin entry has exactly one install template.
- The rendered output contains no literal `null`, `undefined` or `NaN`.

### Staged-tree gate (in `docs.yml` and runnable locally)

`_site/` must contain `index.html`, `catalog.json`, `USER_GUIDE.pdf`, an
`api/<crate>/index.html` and a root stub for every crate built, and no root-absolute
`href="/..."` or `src="/..."`.

### Repo gates

- `typos` and `mlc` pass on the new files; add exclusions only if a check proves
  one is needed.
- `site-check` joins `just check` and the CI job that already guards generated
  marketplaces and footprints.
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
