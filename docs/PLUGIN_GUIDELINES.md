# Plugin guidelines

How to add a plugin to this marketplace, and the standards the checks hold it to. The root
[README](../README.md) says what the marketplace is; [`HACK.md`](../HACK.md) covers maintainer procedure:
releases and tags, the footprint override, marketplace URLs, and adding or removing a whole agent.

Read the section for your kind of plugin (section 2), do the checklist in section 3, and run the commands in
section 5 before you open a pull request. Every path in this document is checked to exist by
`scripts/check-doc-paths.sh`; every statement about a check names the script that enforces it. A file inside
a plugin is written `<plugin folder>/...` in the text, because a bare name would be read as a file at the
repository root; the folder trees are in code blocks.

## 1. How the repository is organised

A plugin is a folder `<host folder>/<family>-<suffix>/`, one per host the plugin supports:

| Host | Folder | Suffix | Cargo workspace member |
|---|---|---|---|
| Claude Code | `claude-code/plugins/` | `-cc` | yes |
| Qwen Code | `qwen/plugins/` | `-qwen` | yes |
| Antigravity | `antigravity/plugins/` | `-agy` | yes |
| OpenCode | `opencode/plugins/` | `-opencode` | no |

- **The registry.** `agents.json` is the one list of hosts and plugins. Most checks enumerate plugins from it
  through `scripts/lib/registry.sh`, so a plugin that is not in it is invisible to them: they cannot fail on
  it, and they do not check it.
- **Names.** Use lowercase letters, digits and `-` only. The family is the folder name without its suffix.
  `docs/site/families.txt` lists every family. A published name may differ from the folder name through the
  `publishedNames` map in `agents.json`.
- **Engines and fronts.** `shared/` holds engines that more than one host's plugin fronts; the plugin folder
  is the thin, host-specific front. Put logic in the engine when a second host will need it. The root
  `README.md` lists the engines.
- **Root `Cargo.toml`.** The Claude Code, Qwen, Antigravity and `shared/` folders are workspace members by
  glob, so a new crate there joins the workspace without an edit. OpenCode plugins are not crates.

## 2. Which kind are you adding

The checks key on two properties: whether the host's folders are Cargo workspace members, and whether the
plugin is flagged `skillsOnly` in `agents.json`.

| Kind | You are adding | Exemplar to copy |
|---|---|---|
| A | A Rust front on Claude Code, Qwen or Antigravity: a crate with `[[bin]]` targets, an MCP server or hooks | `claude-code/plugins/repo-tasks-mcp-cc/`; the bare template is `claude-code/plugins/example/` |
| B | A skills-only plugin on Claude Code, Qwen or Antigravity: Markdown, no crate | `claude-code/plugins/repo-user-guide-cc/` |
| C | An OpenCode plugin with a TypeScript plugin module | `opencode/plugins/todov2-opencode/` (no binary); `opencode/plugins/repo-tasks-mcp-opencode/` (stages a binary) |
| D | A skills-only OpenCode plugin: a manifest and skills, no plugin module | `opencode/plugins/repo-user-guide-opencode/` |

Sections 3.1 to 3.5 give the rows every kind shares and then each kind's own. A row says what to change,
whether the file is generated (never edit a generated file by hand; run the command), and what fails if you
skip it. "Nothing fails" is stated where it is true, because those are the rows that rot.

## 3. The wiring checklist

### 3.1 Every kind

| File | What to do | Generated | What fails if you skip it |
|---|---|---|---|
| `agents.json` | Add the folder name to the host's `plugins` array. Add it to `skillsOnly` for kinds B and D. Add a `publishedNames` entry if the published name differs from the folder name. | no | Nothing: the registry-driven checks never see the plugin. |
| `.claude-plugin/marketplace.json`, `.qwen-plugin/marketplace.json`, `.agy-plugin/marketplace.json`, `.opencode-plugin/marketplace.json` | Run `bash scripts/gen-marketplaces.sh`. | yes | `bash scripts/gen-marketplaces.sh --check`, the CI step "Check generated marketplaces are current". |
| `release-please-config.json` | Run `bash scripts/gen-release-please-packages.sh`; it rewrites only the `packages` table. | yes | `bash scripts/gen-release-please-packages.sh --check`, a CI step. |
| `.release-please-manifest.json` | Add the key `<host folder>/<plugin folder>` with the value `"0.1.0"`. | no | `scripts/check-release-manifest.sh` compares the keys with the registry. It never checks the values. |
| `scripts/check-package-versions.sh` | Add a row to the table of plugin, crate or manifest, host manifest and version field. A plugin with no crate names its manifest twice. | no | Nothing: a plugin missing from the table is not checked. |
| `docs/site/families.txt` | Add your family, one per line, sorted, if it is new. | no | `just site-check`: `scripts/check-site.sh` fails when the catalog's families and this file differ. |
| `docs/footprints/<plugin>.json`, `docs/footprints/budgets.json` | Build the plugin binaries first (the build list in `.github/workflows/footprint-freshness.yml` is the full set), then run `just footprint-regen`. It measures every plugin of Claude Code, OpenCode and Antigravity, ratchets the budgets, and rewrites the README footprint regions. Qwen plugins are outside this gate. | yes | `just footprint` and the CI job "Plugin Footprint": stale or untracked footprint documents. |
| The plugin's own `README.md` | Add the marker pair `<!-- footprint:begin -->` and `<!-- footprint:end -->` once, by hand, where the figure goes. The root `README.md` carries the same pair for the comparison table. | region: yes | `just footprint-regen` stops with an error naming the file. |
| `README.md` | Add the plugin to the documentation index under its host's heading. | no | Nothing checks it. |
| `QWEN.md` | Add the plugin to the tables that list plugins and their folders. | no | Nothing checks it. |

Release tags are never pushed by hand; see `HACK.md`.

### 3.2 Kind A: a Rust front

```text
<plugin folder>/
├── Cargo.toml        its own `version`, not `version.workspace`
├── src/              the front: the MCP server and hook binaries
├── <host manifest>   Claude Code: .claude-plugin/plugin.json; Qwen: qwen-extension.json; Antigravity: plugin.json
├── hooks/  skills/  agents/  commands/   as the plugin needs them
└── README.md         with the footprint marker pair
```

| File | What to do | Generated | What fails if you skip it |
|---|---|---|---|
| `Cargo.lock` | Commit the change `cargo` makes when the crate joins the workspace. | yes | Nothing: no build here passes `--locked`, so the next contributor's build rewrites it. |
| `scripts/check-package-versions.sh` | As in 3.1, and add the crate's `Cargo.toml` to the list of crates that must not inherit the workspace version. | no | Nothing: the list is hardcoded. |
| `Justfile` | Add a `build-<plugin>` recipe that builds the binary and stages it into `<plugin folder>/bin/`; copy the one for `claude-code/plugins/repo-tasks-mcp-cc/`. | no | Nothing by itself; the next two rows depend on it. |
| `.github/workflows/ci.yml`, `.github/workflows/footprint-freshness.yml` | Add `just build-<plugin>` to the build list in each: the "Build plugin binaries" step of the footprint job, and the "Regenerate and upsert correction branch" step. | no | The footprint run cannot start the plugin's server and refuses to publish a figure for it. |
| Hooks and MCP configs | Every binary named in `<plugin folder>/hooks/hooks.json` or `<plugin folder>/.mcp.json` must be a `[[bin]]` target. | no | `scripts/check-plugin-wiring.sh`. |
| `scripts/install-qwen-extension.sh`, `scripts/install-qwen-extension.ps1` | Qwen only: add the extension to the usage list in the shell script and to both `ValidateSet` lists in the PowerShell script, which rejects any name not in them. | no | Nothing checks the lists; the PowerShell installer rejects a name that is missing. |
| Antigravity configs | Antigravity only: binaries named in its configs must resolve. | no | `scripts/check-agy-wiring.sh`, and `just agy-validate` where `agy` is installed. |

Mark Windows and macOS as `not measured` in the plugin's README unless you ran it there, as
`claude-code/plugins/repo-tasks-mcp-cc/README.md` does.

Verification status: not yet walked through.

### 3.3 Kind B: skills-only on a Cargo host

```text
<plugin folder>/
├── <host manifest>              as in 3.2
├── README.md                    with the footprint marker pair
└── skills/<skill>/SKILL.md      one folder per skill
```

There is no `Cargo.toml` and no folder of binaries.

| File | What to do | Generated | What fails if you skip it |
|---|---|---|---|
| `Cargo.toml` | Add the folder to the `exclude` array: the workspace members are globs and a folder with no crate must be listed. | no | Cargo cannot load the workspace. |
| `scripts/check-skills-bundles.sh` | Add a `<host id>:<plugin>` line to the `pairs` list. | no | Nothing: the bundle is not built or checked. |
| The bundle itself | `scripts/bundle-skills-plugin.sh` builds the zip and rejects it without the manifest, `README.md`, a `skills/*/SKILL.md`, `LICENSE`, or if it holds a folder of binaries. Skills adapted from upstream must ship `skills/<skill>/THIRD-PARTY-NOTICE.md`; the exemption for this repository's own skills is hardcoded to the `repo-user-guide-` prefix. | no | `just skills-bundles`. The script refuses to run from a Windows shell; use WSL, Linux or macOS. |
| `scripts/check-package-versions.sh` | As in 3.1, with the manifest named twice. | no | Nothing. |

A **new family** also needs the release plumbing, because tags for crate-less plugins must not reach the
Rust release workflow: add the tag pattern `'<family>-*-v*'` to `.github/workflows/skills-bundles.yml`, add
`'!<family>-*-v*'` after the positive patterns in `.github/workflows/release.yml` and
`.github/workflows/opencode-bundles.yml`, and add the same exclusion to the list in
`scripts/check-skills-release-wiring.sh`, which is hardcoded. That script reads every `skillsOnly` key of
`agents.json`, so `just skills-release-wiring` fails when `.github/workflows/skills-bundles.yml` does not select
your plugin's tag or `.github/workflows/release.yml` does not exclude it. It verifies the exclusion in
`.github/workflows/opencode-bundles.yml` only for the families in its hardcoded list, so extending that list is
what makes it verify yours. A new host's plugin in an existing
family needs none of this: the family's patterns already match.

Verification status: not yet walked through.

### 3.4 Kind C: an OpenCode plugin

```text
<plugin folder>/
├── package.json      name, semantic version, the @opencode/plugin pin
├── plugin.ts         the plugin module
├── opencode.jsonc    MCP servers and settings
├── skills/  agents/  commands/  tool/   as the plugin needs them
├── README.md         with the footprint marker pair
└── INSTALL.md        quotes the pin
```

`scripts/check-opencode-wiring.sh` enforces:

- the plugin module declares an `id` equal to the folder name, and `package.json` has a semantic version;
- `opencode.jsonc` is valid JSON, carries no legacy keys, and every MCP command it names is a built binary;
- each skill's frontmatter name matches its folder, and each agent has a `description:` and no `prompt:`;
- the `@opencode/plugin` version in `<plugin folder>/package.json` equals every other OpenCode plugin's, and
  the plugin's README and install guide quote the same version. Move the pin with `just bump-opencode-plugin <version>`,
  which edits all of them at once.

| File | What to do | Generated | What fails if you skip it |
|---|---|---|---|
| `.github/dependabot.yml` | Add an npm entry for the plugin's folder that ignores `@opencode/plugin` and groups the rest; copy the `todov2-opencode` block. | no | Nothing checks it. |
| `Justfile`, `lefthook.yml` | Add `./opencode/plugins/<plugin>/node_modules` to the `--ignore-path` list of the `links` recipe and of the pre-push `mlc` line, if the plugin has dependencies. | no | Nothing on a clean checkout. Once the plugin's dependencies are installed, `just links` scans them and fails (measured on `repo-tasks-mcp-opencode`, whose entry is missing from the `Justfile` list). |
| `Justfile`, `.github/workflows/ci.yml`, `.github/workflows/footprint-freshness.yml` | Only if the plugin stages a binary: the same `build-<plugin>` recipe and build-list lines as kind A. | no | As kind A. |
| `scripts/check-package-versions.sh` | As in 3.1, with `package.json` named twice. | no | Nothing. |

No `Cargo.toml` edit is needed, and `.gitignore` already ignores `opencode/plugins/*/bin/`. The bundler
`scripts/bundle-opencode-plugin.sh` copies the plugin module, the manifest, the skills, the commands and any
helper-tool folder without an edit. An OpenCode footprint has a third tier, the plugin module, which
loads on every request; its README region reports it.

Verification status: checked by reading only.

### 3.5 Kind D: a skills-only OpenCode plugin

```text
<plugin folder>/
├── package.json
├── README.md                    with the footprint marker pair
├── INSTALL.md                   the bundle must contain it
└── skills/<skill>/SKILL.md
```

There is no plugin module, `opencode.jsonc` or MCP command; the wiring check skips those and still checks
the skills.

| File | What to do | Generated | What fails if you skip it |
|---|---|---|---|
| `scripts/check-skills-bundles.sh` | Add an `opencode:<plugin>` line to the `pairs` list. | no | Nothing: the bundle is not built or checked. |
| The bundle itself | As kind B, and the bundle must also contain the install guide. | no | `just skills-bundles`. |
| `scripts/check-package-versions.sh` | As in 3.1, with `package.json` named twice. | no | Nothing. |

A new family needs the release plumbing described in 3.3. There is no `Cargo.toml` edit.

Verification status: checked by reading only.

## 4. Standards

- **License headers.** `scripts/check-license-headers.sh` requires the header from `LICENSE-HEADER.tmpl` on
  every tracked `*.rs`, `*.sh`, `*.ts`, `*.js`, `*.py` and `*.ps1` file. Exempt: third-party skill copies,
  generated site output, and an OpenCode plugin's plugin module, whose size is the plugin's per-request
  footprint. The pre-commit hook in `lefthook.yml` runs it on staged files and needs `addlicense`.
- **Versions.** A new plugin starts at `0.1.0` in its manifest and in `.release-please-manifest.json`.
  release-please bumps both and mints the tags; never push a tag or run `git push --tags` (see `HACK.md`). A
  crate sets its own `version`; `scripts/check-package-versions.sh` fails a shippable crate that inherits the
  workspace's.
- **Footprint.** The figure is the exact size in bytes of what the host loads: resident on every request,
  invocation only when a skill or agent runs. The committed figure ratchets: `docs/footprints/budgets.json`
  holds a ceiling per plugin, and growth beyond it is refused rather than overridden. The override procedure
  for a measurement correction is in `HACK.md`.
- **The plugin README.** Say what the plugin does in the first paragraph, carry the footprint region, give
  the install line, and mark anything you did not run on a host as `not measured`. Copy the headings of the
  exemplar for your kind.
- **Generated files.** Never edit these by hand; run the command: the four marketplace manifests
  (`bash scripts/gen-marketplaces.sh`), the `packages` table in `release-please-config.json`
  (`bash scripts/gen-release-please-packages.sh`), everything under `docs/footprints/` and the footprint
  regions of the READMEs (`just footprint-regen`), and the skill copies emitted by
  `just emit-devtools-skills`.
- **Names in text.** In documents, write a file that is not at the repository root by its full path, so
  `scripts/check-doc-paths.sh` can check it.

## 5. Before you open a pull request

Run the checks that cover what you touched; `just check` runs all of them and needs the tools that
`just setup` installs.

| You changed | Run |
|---|---|
| `agents.json`, the manifests | `just gen-marketplaces`, then `just marketplace`, `just qwen-marketplace`, `just agy-marketplace` |
| Any plugin's wiring | `just wiring`, `just opencode-wiring`, `just agy-wiring`, `just versions` |
| Skills-only plugins | `just skills-bundles`, `just skills-release-wiring` |
| Footprint or budgets | `just footprint-regen`, then `just footprint` and `just footprint-budgets` |
| The catalog or families | `just site-check` |
| Documents | `just links`, `just spellcheck`, `bash scripts/check-doc-paths.sh` |
| Anything | `just check` |

Releases, tag handling, the footprint override and marketplace URL regeneration after a release are
maintainer procedure: see `HACK.md`.
