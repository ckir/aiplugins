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
- `scripts/install-qwen-extension.sh:37` downloads
  `releases/latest/download/<ext>-extension.zip`, and the PowerShell form accepts
  only three plugins (`ValidateSet` at `install-qwen-extension.ps1:24,34`, which
  excludes `aiplugins-devtools-qwen`). GitHub's "latest" is the repo's newest
  release, which today is `opencode-ctl-mcp-agy-v0.2.0` and carries no
  `*-extension.zip`, so the script's download 404s for the Qwen plugins. The
  `aiplugins-devtools-qwen` README already uses a tag-pinned manual download. The
  site therefore builds Qwen commands from `source.url` and does not use the
  script; the script's defect is logged as an anomaly, not fixed here.
- Install commands live in no manifest. `publishedNames` remaps directory keys for
  four agents (Claude Code, Qwen, OpenCode, Antigravity; for example
  `aiplugins-devtools-cc` is published as `aiplugins-devtools`), and the
  normalized marketplace files already carry the published name. Qwen's native
  install is broken upstream (QwenLM/qwen-code
  issue 10741), so the repo's Qwen READMEs offer `scripts/install-qwen-extension.sh`
  and, for three of the four plugins, a manual download-and-extract; the site uses
  the manual form (next bullets). OpenCode installs
  by file copy or an agent prompt that fetches the plugin's `INSTALL.md`.
  Antigravity unzips a release asset and runs `agy plugin install`, plus a `PATH`
  step for plugins that are not `skillsOnly`.
- `repo-user-guide` has no Qwen build and `todov2` exists only on OpenCode, so the
  agent-by-family matrix is sparse.
- The repo root `package.json` is a stub kept for the OpenCode plugin pin; the site
  must not touch it.

Measured in panel round 2 by running `cargo doc --workspace --no-deps
--document-private-items` into a temporary directory, then re-counted by the
driver on that output: `target/doc` holds 40 top-level crate directories (each with
an `index.html`), 5 non-crate directories without one (`search.index`, `src`,
`static.files`, `trait.impl`, `type.impl`) and 5 top-level files (`.lock`,
`crates.js`, `help.html`, `settings.html`, `src-files.js`), 25 MB in 1,677 files.
None of those names equals a site file name. Sixteen of the 40 crate directories
are unpublished examples, test mocks or maintainer tools. A cold build took about
37 seconds.

Not yet verified (plan step 0): that the Pages source is already set to GitHub
Actions (a read-only `gh api repos/ckir/aiplugins/pages`), and a re-run of the
`cargo doc` measurement above on the branch the plan starts from.

## Architecture

### Source layout

- `docs/site/index.html`, `docs/site/styles.css`, `docs/site/app.js`: hand-written,
  unminified, no external requests. `index.html` holds the placeholder comment
  `<!-- NOSCRIPT -->` on a line of its own, where the staging step injects the
  generated `<noscript>` plugin list.
- `scripts/gen-site.sh`: builds `catalog.json` with `jq`.
- `scripts/stage-site.sh`: assembles `target/site/` (steps under Deploy; `target/`
  is already ignored by git and by the `mlc` link check). Both `docs.yml`
  and a pull-request CI job call it, so the staging logic exists once.
- `scripts/check-site.sh` (behind `just site-check`): the invariants below.
- `tests/site/` fixtures: `tests/site/root/` (a small fake `agents.json`,
  marketplace files and footprint set, with one description containing `&`),
  `tests/site/doc/` (the synthetic rustdoc tree for selftest part (c)) and the
  expected `catalog.json`.

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

**Input fields (measured).** `agents.json`: `.agents[]` with `id`, `plugins[]`,
`publishedNames{}`, `skillsOnly[]` and `marketplace`, the path of that agent's
marketplace file (`.claude-plugin/marketplace.json`, `.qwen-plugin/marketplace.json`,
`.opencode-plugin/marketplace.json`, `.agy-plugin/marketplace.json`). Marketplace
files: `.plugins[]` with `name`, `description`, `homepage` and `source.url`.
Footprint files `docs/footprints/<dirKey>.json`: `pluginVersion` and
`tiers.{resident,invocation,setup}.bytes`. `budgets.json`:
`<dirKey>.{residentBytes,setupBytes}`. A plugin "ships binaries" when its key is not
in its own agent's `skillsOnly` list.

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
        "version": "0.1.0",
        "description": "string",
        "skillsOnly": true,
        "readme": "https://github.com/ckir/aiplugins/tree/main/claude-code/plugins/aiplugins-devtools-cc",
        "install": [{ "label": "string", "command": "string" }],
        "footprint": { "resident": 0, "invocation": 0, "setup": null,
                       "budget": { "residentBytes": 2369 } }
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
- `version` is the bare `<semver>` after the `-v` in the entry's `source.url`, with
  no `v` prefix; `app.js` adds the `v` when it displays it.
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
| Claude Code | two entries: `claude plugin marketplace add ckir/aiplugins`, then `claude plugin install <name>@aiplugins`, so a card works on its own |
| Qwen | two entries (shell, PowerShell): download the entry's tag-pinned `source.url` and extract it into `~/.qwen/extensions/<name>`, plus the executable-bit step for plugins that ship binaries. `install-qwen-extension.sh` is not used (see Facts) |
| OpenCode | the agent prompt that fetches the plugin's `INSTALL.md` |
| Antigravity | download and unzip the release asset, `agy plugin install <dir>`, plus the `PATH` step unless `skillsOnly` |

Claude Code and Qwen plugins each have exactly two labeled `install` entries;
every other plugin has at least one, and the check asserts those counts per agent.
Labels are fixed strings: Claude Code "Add the marketplace" and "Install"; Qwen
"Linux and macOS" and "Windows (PowerShell)"; OpenCode "Ask your agent";
Antigravity "Install"; the `repo-user-guide-agy` override "Copy the skill".
Release URLs and versions are never copied from README
text, which lags the marketplace; they come from the entry's `source.url`. Only the
command shape and argument (directory key or published name) are taken from each
plugin's README or `INSTALL.md`, and the plan cites the `path:line` for every one
before the table is written. Qwen is the documented exception: its shape is
synthesized from the manual download-and-extract sections of the
`rtk-mcp-qwen`, `re-ghidra-mcp-qwen` and `aiplugins-devtools-qwen` READMEs (the
`unzip` and `Expand-Archive` lines at `rtk-mcp-qwen/README.md:55,61`,
`re-ghidra-mcp-qwen/README.md:96,102` and `aiplugins-devtools-qwen/README.md:38,44`)
plus the `chmod 755` step at `scripts/install-qwen-extension.sh:88`, because
`opencode-ctl-mcp-qwen`'s README documents only the script. A plugin whose own
documentation differs from its
agent's shape (`repo-user-guide-agy` copies a skill folder instead of running
`agy plugin install`) gets a per-plugin override in the table, and the check
requires every plugin to resolve to one shape or one override.

### Deploy (`docs.yml`, the only Pages workflow)

`docs.yml` builds `cargo doc` as today, then calls `scripts/stage-site.sh`. With its
defaults the script works on `.`, `target/doc` and `target/site`; every step below
uses the `--root`, `--doc-dir` and `--out` values it was given and never a fixed
path, so the selftest can run it over fixtures without touching the real output.
It:

1. Starts the output directory clean and copies the doc directory into it (a copy,
   so re-running the script locally works without rebuilding rustdoc).
2. Fails if any site file name already exists in the copy: `index.html`,
   `styles.css`, `app.js`, `catalog.json`, `USER_GUIDE.pdf` or `api`. The old
   `docs.yml` step that wrote a root redirect to `rtk_mcp_qwen` is removed, so
   rustdoc never supplies a root `index.html`. The message names the colliding
   path and says to run `cargo clean --doc`, since a stale `target/doc/index.html`
   from an older local run is the likely cause.
3. Runs `bash scripts/gen-site.sh --root <root> <out>/catalog.json`, then
   `bash scripts/check-site.sh --root <root> <out>/catalog.json`, so a bad catalog
   stops the script here. Copies in the files from `docs/site/` and injects the
   generated `<noscript>` list at the placeholder: one `<li>` per plugin entry, with
   the name linked to its `readme`, the agent label and the description. The list
   is HTML-escaped (`&`, `<`, `>`,
   `"`) and spliced in by printing the file around the marker line, not by a
   substitution that treats `&` specially.
4. Writes `api/index.html`, a plain list linking each crate's
   `../<crate>/index.html`. A crate is a top-level directory of the copy that
   contains an `index.html`, found before `api/` exists (so `api` is never its own
   entry), and whose name matches none of the deny patterns `*example*`, `mock_*`,
   `probe_target`, `footprint_gate` and `plugin_footprint` (unpublished examples,
   test mocks and maintainer tools: 16 of today's 40). Denied crates stay on the
   site and reachable by URL; they are only left out of the list.
5. Copies `docs/USER_GUIDE.pdf` to the root.
6. Runs the staged-tree gate.

`docs.yml` then uploads `target/site/` as one artifact. If the gate fails, nothing
is uploaded and the previously deployed site stays live. A pull-request CI job runs
the same script, so a staging failure is seen before the merge and not after it.
That job pays a cold `cargo doc` (about 37 seconds, measured); no cache for
rustdoc output is assumed.

The built site is never committed. **Rollback:** revert the merge commit; `docs.yml`
redeploys the previous layout.

## Page design

One scrollable page, no router.

1. **Hero:** one-line pitch, the `claude plugin marketplace add ckir/aiplugins`
   command labeled for Claude Code with a copy button, and four agent chips that
   double as the filter. Each card also carries its own full install steps, so no
   card depends on the hero command.
2. **Catalog:** one card per plugin family. Each card has the description, a
   version, and an agent tab strip. Agents without the family are greyed out, not
   hidden. Picking a tab swaps the description, install command, version and
   footprint bar, because each agent's description is its own. A card renders every
   entry of `install[]`, each with its label and its own copy button. With the "All"
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
4. **Why it matters:** a short explanation of resident, invocation and (OpenCode
   only) setup cost, in the wording of the README footprint section.
5. **Footer:** links to the API docs, the User Guide PDF, GitHub, and the changelog
   and license as GitHub blob URLs (those two files are not staged into the site).
   Each card's README link is the entry's `homepage`, also a full GitHub URL.

Look and feel: system font stack, colors as tokens on `:root`, dark theme through
`prefers-color-scheme` with a manual toggle remembered in `localStorage` (wrapped
in try/catch so the page works without it), responsive down to phone width,
all URLs relative because the site lives under `/aiplugins/`.

Accessibility: keyboard navigation, ARIA roles on the tabs, visible focus, a live
region announcing "Copied". Without JavaScript, a `<noscript>` list gives one
item per plugin entry (20 today): the name linked to its README, the agent, and the
description.

## Verification gates

### `just site-check` (generator output)

The recipe runs, in order: `bash scripts/gen-marketplaces.sh --check` (so it never
validates against stale marketplace files); `bash scripts/gen-site.sh
target/site-check/catalog.json` (under `target/`, so the working tree stays
clean); `bash scripts/check-site.sh target/site-check/catalog.json`; and `bash
scripts/check-site.sh --selftest`.

Command-line conventions, so the three scripts and the recipe agree: every script
is invoked as `bash scripts/<name>.sh`. `gen-site.sh [--root DIR] OUT` and
`check-site.sh [--root DIR] CATALOG` default `--root` to `.`; `stage-site.sh
[--root DIR] [--doc-dir DIR] [--out DIR]` defaults to `.`, `target/doc` and
`target/site`, and passes its `--root` to both of the other scripts. The `--root`,
`--doc-dir` and `--out` arguments exist so the selftest can run the real scripts
over fixtures without touching the repository. `check-site.sh` never calls
`gen-marketplaces.sh` itself; the recipe does. The check fails if any of these is
false:

- Every plugin entry in `agents.json` appears exactly once.
- Each footprint file's tier totals match the catalog byte for byte, including the
  `setup` tier.
- `footprint` is `null` exactly when `docs/footprints/<dirKey>.json` does not
  exist, and non-null otherwise. (The "Not measured" badge is drawn from that null
  by `app.js` and is not machine-checked.)
- For every agent, `name` equals the marketplace entry name (the published name
  where `publishedNames` remaps it), never the directory key.
- Every plugin resolves to exactly one install shape or override, has at least one
  `install` entry, and no install command is empty.
- In `catalog.json`, `null` appears only as `footprint` of a plugin with no
  footprint file, or as `setup` of a plugin whose footprint file has no
  `tiers.setup`; no string field is empty, and none contains `undefined` or `NaN`.
- `catalog.json`'s `agents` list equals the registry's agents in the UI order
  claude-code, opencode, antigravity, qwen, with the labels `Claude Code`,
  `OpenCode`, `Antigravity` and `Qwen` (the `agent-set` invariant).
- Each `version` matches `^[0-9]+\.[0-9]+\.[0-9]+$`, equals the tag in its entry's
  `source.url`, and equals `pluginVersion` in the plugin's footprint file whenever
  one exists (measured equal for all 16 today).
- `readme` equals the entry's `homepage`, and every URL in `readme` and inside
  `install[].command` starts with `https://github.com/ckir/aiplugins/` or
  `https://raw.githubusercontent.com/ckir/aiplugins/` (the OpenCode `INSTALL.md`
  prompt uses the second form).
- Claude Code and Qwen plugins have exactly two `install` entries; every other
  plugin has at least one.
- `footprint.budget` equals `budgets.json[dirKey]`, and `skillsOnly` equals
  membership in that agent's `skillsOnly` list in `agents.json`.
- `check-site.sh --selftest` has three parts. (a) For each fixture input set under
  `tests/site/`, `gen-site.sh --root` plus `check-site.sh --root` must accept the
  positive fixture, and `gen-site.sh` must refuse inputs that break a generator
  precondition (for example a plugin key with no known agent suffix). (b) From the
  positive fixture's generated catalog, a loop of one-line `jq` mutations in a
  temporary directory, one per invariant in this list (plugin set by dropping or
  duplicating an entry, tier totals, budget, `skillsOnly`, name, version, readme,
  URL prefix, install counts, null and empty fields, the family set, the agent
  list), is fed
  straight to `check-site.sh`, without
  regenerating, and each must fail naming that invariant. Part (b) is what keeps
  the invariants from being circular: a working generator always agrees with its
  own inputs. (c) `stage-site.sh --root tests/site/root --doc-dir tests/site/doc
  --out <temp>` runs over the fixture inputs and a small synthetic doc tree (two
  crate directories, one denied crate, a `src` directory with no `index.html`;
  one fixture description contains `&`). The positive run must stage and pass
  the gate, the staged `index.html` must contain the description as `&amp;` and
  no raw `&` from it, and a copy of the doc tree with a root `index.html` must be
  refused as a collision. This tests the staging logic in well under a second
  without a cold `cargo doc`. Part (c) needs `stage-site.sh` and the real
  `docs/site/` files, so it joins the selftest in rollout step 3; steps 1 and 2
  run parts (a) and (b) only.
- The family key rule yields exactly the six expected families and every plugin
  maps to exactly one.
- Each failure names the invariant and the offending plugin. The family-set and
  missing-install-shape failures also name the file to update (the expected
  families in `check-site.sh`, the install table in `gen-site.sh`), so adding a
  seventh family is a guided edit and not a puzzle.

### Staged-tree gate (in `docs.yml` and runnable locally)

`target/site/` must contain `index.html` (with the `<noscript>` placeholder
replaced), `styles.css`, `app.js`, `catalog.json`, `USER_GUIDE.pdf` and
`api/index.html`.

The gate recomputes the crate set itself, independently of step 4: the top-level
directories of `target/site/`, other than `api`, that contain an `index.html`,
minus the deny patterns (one constant in `stage-site.sh`, used by both step 4 and
the gate). That set must be non-empty, and `api/index.html` must link exactly that
set, with each link resolved relative to `api/` (`../<crate>/index.html` must exist
as `target/site/<crate>/index.html`) and no denied crate listed. Recomputing it
means a bug in step 4's scan or filter shows up as a mismatch, not as a gate that
passes on zero links. The footer's API link must point at `api/index.html`.

The files this script writes or copies from `docs/site/` contain no root-absolute
`href="/..."` or `src="/..."` (rustdoc's own files are not checked). `app.js`
contains no `innerHTML`; catalog strings go in through `textContent` or attribute
setters, so placeholders such as `<dir>` display as text. That check lives in the
gate, which `site-check` first reaches through selftest part (c) in rollout step 3;
it cannot run earlier because `app.js` does not exist until rollout step 2.

The client-side rendering in `app.js` (cards, tabs, footprint bars, copy button)
is not machine-gated, because there is no Node or headless browser in the
toolchain. It is checked by the manual preview below, and that limit is stated
here so a green gate is not read as proof the page renders correctly.

### Repo gates

- `typos` and `mlc` pass on the new files; add exclusions only if a check proves
  one is needed.
- `site-check` (which includes the `--selftest` run) joins `just check` and the
  `wiring` job of `ci.yml`, the job that already runs
  `gen-marketplaces.sh --check`. A second job runs `stage-site.sh` on pull requests
  only (`if: github.event_name == 'pull_request'`), because `ci.yml` also runs on
  push to `main`, where `docs.yml` already stages.
- Before any push, preview `target/site/` under a `/aiplugins/` prefix locally: a
  `just site-preview` recipe copies it to `target/preview/aiplugins` (a copy, not
  a symlink, because creating symlinks needs elevation on Windows) and a static
  server serves `target/preview`. Check at phone and desktop widths in light and
  dark. No remote CI iteration to find problems a
  local run would show.

### Tests

Fixture-based, written first: fake inputs in, expected `catalog.json` out. Fixtures
cover a sparse family, a missing-footprint plugin, a `setup`-only plugin and a
`publishedNames` remap. The checker gets in-memory mutations of the positive
catalog (see `--selftest`), one per invariant, and the staging script gets the
synthetic `tests/site/doc/` tree. The real `cargo doc` run stays in the
pull-request job, which tests the staging against actual rustdoc output.

## Rollout

Steps, one commit each, execution index in memory refreshed after each:

1. `.gitattributes` pins (`docs/site/**` and `tests/site/**` as `text eol=lf`, so
   the marker-line splice and the fixture comparisons behave on a Windows
   checkout), then fixtures, `gen-site.sh` and `check-site.sh` until `just
   site-check` passes with selftest parts (a) and (b). `gen-site.sh` writes LF
   output through the same carriage-return helper it reads with.
2. Static page and local preview (`just site-preview`, which serves
   `target/preview` with `python3 -m http.server --directory target/preview`, or
   `python` where `python3` is absent; a preview aid, not a gate).
3. `stage-site.sh`, the staged-tree gate, selftest part (c) with its
   `tests/site/root` and `tests/site/doc` fixtures, and the `docs.yml` rewrite.
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
- A new internal crate whose name matches no deny pattern shows up in
  `api/index.html` until it is added to the deny constant. Mitigation: it is one
  constant in `stage-site.sh`, and plan step 0 prints today's list for review. The
  crate list itself is derived at build time, never hardcoded.

## Out of scope

Qwen footprint measurement, per-plugin sub-pages (cards link to each plugin's
README on GitHub), a custom domain, analytics, fixing
`scripts/install-qwen-extension.sh` (the site does not use it; the defect is logged
as an anomaly), and the other untriaged anomalies in `.clavity/local-anomalies.md`,
which get their own triage.

## Review record

Adversarial panel, five rounds (solo seats plus the live agy peer each round). Every
finding from rounds 1-5 was folded after the driver measured it; none was stood
down as below-floor. The owner stopped the panel at the round-6 cap and took the
spec to their own review. The panel did not reach a round with no live challenge,
so this is not a GREEN verdict.

## Stand-downs

- `UNVERIFIED-ACCEPTED: client-side rendering in app.js (cards, tabs, footprint
  bars, copy button) is not machine-gated` because the toolchain has no Node or
  headless browser. Covered by the manual preview in the verification gates. Pending
  the owner's confirmation at spec review.
