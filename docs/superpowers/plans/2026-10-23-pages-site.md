# GitHub Pages storefront site Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish a filterable plugin storefront on GitHub Pages, generated from repo data on every push to `main`, with rustdoc still served from the site root.

**Architecture:** `scripts/gen-site.sh` (jq) joins `agents.json`, the four marketplace files and the footprint files into `catalog.json`; hand-written `docs/site/{index.html,styles.css,app.js}` render it; `scripts/stage-site.sh` copies `target/doc` into `target/site`, overlays the site, and runs a gate; `docs.yml` uploads `target/site`. `scripts/check-site.sh` guards the catalog and has a self-test.

**Tech Stack:** bash, jq, plain HTML/CSS/ES module. No Node. Reuses `scripts/lib/registry.sh`.

**Spec:** `docs/superpowers/specs/2026-10-22-pages-site-design.md` (read it first; the plan only records decisions it leaves to the implementer).

## Global Constraints

- No Node, npm or SSG binary; `docs/site/app.js` contains no `innerHTML` (use `textContent` or attribute setters).
- Every script is run as `bash scripts/<name>.sh`; follow `scripts/gen-marketplaces.sh`: `set -euo pipefail`, `cd "$(dirname "$0")/.."`, `source scripts/lib/registry.sh` after `set --`, and read every `jq -r` result through a `tr -d '\r'` helper (`jqr`, `gen-marketplaces.sh:55-57`). Output files are LF.
- CLI: `gen-site.sh [--root DIR] OUT`, `check-site.sh [--root DIR] CATALOG` and `check-site.sh --selftest`, `stage-site.sh [--root DIR] [--doc-dir DIR] [--out DIR]`; defaults `.`, `target/doc`, `target/site`.
- Site file names the staging guard refuses to overwrite: `index.html`, `styles.css`, `app.js`, `catalog.json`, `USER_GUIDE.pdf`, `api`.
- Deny patterns (one constant, `DENY_PATTERNS`, in `stage-site.sh`): `*example*`, `mock_*`, `probe_target`, `footprint_gate`, `plugin_footprint`.
- `version` is bare semver (`^[0-9]+\.[0-9]+\.[0-9]+$`) from the `-v<semver>` tag in `source.url`, equal to `pluginVersion` of the footprint file when one exists.
- Allowed URL prefixes in `readme` and `install[].command`: `https://github.com/ckir/aiplugins/`, `https://raw.githubusercontent.com/ckir/aiplugins/`.
- `setup` is `null` exactly when the footprint file has no `tiers.setup`; `footprint` is `null` exactly when `docs/footprints/<dirKey>.json` is absent. Invocation has no budget and is never drawn against one.
- Claude Code and Qwen have exactly two `install` entries; every other plugin at least one. Labels: Claude Code "Add the marketplace" and "Install"; Qwen "Linux and macOS" and "Windows (PowerShell)"; OpenCode "Ask your agent"; Antigravity "Install"; `repo-user-guide-agy` "Copy the skill".
- Placeholder line in `index.html`: `<!-- NOSCRIPT -->`. All site URLs are relative (the site lives under `/aiplugins/`).
- `typos` must pass on every new file (lefthook runs it on `*`, `lefthook.yml:10-12`).

## Review Focus

Failure modes the spec implies but no obvious task exercises; each has a named test in the owning task.

1. A plugin added to `agents.json` with no install shape or override: generator refuses and names `gen-site.sh` (Task 2, `gen-test.sh` case `no-install-shape`).
2. A marketplace `source.url` with no `-v<semver>` tag: generator refuses (Task 2, case `no-version-tag`).
3. A description containing `&`, `<` or `"`: staged `<noscript>` shows it escaped (Task 5, selftest part (c) `escapes-ampersand`).
4. Windows `jq` emitting CRLF into the catalog (Task 2, case `crlf-input`).
5. A rustdoc output path equal to a site file name, or a new internal crate not matching the deny patterns (Task 5, cases `collision-refused`, `deny-filter`).

## Decisions the spec left open

- The expected family list lives in `docs/site/families.txt` (one key per line, read relative to `--root`) so fixtures can supply their own; the spec said "in `check-site.sh`". Failure messages name that file.
- The OpenCode prompt string is "Fetch and follow the installation instructions at <raw INSTALL.md URL>, and ask me whether to install project-local or global first." (`README.md:44-49` describes the behavior but has no literal prompt.) INSTALL URL shape: `README.md:50-53`.
- The Antigravity PATH line is `export PATH="$HOME/.gemini/config/plugins/<name>/bin:$PATH"` (`antigravity/plugins/rtk-mcp-agy/README.md:78-92` describes the directory without a literal line).
- CI runs `just site-check` as the last step of the existing `wiring` job, which already installs `just` (`ci.yml:145-146`).
- `stage-site.sh --gate-only --out DIR` runs only the staged-tree gate on an existing output directory, so the selftest can mutate a staged tree and require the gate to reject it. `--root` is forwarded to `gen-site.sh` and `check-site.sh` only; `docs/site/{index.html,styles.css,app.js}` and `docs/USER_GUIDE.pdf` are always read from the repository root (only those three site files are copied, never `families.txt`).
- `gen-site.sh` handles each plugin in this order, so each refusal test reaches its own error: `family_of KEY` (suffix), marketplace lookup, `version_of URL`, `install_entries`.

## Install command templates

`<name>` is the marketplace entry name, `<url>` its `source.url`, `<zip>` the last path segment of `<url>`. Joined into one `command` string with newlines. The README lines cited in Task 2 give the extract and install lines; the download lines are plan decisions (the Qwen manual sections link a table URL instead of a command).

- Claude Code: entry "Add the marketplace" `claude plugin marketplace add ckir/aiplugins`; entry "Install" `claude plugin install <name>@aiplugins`.
- Qwen "Linux and macOS": `curl -fsSL -o <zip> <url>`, `mkdir -p ~/.qwen/extensions/<name>`, `unzip -q -o <zip> -d ~/.qwen/extensions/<name>/`, and, only when the plugin is not in its agent's `skillsOnly`, `find ~/.qwen/extensions/<name>/bin -type f ! -name '*.exe' -exec chmod 755 {} +` (the `find` line of `scripts/install-qwen-extension.sh:87-89`, without its `if`).
- Qwen "Windows (PowerShell)": `Invoke-WebRequest -Uri <url> -OutFile <zip>`, `New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.qwen\extensions\<name>"`, `Expand-Archive -Path <zip> -DestinationPath "$env:USERPROFILE\.qwen\extensions\<name>" -Force`.
- OpenCode "Ask your agent": the prompt string above.
- Antigravity "Install": `curl -fsSL -o /tmp/<zip> <url>`, `unzip -q -o /tmp/<zip> -d /tmp/<name>`, `agy plugin install /tmp/<name>`, and, only when not `skillsOnly`, the `export PATH=...` line above.
- `repo-user-guide-agy` "Copy the skill": `curl -fsSL -o /tmp/<zip> <url>`, `unzip -q -o /tmp/<zip> -d /tmp/<name>`, `mkdir -p ~/.gemini/config/skills/repo-user-guide`, `cp -r /tmp/<name>/skills/repo-user-guide/. ~/.gemini/config/skills/repo-user-guide/` (global destination from `antigravity/plugins/repo-user-guide-agy/INSTALL.md:10-14`; confirm the zip really holds `skills/repo-user-guide/` with `unzip -l` on the release asset before fixing this string).

---

### Task 1: Pins and fixtures

**Files:**
- Modify: `.gitattributes` (append)
- Create: `tests/site/root/agents.json`, `tests/site/root/{.claude-plugin,.qwen-plugin,.opencode-plugin,.agy-plugin}/marketplace.json`, `tests/site/root/docs/footprints/*.json` and `budgets.json`, `tests/site/root/docs/site/families.txt`, `tests/site/expected/catalog.json`

**Interfaces:**
- Produces: a fixture root readable by `gen-site.sh --root tests/site/root`; `tests/site/expected/catalog.json`, written by hand from the fixture inputs, never produced by running the generator.

Fixture content (small, real field names from spec "Input fields"): two families, `alpha-mcp` (plugins `alpha-mcp-cc`, `alpha-mcp-qwen`, `alpha-mcp-opencode`, `alpha-mcp-agy`) and `repo-user-guide` (plugins `repo-user-guide-cc` published as `repo-user-guide`, `repo-user-guide-agy` published as `repo-user-guide`, both `skillsOnly`). Coverage the fixture must give: Qwen plugin with no footprint file; an OpenCode plugin with a `setup` tier; one setup-only plugin (`alpha-mcp-opencode`: resident 0, invocation 0, setup above 0, like `todov2-opencode`); one skills-only plugin with no `setup`; a `publishedNames` remap; one description containing `&`; `pluginVersion` equal to the `source.url` tag for every footprint file. `families.txt` holds `alpha-mcp` and `repo-user-guide`.

- [ ] **Step 1: Create the fixture inputs and the hand-written expected catalog**, using the schema in spec "catalog.json schema" and the value rules in Global Constraints.
- [ ] **Step 2: Append to `.gitattributes`** the two lines `docs/site/** text eol=lf` and `tests/site/** text eol=lf` with a comment naming the marker-line splice and fixture comparisons.
- [ ] **Step 3: Verify**
  Run: `git check-attr eol -- docs/site/index.html tests/site/root/agents.json` — Expected: both report `eol: lf`.
  Run: `typos tests/site` — Expected: no output, exit 0.
  Run: `jq -e . tests/site/expected/catalog.json >/dev/null` — Expected: exit 0.
- [ ] **Step 4: Commit** — `git add .gitattributes tests/site && git commit -m "test(site): LF pins and catalog fixtures"`

### Task 2: `gen-site.sh`

**Files:**
- Create: `scripts/gen-site.sh`, `tests/site/gen-test.sh`
- Test: `tests/site/gen-test.sh`

**Interfaces:**
- Consumes: `registry_plugins`, `registry_published_name`, `registry_is_skills_only`, `registry_field` from `scripts/lib/registry.sh:10-45`; honour `REGISTRY="$root/agents.json"` so `--root` works.
- Produces: `bash scripts/gen-site.sh [--root DIR] OUT` writes the catalog (spec "catalog.json schema": top-level `agents[]` in UI order claude-code, opencode, antigravity, qwen, and `families[]` sorted by key; keys sorted with `jq -S`), exits 1 with `gen-site: <reason>` on any refusal. `agents[].label` values are `Claude Code`, `OpenCode`, `Antigravity`, `Qwen`. Internal bash functions, called in the order given under "Decisions": `family_of KEY`, `version_of URL`, `install_entries AGENT DIRKEY NAME URL SKILLS_ONLY`. `gen-site.sh` does not read `families.txt`.

- [ ] **Step 1: Write `tests/site/gen-test.sh`** with these named cases, each printing `ok <name>` or failing:
  - `matches-expected`: `gen-site.sh --root tests/site/root` output equals `tests/site/expected/catalog.json` after `jq -S .` on both.
  - `no-install-shape`: copy the root to a temp dir and add an agent `zeta` (with one plugin `beta-mcp-cc`, whose valid suffix means only the missing table row can fail, and a matching marketplace entry) to `agents.json`; the install table has no row for agent `zeta`, so expect exit 1 and a message containing `install table in scripts/gen-site.sh`.
  - `no-version-tag`: strip `-v0.1.0` from one `source.url`; expect exit 1 containing `-v<semver>`.
  - `unknown-suffix`: rename one plugin key to `alpha-mcp-zzz` in `agents.json` only; `family_of` runs first, so expect exit 1 containing `agent suffix`.
  - `crlf-input`: rewrite every fixture JSON with CRLF line endings; expect output byte-identical to the LF run and containing no `\r`.
- [ ] **Step 2: Run it to confirm it fails** — Run: `bash tests/site/gen-test.sh` — Expected: FAIL (`scripts/gen-site.sh: No such file`).
- [ ] **Step 3: Implement `scripts/gen-site.sh`** per the Interfaces block. Install shapes are the "Install command templates" above, one table in the script. Sources for the extract and install lines: Claude Code `README.md:18` and `claude-code/plugins/aiplugins-devtools-cc/README.md:15`; Qwen `qwen/plugins/rtk-mcp-qwen/README.md:51-61`, `re-ghidra-mcp-qwen/README.md:96,102`, `aiplugins-devtools-qwen/README.md:38,44`, chmod `scripts/install-qwen-extension.sh:87-89`; OpenCode `README.md:44-53`; Antigravity `antigravity/plugins/rtk-mcp-agy/README.md:43-58` and `:78-92`, `aiplugins-devtools-agy/README.md:20-22,32`; override `antigravity/plugins/repo-user-guide-agy/INSTALL.md:9-14`.
- [ ] **Step 4: Run** `bash tests/site/gen-test.sh` — Expected: five `ok` lines, exit 0.
- [ ] **Step 5: Run against the real repo** — `bash scripts/gen-site.sh target/site-check/catalog.json && jq '[.families[].plugins|length]|add' target/site-check/catalog.json` — Expected: `20`.
- [ ] **Step 6: Commit** — `git add scripts/gen-site.sh tests/site/gen-test.sh && git commit -m "feat(site): catalog generator"`

### Task 3: `check-site.sh`, self-test and `just site-check`

**Files:**
- Create: `scripts/check-site.sh`, `docs/site/families.txt`
- Modify: `Justfile` (add recipe `site-check`; add it to the `check:` dependency list at `Justfile:325`)
- Test: `scripts/check-site.sh --selftest`

**Interfaces:**
- Consumes: Task 2's `gen-site.sh` and `tests/site/gen-test.sh`.
- Produces: `bash scripts/check-site.sh [--root DIR] CATALOG` (exit 1, one line per failure `FAIL <id>: <plugin>: <detail>`) and `--selftest`. Invariant ids, fixed: `plugin-set`, `tier-totals`, `footprint-null`, `name`, `install-shape`, `install-count`, `null-fields`, `url-prefix`, `version`, `readme`, `budget`, `skills-only`, `family-set`. `install-shape` is checked against the inputs, not the generator: Claude Code entries equal the two literal commands; each Qwen entry contains the marketplace entry's `source.url` verbatim, the shell one contains `unzip` and the PowerShell one `Expand-Archive`; the OpenCode command contains that plugin's `INSTALL.md` raw URL; each Antigravity command contains the `source.url` and `agy plugin install` (the `repo-user-guide-agy` override contains `cp -r` instead); its message names `scripts/gen-site.sh`. `family-set` compares the catalog's `[.families[].key]`, and each plugin's `dirKey` with its agent suffix stripped, against `$root/docs/site/families.txt`; its message names that file. Only `check-site.sh` reads `families.txt`. `docs/site/families.txt` holds the six real families.

- [ ] **Step 1: Write the selftest first** in `check-site.sh --selftest`: part (a) runs `bash tests/site/gen-test.sh`, then generates from `tests/site/root` and requires `check-site.sh --root tests/site/root` to pass; part (b) loops over fourteen one-line `jq` mutations of the fixture catalog, at least one per invariant id above (for example `tier-totals`: add 1 to a resident byte count; `plugin-set` has two, delete one plugin and duplicate one; `install-shape`: replace a Qwen command with an Antigravity one; `url-prefix`: set a command to `curl https://example.com/x`), each fed straight to `check-site.sh` and required to exit 1 with `FAIL <id>`. Print `ok <id>` per case.
- [ ] **Step 2: Run** `bash scripts/check-site.sh --selftest` — Expected: FAIL (checker not implemented).
- [ ] **Step 3: Implement the thirteen checks** from spec "Verification gates → `just site-check`" (read it; value rules are in Global Constraints). Part (a)/(b) are the only parts in this task; `--selftest` part (c) arrives in Task 5.
- [ ] **Step 4: Add the recipe** `site-check:` running, in order, `bash scripts/gen-marketplaces.sh --check`, `bash scripts/gen-site.sh target/site-check/catalog.json`, `bash scripts/check-site.sh target/site-check/catalog.json`, `bash scripts/check-site.sh --selftest`; add `site-check` to the `check:` list.
- [ ] **Step 5: Run** `just site-check` — Expected: exit 0, fourteen part-(b) `ok` lines plus part (a).
- [ ] **Step 6: Commit** — `git add scripts/check-site.sh docs/site/families.txt Justfile && git commit -m "feat(site): catalog checker and just site-check"`

### Task 4: The static page

**Files:**
- Create: `docs/site/index.html`, `docs/site/styles.css`, `docs/site/app.js`

**Interfaces:**
- Consumes: the catalog schema (spec "catalog.json schema").
- Produces: `index.html` containing `<!-- NOSCRIPT -->` on its own line; `app.js` as an ES module exporting nothing and exposing these functions internally: `loadCatalog(): Promise<Catalog>` (fetches `./catalog.json`), `renderCatalog(catalog: Catalog, filter: string): void`, `renderCard(family: Family, agentId: string): HTMLElement`, `renderFootprint(fp: Footprint | null): HTMLElement`, `copyText(text: string): Promise<void>`, `initTheme(): void`.

Page content is spec "Page design" (hero, catalog, footprint bar, why-it-matters, footer). Decisions the spec fixes that must show in the code: tab order claude-code, opencode, antigravity, qwen; greyed (not hidden) agents without the family; a card renders every `install[]` entry with its own copy button; invocation drawn as a plain number; a note repeating the README's "compare between plugins, not a context budget"; each card's README link is the entry's `readme`; "Not measured" badge when `footprint` is `null`; "0 B" for a measured zero; theme tokens on `:root` with `prefers-color-scheme` and a toggle remembered in `localStorage` inside try/catch; system font stack; no external requests; ARIA `tablist` roles, visible focus, a live region announcing "Copied".

- [ ] **Step 1: Write `index.html` and `styles.css`**, including the hero command `claude plugin marketplace add ckir/aiplugins` labeled for Claude Code and footer links (API `api/index.html`, `USER_GUIDE.pdf`, GitHub, changelog and license as `https://github.com/ckir/aiplugins/blob/main/…`).
- [ ] **Step 2: Write `app.js`** to the signatures above.
- [ ] **Step 3: Check** — Run: `! grep -q innerHTML docs/site/app.js` — Expected: exit 0. Run: `! grep -nE '(href|src)="/' docs/site/index.html` — Expected: exit 0, no output. Run: `typos docs/site` — Expected: exit 0.
- [ ] **Step 4: Manual preview** (no Node, so this is the check the spec accepts): serve `docs/site` plus a generated `catalog.json` from a temp dir and confirm in a browser at phone and desktop widths, light and dark: filter chips, tab switch swaps description/install/version/footprint, Qwen cards show "Not measured", `todov2-opencode` shows setup 1,603 B and resident 0 B, both Claude Code install entries copy separately, keyboard navigation works, and `<dir>`-style text in a command displays literally.
- [ ] **Step 5: Commit** — `git add docs/site && git commit -m "feat(site): storefront page"`

### Task 5: `stage-site.sh`, synthetic doc tree and self-test part (c)

**Files:**
- Create: `scripts/stage-site.sh`, `tests/site/doc/` (see below)
- Modify: `scripts/check-site.sh` (add part (c) to `--selftest`)

**Interfaces:**
- Consumes: `gen-site.sh`, `check-site.sh`, `docs/site/*`.
- Produces: `bash scripts/stage-site.sh [--root DIR] [--doc-dir DIR] [--out DIR]` running the six steps in spec "Deploy"; `--gate-only --out DIR`; bash functions `crate_set DIR` (step 4: top-level directories with an `index.html`, minus `api` and `DENY_PATTERNS`) and `gate_staged_tree OUT`, which uses its own separate scan function `gate_crate_scan OUT` and never calls `crate_set`, so a bug in one shows as a mismatch. Collision message names the path and says `cargo clean --doc`. The gate requires: the files listed in spec "Staged-tree gate", the `<!-- NOSCRIPT -->` placeholder gone, the footer link `href="api/index.html"` present in `index.html`, a non-empty scanned set equal to the set linked from `api/index.html` with no denied crate, links resolving relative to `api/`, no root-absolute `href`/`src` in site-written files, and no `innerHTML` in the staged `app.js`.

`tests/site/doc/` content: `rtk_mcp_cc/index.html`, `alpha_crate/index.html`, `mock_rtk/index.html` (denied), `src/x/y.rs.html` (a directory with no `index.html`), `static.files/a.css`, `crates.js`.

- [ ] **Step 1: Add selftest part (c)** with cases: `stages-and-gates` (stage `--root tests/site/root --doc-dir tests/site/doc --out <temp>`; exit 0), `deny-filter` (`mock_rtk` absent from `api/index.html`, `alpha_crate` present), `escapes-ampersand` (staged `index.html` contains the `&`-description as `&amp;` and no raw `&` from it), `collision-refused` (a copy of the doc tree with a root `index.html`; expect non-zero and `cargo clean --doc` in the message), `gate-catches-empty-set` (a doc tree with no crate directories; expect non-zero), and six `gate-rejects-<defect>` cases that mutate a staged copy and run `stage-site.sh --gate-only --out <copy>`, each required to exit non-zero: `missing-styles` (delete `styles.css`), `placeholder-left` (restore `<!-- NOSCRIPT -->`), `denied-crate-listed` (add a `mock_rtk` link to `api/index.html`), `root-absolute-href` (add `href="/x"`), `innerhtml-in-app` (append `innerHTML` to `app.js`), `footer-link-missing` (remove `href="api/index.html"`).
- [ ] **Step 2: Run** `bash scripts/check-site.sh --selftest` — Expected: FAIL on part (c) (`stage-site.sh` missing).
- [ ] **Step 3: Implement `scripts/stage-site.sh`.** Splice the `<noscript>` list by printing the file around the marker line (an `awk` that prints, not substitutes), after HTML-escaping `&`, `<`, `>` and `"`; one `<li>` per plugin entry with name linked to `readme`, the agent label and the description.
- [ ] **Step 4: Run** `bash scripts/check-site.sh --selftest` — Expected: all part (a), (b), (c) cases `ok`.
- [ ] **Step 5: Real rustdoc run (spec "Not yet verified")** — Run: `cargo doc --workspace --no-deps --document-private-items && bash scripts/stage-site.sh && ls target/site | head -60`. Expected: exit 0; `target/site/api/index.html` and `target/site/index.html` exist. Then print the listed and denied crate names (`grep -o 'href="\.\./[^/]*' target/site/api/index.html`) and compare with the 24 listed / 16 denied measured in the spec; any new crate not matching a deny pattern goes to the owner, not into the constant silently.
- [ ] **Step 6: Commit** — `git add scripts/stage-site.sh scripts/check-site.sh tests/site/doc && git commit -m "feat(site): staging script and gate"`

### Task 6: Workflows, preview recipe and README link

**Files:**
- Modify: `.github/workflows/docs.yml:39-53`, `.github/workflows/ci.yml` (append a step at the end of the `wiring` job, after "Check plugin bin dispatcher" and before the `# Fork 7a` comment that opens the `footprint` job at line 180; add a job), `Justfile` (recipe `site-preview`), `README.md`

**Interfaces:**
- Consumes: Tasks 3 and 5.

- [ ] **Step 1: Edit `docs.yml`**: delete the "Create index.html redirect" step (`docs.yml:39-43`); after "Build documentation" add a step running `bash scripts/stage-site.sh`; change the upload `path` at `docs.yml:53` to `target/site`; keep `concurrency`, `permissions` and the trigger unchanged.
- [ ] **Step 2: Edit `ci.yml`**: in `wiring` (which already installs `just` at lines 145-146), add a final step running `just site-check`; add a job `site-stage` with `if: github.event_name == 'pull_request'` that checks out, installs Rust, caches with `Swatinem/rust-cache@v2`, runs `cargo doc --workspace --no-deps --document-private-items` and `bash scripts/stage-site.sh`.
- [ ] **Step 3: Add `site-preview`** to the `Justfile`: run `bash scripts/stage-site.sh`, copy `target/site` to `target/preview/aiplugins` (copy, not symlink), then `python3 -m http.server --directory target/preview 8000` (note in a comment: use `python` where `python3` is absent; preview aid, not a gate).
- [ ] **Step 4: Add one README line** linking `https://ckir.github.io/aiplugins/`; run `mlc --offline` through `just links` — Expected: exit 0.
- [ ] **Step 5: Validate the workflows locally** — Run: `python3 -I -c "import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]" .github/workflows/docs.yml .github/workflows/ci.yml` — Expected: exit 0 (PyYAML is already a dev dependency, `Justfile:15`).
- [ ] **Step 6: Local end-to-end** — Run `just check` — Expected: exit 0. Then `just site-preview` and walk the Task 4 manual checklist under `http://localhost:8000/aiplugins/`, and confirm an old rustdoc URL such as `/aiplugins/rtk_mcp_qwen/index.html` still loads.
- [ ] **Step 7: Commit** — `git add .github/workflows Justfile README.md && git commit -m "ci(site): deploy the storefront with rustdoc at the root"`

## After the tasks

After each task's commit, update the execution index memory (`project_pages_site_execution.md`) with the commit SHA and the next task as the resume point; a commit not in the index is a recovery hole.

Push the branch and open a PR only on the owner's say. Before declaring complete: AGY-CAPSTONE on the committed range, then AGY-TEST-AUDIT, in a fresh session (memory `agy-capstone-cost-fresh-session`). After merge, confirm the first deploy by loading `https://ckir.github.io/aiplugins/` and one old rustdoc URL; rollback is reverting the merge commit.
