# OpenCode V2 Parity with Claude Code (G1–G8) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every remaining `claude-code/` → `opencode/` parity gap: port the `repo-user-guide` skills plugin (G1), port the re-ghidra `.local.md` settings layer and preflight to the OpenCode host (G2), and close the footprint/CI/version-pin/dependabot/doc hygiene gaps (G3–G8).

**Architecture:** Registry-first: `agents.json` is the single source of truth; derived files (marketplace manifests, release-please config, footprint docs) are regenerated, never hand-edited. The G2 port splits host-aware settings resolution into the existing `re-ghidra-mcp-cc` Rust crate (host selected by `RE_GHIDRA_HOST`, default `claude` for back-compat) with a pure, mirrored TypeScript preflight in the OpenCode `plugin.ts` (silent-when-healthy, never-blocks, fail-open).

**Tech Stack:** Rust (existing `re-ghidra-mcp-cc` crate, `plugin-footprint` tool), TypeScript/Bun (`@opencode/plugin` 2.0.26), bash checks wired into `just check` + GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-17-opencode-parity-design.md`

## Global Constraints

- No new Rust crates; `shared/ghidra-mcp` stays untouched (G2 touches only `claude-code/plugins/re-ghidra-mcp-cc/`).
- OpenCode side is V2-native only (`@opencode/plugin` `2.0.26`); no V1 shims.
- `repo-user-guide-opencode` skill files are **byte-identical copies** of the `repo-user-guide-cc` originals (host-portable). The rtk/doctor opencode skills are deliberately host-ported — never `cmp` those.
- Anything running inside the OpenCode host is **fail-open**: never throws, never blocks a session.
- `RE_GHIDRA_HOST`: unset, empty, or any value other than exactly `opencode` → `claude` (back-compat). Host-specific settings file only — an opencode session never reads `.claude/re-ghidra-mcp-cc.local.md`, and vice versa.
- Registry keys, ids, env prefixes exactly as specified below. `agents.json` + `registry.sh` are the source of truth.
- `sed -i` is forbidden (macOS incompatibility — repo precedent): use `sed '…' file > file.tmp && mv file.tmp file`.
- Bash changes must stay POSIX-friendly for macOS/Windows Git Bash (`[[ ]]`, arrays, `mapfile` are allowed in existing scripts that already use bash; do not introduce new platform-specific tools).
- Commands run from the repo root: `/home/user/Development/Rust/aiplugins`.

## Conventions

- **TDD order:** write/extend the failing test first where a test harness exists (Rust `cargo test`, Bun `bun test`); for bash checks, verify pass-then-fail (mutation) by temporarily breaking the input.
- **Commit after every task**, message per the task's "Commit" step.
- `just check` is the full suite; run the task's named checks first, run `just check` at the end (it is long).
- Where a step says *regenerate*, run the generator and commit its output — never hand-edit the derived file.

---

## Task 1: Scaffold `opencode/plugins/repo-user-guide-opencode/`

Files created: `package.json`, `README.md`, `INSTALL.md`, and a byte-copy of the `repo-user-guide` skill tree.

- [ ] **1.1 Create `package.json`** (family name `repo-user-guide`, mirroring how `aiplugins-devtools-opencode` carries `aiplugins-devtools`; `check-marketplace.sh` asserts the marketplace entry name equals `registry_published_name`, which Task 2 maps to `repo-user-guide`):

```json
{
  "name": "repo-user-guide",
  "version": "0.2.0",
  "private": true,
  "description": "Create a PDF user guide for a code repository, with usage examples that are run and verified against the repository.",
  "author": {
    "name": "AI Plugins Marketplace Contributors"
  },
  "keywords": [
    "documentation",
    "user-guide",
    "pdf",
    "skills"
  ],
  "license": "PolyForm-Noncommercial-1.0.0"
}
```

No `dependencies`: pure files, no plugin code (matches `aiplugins-devtools-opencode`; also required — check 8/9 in `check-opencode-wiring.sh` skip plugins with no `@opencode/plugin` pin).

- [ ] **1.2 Copy the skill tree byte-for-byte:**

```bash
mkdir -p opencode/plugins/repo-user-guide-opencode
cp -r claude-code/plugins/repo-user-guide-cc/skills \
      opencode/plugins/repo-user-guide-opencode/skills
```

Copied files (must all exist afterwards): `skills/repo-user-guide/SKILL.md`, `skills/repo-user-guide/assets/guide-template.html`, `skills/repo-user-guide/scripts/check_pre.py`, `skills/repo-user-guide/scripts/escape_pre.py`, `skills/repo-user-guide/scripts/html_to_pdf.sh`.

- [ ] **1.3 Create `README.md`:**

```markdown
# repo-user-guide-opencode

An OpenCode plugin that ships the `repo-user-guide` skill: produce a PDF user
guide for a code repository, with usage examples that are run and verified
against that repository before they go into the guide.

It is the OpenCode member of a family: `claude-code/plugins/repo-user-guide-cc`
does this for Claude Code. The skill files are byte-identical copies of that
plugin's originals — the skill body is host-portable — and
`check-opencode-wiring.sh` keeps them that way.

Invoked as `repo-user-guide` (OpenCode keys skills by folder name, no prefix).

Requirements (mirroring the cc twin):

- **Chrome or Chromium** on the machine: the guide is rendered to PDF with headless Chrome.
  Without it, the skill leaves the HTML and says so.
- **poppler** (`pdftoppm`, `pdftotext`, `pdfinfo`) is recommended: the skill uses it to check
  the rendered pages. Without it, it falls back to a Chrome screenshot of the HTML.
- **Python 3**, for the two small helper scripts (`escape_pre.py`, `check_pre.py`).

Install is a file copy of `skills/repo-user-guide/` into OpenCode's skills
directory: see [INSTALL.md](INSTALL.md). There is no `opencode.jsonc`, no
plugin code and no binary.

<!-- footprint:begin -->
### Context footprint

Generated by `tools/plugin-footprint`; regenerate with `just footprint-regen`.
<!-- footprint:end -->
```

- [ ] **1.4 Create `INSTALL.md`:**

```markdown
# Installing repo-user-guide-opencode for OpenCode (agent instructions)

> **If you are an AI agent:** your user pointed you here so you can install
> this plugin for them. Follow this document top to bottom, in order. Do not
> skip the verification step. If any step fails, stop and tell the user
> exactly which step failed and what you saw; do not improvise workarounds.
>
> Everything below comes from a versioned release asset: no branch files, no
> unpinned URLs. Agree the version with the user first (`0.2.0` is current as
> of this writing; check https://github.com/ckir/aiplugins/releases for what
> is newest) and use it for every URL below.

You will install `repo-user-guide-opencode`: a pure skills plugin that ships
the `repo-user-guide` skill (PDF user guides with verified examples). The
install is a file copy of one folder. There is no `opencode.jsonc` to merge,
no binary to stage and no dependency to install.

This document is the executable form of `README.md`. If the two ever disagree,
this file wins for install mechanics and the mismatch is a bug; tell the user.

## Step 0 - prerequisites

1. OpenCode V2 is installed: `opencode --version` prints a V2 version.
2. Download tools: `curl` and `unzip` (Git Bash on Windows has both;
   PowerShell alternative: `Invoke-WebRequest` and `Expand-Archive`).
3. The skill's own runtime requirements matter at use time, not install time:
   Chrome/Chromium, Python 3, and (recommended) poppler.

## Step 1 - ask where to install

Ask the user: **project-local** (this project only) or **global** (every
project)? Then set `DEST` and use it for every path below:

- Project-local: `DEST` = `<project root>/.opencode`
- Global: `DEST` = `~/.config/opencode`

Do not proceed on an assumed default.

## Step 2 - fetch the bundle and copy the skill

Set the agreed version once:

```bash
VERSION=0.2.0
```

Download the versioned bundle: asset `repo-user-guide-opencode-opencode.zip`
on release `repo-user-guide-opencode-v$VERSION` (the doubled name is
`<package>-opencode.zip` for package `repo-user-guide-opencode`). Stage it
outside `$DEST`:

```bash
curl -fsSL -o /tmp/repo-user-guide-opencode.zip \
  "https://github.com/ckir/aiplugins/releases/download/repo-user-guide-opencode-v$VERSION/repo-user-guide-opencode-opencode.zip"
rm -rf /tmp/repo-user-guide-opencode-stage && mkdir -p /tmp/repo-user-guide-opencode-stage
unzip -q -o /tmp/repo-user-guide-opencode.zip -d /tmp/repo-user-guide-opencode-stage
```

Copy the skill folder:

```bash
rm -rf "$DEST/skills/repo-user-guide"
mkdir -p "$DEST/skills"
cp -r "/tmp/repo-user-guide-opencode-stage/skills/repo-user-guide" \
      "$DEST/skills/repo-user-guide"
```

## Step 3 - verify

Confirm each; stop and report if one fails:

```bash
test -s "$DEST/skills/repo-user-guide/SKILL.md" && echo OK skill
test -s "$DEST/skills/repo-user-guide/assets/guide-template.html" && echo OK template
test -s "$DEST/skills/repo-user-guide/scripts/html_to_pdf.sh" && echo OK html_to_pdf
test -s "$DEST/skills/repo-user-guide/scripts/check_pre.py" && echo OK check_pre
test -s "$DEST/skills/repo-user-guide/scripts/escape_pre.py" && echo OK escape_pre
```

Also confirm the `frontmatter name:` in
`$DEST/skills/repo-user-guide/SKILL.md` is `repo-user-guide` — OpenCode keys
skills by folder name, so folder and name must match.

## Step 4 - clean up

```bash
rm -rf /tmp/repo-user-guide-opencode.zip /tmp/repo-user-guide-opencode-stage
```

## Usage

OpenCode auto-discovers the skill; ask for "a user guide for this repo" (or
invoke `repo-user-guide` directly). The skill writes the HTML and PDF next to
the repository. Nothing is registered in any `opencode.jsonc`.
```

- [ ] **1.5 Verify (structure only — the real gates come after Task 2):**

```bash
ls opencode/plugins/repo-user-guide-opencode/package.json \
   opencode/plugins/repo-user-guide-opencode/README.md \
   opencode/plugins/repo-user-guide-opencode/INSTALL.md
diff -r claude-code/plugins/repo-user-guide-cc/skills \
        opencode/plugins/repo-user-guide-opencode/skills \
  && echo "skills byte-identical"
grep -m1 '^name:' opencode/plugins/repo-user-guide-opencode/skills/repo-user-guide/SKILL.md
```

Expected: three files listed, `skills byte-identical`, `name: repo-user-guide`.

- [ ] **Commit:** `feat(opencode): scaffold repo-user-guide-opencode skills plugin (G1)`

---

## Task 2: Registry + derived files + bundler/check generalizations

- [ ] **2.1 `agents.json`** — in the opencode agent entry, after
`"aiplugins-devtools-opencode"`:

- `plugins[]`: append `"repo-user-guide-opencode"`.
- `skillsOnly[]`: append `"repo-user-guide-opencode"`.
- `publishedNames`: add `"repo-user-guide-opencode": "repo-user-guide"`
  (so `registry_published_name(opencode, repo-user-guide-opencode)` =
  `repo-user-guide`, the marketplace entry name Task 1's `package.json`
  carries — same pattern as `aiplugins-devtools-opencode` → `aiplugins-devtools`).

- [ ] **2.2 `scripts/lib/registry.sh` self-test** — the claude-code front's
plugin list is asserted but the opencode front's is not. Right after the
claude-code `plugins` assertion inside the existing self-test block, add:

```bash
    [ "$(registry_plugins opencode | tr '\n' ' ')" = "rtk-mcp-opencode re-ghidra-mcp-opencode todov2-opencode aiplugins-devtools-opencode repo-user-guide-opencode " ] \
        || { echo "FAIL plugins opencode"; failures=$((failures + 1)); }
```

(If the block exits differently on failure, mirror the exact failure style of
the claude-code assertions two lines above.) Run it:

```bash
bash scripts/lib/registry.sh --self-test
```

Expected: all assertions pass (the claude-code list is unchanged — it already
contains `repo-user-guide-cc`).

- [ ] **2.3 `scripts/check-package-versions.sh`** — add the new row after the
`aiplugins-devtools-opencode` row:

```
"repo-user-guide-opencode|opencode/plugins/repo-user-guide-opencode/package.json|opencode/plugins/repo-user-guide-opencode/package.json|.version" \
```

Run `bash scripts/check-package-versions.sh` → pass.

- [ ] **2.4 `scripts/check-skills-bundles.sh`**

1. Extend the `pairs` list:

```bash
pairs="claude-code:aiplugins-devtools-cc
qwen:aiplugins-devtools-qwen
antigravity:aiplugins-devtools-agy
opencode:aiplugins-devtools-opencode
claude-code:repo-user-guide-cc
opencode:repo-user-guide-opencode"
```

2. The pair loop hardcodes the devtools manifest name. In the loop body,
replace:

```bash
    [ "$zname" = "aiplugins-devtools" ] ||
        bad "$agent/$key: manifest name is '$zname', expected 'aiplugins-devtools'"
```

with the registry-derived expectation:

```bash
    expected=$(registry_published_name "$agent" "$key")
    [ "$zname" = "$expected" ] ||
        bad "$agent/$key: manifest name is '$zname', expected '$expected'"
```

(`registry_published_name` is already sourced via `scripts/lib/registry.sh`.)

- [ ] **2.5 `scripts/bundle-skills-plugin.sh`** — the third-party notice rule
currently keys on the exact cc name. Replace:

```bash
if [ "$key" != repo-user-guide-cc ]; then
    printf '%s\n' "$listing" | grep -q '^skills/.*THIRD-PARTY-NOTICE\.md$' ||
        report "skills/**/THIRD-PARTY-NOTICE.md"
fi
```

with a case over the family (devtools skills adapt upstream and must carry its
notice; repo-user-guide skills are this repo's own and ship none):

```bash
case "$key" in
    repo-user-guide-*) ;;
    *)
        printf '%s\n' "$listing" | grep -q '^skills/.*THIRD-PARTY-NOTICE\.md$' ||
            report "skills/**/THIRD-PARTY-NOTICE.md"
        ;;
esac
```

- [ ] **2.6 `.github/workflows/opencode-bundles.yml`** — the `*-opencode-v*`
positive pattern would also fire for `repo-user-guide-opencode-v*`, racing
`skills-bundles.yml` over the same asset name. Directly after the existing
`- '!aiplugins-devtools-*-v*'` line, add:

```yaml
      # skills-only (no plugin.ts): bundled by skills-bundles.yml, which
      # already carries the repo-user-guide-*-v* trigger.
      - '!repo-user-guide-*-v*'
```

- [ ] **2.7 `scripts/check-skills-release-wiring.sh`**

1. Generalize check (a) from a single hardcoded exclusion to both
exclusions. Replace the `EXCL='!aiplugins-devtools-*-v*'` line and its
`(a)` loop with:

```bash
# (a) each exclusion present and AFTER every positive pattern.
for excl in '!aiplugins-devtools-*-v*' '!repo-user-guide-*-v*'; do
  for f in release opencode-bundles; do
    excl_line="$(grep -n -x -F -e "$excl" "$scratch/$f.tags" | head -1 | cut -d: -f1 || true)"
    if [ -z "$excl_line" ]; then
      bad "$f.yml on.push.tags lacks '$excl'"
      continue
    fi
    last_pos="$(grep -n -v '^!' "$scratch/$f.tags" | tail -1 | cut -d: -f1 || true)"
    if [ -n "$last_pos" ] && [ "$excl_line" -le "$last_pos" ]; then
      bad "$f.yml: '$excl' must come AFTER the positive tag patterns (order matters)"
    fi
  done
done
```

(Keep the file's existing indentation style. If `EXCL` is referenced anywhere
else in the script, replace those references with `excl` or keep a local
`EXCL="$excl"` alias — grep first.)

2. In the `(c)` block, beside the existing
`expect no opencode-bundles.yml aiplugins-devtools-opencode-v0.1.0 …` line,
add:

```bash
expect no opencode-bundles.yml repo-user-guide-opencode-v0.1.0 opencode-bundles
```

(The registry-driven `skillsOnly` loop already adds
`expect yes skills-bundles.yml repo-user-guide-opencode-v0.1.0` and
`expect no release.yml repo-user-guide-opencode-v0.1.0` automatically.)

3. Run `bash scripts/check-skills-release-wiring.sh`. **Contingency:** if it
fails with `release.yml: '!repo-user-guide-*-v*' must come AFTER the positive
tag patterns`, move that exclusion line in `.github/workflows/release.yml`
below the last positive tag pattern (order matters; GitHub evaluates the last
matching pattern).

- [ ] **2.8 Regenerate derived files:**

```bash
bash scripts/gen-release-please-packages.sh
bash scripts/gen-marketplaces.sh
git diff -- release-please-config.json .opencode-plugin/marketplace.json
```

Expected diff: release-please gains exactly one package
(`opencode/plugins/repo-user-guide-opencode`, `component:
repo-user-guide-opencode`, `release-type: simple`, `extra-files:
["package.json"]`); `.opencode-plugin/marketplace.json` gains exactly one
entry with `"name": "repo-user-guide"` and source URL
`…/repo-user-guide-opencode-v0.2.0/repo-user-guide-opencode-opencode.zip`.
No other marketplace file changes.

- [ ] **2.9 Run the gates:**

```bash
bash scripts/check-skills-bundles.sh          # passes with the 6 pairs
bash scripts/check-marketplace.sh             # entry names == published names
bash scripts/check-qwen-marketplace.sh
bash scripts/check-package-versions.sh
bash scripts/lib/registry.sh --self-test
bash scripts/gen-release-please-packages.sh --check
```

- [ ] **2.10 Mutation check** (both bundler changes actually bite): with the
repo-user-guide case change reverted temporarily, `check-skills-bundles.sh`
must FAIL with `opencode/repo-user-guide-opencode: bundler failed` (bundler
demands a THIRD-PARTY-NOTICE the plugin doesn't have); restore and re-run →
pass. Same idea for the `zname` expectation: temporarily setting the
`expected` comparison to a literal wrong value must produce a `manifest name`
FAIL; revert.

- [ ] **Commit:** `feat(opencode): wire repo-user-guide-opencode into registry, bundles and release (G1)`

---

## Task 3: Byte-copy gate in `check-opencode-wiring.sh`

- [ ] **3.1** Inside check 7's pair loop in `scripts/check-opencode-wiring.sh`
(the `pairs="claude-code:rtk-mcp-cc opencode:rtk-mcp-opencode claude-code:doctor opencode:doctor"`
loop), append this after the existing pair `cmp` handling — but gate it so only
the repo-user-guide pair runs it (add it after the loop instead, simpler and
safer):

```bash
# The repo-user-guide skill is a byte-copy of the Claude original: host-portable,
# unlike the rtk/doctor skills above, which carry host-specific names and so are
# compared per-content instead. A drift here means the port drifted apart.
src_skills="claude-code/plugins/repo-user-guide-cc/skills"
dst_skills="opencode/plugins/repo-user-guide-opencode/skills"
checked=$((checked + 1))
if [ ! -d "$dst_skills" ]; then
    fail "copy missing: $dst_skills"
elif ! diff -r -q "$src_skills" "$dst_skills" >/dev/null 2>&1; then
    fail "stale copy: $dst_skills differs from $src_skills"
else
    printf '  ok       copy %-40s\n' "$dst_skills"
fi
```

Placement: immediately after the `pairs` loop closes, before check 8.

- [ ] **3.2 Verify pass, then fail:** `bash scripts/check-opencode-wiring.sh`
→ all checks pass (the new copy line prints `ok`). Then append a byte to
`opencode/plugins/repo-user-guide-opencode/skills/repo-user-guide/SKILL.md`,
re-run → FAIL `stale copy`; revert with
`git checkout -- opencode/plugins/repo-user-guide-opencode/skills` and re-run →
pass.

- [ ] **Commit:** `feat(checks): gate repo-user-guide opencode copy against the cc original (G1)`

---

## Task 4: Rust host-aware settings + guidance (G2, part 1)

File: `claude-code/plugins/re-ghidra-mcp-cc/` — `src/lib.rs`, `src/bin/hook.rs`,
`src/bin/mcp.rs`. Interface (additions to `lib.rs`):

```rust
pub enum Host { Claude, Opencode }
impl Host { pub fn settings_rel_path(self) -> &'static str; }
pub fn host_from_str(value: Option<&str>) -> Host;
pub fn host_from_env() -> Host;
pub fn settings_path(project_dir: &Path, host: Host) -> PathBuf;
pub fn load_settings(project_dir: &Path, host: Host) -> RawConfig;          // signature change
pub fn preflight(cfg: &RawConfig, probe: &Probe, host: Host) -> Vec<String>; // signature change
```

- [ ] **4.1 TDD — add the failing tests first** in `src/lib.rs`'s existing
`#[cfg(test)] mod settings_tests` and `mod preflight_tests`:

```rust
#[test]
fn settings_path_is_per_host() {
    let dir = Path::new("/proj");
    assert!(settings_path(dir, Host::Claude)
        .ends_with(".claude/re-ghidra-mcp-cc.local.md"));
    assert!(settings_path(dir, Host::Opencode)
        .ends_with(".opencode/re-ghidra-mcp-opencode.local.md"));
}

#[test]
fn host_from_str_defaults_to_claude() {
    assert_eq!(host_from_str(None), Host::Claude);
    assert_eq!(host_from_str(Some("")), Host::Claude);
    assert_eq!(host_from_str(Some("claude")), Host::Claude);
    assert_eq!(host_from_str(Some("OPENCODE")), Host::Claude); // exact match only
    assert_eq!(host_from_str(Some("opencode")), Host::Opencode);
}
```

Then update every existing `preflight(&cfg, &probe)` call site in the tests
(`fully_configured_reports_nothing`,
`install_dir_set_but_missing_dir_is_reported`, `absent_java_is_reported`,
`old_java_is_reported`, `missing_project_values_are_reported`) to
`preflight(&cfg, &probe, Host::Claude)`, and add the guidance-host test:

```rust
#[test]
fn guidance_names_the_active_hosts_file() {
    let cfg = fully_configured();      // reuse/inline the existing helper shape
    let probe = Probe { launcher_present: true, java_major: 21 };
    let f = preflight(&cfg, &probe, Host::Opencode);
    assert!(f[0].contains(".opencode/re-ghidra-mcp-opencode.local.md"), "{}", f[0]);
    let f = preflight(&cfg, &probe, Host::Claude);
    assert!(f[0].contains(".claude/re-ghidra-mcp-cc.local.md"), "{}", f[0]);
}
```

(If the existing helpers are built inline per test rather than shared
functions, build the complete cfg the same way those tests do — the point is
one finding whose text names the host file.)

`cargo test -p re-ghidra-mcp-cc` → compile errors (new API missing) = red.

- [ ] **4.2 Implement in `src/lib.rs`** — insert before the settings section:

```rust
// ── Host ───────────────────────────────────────────────────────────────

/// Which agent front is running. The settings file is per-host: a session on
/// one host must never read the other host's file, which is what makes
/// `.opencode/…local.md` and `.claude/…local.md` disjoint rather than a
/// migration path between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Claude,
    Opencode,
}

impl Host {
    /// The settings file this host reads, relative to a project root.
    pub fn settings_rel_path(self) -> &'static str {
        match self {
            Host::Claude => ".claude/re-ghidra-mcp-cc.local.md",
            Host::Opencode => ".opencode/re-ghidra-mcp-opencode.local.md",
        }
    }
}

/// The host named by `RE_GHIDRA_HOST`. Unset or anything but exactly
/// `"opencode"` means [`Host::Claude`]: the Claude front has never read this
/// variable, so defaulting there keeps every existing install on today's
/// behavior. OpenCode sets the variable in `opencode.jsonc`'s
/// `mcp.servers.ghidra.environment` (Task 5).
pub fn host_from_str(value: Option<&str>) -> Host {
    match value {
        Some("opencode") => Host::Opencode,
        _ => Host::Claude,
    }
}

/// [`host_from_str`] over the process environment.
pub fn host_from_env() -> Host {
    host_from_str(std::env::var("RE_GHIDRA_HOST").ok().as_deref())
}
```

Then migrate the three functions:

1. `pub fn settings_path(project_dir: &Path, host: Host) -> PathBuf` — body
   `project_dir.join(host.settings_rel_path())`.
2. `pub fn load_settings(project_dir: &Path, host: Host) -> RawConfig` — body
   reads `settings_path(project_dir, host)`; update its doc comment (lowest
   config layer, path chosen by `host`).
3. `pub fn preflight(cfg: &RawConfig, probe: &Probe, host: Host) -> Vec<String>`
   — only change is the final "Unset:" finding:

```rust
    findings.push(format!(
        "Unset: {}. Set them in `{}` (see the plugin's examples/ directory) or as \
         GHIDRA_MCP_PROJECT_DIR / GHIDRA_MCP_PROJECT_NAME / \
         GHIDRA_MCP_BOOTSTRAP_PROGRAM.",
        missing.join(", "),
        host.settings_rel_path()
    ));
```

Also update the module doc comment at the top of `lib.rs` (it says the crate
reads `.claude/re-ghidra-mcp-cc.local.md`): now "reads the host's settings
file — `.claude/re-ghidra-mcp-cc.local.md` on Claude, `.opencode/re-ghidra-mcp-opencode.local.md`
on OpenCode, selected by `RE_GHIDRA_HOST` — and the preflight the `SessionStart`
hook reports."

- [ ] **4.3 Call sites**

`src/bin/hook.rs` (behaviorally unchanged — this binary runs under Claude Code
and explicitly pins the Claude host):

```rust
use re_ghidra_mcp_cc::{load_settings, parse_java_major, preflight, Host, Probe};
// ...
let mut cfg = load_settings(&project_dir, Host::Claude);
// ...
let findings = preflight(&cfg, &probe, Host::Claude);
```

`src/bin/mcp.rs` (the shared binary both hosts launch — host comes from the
env):

```rust
let file_config =
    re_ghidra_mcp_cc::load_settings(&project_dir, re_ghidra_mcp_cc::host_from_env());
```

Update `mcp.rs`'s module doc comment (it says "reading `.claude/re-ghidra-mcp-cc.local.md`
as the lowest config layer" → name both files and `RE_GHIDRA_HOST`).

- [ ] **4.4 Verify:**

```bash
cargo test -p re-ghidra-mcp-cc
cargo build --workspace
```

Green, plus the new tests. `grep -rn "load_settings\|preflight(" claude-code/plugins/re-ghidra-mcp-cc/`
confirms no unmigrated call sites (`tests/e2e.rs` drives the binaries, not the
functions — no changes expected there; if it fails to compile, update it the
same way).

- [ ] **Commit:** `feat(re-ghidra): host-aware settings file and guidance (G2)`

---

## Task 5: opencode.jsonc env + settings docs (G2, part 2)

- [ ] **5.1 `opencode/plugins/re-ghidra-mcp-opencode/opencode.jsonc`** — add
the environment passthrough to the ghidra server (schema-verified:
`Mcp.LocalConfig.environment: Record<string,string>` exists in
`@opencode/schema` 2.0.26 — `node_modules/@opencode/schema/dist/mcp.d.ts:16`):

```jsonc
    "mcp": {
      "servers": {
        "ghidra": {
          "type": "local",
          "command": ["./bin/re-ghidra-cc-mcp"],
          "environment": { "RE_GHIDRA_HOST": "opencode" },
          "enabled": true,
          "disabled": false
        }
      }
    },
```

Preserve the file's existing key order and formatting — insert only the
`environment` line after `command`. (`check-opencode-wiring.sh` check 3 parses
it as JSONC; `environment` is not one of the rejected legacy keys.)
Verify: `jq empty` after stripping comments is what check 3 does — just run
`bash scripts/check-opencode-wiring.sh` after the edit → pass.

- [ ] **5.2 New example file `opencode/plugins/re-ghidra-mcp-opencode/examples/re-ghidra-mcp-opencode.local.md`:**
copy `claude-code/plugins/re-ghidra-mcp-cc/examples/re-ghidra-mcp-cc.local.md`
and replace every `.claude/re-ghidra-mcp-cc.local.md` occurrence with
`.opencode/re-ghidra-mcp-opencode.local.md`, then append this note at the end:

```markdown
> **Host selection:** `opencode.jsonc` sets `RE_GHIDRA_HOST=opencode` on the
> MCP server, so the server reads `.opencode/re-ghidra-mcp-opencode.local.md`
> (never the `.claude/` file). Precedence inside the plugin: CLI flag > `GHIDRA_*`
> environment > this file. The `SessionStart`-equivalent preflight is built
> into `plugin.ts` and reports to the session instead of to Claude Code's hook
> output.
```

- [ ] **5.3 `opencode/plugins/re-ghidra-mcp-opencode/README.md` changes:**

1. Replace the paragraph at lines ~56-58 ("The `SessionStart` preflight
   (`re-ghidra-cc-hook`) is deliberately NOT ported: interactive diagnosis
   belongs to the `doctor` skill.") with:

```markdown
The `SessionStart` hook itself is Claude Code-shaped and stays there — but its
diagnosis is ported: `plugin.ts` runs the same preflight (install dir, Java,
project values) once per session and, only when something is wrong, appends a
finding to the first session context. Silent when healthy, never blocks.
```

2. Replace the Settings row of the file table (line 57):
   `| Settings | `examples/re-ghidra-mcp-cc.local.md` | Per-project config via `.claude/*.local.md` |` →

```markdown
| Settings | `examples/re-ghidra-mcp-opencode.local.md` | Per-project config via `.opencode/*.local.md`, selected by `RE_GHIDRA_HOST` (set by `opencode.jsonc`) |
```

3. Replace lines 131-132 ("Either way, configure the project next — see
   `examples/re-ghidra-mcp-cc.local.md` / and copy it to `.claude/re-ghidra-mcp-cc.local.md`.")
   with:

```markdown
Either way, configure the project next — see
`examples/re-ghidra-mcp-opencode.local.md` and copy it to
`.opencode/re-ghidra-mcp-opencode.local.md`.
```

4. Add a **Settings layer** subsection after the environment-variable table
   (line 97):

```markdown
#### Settings layer (`.opencode/re-ghidra-mcp-opencode.local.md`)

Why a settings file matters more for this plugin than for most: **one server
is one Ghidra project.** The project directory, project name and bootstrap
program are per-workspace by nature, so without a settings layer every repo
would duplicate the plugin's registration in an `opencode.jsonc` just to carry
three strings.

Copy `examples/re-ghidra-mcp-opencode.local.md` to
`.opencode/re-ghidra-mcp-opencode.local.md` in the project root. Precedence is
CLI flag > `GHIDRA_*` environment > this file (decided by the shared config
resolver). The MCP server picks the file with `RE_GHIDRA_HOST=opencode`, which
`opencode.jsonc` sets — an OpenCode session never reads
`.claude/re-ghidra-mcp-cc.local.md`.
```

- [ ] **5.4 `opencode/plugins/re-ghidra-mcp-opencode/skills/doctor/SKILL.md`** —
two wrong-host references:

- Line 65: `.claude/re-ghidra-mcp-cc.local.md` →
  `.opencode/re-ghidra-mcp-opencode.local.md`
- Line 130: `then `.claude/re-ghidra-mcp-cc.local.md`` →
  `then `.opencode/re-ghidra-mcp-opencode.local.md``

(Verify with `grep -n 'claude' opencode/plugins/re-ghidra-mcp-opencode/skills/doctor/SKILL.md`
afterwards — there must be no `.claude/` path left. This skill is deliberately
host-ported, so editing it does not violate any byte-copy rule.)

- [ ] **5.5 `claude-code/plugins/re-ghidra-mcp-cc/README.md`** — in the
environment/`GHIDRA_*` documentation (the table near line 53 or the settings
paragraph at 131-132), add one row/sentence:

```markdown
| `RE_GHIDRA_HOST` | (`claude` default) | Which host's settings file the server reads: `claude` → `.claude/re-ghidra-mcp-cc.local.md`, `opencode` → `.opencode/re-ghidra-mcp-opencode.local.md`. Only the OpenCode front sets it. |
```

- [ ] **5.6 Verify:**

```bash
bash scripts/check-opencode-wiring.sh
grep -rn '\.claude/re-ghidra-mcp-cc\.local\.md' opencode/plugins/re-ghidra-mcp-opencode/ \
  | grep -v node_modules   # expected: no matches in README/INSTALL/skills;
                           # examples/ and README comparisons naming the cc file as "the other host's" are fine
```

- [ ] **Commit:** `feat(re-ghidra-opencode): settings-file layer, host selection and preflight docs (G2)`

---

## Task 6: TypeScript preflight in `plugin.ts` (G2, part 3)

Files: `opencode/plugins/re-ghidra-mcp-opencode/plugin.ts`,
`tests/hooks.test.ts`. The preflight is a **pure mirror** of
`claude-code/plugins/re-ghidra-mcp-cc/src/lib.rs`: same settings keys, same
env overlay, same `MIN_JAVA_MAJOR = 21`, same finding texts, with the
Claude-specific settings-file names replaced by the OpenCode one. Never blocks:
every path is wrapped so a failure degrades to *no report*.

**Interface** (all exported for tests):

```ts
export const MIN_JAVA_MAJOR = 21;
export const SETTINGS_REL_PATH = join(".opencode", "re-ghidra-mcp-opencode.local.md");
export interface RawSettings {
  ghidra_install_dir?: string; project_dir?: string; project_name?: string;
  bootstrap_program?: string; bootstrap_program_path?: string; max_heap?: string;
}
export interface Probe { launcherPresent: boolean; javaMajor: number | null }
export type ProbeFn = (ghidraDir: string | undefined) => Probe;
export function settingsFromMarkdown(source: string): RawSettings;
export function overlayEnv(cfg: RawSettings, env?: Record<string, string | undefined>): RawSettings;
export function parseJavaMajor(versionOutput: string): number | null;
export function collectProbe(ghidraDir: string | undefined): Probe;
export function preflight(cfg: RawSettings, probe: Probe): string[];
export function preflightFindings(projectDir: string, env?: Record<string, string | undefined>, probeFn?: ProbeFn): string[];
export function preflightReport(findings: string[]): string;
```

- [ ] **6.1 TDD — write the tests first** in `tests/hooks.test.ts`. Adjust the
existing harness:

```ts
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  preflight, overlayEnv, parseJavaMajor, settingsFromMarkdown,
  preflightFindings, preflightReport,
  type Probe,
} from "../plugin.ts";

const healthyProbe: Probe = { launcherPresent: true, javaMajor: 21 };

function healthyProjectDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "re-ghidra-opencode-"));
  mkdirSync(join(dir, ".opencode"), { recursive: true });
  writeFileSync(
    join(dir, ".opencode", "re-ghidra-mcp-opencode.local.md"),
    "---\nproject_dir: /re\nproject_name: p\nbootstrap_program: add.exe\n---\n",
  );
  return dir;
}
```

1. `fakeCtx(directory = healthyProjectDir())` — add
   `location: { directory }` to the inner `ctx` object (the real
   `Context.location.directory` is the project root; `Location.Info` in
   `@opencode/schema`).
2. Every existing `setup(ctx)` call becomes
   `setup(ctx, () => healthyProbe)` — the setup signature gains an optional
   second parameter (see 6.3) purely as a test seam; real hosts call
   `setup(ctx)` and get `collectProbe`.
3. Add `describe("preflight", …)` with:

```ts
test("settingsFromMarkdown reads the documented keys", () => {
  const s = settingsFromMarkdown(
    "\uFEFF---\r\nproject_dir: C:\\re\r\nproject_name: \"demo\"\r\nunknown_key: x\r\n---\r\nbody",
  );
  expect(s.project_dir).toBe("C:\\re");
  expect(s.project_name).toBe("demo");
  expect((s as Record<string, unknown>).unknown_key).toBeUndefined();
});

test("settingsFromMarkdown without frontmatter returns {}", () => {
  expect(settingsFromMarkdown("# just a file")).toEqual({});
  expect(settingsFromMarkdown("---\nno closing fence")).toEqual({});
  expect(settingsFromMarkdown("---\nproject_dir:\n---\n")).toEqual({}); // blank = unset
});

test("overlayEnv overrides and treats empty as unset", () => {
  const cfg = overlayEnv(
    { project_dir: "file-dir" },
    { GHIDRA_MCP_PROJECT_DIR: "env-dir", GHIDRA_MCP_PROJECT_NAME: "" },
  );
  expect(cfg.project_dir).toBe("env-dir");
  expect(cfg.project_name).toBeUndefined(); // absent or empty -> falls back to file
  expect(cfg.bootstrap_program).toBeUndefined();
});

test("parseJavaMajor handles modern, legacy and junk output", () => {
  expect(parseJavaMajor('openjdk version "21.0.4" 2024-07-16')).toBe(21);
  expect(parseJavaMajor('java version "1.8.0_402"')).toBe(8);
  expect(parseJavaMajor('garbage')).toBeNull();
  expect(parseJavaMajor("")).toBeNull();
});

test("preflight mirrors the Rust finding matrix", () => {
  const good = { launcherPresent: true, javaMajor: 21 } satisfies Probe;
  const complete = {
    ghidra_install_dir: "/opt/ghidra", project_dir: "/re",
    project_name: "p", bootstrap_program: "add.exe",
  };
  expect(preflight(complete, good)).toEqual([]);
  expect(preflight({}, { launcherPresent: false, javaMajor: null }).length).toBe(3);
  expect(preflight({ ghidra_install_dir: "/bad" }, good)[0])
    .toContain("not a Ghidra install root");
  expect(preflight(complete, { launcherPresent: true, javaMajor: 17 })[0])
    .toContain("version 17");
  const missing = preflight(
    { ghidra_install_dir: "/opt/ghidra" }, good,
  );
  expect(missing).toHaveLength(1);
  expect(missing[0]).toContain("Unset: project_dir, project_name, bootstrap_program");
  expect(missing[0]).toContain(".opencode/re-ghidra-mcp-opencode.local.md");
});

test("preflightFindings: healthy project reports nothing", () => {
  expect(preflightFindings(healthyProjectDir(), {}, () => healthyProbe)).toEqual([]);
});

test("preflightFindings: missing file yields the project-values finding only", () => {
  const dir = mkdtempSync(join(tmpdir(), "re-ghidra-empty-"));
  const f = preflightFindings(
    dir, { GHIDRA_INSTALL_DIR: "/opt/ghidra" }, () => healthyProbe,
  );
  expect(f).toHaveLength(1);
  expect(f[0]).toContain("Unset: project_dir, project_name, bootstrap_program");
});

test("preflightFindings fails open on any throw", () => {
  expect(preflightFindings("/nonexistent", {}, () => {
    throw new Error("probe exploded");
  })).toEqual([]);
});
```

4. Add a setup-level test:

```ts
test("setup appends exactly one preflight report on the first context event", async () => {
  const { hooks, ctx } = fakeCtx(mkdtempSync(join(tmpdir(), "re-ghidra-broken-")));
  await (plugin as unknown as {
    setup: (c: unknown, p?: ProbeFn) => Promise<void>;
  }).setup(ctx, () => ({ launcherPresent: false, javaMajor: null }));
  const calls = hooks["session:context"]!;
  const mk = () => ({ system: [] as Array<{ type: string; text: string }>, tools: {} });
  const e1 = mk();
  await calls[0]!(e1);
  expect(e1.system).toHaveLength(1);
  expect(e1.system[0]!.text).toContain("re-ghidra-mcp-opencode is not ready");
  expect(e1.system[0]!.text).toContain("No `java` on PATH"); // forced by the probe, env-independent
  const e2 = mk();
  await calls[0]!(e2);
  expect(e2.system).toHaveLength(0); // pushed once, then silent
});
```

(Why the probe is forced: `preflightFindings` inside `setup` reads
`process.env`, which a dev machine may populate — `javaMajor: null` guarantees
the "No `java`" finding regardless.)

`bun test` → red (exports don't exist yet).

- [ ] **6.2 Implement in `plugin.ts`.** New imports at the top:

```ts
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
```

Then the preflight block (placed after the driver-pointer helpers, exported):

```ts
// ── Preflight ──────────────────────────────────────────────────────────
// A pure mirror of claude-code/plugins/re-ghidra-mcp-cc/src/lib.rs::preflight:
// same settings keys, same env overlay, same thresholds, same finding texts
// (with the OpenCode settings-file name). Silent when healthy; every path
// fails open so a broken preflight can never block a session.

export const MIN_JAVA_MAJOR = 21;
export const SETTINGS_REL_PATH = join(".opencode", "re-ghidra-mcp-opencode.local.md");

export interface RawSettings { /* six optional fields, as in the interface block */ }

const SETTING_KEYS = [
  "ghidra_install_dir", "project_dir", "project_name",
  "bootstrap_program", "bootstrap_program_path", "max_heap",
] as const;

const ENV_KEYS: Array<[keyof RawSettings, string]> = [
  ["ghidra_install_dir", "GHIDRA_INSTALL_DIR"],
  ["project_dir", "GHIDRA_MCP_PROJECT_DIR"],
  ["project_name", "GHIDRA_MCP_PROJECT_NAME"],
  ["bootstrap_program", "GHIDRA_MCP_BOOTSTRAP_PROGRAM"],
  ["bootstrap_program_path", "GHIDRA_MCP_BOOTSTRAP_PROGRAM_PATH"],
  ["max_heap", "GHIDRA_MCP_MAX_HEAP"],
];

export function settingsFromMarkdown(source: string): RawSettings {
  const out: RawSettings = {};
  const body = source.startsWith("\uFEFF") ? source.slice(1) : source;
  if (!body.startsWith("---")) return out;
  const rest = body.slice(3).replace(/^\r/, "");
  if (!rest.startsWith("\n")) return out;
  const end = rest.indexOf("\n---");
  if (end < 0) return out;
  for (const rawLine of rest.slice(1, end).split("\n")) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const colon = line.indexOf(":");
    if (colon < 0) continue;
    const key = line.slice(0, colon).trim();
    let value = line.slice(colon + 1).trim();
    if (value.startsWith('"') || value.startsWith("'")) value = value.slice(1);
    if (value.endsWith('"') || value.endsWith("'")) value = value.slice(0, -1);
    if (!value) continue; // blank = unset, same as the Rust side
    if ((SETTING_KEYS as readonly string[]).includes(key)) {
      (out as Record<string, string>)[key] = value;
    }
  }
  return out;
}

export function overlayEnv(
  cfg: RawSettings,
  env: Record<string, string | undefined> = process.env,
): RawSettings {
  const out: RawSettings = { ...cfg };
  for (const [slot, key] of ENV_KEYS) {
    const v = env[key];
    if (v) (out as Record<string, string>)[slot] = v; // empty counts as unset
  }
  return out;
}

export function parseJavaMajor(versionOutput: string): number | null {
  const quoted = versionOutput.split('"')[1];
  if (quoted === undefined) return null;
  const parts = quoted.split(/[.\-_]/);
  const first = Number.parseInt(parts[0] ?? "", 10);
  if (Number.isNaN(first)) return null;
  if (first === 1) {
    const second = Number.parseInt(parts[1] ?? "", 10);
    return Number.isNaN(second) ? null : second;
  }
  return first;
}

export interface Probe { launcherPresent: boolean; javaMajor: number | null }
export type ProbeFn = (ghidraDir: string | undefined) => Probe;

export function collectProbe(ghidraDir: string | undefined): Probe {
  const launcherPresent =
    ghidraDir !== undefined &&
    (existsSync(join(ghidraDir, "support", "analyzeHeadless")) ||
      existsSync(join(ghidraDir, "support", "analyzeHeadless.bat")));
  let javaMajor: number | null = null;
  try {
    const res = spawnSync("java", ["-version"], { encoding: "utf8" });
    if (!res.error) {
      javaMajor =
        parseJavaMajor(String(res.stderr ?? "")) ?? parseJavaMajor(String(res.stdout ?? ""));
    }
  } catch {
    // no java (or spawn unsupported): stays null -> a finding, never a throw
  }
  return { launcherPresent, javaMajor };
}

export function preflight(cfg: RawSettings, probe: Probe): string[] {
  const findings: string[] = [];
  if (cfg.ghidra_install_dir === undefined) {
    findings.push(
      "GHIDRA_INSTALL_DIR is not set. Point it at your Ghidra install root (the directory containing `support/`).",
    );
  } else if (!probe.launcherPresent) {
    findings.push(
      `GHIDRA_INSTALL_DIR is set to \`${cfg.ghidra_install_dir}\` but there is no \`support/analyzeHeadless\` there, so it is not a Ghidra install root.`,
    );
  }
  if (probe.javaMajor === null) {
    findings.push(
      "No `java` on PATH. Ghidra 12.1.2 requires JDK 21 (`application.java.min=21`).",
    );
  } else if (probe.javaMajor < MIN_JAVA_MAJOR) {
    findings.push(
      `\`java\` on PATH reports version ${probe.javaMajor}; Ghidra 12.1.2 requires JDK ${MIN_JAVA_MAJOR} or newer.`,
    );
  }
  const missing = (["project_dir", "project_name", "bootstrap_program"] as const).filter(
    (name) => cfg[name] === undefined,
  );
  if (missing.length > 0) {
    findings.push(
      `Unset: ${missing.join(", ")}. Set them in \`.opencode/re-ghidra-mcp-opencode.local.md\` (see the plugin's examples/ directory) or as GHIDRA_MCP_PROJECT_DIR / GHIDRA_MCP_PROJECT_NAME / GHIDRA_MCP_BOOTSTRAP_PROGRAM.`,
    );
  }
  return findings;
}

export function preflightFindings(
  projectDir: string,
  env: Record<string, string | undefined> = process.env,
  probeFn: ProbeFn = collectProbe,
): string[] {
  try {
    let text = "";
    try {
      text = readFileSync(join(projectDir, SETTINGS_REL_PATH), "utf8");
    } catch {
      // missing/unreadable contributes nothing, like the Rust side
    }
    const cfg = overlayEnv(settingsFromMarkdown(text), env);
    return preflight(cfg, probeFn(cfg.ghidra_install_dir));
  } catch {
    return []; // fail-open: the preflight may never break a session
  }
}

export function preflightReport(findings: string[]): string {
  return [
    "re-ghidra-mcp-opencode is not ready to attach to a Ghidra project:",
    ...findings.map((f) => `  - ${f}`),
  ].join("\n");
}
```

- [ ] **6.3 Wire into `setup`.** Change the signature to
`async setup(ctx, probeFn: ProbeFn = collectProbe)` (optional extra parameter —
still assignable to `Plugin.define`'s `setup(context)` type) and add before the
compaction hook:

```ts
    // Silent when healthy, never blocks: any failure degrades to no report.
    let pending: string[] = [];
    try {
      const findings = preflightFindings(ctx.location.directory, process.env, probeFn);
      if (findings.length > 0) pending = [preflightReport(findings)];
    } catch {
      pending = [];
    }
```

At the top of the existing context handler (before the driver-pointer push,
after the `Array.isArray(event.system)` guard):

```ts
      if (pending.length > 0) {
        event.system.push({ type: "text", text: pending[0]! });
        pending = [];
      }
```

Nothing else changes: still exactly the `compaction` + `context` hooks.

- [ ] **6.4 Verify:**

```bash
cd opencode/plugins/re-ghidra-mcp-opencode && bun install && bun test
```

All green — including the pre-existing hook-registration/context tests (they
now run against `healthyProjectDir()` + `healthyProbe`, so they stay silent
when healthy). If `bun install` rewrites `bun.lock`, include the lock in the
commit.

- [ ] **Commit:** `feat(re-ghidra-opencode): session preflight, silent when healthy (G2)`

---

## Task 7: Footprint covers skills-only opencode plugins (G3)

Two layers: the footprint tool must accept such plugins, then the regen loop
and budgets check must enumerate them.

- [ ] **7.1 `tools/plugin-footprint/src/manifest.rs` — `looks_like_a_plugin`.**
Current definition requires `.claude-plugin/plugin.json` or the
`plugin.ts` + `opencode.jsonc` pair, so a skills-only opencode dir (only
`package.json`) is refused as "not a plugin". Replace:

```rust
pub fn looks_like_a_plugin(dir: &Path) -> bool {
    dir.join(".claude-plugin").join("plugin.json").is_file()
        || (dir.join("plugin.ts").is_file() && dir.join("opencode.jsonc").is_file())
        // A skills-only OpenCode plugin ships no plugin.ts and no
        // opencode.jsonc; package.json is its manifest (bundle-skills-plugin
        // and the registry both treat it that way).
        || dir.join("package.json").is_file()
}
```

Extend the doc comment above it with the third case. Add a unit test (append to
manifest.rs's existing `#[cfg(test)]` module, or a new one if absent — this
repo has no `tempfile` dev-dep, so create/clean manually):

```rust
#[test]
fn skills_only_opencode_package_json_looks_like_a_plugin() {
    let dir = std::env::temp_dir().join(format!("pf-looks-like-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("package.json"), "{}").unwrap();
    assert!(looks_like_a_plugin(&dir));
    assert!(looks_like_a_plugin(Path::new("claude-code/plugins/repo-user-guide-cc")));
    std::fs::remove_dir_all(&dir).unwrap();
}
```

- [ ] **7.2 `tools/plugin-footprint/src/main.rs` — agent classification in
`measure`.** `is_opencode = plugin_dir.join("opencode.jsonc").is_file()` would
label a skills-only opencode dir `claude-code`. Replace that line with:

```rust
    let is_opencode = plugin_dir.join("opencode.jsonc").is_file()
        || (plugin_dir.join("package.json").is_file()
            && !plugin_dir.join(".claude-plugin").join("plugin.json").is_file());
```

Also update the error message in `measure` that says
`"… (no .claude-plugin/plugin.json)"` to
`"… (no .claude-plugin/plugin.json, plugin.ts+opencode.jsonc, or package.json)"`.
Everything downstream already handles the skills-only shape: no `.mcp.json` /
no `opencode.jsonc` → zero servers; no `plugin.ts` → no Setup tier
(`read_plugin_source` returns `None`); `opencode_plugins()` and `publish`
iterate the same `looks_like_a_plugin` filter, so the new plugins flow into
publish automatically.

- [ ] **7.3 Tests:** `cargo test -p plugin-footprint` — green, and **no existing
snapshot may change** (this must not alter any committed footprint doc for
plugins that were already measurable). If an insta snapshot fails, stop and
investigate rather than accepting it.

- [ ] **7.4 `Justfile` — `footprint-regen` opencode loop.** Replace the loop
that filters on `[ -f "$dir/plugin.ts" ]` with the registry-driven form the
claude-code loop already uses:

```bash
    for plugin in $(bash scripts/registry-plugins.sh opencode | tr -d '\r'); do
        dir="opencode/plugins/$plugin"
        cargo run -q -p plugin-footprint --bin plugin-footprint -- measure "$dir" --out "docs/footprints/$plugin.json"
        cargo run -q -p plugin-footprint --bin plugin-footprint -- ratchet \
            --measured "docs/footprints/$plugin.json" --budgets docs/footprints/budgets.json
        echo "regenerated docs/footprints/$plugin.json"
    done
```

(`registry-plugins.sh` with no flags lists every registry plugin and already
excludes `notPublished` entries — same as the claude-code loop. Keep the
claude-code half of the recipe byte-identical.)

- [ ] **7.5 `scripts/check-footprint-budgets.sh` — "live" definition.** Replace
the `live=$(…)` block's opencode half (the `for dir in …/*/` + `[ -f
"$dir/plugin.ts" ]` loop) with:

```bash
live=$(
    bash scripts/registry-plugins.sh claude-code | tr -d '\r'
    bash scripts/registry-plugins.sh opencode | tr -d '\r'
)
```

Update its leading comment: live = every registry key on both fronts (skills-only
plugins are measured too — a footprint doc without a budget key is unguarded).

- [ ] **7.6 Footprint markers.** `plugin-footprint publish` hard-errors on a
README without a marker pair (`publish.rs`: "has no `<!-- footprint:begin -->` /
`<!-- footprint:end -->` region"). `repo-user-guide-opencode`'s README already
has the pair (Task 1.3). Add one to
`opencode/plugins/aiplugins-devtools-opencode/README.md` — insert directly
above the `## Attribution` line:

```markdown
<!-- footprint:begin -->
### Context footprint

Generated by `tools/plugin-footprint`; regenerate with `just footprint-regen`.
<!-- footprint:end -->
```

(Region contents are rewritten wholesale by publish; the pair is what matters.)

- [ ] **7.7 Regenerate:**

```bash
just build-rtk-mcp-cc build-re-ghidra-mcp-cc build-rtk-mcp-opencode build-re-ghidra-mcp-opencode
just footprint-regen
git status --short docs/footprints/ README.md opencode/plugins/*/README.md claude-code/plugins/*/README.md
```

Expected: new `docs/footprints/aiplugins-devtools-opencode.json` and
`docs/footprints/repo-user-guide-opencode.json`; `budgets.json` gains both
keys; footprint regions update in the root README and the two (now measured)
opencode plugin READMEs. **No byte change** to any previously published
region/doc except where the root table gains the two rows. Run
`git diff` on one known-good file (e.g. `docs/footprints/rtk-mcp-cc.json`) to
confirm stability before committing; if a previously stable doc changed,
the local binaries differ from what CI measured — rebuild and re-run before
proceeding.

- [ ] **7.8 Verify:** `bash scripts/check-footprint-budgets.sh` → pass with the
new keys. If CI's footprint-freshness job is the authority, also re-run
`just footprint-regen && git diff --exit-code docs/` → clean.

- [ ] **Commit:** `feat(footprint): measure skills-only opencode plugins and ratchet them (G3)`

---

## Task 8: Pin drift gate + pin fixes (G5)

- [ ] **8.1 Fix the stale pins.** Every `2.0.24` occurrence in the plugin docs
is an `@opencode/plugin` claim; `package.json` says `2.0.26` everywhere:

```bash
for f in \
  opencode/plugins/rtk-mcp-opencode/README.md \
  opencode/plugins/rtk-mcp-opencode/INSTALL.md \
  opencode/plugins/re-ghidra-mcp-opencode/README.md \
  opencode/plugins/re-ghidra-mcp-opencode/INSTALL.md \
  opencode/plugins/todov2-opencode/INSTALL.md
do
  sed 's/2\.0\.24/2.0.26/g' "$f" > "$f.tmp" && mv "$f.tmp" "$f"
done
grep -rn '2\.0\.24' opencode/plugins/ --include='*.md' | grep -v node_modules
```

Expected: no matches left (portable `sed`, no `-i`).

- [ ] **8.2 Fix the code comments** that cite the observed type shape
(`re-ghidra-mcp-opencode/plugin.ts` line 22; `tests/hooks.test.ts` lines ~40
and ~52): `@opencode/plugin@2.0.21` → `@opencode/plugin@2.0.26`. The claim is
verified by the suite itself — tests run against the installed 2.0.26.

**Do NOT touch** `opencode/plugins/rtk-mcp-opencode/tests/shell-shape.json`:
its `2.0.21` is recorded provenance (what the shell shapes were recorded
against), not a current pin.

- [ ] **8.3 Refresh lockfiles** (they still resolve `2.0.24` while
`package.json` pins `2.0.26`):

```bash
(cd opencode/plugins/rtk-mcp-opencode && bun install)
(cd opencode/plugins/re-ghidra-mcp-opencode && bun install)
git diff --stat opencode/plugins/*/bun.lock
```

Commit the refreshed locks.

- [ ] **8.4 Add gate check 9 to `scripts/check-opencode-wiring.sh`.** Define
next to the existing `jqr()` helper:

```bash
# Every @opencode/plugin version a README/INSTALL claims, one per line:
#   @opencode/plugin@2.0.26
#   `@opencode/plugin` **2.0.26**
#   `@opencode/plugin` than `2.0.26`
# Anchored on the plugin token so the OpenCode release number on the same
# line ("Tested with OpenCode v2.0.20 and …") is never captured.
doc_pins() {
    sed -n \
        -e 's/.*@opencode\/plugin@\([0-9][0-9.]*\).*/\1/p' \
        -e 's/.*@opencode\/plugin` \*\*\([0-9][0-9.]*\)\*\*.*/\1/p' \
        -e 's/.*@opencode\/plugin` than `\([0-9][0-9.]*\)`.*/\1/p' \
        "$1"
}
```

After check 8, append:

```bash
# 9. Doc pins: every @opencode/plugin version claimed in a README/INSTALL must
#    equal package.json, so a dependency bump fails until the docs move too.
for name in $(registry_plugins opencode | sort); do
    dir="opencode/plugins/$name"
    pkg="$dir/package.json"
    [ -f "$pkg" ] || continue
    pin=$(jqr '.dependencies["@opencode/plugin"] // ""' "$pkg" 2>/dev/null || echo "")
    [ -n "$pin" ] || continue # skills-only plugins carry no pin (as check 8)
    for doc in "$dir/README.md" "$dir/INSTALL.md"; do
        [ -f "$doc" ] || continue
        while IFS= read -r claim; do
            [ -n "$claim" ] || continue
            checked=$((checked + 1))
            [ "$claim" = "$pin" ] ||
                fail "$doc claims @opencode/plugin $claim, package.json pins $pin"
        done < <(doc_pins "$doc")
    done
done
```

- [ ] **8.5 Verify pass, then fail:** `bash scripts/check-opencode-wiring.sh`
→ pass. Temporarily revert one line of `rtk-mcp-opencode/README.md` to
`2.0.24`, re-run → FAIL
`… claims @opencode/plugin 2.0.24, package.json pins 2.0.26`; restore → pass.

- [ ] **Commit:** `fix(opencode): sync @opencode/plugin pins to 2.0.26 and gate doc claims (G5)`

---

## Task 9: PR-time bundle smoke (G4)

New `scripts/smoke-opencode-bundle.sh`, `just smoke-opencode-bundle`, CI job.
Covers the **plugin.ts plugins**; skills-only zips are already assembled and
checked by `check-skills-bundles.sh` in `just check` — this closes the gap that
nothing at PR time ever runs `bundle-opencode-plugin.sh` (tag-time only today).

- [ ] **9.1 Create `scripts/smoke-opencode-bundle.sh`** (executable, `chmod +x`):

```bash
#!/usr/bin/env bash
#
# Assemble every OpenCode plugin.ts bundle and check the zip contents — the
# PR-time counterpart to opencode-bundles.yml's tag-time verify job.
#
# Skills-only plugins are NOT covered here: check-skills-bundles.sh already
# assembles and checks those (they go through bundle-skills-plugin.sh, a
# different script). This covers the plugin.ts plugins through
# bundle-opencode-plugin.sh — the same script the tag workflow runs — so "the
# zip we would publish carries the file set an install needs" is checked on
# every PR instead of only when a tag lands.
#
# Why Ubuntu only in CI: both bundler scripts refuse Windows shells (FAT
# attributes, MSYS path translation) — see their headers. macOS would be
# redundant: the file set is platform-independent.
set -euo pipefail
cd "$(dirname "$0")/.."

source scripts/lib/registry.sh

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail=0
bad() { echo "  FAIL  $1" >&2; fail=$((fail + 1)); }

for name in $(registry_plugins opencode --exclude-skills-only | sort); do
    zip="$tmp/$name-opencode.zip"
    if ! bash scripts/bundle-opencode-plugin.sh "opencode/plugins/$name" "$tmp" >/dev/null; then
        bad "$name: bundler failed"
        continue
    fi
    [ -f "$zip" ] || { bad "$name: $zip missing after bundling"; continue; }

    out="$tmp/extract-$name"
    mkdir -p "$out"
    unzip -q "$zip" -d "$out"

    # The exact set bundle-opencode-plugin.sh stages as required, plus what the
    # sources actually ship (the tag workflow's verify step checks the same).
    for f in plugin.ts opencode.jsonc package.json README.md INSTALL.md; do
        [ -f "$out/$f" ] || bad "$name: $f missing from the bundle"
    done
    jq -e . "$out/opencode.jsonc" > /dev/null 2>&1 ||
        bad "$name: bundled opencode.jsonc is not valid JSON"

    src="opencode/plugins/$name"
    for optional in skills agents tool tui.tsx tsconfig.json; do
        if [ -e "$src/$optional" ]; then
            [ -e "$out/$optional" ] || bad "$name: $optional missing from the bundle"
        fi
    done

    # The zip is versioned source only: build inputs must never leak in.
    if unzip -Z1 "$zip" | grep -q '^bin/\|^tests/\|^node_modules/\|^bun\.lock$'; then
        bad "$name: bundle carries build inputs (bin/, tests/, node_modules/ or bun.lock)"
    fi
done

[ "$fail" -eq 0 ] || exit 1
echo "All OpenCode plugin bundles assemble with the expected contents."
```

- [ ] **9.2 `Justfile`** — add next to the existing `opencode-*` recipes:

```
# Assemble every OpenCode plugin zip locally and check its contents (PR-time
# smoke; tag-time bundling and published-zip verification stay in
# opencode-bundles.yml, skills-only zips in check-skills-bundles.sh).
smoke-opencode-bundle:
    bash scripts/smoke-opencode-bundle.sh
```

- [ ] **9.3 `.github/workflows/ci.yml`** — add a job beside `opencode-smoke`
(aligning with the file's existing indentation):

```yaml
  # PR-time assembly: every plugin.ts plugin's zip builds and carries the file
  # set an install needs. Tag-time bundling stays in opencode-bundles.yml;
  # skills-only zips are covered by check-skills-bundles.sh in the checks job.
  # Ubuntu only: both bundlers refuse Windows shells.
  opencode-bundle-smoke:
    name: OpenCode Bundle Smoke
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - name: Assemble and verify every opencode plugin bundle
        run: bash scripts/smoke-opencode-bundle.sh
```

- [ ] **9.4 Verify:** `bash scripts/smoke-opencode-bundle.sh` → pass for
`rtk-mcp-opencode`, `re-ghidra-mcp-opencode`, `todov2-opencode` (the
`--exclude-skills-only` list; note no cargo build is needed — the bundlers
ship source only, binaries are staged at INSTALL time). Mutation: temporarily
`mv opencode/plugins/todov2-opencode/INSTALL.md /tmp/` → bundler-failed FAIL;
restore → pass.

- [ ] **Commit:** `ci(opencode): PR-time bundle assembly smoke (G4)`

---

## Task 10: dependabot todov2 + `just bundle-opencode` (G6, G7)

- [ ] **10.1 `.github/dependabot.yml`** — the `todov2-opencode` dir has npm
deps (`@opencode/plugin`, `@opentui/*`) but no entry, so nothing watches them.
Add after the `aiplugins-devtools-opencode` block (indentation per file):

```yaml
  - package-ecosystem: "npm"
    directory: "/opencode/plugins/todov2-opencode"
    schedule:
      interval: "weekly"
    groups:
      npm-todov2:
        patterns:
          - "*"
```

- [ ] **10.2 `Justfile`** — add beside the `bundle-plugins` /
`bundle-qwen-plugins` recipes:

```
# Assemble the installable OpenCode plugin zips from a working tree — the
# local pre-tag counterpart of opencode-bundles.yml (no releases involved:
# bundle-opencode-plugin.sh stages versioned source only).
bundle-opencode:
    #!/usr/bin/env bash
    set -euo pipefail
    names=$(bash scripts/registry-plugins.sh opencode --exclude-skills-only | tr -d '\r')
    rm -rf target/opencode-bundles
    mkdir -p target/opencode-bundles
    for name in $names; do
        bash scripts/bundle-opencode-plugin.sh "opencode/plugins/$name" target/opencode-bundles
    done
```

- [ ] **10.3 Verify:** `python3 -c 'import yaml,sys; yaml.safe_load(open(".github/dependabot.yml"))'`
→ valid; `just --list` shows both recipes; `just bundle-opencode` produces
`target/opencode-bundles/{rtk-mcp-opencode,re-ghidra-mcp-opencode,todov2-opencode}-opencode.zip`.
`bash scripts/check-skills-release-wiring.sh` still passes (dependabot is not
in its scope, cheap confirmation nothing else reads this file's shape).

- [ ] **Commit:** `chore: watch todov2-opencode deps and add just bundle-opencode (G6, G7)`

---

## Task 11: Doc fixes (G8)

- [ ] **11.1 `docs/specs/2026-10-01-opencode-v2-design.md` status header** —
replace the four-line status paragraph:

old:

```markdown
**Status:** SPEC, brainstormed 2026-10-01; unimplemented. All five design
sections were presented and approved in chat before writing. Approach A
(mirror the agent dirs 1:1) was chosen over a shared TS library (B) and
npm publishing (C, deferred to a later phase).
```

new:

```markdown
**Status:** SHIPPED (2026-10). All five design sections were presented
and approved in chat before writing. Approach A (mirror the agent dirs
1:1) was chosen over a shared TS library (B) and npm publishing (C,
deferred). Superseded in part by
`docs/superpowers/specs/2026-10-17-opencode-parity-design.md`: the
"deliberately NOT ported" preflight and the `.claude`-only settings path
are ported there, and §5's "no marketplace manifest" yielded to the
first-class bundles plan.
```

- [ ] **11.2 Root `README.md`:**

1. Grep for the stale sentence: `grep -n 'Fronted today by' README.md`
   (around line 110). Replace it with:

```markdown
Fronted by all four agents: [`claude-code/plugins/re-ghidra-mcp-cc`](claude-code/plugins/re-ghidra-mcp-cc/), [`opencode/plugins/re-ghidra-mcp-opencode`](opencode/plugins/re-ghidra-mcp-opencode/), [`antigravity/plugins/re-ghidra-mcp-agy`](antigravity/plugins/re-ghidra-mcp-agy/) and [`qwen/plugins/re-ghidra-mcp-qwen`](qwen/plugins/re-ghidra-mcp-qwen/).
```

2. In the Documentation index's claude-code section, after the
   `re-ghidra-mcp-cc` row, add:

```markdown
  - [`repo-user-guide-cc`](claude-code/plugins/repo-user-guide-cc/) — PDF user guides for a repository, with verified examples; published as `repo-user-guide`.
```

3. In the opencode section (after the `todov2-opencode` row), add:

```markdown
  - [`repo-user-guide-opencode`](opencode/plugins/repo-user-guide-opencode/) — PDF user guides for a repository, with verified examples; published as `repo-user-guide`.
```

- [ ] **11.3 Create `claude-code/skills/README.md`** — the standalone-skills
dirs are asymmetric (opencode's has a README, claude-code's doesn't):

```markdown
# claude-code / skills

Standalone skills for this host. Plugin-bundled skills stay inside `plugins/*/skills/` and are not duplicated here.
```

- [ ] **11.4 Verify:** `grep -rn 'unimplemented' docs/specs/2026-10-01-opencode-v2-design.md`
→ only historical mentions (none in the status header);
`grep -n 'repo-user-guide' README.md` shows both index rows;
`bash scripts/check-repo-structure.sh` and `bash scripts/check-readme.sh`
still pass (the index/structure checks enumerate files).

- [ ] **Commit:** `docs: fix shipped-status headers, stale front list, skills README asymmetry (G8)`

---

## Final verification

- [ ] Run the full suite: `just check` (long — every gate, both smoke suites,
all unit tests). Fix anything it flags before finishing.
- [ ] `git log --oneline` — one clean commit per task, in task order.
- [ ] Summary of what changed, per gap G1–G8, for the user.

## Spec corrections applied during planning (no design change)

Recorded for transparency — the approved spec was edited to match reality in
five places: (1) `package.json` name is the shared family `repo-user-guide`
via `publishedNames`, not `repo-user-guide-opencode`; (2) release-please config
is generated, not hand-edited; (3) a workflow change IS needed
(`opencode-bundles.yml` exclusion); (4) `registry.sh` has no opencode list
assertion today — the plan adds one; (5) G3 requires footprint-tool code
changes (`looks_like_a_plugin`, agent classification), not just loop fixes.
