# Install-Document Pins and Root README Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the 14 install documents on `main` to the current releases, make CI fail when one drifts again, and list every OpenCode plugin in the root README.

**Architecture:** One script, `scripts/check-install-pins.sh`, reads each plugin's version from its own manifest and checks (default) or rewrites (`--write`) the release pins in the OpenCode `INSTALL.md` files and the Antigravity and Qwen READMEs. `.github/workflows/release-refresh.yml` runs it with `--write` on the release branch; CI and `just check` run the check. A second script, `scripts/check-readme-coverage.sh`, checks the `## Installing` section of the root README against the registry.

**Tech Stack:** bash, POSIX awk, `jq`, GitHub Actions, `just`.

**Spec:** `docs/superpowers/specs/2026-11-21-install-pins-design.md` (approved by the owner on 2026-11-21). Executors read both documents.

## Global Constraints

- **Exactly two commits, in this order** (spec 4.5). Task 1 ends with commit 1. Tasks 2 to 6 build commit 2 and only Task 6 commits. Do not commit at the end of Tasks 2 to 5: `scripts/check-gate-wiring.sh` fails `just check` for a `scripts/check-*.sh` that has no wiring, and `scripts/check-doc-paths.sh` fails for a path that `docs/PLUGIN_GUIDELINES.md` names before it exists.
- **Commit subjects, exactly:** commit 1 `docs: reword stale version claims and list every OpenCode plugin in the root README`; commit 2 `chore: generate and gate the install-document release pins`. Neither is `feat` or `fix`, so release-please cuts no release for the plugin folders these commits touch. Measured in this repository's history: the document-only commit `b8eb834` (`docs(repo-tasks): ...`) touched plugin folders and appears in no `CHANGELOG.md`, and the changelog sections release-please has written here are only Features, Bug Fixes and Reverts. If the release PR that follows the merge still proposes a version bump caused only by these two commits, stop and tell the owner.
- **Commit trailer:** end each commit message with the `Co-Authored-By:` line that the session's attribution reminder names at commit time.
- **Branch:** work on `docs/install-pins`, which already carries the spec and this plan and was cut from `origin/main` at `7d1656a`. Before Task 1 run `git fetch origin main && git rev-parse --short origin/main`; if it no longer prints `7d1656a`, rebase onto it and re-run Step 0 of every task, because the line numbers below were read at that commit.
- **Never type a version number into a document.** The numbers in Task 6 come from `bash scripts/check-install-pins.sh --write`. The spec section 1 tables say what they must be.
- **Portability:** `just check` runs on macOS (BSD awk and sed) and Git Bash. Both scripts use POSIX awk only (no intervals, no `gensub`, no `match()` arrays), no `sed -i`, no `\b`, no `grep -P`. Do not "simplify" them with GNU-only features.
- **License header:** both new scripts start with the 8-line header every script here carries; the lefthook pre-commit hook runs `scripts/check-license-headers.sh` on staged files. The file contents below already include it.
- **The repository's gate as it is:** `just check`, with no added flags. Do not make any check stricter than the repository does.
- **Scratch files** go in a directory outside the repository. The steps write it as `$SCRATCH`; set it once with `SCRATCH=$(mktemp -d)`.
- **Shape rule for implementers:** paste the file contents and the replacement blocks below unchanged. If something does not apply or does not pass, stop and report it as `STATE_MISMATCH: <what differs>` or `[original] -> [yours] because <reason>`. Do not adapt the text to make a check pass, and do not edit a selftest to match the code: the spec is the oracle.

## How this plan was verified

The plan was written against the repository at `00c6d87` and replayed before it was saved: on a fresh export of that commit, applying every edit below, creating both scripts from the blocks below and running `--write` reproduced, byte for byte, the tree in which everything in this section was measured. Measured there, on Linux:

- both selftests pass (30 and 16 cases), under GNU awk, `gawk --posix`, mawk and busybox awk;
- 25 line-level mutants each turn a named selftest case red (Task 2 Step 4 and Task 3 Step 4);
- `shellcheck -x` is clean on both scripts, and `typos` is clean;
- after commit 1 and again after commit 2, each of these passes: `just doc-paths`, `just spellcheck`, `just links`, `just opencode-wiring`, `just agy-wiring`, `scripts/check-gate-wiring.sh` and its selftest, `scripts/check-skills-bundles.sh`, `scripts/check-skills-release-wiring.sh`, `scripts/gen-marketplaces.sh --check`, the three marketplace checks, `scripts/check-package-versions.sh`, `scripts/check-release-manifest.sh` and `scripts/check-devtools-skills.sh`; `just site-check` passes after commit 2 (it was not run after commit 1);
- the verification commands of Task 1 Step 3, Task 4 Step 4, Task 5 Step 2 and Task 6 Steps 1 and 2 were run as written, and their expected output below is what they printed;
- `actionlint` reports nothing for `.github/workflows/release-refresh.yml`. It reports three `SC2086` notes at `.github/workflows/ci.yml:344`, a step this plan does not touch, with or without this change.

Not measured, and so the plan measures them during execution: the full `just check` (it builds the Rust workspace), `just footprint` and `just footprint-budgets`, a real macOS or Git Bash run, and a live run of the release-refresh workflow (it only runs on the release-please branch).

## File structure

| File | Commit | Responsibility |
|---|---|---|
| `opencode/plugins/*/INSTALL.md` (6 of 7), `antigravity/plugins/rtk-mcp-agy/README.md`, `antigravity/plugins/re-ghidra-mcp-agy/README.md` | 1 | Lose the phrase `current as of this writing` (ten occurrences). |
| `README.md` | 1 | Three more OpenCode table rows and three more raw-URL bullets. |
| `scripts/check-install-pins.sh` (new) | 2 | Check or rewrite the release pins; its own selftest. |
| `scripts/check-readme-coverage.sh` (new) | 2 | Check the root README's `## Installing` section; its own selftest. |
| `Justfile` | 2 | Four recipes, added to `check`. |
| `.github/workflows/ci.yml` | 2 | Four steps in the `wiring` job. |
| `.github/workflows/release-refresh.yml` | 2 | Run `--write`, stage the documents, fail the job when it failed. |
| `docs/PLUGIN_GUIDELINES.md` | 2 | Tell plugin authors the pins are rewritten by the script and name both checks. |
| 7 `INSTALL.md`, 4 agy READMEs, 3 qwen READMEs | 2 | The version numbers, written by `--write`. |

---

### Task 1: Hand edits (commit 1)

**Files:**
- Modify: the eight documents and `README.md` named in the edits below.

**Interfaces:**
- Consumes: nothing.
- Produces: a tree with no `current as of this writing` in any covered document, which Task 6's `--write` requires (the script refuses to write while the phrase exists), and a root README that Task 3's check passes.

- [ ] **Step 0: Confirm the starting state**

Run:

```bash
for f in opencode/plugins/*/INSTALL.md antigravity/plugins/*/README.md qwen/plugins/*/README.md; do
  n=$(awk '{ sub(/\r$/, ""); sub(/^ *>? ?/, ""); printf "%s ", $0 }' "$f" | tr -s '[:space:]' ' ' | grep -oF 'current as of this writing' | wc -l)
  [ "$n" -gt 0 ] && echo "$n $f"
done
grep -c '^| \[`.*`\](opencode/plugins/' README.md
grep -c '^- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/' README.md
```

Expected, exactly (ten occurrences in eight documents; the loop joins lines because two occurrences wrap across blockquote lines and a one-line `grep` misses them):

```text
1 opencode/plugins/aiplugins-devtools-opencode/INSTALL.md
1 opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md
2 opencode/plugins/re-ghidra-mcp-opencode/INSTALL.md
1 opencode/plugins/repo-user-guide-opencode/INSTALL.md
2 opencode/plugins/rtk-mcp-opencode/INSTALL.md
1 opencode/plugins/todov2-opencode/INSTALL.md
1 antigravity/plugins/re-ghidra-mcp-agy/README.md
1 antigravity/plugins/rtk-mcp-agy/README.md
4
4
```

If it differs, stop and report `STATE_MISMATCH`.

- [ ] **Step 1: Reword the ten occurrences**

Each edit replaces a claim that repeats a version number with a pointer to the pinned line or URL. The instruction to agree the versions with the user stays. Edits 5 to 8 are the same two texts in two files (`rtk-mcp-opencode` and `re-ghidra-mcp-opencode`); they are written out twice on purpose. The em dashes in edits 5 to 10 are in the files today; keep them.

**Edit 1.** `opencode/plugins/aiplugins-devtools-opencode/INSTALL.md:10-12`

Find exactly:

````text
> unpinned URLs. Agree the version with the user first (`0.1.0` is current as
> of this writing; check https://github.com/ckir/aiplugins/releases for what
> is newest) and use it for every URL below.
````

Replace with:

````text
> unpinned URLs. Agree the version with the user first (the `VERSION=` line
> in Step 2 holds the release this document was last refreshed for; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use it
> for every URL below.
````

**Edit 2.** `opencode/plugins/repo-user-guide-opencode/INSTALL.md:9-11`

Find exactly:

````text
> unpinned URLs. Agree the version with the user first (`0.2.0` is current as
> of this writing; check https://github.com/ckir/aiplugins/releases for what
> is newest) and use it for every URL below.
````

Replace with:

````text
> unpinned URLs. Agree the version with the user first (the `VERSION=` line
> in Step 2 holds the release this document was last refreshed for; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use it
> for every URL below.
````

**Edit 3.** `opencode/plugins/todov2-opencode/INSTALL.md:10-13`

Find exactly:

````text
> no unpinned URLs. Agree the version with the user first (`0.3.0` is
> current as of this writing; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use it
> for every URL below.
````

Replace with:

````text
> no unpinned URLs. Agree the version with the user first (the `VERSION=`
> line in Step 2 holds the release this document was last refreshed for;
> check https://github.com/ckir/aiplugins/releases for what is newest) and
> use it for every URL below.
````

**Edit 4.** `opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md:10-13`

Find exactly:

````text
> with the user first (plugin and MCP server binary version independently;
> `0.1.0` / `0.1.0` are current as of this writing; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use
> them for every URL below.
````

Replace with:

````text
> with the user first (plugin and MCP server binary version independently;
> the `VERSION=` and `CC_VERSION=` lines in Step 2 hold the releases this
> document was last refreshed for; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use
> them for every URL below.
````

**Edit 5.** `opencode/plugins/rtk-mcp-opencode/INSTALL.md:11-14`

Find exactly:

````text
> and MCP server binary version independently — `0.7.3` / `0.8.0` are
> current as of this writing; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use
> them for every URL below.
````

Replace with:

````text
> and MCP server binary version independently — the `VERSION=` and
> `CC_VERSION=` lines in Step 2 hold the releases this document was last
> refreshed for; check https://github.com/ckir/aiplugins/releases for what
> is newest) and use them for every URL below.
````

**Edit 6.** `opencode/plugins/rtk-mcp-opencode/INSTALL.md:56-58`

Find exactly:

````text
binary below come from different packages on independent versions (shown
with the versions current as of this writing — substitute the agreed
versions everywhere `VERSION` and `CC_VERSION` appear):
````

Replace with:

````text
binary below come from different packages on independent versions (the
two lines below hold the releases this document was last refreshed for —
substitute the agreed versions everywhere `VERSION` and `CC_VERSION` appear):
````

**Edit 7.** `opencode/plugins/re-ghidra-mcp-opencode/INSTALL.md:11-14`

Find exactly:

````text
> and MCP server binary version independently — `0.7.3` / `0.8.0` are
> current as of this writing; check
> https://github.com/ckir/aiplugins/releases for what is newest) and use
> them for every URL below.
````

Replace with:

````text
> and MCP server binary version independently — the `VERSION=` and
> `CC_VERSION=` lines in Step 2 hold the releases this document was last
> refreshed for; check https://github.com/ckir/aiplugins/releases for what
> is newest) and use them for every URL below.
````

**Edit 8.** `opencode/plugins/re-ghidra-mcp-opencode/INSTALL.md:66-68`

Find exactly:

````text
binary below come from different packages on independent versions (shown
with the versions current as of this writing — substitute the agreed
versions everywhere `VERSION` and `CC_VERSION` appear):
````

Replace with:

````text
binary below come from different packages on independent versions (the
two lines below hold the releases this document was last refreshed for —
substitute the agreed versions everywhere `VERSION` and `CC_VERSION` appear):
````

**Edit 9.** `antigravity/plugins/re-ghidra-mcp-agy/README.md:100-101`

Find exactly:

````text
Pick the version you want (`0.7.2` is current as of this writing) and
fetch its `-agy-plugin.zip` — the doubled name is
````

Replace with:

````text
Pick the version you want (the URL below names the release this document
was last refreshed for) and fetch its `-agy-plugin.zip` — the doubled name is
````

**Edit 10.** `antigravity/plugins/rtk-mcp-agy/README.md:38-39`

Find exactly:

````text
   pick the version you want (`0.7.2` is current as of this writing) and
   fetch its `-agy-plugin.zip` — the doubled name is
````

Replace with:

````text
   pick the version you want (the URL below names the release this document
   was last refreshed for) and fetch its `-agy-plugin.zip` — the doubled name is
````


- [ ] **Step 2: Add the three missing OpenCode plugins to the root README**

Each new row's cells were read from that plugin's own install guide, not from memory:
`opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md:57-59` (the module is renamed on install, two skills are copied) and `Justfile:478`;
`opencode/plugins/repo-tasks-mcp-opencode/INSTALL.md:32` (the module is renamed), `:85` (the guide creates the command file, the zip does not carry it) and `Justfile:503`;
`opencode/plugins/repo-user-guide-opencode/INSTALL.md:64-65` (one skill folder, no binary).

**Edit 11.** `README.md:41`

Find exactly:

````text
| [`todov2-opencode`](opencode/plugins/todov2-opencode/) | whole folder → `.opencode/plugins/todov2-opencode/` (multi-file: `plugin.ts`, `tui.tsx`, `tool/`) | none (pure TypeScript, no binary) |
````

Replace with:

````text
| [`todov2-opencode`](opencode/plugins/todov2-opencode/) | whole folder → `.opencode/plugins/todov2-opencode/` (multi-file: `plugin.ts`, `tui.tsx`, `tool/`) | none (pure TypeScript, no binary) |
| [`opencode-ctl-mcp-opencode`](opencode/plugins/opencode-ctl-mcp-opencode/) | `plugin.ts` → `.opencode/plugins/opencode-ctl-mcp-opencode.ts`, `skills/*` → `.opencode/skills/` | `just build-opencode-ctl-mcp-opencode` |
| [`repo-tasks-mcp-opencode`](opencode/plugins/repo-tasks-mcp-opencode/) | `plugin.ts` → `.opencode/plugins/repo-tasks-mcp-opencode.ts`; the install guide also creates `.opencode/command/repo-tasks.md` | `just build-repo-tasks-mcp-opencode` |
| [`repo-user-guide-opencode`](opencode/plugins/repo-user-guide-opencode/) | `skills/repo-user-guide/` → `.opencode/skills/repo-user-guide/` | none (a skill, no binary) |
````

**Edit 12.** `README.md:55`

Find exactly:

````text
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/todov2-opencode/INSTALL.md`
````

Replace with:

````text
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/todov2-opencode/INSTALL.md`
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md`
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/repo-tasks-mcp-opencode/INSTALL.md`
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/repo-user-guide-opencode/INSTALL.md`
````


- [ ] **Step 3: Verify**

Run the Step 0 commands again. Expected: the loop prints nothing, and the two counts are:

```text
7
7
```

Then run the document gates:

```bash
just doc-paths && just spellcheck && just links && just opencode-wiring && just agy-wiring
```

Expected: each ends with its success line (`All documented paths exist.`, only the echoed command `typos` from the spelling check, `Errors   0` in the link summary, `All 41 opencode reference(s) resolve.`, `All 43 antigravity reference(s) resolve.`), exit 0.

- [ ] **Step 4: Run the repository gate**

Run: `just check`
Expected: exit 0. This was not measured when the plan was written (it builds the workspace). If it fails, read the failing recipe's output; a failure in a recipe that reads none of these nine files is not caused by this task, so report it and stop.

- [ ] **Step 5: Commit**

```bash
git add README.md opencode/plugins/aiplugins-devtools-opencode/INSTALL.md \
  opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md opencode/plugins/re-ghidra-mcp-opencode/INSTALL.md \
  opencode/plugins/repo-user-guide-opencode/INSTALL.md opencode/plugins/rtk-mcp-opencode/INSTALL.md \
  opencode/plugins/todov2-opencode/INSTALL.md antigravity/plugins/re-ghidra-mcp-agy/README.md \
  antigravity/plugins/rtk-mcp-agy/README.md
git status --short   # nothing else may be modified
git commit -m "docs: reword stale version claims and list every OpenCode plugin in the root README" \
  -m "The install documents said a version was 'current as of this writing', which went stale at the next release. They now point at the pinned line or URL. The root README's OpenCode table and URL list gain the three plugins they were missing." \
  -m "<the Co-Authored-By line from the attribution reminder>"
```

Then update the execution index (`project_install_pins_execution.md` in the session memory directory) with the commit SHA and the resume point "Task 2".

---

### Task 2: `scripts/check-install-pins.sh` (part of commit 2; do not commit)

**Files:**
- Create: `scripts/check-install-pins.sh`

**Interfaces:**
- Consumes: `registry_plugins <agent>` and `registry_manifest_path <agent> <plugin>` from `scripts/lib/registry.sh:18-26`, with `REGISTRY` pointed at `<root>/agents.json` (the variable that file reads at `:14`).
- Produces: the command line `check-install-pins.sh [--write | --selftest] [--root DIR]`. Exit 0 when every pin is current (or after a successful `--write`), 1 on a stale pin or a structural problem, 2 on a usage error. Tasks 4 and 6 call it by these exact forms: `bash scripts/check-install-pins.sh`, `bash scripts/check-install-pins.sh --write`, `bash scripts/check-install-pins.sh --selftest`.

What the script decides, so a reviewer can check it against spec 4.1 and 4.2:

- The version oracle is the manifest `agents.json` names for the plugin. `CC_VERSION` is the version of `<plugin minus -opencode>-cc`, required exactly when `opencode.jsonc` declares a local MCP server (the `jq` filter of `scripts/check-opencode-wiring.sh:98`); no `opencode.jsonc` means none is required.
- A stale pin is a document whose rewritten form differs from the file. A structural problem needs a hand edit. With any structural problem present, nothing is written.
- Beyond the failures spec 4.2 lists, the script also treats these as structural, under the spec's "it never skips silently": a registry plugin with no install document, an `opencode.jsonc` that `jq` cannot read, a missing `agents.json`, and a release URL for the plugin's own asset under a tag of any other shape (spec 4.1's "must equal the current version").
- A literal `<family>-cc-v<x.y.z>` token in an `INSTALL.md` is kept current whenever the sibling manifest exists.

- [ ] **Step 0: Confirm the starting state**

Run: `ls scripts/check-install-pins.sh 2>&1; git log --oneline -1`
Expected: `No such file or directory`, and the head commit is Task 1's. Otherwise report `STATE_MISMATCH`.

- [ ] **Step 1: Create the script**

The selftest lives in the same file, as in `scripts/bump-opencode-plugin.sh` and `scripts/check-gate-wiring.sh`, so the tests are already written: create the file with exactly the content of the block below and make it executable (the other gate scripts are mode `100755`). Do not retype it. This extracts the block from this plan and proves the copy is exact:

```bash
awk '/^### Task 2:/ { t = 1 } t && /^````bash$/ { on = 1; next } on && /^````$/ { exit } on' \
  docs/superpowers/plans/2026-11-21-install-pins.md > scripts/check-install-pins.sh
chmod +x scripts/check-install-pins.sh
sha256sum scripts/check-install-pins.sh
```

Expected: `4bca0aeae517bdc64e6198f5eaa693684fcdf0b6c6d6f979dc9d0a68b0ec3402  scripts/check-install-pins.sh` (on macOS use `shasum -a 256`). A different sum means the copy is not the script that was tested: stop and report it.

````bash
#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Keep the release pins in the install documents equal to the plugin manifests.
#
#   bash scripts/check-install-pins.sh [--root DIR]           check (the default)
#   bash scripts/check-install-pins.sh --write [--root DIR]   rewrite stale pins
#   bash scripts/check-install-pins.sh --selftest
#
# Why: users install from a document on `main` (an OpenCode plugin from its
# INSTALL.md, an Antigravity or Qwen plugin from its README.md). Those documents
# pin a release by hand and nothing moved the pin on release, so they went
# stale. .github/workflows/release-refresh.yml runs this with --write on the
# release branch, and CI runs the check.
#
# The version comes from the plugin's own manifest (agents.json names it).
# Two pin styles:
#   variable-line (opencode/plugins/<plugin>/INSTALL.md)
#     Exactly one line starting `VERSION=`. Exactly one line starting
#     `CC_VERSION=` if and only if the plugin's opencode.jsonc declares a local
#     MCP server; its value is the version of the Claude Code sibling
#     (<plugin minus -opencode>-cc), whose release carries the binary.
#   literal-tag (antigravity and qwen README.md, and INSTALL.md too)
#     Every `<plugin>-v<x.y.z>` token carries the current version. The legacy
#     form `releases/download/v<x.y.z>/<plugin>-...` becomes
#     `releases/download/<plugin>-v<version>/<plugin>-...`. A release URL for
#     this plugin's asset under any other tag is a structural problem.
# `$VERSION` and `$CC_VERSION` in a URL are not literals and are left alone.
#
# A stale pin is fixed by --write. A structural problem (a missing or duplicated
# line, a bad manifest version, the phrase `current as of this writing`) needs a
# hand edit, and --write writes nothing while one exists.
#
# POSIX awk and sed only: this runs on macOS (BSD awk) and Git Bash too.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
self=$here/$(basename "${BASH_SOURCE[0]}")

# --- the rewriter -----------------------------------------------------------
# Reads one document, prints it with every pin set to the current version, and
# writes to the file named by `report`:
#   ERR<TAB>message              a structural problem on a line
#   COUNT<TAB>pins<TAB>VERSION= lines<TAB>CC_VERSION= lines
# Variables: style (var|lit), plugin, ver, cc (sibling name), ccver ("" = none).
AWK_PINS=$(cat <<'AWK'
function verlen(s, pos,   rest) {
    rest = substr(s, pos)
    if (match(rest, /^[0-9]+\.[0-9]+\.[0-9]+/)) return RLENGTH
    return 0
}
# Every `prefix<x.y.z>` that starts a token becomes `prefix want`.
function retag(s, prefix, want,   out, i, n, pch, start) {
    out = ""
    while ((i = index(s, prefix)) > 0) {
        start = i + length(prefix)
        n = verlen(s, start)
        pch = (i > 1) ? substr(s, i - 1, 1) : substr(out, length(out), 1)
        if (n > 0 && pch !~ /[A-Za-z0-9_-]/) {
            out = out substr(s, 1, i - 1) prefix want
            s = substr(s, start + n)
            pins++
        } else {
            out = out substr(s, 1, start - 1)
            s = substr(s, start)
        }
    }
    return out s
}
# releases/download/v<x.y.z>/<plugin>-...  ->  releases/download/<plugin>-v<ver>/<plugin>-...
function relegacy(s,   out, i, n, p, tail, start) {
    p = "releases/download/v"
    tail = "/" plugin "-"
    out = ""
    while ((i = index(s, p)) > 0) {
        start = i + length(p)
        n = verlen(s, start)
        if (n > 0 && substr(s, start + n, length(tail)) == tail) {
            out = out substr(s, 1, i - 1) "releases/download/" plugin "-v" ver
            s = substr(s, start + n)
            pins++
        } else {
            out = out substr(s, 1, start - 1)
            s = substr(s, start)
        }
    }
    return out s
}
# A release URL for this plugin's asset (or its sibling's) must sit under the right tag.
function urlcheck(s,   p, i, j, tag, asset) {
    p = "releases/download/"
    while ((i = index(s, p)) > 0) {
        s = substr(s, i + length(p))
        j = index(s, "/")
        if (j == 0) break
        tag = substr(s, 1, j - 1)
        asset = substr(s, j + 1)
        if (index(asset, plugin "-") == 1 && tag != plugin "-v" ver && !(style == "var" && tag == plugin "-v$VERSION"))
            print "ERR\tline " NR ": a release URL for " plugin " is under the tag `" tag "`; write the tag as " plugin "-v<x.y.z>" > report
        if (style == "var" && ccver != "" && index(asset, cc "-") == 1 && tag != cc "-v" ccver && tag != cc "-v$CC_VERSION")
            print "ERR\tline " NR ": a release URL for " cc " is under the tag `" tag "`; write the tag as " cc "-v$CC_VERSION" > report
    }
}
BEGIN { pins = 0; vlines = 0; cclines = 0 }
{
    line = $0
    cr = ""
    if (sub(/\r$/, "", line)) cr = "\r"
    if (style == "var") {
        if (line ~ /^VERSION=/) {
            vlines++; pins++
            sub(/^VERSION=[^ \t]*/, "VERSION=" ver, line)
        } else if (line ~ /^CC_VERSION=/) {
            cclines++; pins++
            if (ccver != "") sub(/^CC_VERSION=[^ \t]*/, "CC_VERSION=" ccver, line)
        }
    }
    line = retag(line, plugin "-v", ver)
    if (style == "var" && ccver != "") line = retag(line, cc "-v", ccver)
    line = relegacy(line)
    urlcheck(line)
    printf "%s%s\n", line, cr
}
END { print "COUNT\t" pins "\t" vlines "\t" cclines > report }
AWK
)

# has_stale_claim FILE: the phrase `current as of this writing`, also when it
# wraps across lines or across the lines of a blockquote.
has_stale_claim() {
    awk '{ sub(/\r$/, ""); sub(/^ *>? ?/, ""); printf "%s ", $0 }' "$1" \
        | tr -s '[:space:]' ' ' \
        | grep -qF 'current as of this writing'
}

is_semver() { [[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; }

# manifest_version FILE: its .version, or nothing.
manifest_version() {
    [ -f "$1" ] || return 0
    jq -r '.version // empty' "$1" 2> /dev/null | tr -d '\r' || true
}

structural=()
stale=()
outs=()
total_pins=0
documents=0

problem() { structural+=("$1"); }

# scan ROOT REL STYLE PLUGIN VER CC CCVER NEED_CC: one document.
scan() {
    local root=$1 rel=$2 style=$3 plugin=$4 ver=$5 cc=$6 ccver=$7 need_cc=$8
    local doc=$root/$rel out rep kind a b c
    if [ ! -f "$doc" ]; then
        problem "$rel: the file does not exist; every registry plugin on this host needs it"
        return 0
    fi
    documents=$((documents + 1))
    out=$work/$documents.out
    rep=$work/$documents.rep
    : > "$rep"
    awk -v style="$style" -v plugin="$plugin" -v ver="$ver" -v cc="$cc" -v ccver="$ccver" \
        -v report="$rep" "$AWK_PINS" "$doc" > "$out"
    # awk ends its last line with a newline; a file that had none keeps none.
    if [ -s "$doc" ] && [ -n "$(tail -c 1 "$doc")" ]; then
        printf '%s' "$(cat "$out")" > "$out.n" && mv "$out.n" "$out"
    fi
    while IFS=$'\t' read -r kind a b c; do
        case $kind in
            ERR) problem "$rel: $a" ;;
            COUNT)
                total_pins=$((total_pins + a))
                if [ "$style" = var ]; then
                    if [ "$b" -eq 0 ]; then
                        problem "$rel: no line starts with VERSION=; add \`VERSION=$ver\` to the Step 2 code block"
                    elif [ "$b" -gt 1 ]; then
                        problem "$rel: $b lines start with VERSION=; keep one and rename the others"
                    fi
                    if [ "$need_cc" -eq 1 ] && [ "$c" -eq 0 ]; then
                        problem "$rel: opencode.jsonc declares a local MCP server but no line starts with CC_VERSION=; add \`CC_VERSION=$ccver\` under the VERSION= line"
                    elif [ "$need_cc" -eq 1 ] && [ "$c" -gt 1 ]; then
                        problem "$rel: $c lines start with CC_VERSION=; keep one and rename the others"
                    elif [ "$need_cc" -eq 0 ] && [ "$c" -gt 0 ]; then
                        problem "$rel: a line starts with CC_VERSION= but opencode.jsonc declares no local MCP server; delete the line"
                    fi
                fi ;;
        esac
    done < "$rep"
    if has_stale_claim "$doc"; then
        problem "$rel: says \`current as of this writing\`; reword it to point at the pinned line or URL instead of repeating a version"
    fi
    if ! cmp -s "$out" "$doc"; then
        stale+=("$rel")
        outs+=("$out")
    fi
}

# run ROOT MODE(check|write)
run() {
    local root=$1 mode=$2 p dir ver cc ccman ccver need_cc n manifest agent i
    REGISTRY=$root/agents.json
    if [ ! -f "$REGISTRY" ]; then
        echo "check-install-pins: $REGISTRY not found" >&2
        return 1
    fi
    # shellcheck source=scripts/lib/registry.sh
    source "$here/lib/registry.sh"
    REGISTRY=$root/agents.json
    work=$(mktemp -d)
    # shellcheck disable=SC2064  # expand now: the directory name is fixed here
    trap "rm -rf '$work'" EXIT

    for p in $(registry_plugins opencode); do
        dir=opencode/plugins/$p
        manifest=$(registry_manifest_path opencode "$p")
        ver=$(manifest_version "$root/$manifest")
        if ! is_semver "$ver"; then
            problem "$manifest: version '$ver' is missing or is not x.y.z"
            continue
        fi
        need_cc=0
        if [ -f "$root/$dir/opencode.jsonc" ]; then
            if ! n=$(jq '[.mcp.servers // {} | .[] | select(.type == "local")] | length' "$root/$dir/opencode.jsonc" 2> /dev/null | tr -d '\r') || [ -z "$n" ]; then
                problem "$dir/opencode.jsonc: not valid JSON, so the script cannot tell whether a binary is needed"
                continue
            fi
            [ "$n" -gt 0 ] && need_cc=1
        fi
        cc=${p%-opencode}-cc
        ccman=$(registry_manifest_path claude-code "$cc")
        ccver=""
        if [ -f "$root/$ccman" ]; then
            ccver=$(manifest_version "$root/$ccman")
            if ! is_semver "$ccver"; then
                problem "$ccman: version '$ccver' is missing or is not x.y.z"
                continue
            fi
        elif [ "$need_cc" -eq 1 ]; then
            problem "$dir/INSTALL.md: needs a CC_VERSION= pin but the sibling manifest $ccman does not exist"
            continue
        fi
        scan "$root" "$dir/INSTALL.md" var "$p" "$ver" "$cc" "$ccver" "$need_cc"
    done
    for agent in antigravity qwen; do
        for p in $(registry_plugins "$agent"); do
            manifest=$(registry_manifest_path "$agent" "$p")
            ver=$(manifest_version "$root/$manifest")
            if ! is_semver "$ver"; then
                problem "$manifest: version '$ver' is missing or is not x.y.z"
                continue
            fi
            scan "$root" "$(dirname "$manifest")/README.md" lit "$p" "$ver" "" "" 0
        done
    done

    if [ "$total_pins" -eq 0 ] && [ ${#structural[@]} -eq 0 ]; then
        problem "no pin found in any document under $root; the registry or the layout has gone stale"
    fi

    if [ ${#structural[@]} -gt 0 ]; then
        echo "check-install-pins: ${#structural[@]} structural problem(s); each needs a hand edit:" >&2
        for p in "${structural[@]}"; do echo "  FAIL     $p" >&2; done
        if [ ${#stale[@]} -gt 0 ]; then
            echo "${#stale[@]} document(s) also carry a stale pin. After the edits above, run: bash scripts/check-install-pins.sh --write" >&2
        fi
        [ "$mode" = write ] && echo "Nothing was written." >&2
        return 1
    fi
    if [ ${#stale[@]} -eq 0 ]; then
        echo "check-install-pins: $total_pins pin(s) in $documents document(s) are current."
        return 0
    fi
    if [ "$mode" = write ]; then
        for i in "${!stale[@]}"; do
            cat "${outs[$i]}" > "$root/${stale[$i]}"
            echo "  rewrote  ${stale[$i]}"
        done
        echo "check-install-pins: rewrote ${#stale[@]} of $documents document(s); $total_pins pin(s) are current."
        return 0
    fi
    echo "check-install-pins: ${#stale[@]} of $documents document(s) carry a stale pin ($total_pins pin(s) checked):" >&2
    for p in "${stale[@]}"; do echo "  stale    $p" >&2; done
    echo "Fix: bash scripts/check-install-pins.sh --write" >&2
    return 1
}

# --- self-test --------------------------------------------------------------
# Every case runs the real script against a throwaway tree. The documents are
# copies of the real shapes: a blockquoted header, a URL in a table cell, a
# backticked prose tag followed by a full stop, a `-v$CC_VERSION` prose token.
# shellcheck disable=SC2016  # the backticks and $VERSION are literal document text
selftest() {
    local n=0 out rc t
    # (global: the EXIT trap runs after this function's locals are gone)
    fx=$(mktemp -d)
    trap 'rm -rf "$fx"' EXIT

    check() { # check NAME RESULT(0 = pass) DETAIL
        n=$((n + 1))
        if [ "$2" -eq 0 ]; then echo "ok $1"; else echo "FAIL $1: $3" >&2; printf '%s\n' "$out" | sed 's/^/     /' >&2; exit 1; fi
    }
    # go ARGS...: run the script on $t, keep its output and exit code.
    go() {
        set +e
        out=$(bash "$self" "$@" --root "$t" 2>&1)
        rc=$?
        set -e
    }
    has() { printf '%s\n' "$out" | grep -qF -e "$1"; }
    same() { diff -r "$t" "$t.before" > /dev/null; }
    snap() { rm -rf "$t.before"; cp -R "$t" "$t.before"; }

    # tree OLD: a stale tree whose every pin reads OLD (manifests are ahead of it).
    #   a-opencode  local MCP server, sibling a-cc      s-opencode  skills-only, sibling s-cc, no opencode.jsonc
    #   t-opencode  opencode.jsonc, no local server    a-agy  URL and prose tag    n-agy  no URL
    #   a-qwen      legacy lockstep tag in a command line and in a table cell
    tree() {
        local d=$t old=$1
        rm -rf "$d"
        mkdir -p "$d/opencode/plugins/a-opencode" "$d/opencode/plugins/s-opencode" "$d/opencode/plugins/t-opencode" \
            "$d/claude-code/plugins/a-cc/.claude-plugin" "$d/claude-code/plugins/s-cc/.claude-plugin" \
            "$d/antigravity/plugins/a-agy" "$d/antigravity/plugins/n-agy" "$d/qwen/plugins/a-qwen"
        cat > "$d/agents.json" <<'JSON'
{"agents": [
  {"id": "claude-code", "plugins": ["a-cc", "s-cc"], "manifestPattern": "claude-code/plugins/{plugin}/.claude-plugin/plugin.json"},
  {"id": "opencode", "plugins": ["a-opencode", "s-opencode", "t-opencode"], "manifestPattern": "opencode/plugins/{plugin}/package.json"},
  {"id": "antigravity", "plugins": ["a-agy", "n-agy"], "manifestPattern": "antigravity/plugins/{plugin}/plugin.json"},
  {"id": "qwen", "plugins": ["a-qwen"], "manifestPattern": "qwen/plugins/{plugin}/qwen-extension.json"}
]}
JSON
        printf '{"name": "a-opencode", "version": "1.2.3"}\n' > "$d/opencode/plugins/a-opencode/package.json"
        printf '{"name": "s-opencode", "version": "2.0.0"}\n' > "$d/opencode/plugins/s-opencode/package.json"
        printf '{"name": "t-opencode", "version": "3.10.1"}\n' > "$d/opencode/plugins/t-opencode/package.json"
        printf '{"version": "4.5.6"}\n' > "$d/claude-code/plugins/a-cc/.claude-plugin/plugin.json"
        printf '{"version": "7.7.7"}\n' > "$d/claude-code/plugins/s-cc/.claude-plugin/plugin.json"
        printf '{"version": "5.0.0"}\n' > "$d/antigravity/plugins/a-agy/plugin.json"
        printf '{"version": "5.1.0"}\n' > "$d/antigravity/plugins/n-agy/plugin.json"
        printf '{"version": "6.0.0"}\n' > "$d/qwen/plugins/a-qwen/qwen-extension.json"
        printf '{"mcp": {"servers": {"a": {"type": "local", "command": ["./bin/a-cc-mcp"]}}}}\n' > "$d/opencode/plugins/a-opencode/opencode.jsonc"
        printf '{"mcp": {"servers": {"r": {"type": "remote", "url": "https://example.com"}}}}\n' > "$d/opencode/plugins/t-opencode/opencode.jsonc"
        cat > "$d/opencode/plugins/a-opencode/INSTALL.md" <<MD
> Everything below comes from versioned release assets. Agree the versions
> with the user first; Step 2 pins them.

## Step 2

\`\`\`bash
VERSION=$old
CC_VERSION=$old
curl -fsSL -o /tmp/a.zip \\
  "https://github.com/ckir/aiplugins/releases/download/a-opencode-v\$VERSION/a-opencode-opencode.zip"
\`\`\`

The binary ships in asset \`a-cc-plugin.zip\` on release \`a-cc-v\$CC_VERSION\` - reuse that zip:

     "https://github.com/ckir/aiplugins/releases/download/a-cc-v\$CC_VERSION/a-cc-plugin.zip"

Run \`bun add @opencode/plugin@2.0.32\` (a pin this script does not own).
Another plugin: https://github.com/ckir/aiplugins/releases/download/z-opencode-v0.0.1/z-opencode-opencode.zip
Not a release: https://github.com/ckir/aiplugins/tree/a-opencode-vNEXT/docs
A longer name that only ends like this one: pre-a-opencode-v0.0.1
MD
        cat > "$d/opencode/plugins/s-opencode/INSTALL.md" <<MD
Set the agreed version once:

\`\`\`bash
VERSION=$old
\`\`\`

on release \`s-opencode-v\$VERSION\`.
MD
        cat > "$d/opencode/plugins/t-opencode/INSTALL.md" <<MD
\`\`\`bash
VERSION=$old
\`\`\`
MD
        cat > "$d/antigravity/plugins/a-agy/README.md" <<MD
1. **Download the plugin bundle** for tag \`a-agy-v$old\`. The
   asset name is \`<package>-agy-plugin.zip\`:

   \`\`\`bash
   curl -fsSL -o /tmp/a-agy-agy-plugin.zip \\
     https://github.com/ckir/aiplugins/releases/download/a-agy-v$old/a-agy-agy-plugin.zip
   \`\`\`
MD
        printf '# n-agy\n\nBuild it from source; there is no release URL here.\n' > "$d/antigravity/plugins/n-agy/README.md"
        cat > "$d/qwen/plugins/a-qwen/README.md" <<MD
As of Qwen Code v0.22.3 the installer accepts a URL:

qwen extensions install https://github.com/ckir/aiplugins/releases/download/v$old/a-qwen-extension.zip

   | Platform | URL |
   |---|---|
   | All | https://github.com/ckir/aiplugins/releases/download/v$old/a-qwen-extension.zip |
MD
    }
    A=opencode/plugins/a-opencode/INSTALL.md

    # --- 1: the default mode fails on a stale tree, names --write, writes nothing
    t=$fx/t1; tree 0.1.0; snap
    go
    check stale-check-fails "$([ "$rc" -eq 1 ] && has "stale    $A" && has 'Fix: bash scripts/check-install-pins.sh --write' && echo 0 || echo 1)" "exit $rc"
    check check-writes-nothing "$(same && echo 0 || echo 1)" "the default mode changed a file"

    # --- 2: --write brings every pin to its manifest
    go --write
    check write-runs "$([ "$rc" -eq 0 ] && echo 0 || echo 1)" "exit $rc"
    ok=0
    grep -qx 'VERSION=1.2.3' "$t/$A" || ok=1
    grep -qx 'CC_VERSION=4.5.6' "$t/$A" || ok=1
    grep -qx 'VERSION=2.0.0' "$t/opencode/plugins/s-opencode/INSTALL.md" || ok=1
    grep -qx 'VERSION=3.10.1' "$t/opencode/plugins/t-opencode/INSTALL.md" || ok=1
    check variable-pins-moved "$ok" "a VERSION= or CC_VERSION= line is not at its manifest version"
    ok=0
    grep -qF 'for tag `a-agy-v5.0.0`. The' "$t/antigravity/plugins/a-agy/README.md" || ok=1
    grep -qF 'download/a-agy-v5.0.0/a-agy-agy-plugin.zip' "$t/antigravity/plugins/a-agy/README.md" || ok=1
    check literal-tag-moved "$ok" "the agy prose tag or URL is not at 5.0.0"
    ok=0
    [ "$(grep -c 'download/a-qwen-v6.0.0/a-qwen-extension.zip' "$t/qwen/plugins/a-qwen/README.md")" -eq 2 ] || ok=1
    grep -q 'download/v0' "$t/qwen/plugins/a-qwen/README.md" && ok=1
    grep -qF '| All | https://github.com/ckir/aiplugins/releases/download/a-qwen-v6.0.0/a-qwen-extension.zip |' "$t/qwen/plugins/a-qwen/README.md" || ok=1
    check legacy-tag-migrated "$ok" "a legacy v<x.y.z> URL was not migrated in both places"
    ok=0
    grep -qF 'download/a-opencode-v$VERSION/a-opencode-opencode.zip' "$t/$A" || ok=1
    grep -qF 'on release `a-cc-v$CC_VERSION` - reuse' "$t/$A" || ok=1
    grep -qF 'download/a-cc-v$CC_VERSION/a-cc-plugin.zip' "$t/$A" || ok=1
    grep -qF '@opencode/plugin@2.0.32' "$t/$A" || ok=1
    grep -qF 'download/z-opencode-v0.0.1/z-opencode-opencode.zip' "$t/$A" || ok=1
    grep -qF 'tree/a-opencode-vNEXT/docs' "$t/$A" || ok=1
    grep -qF 'pre-a-opencode-v0.0.1' "$t/$A" || ok=1
    grep -qF 'Qwen Code v0.22.3' "$t/qwen/plugins/a-qwen/README.md" || ok=1
    grep -q 'CC_VERSION=' "$t/opencode/plugins/s-opencode/INSTALL.md" && ok=1
    cmp -s "$t/antigravity/plugins/n-agy/README.md" "$t.before/antigravity/plugins/n-agy/README.md" || ok=1
    check decoys-untouched "$ok" "something that is not this plugin's pin was rewritten"

    # --- 3: a fresh tree passes the check, and a second --write changes nothing
    snap
    go
    check fresh-check-passes "$([ "$rc" -eq 0 ] && has '8 pin(s) in 6 document(s) are current' && echo 0 || echo 1)" "exit $rc"
    go --write
    check idempotent "$([ "$rc" -eq 0 ] && same && echo 0 || echo 1)" "a second --write changed files"

    # --- 4: a hardcoded stale URL beside a correct VERSION= line
    t=$fx/t4; tree 0.1.0; go --write
    printf 'Mirror: https://github.com/ckir/aiplugins/releases/download/a-opencode-v0.9.9/a-opencode-opencode.zip\n' >> "$t/$A"
    printf 'Binary mirror: https://github.com/ckir/aiplugins/releases/download/a-cc-v0.9.9/a-cc-plugin.zip\n' >> "$t/$A"
    go
    check literal-in-install-fails "$([ "$rc" -eq 1 ] && has "stale    $A" && echo 0 || echo 1)" "exit $rc"
    go --write
    check literal-in-install-rewritten "$(grep -qF 'download/a-opencode-v1.2.3/a-opencode-opencode.zip' "$t/$A" && echo 0 || echo 1)" "the literal URL was not rewritten"
    check literal-sibling-tag-rewritten "$(grep -qF 'download/a-cc-v4.5.6/a-cc-plugin.zip' "$t/$A" && echo 0 || echo 1)" "the literal sibling URL was not rewritten"

    # --- 5: structural problems. Each fails, names the file, and does not offer --write.
    # bad NAME FILE-NAMED: judge the last `go`.
    bad() {
        check "$1" "$([ "$rc" -eq 1 ] && has "$2" && ! has '--write' && echo 0 || echo 1)" "exit $rc"
    }
    fresh() { t=$fx/$1; tree 0.1.0; go --write; }

    fresh s1; sed '/^VERSION=/d' "$t/$A" > "$t/x" && mv "$t/x" "$t/$A"; go
    bad missing-version-line "$A: no line starts with VERSION="
    fresh s2; printf 'VERSION=1.2.3\n' >> "$t/$A"; go
    bad duplicate-version-line "$A: 2 lines start with VERSION="
    fresh s3; sed '/^CC_VERSION=/d' "$t/$A" > "$t/x" && mv "$t/x" "$t/$A"; go
    bad missing-cc-line "$A: opencode.jsonc declares a local MCP server but no line starts with CC_VERSION="
    fresh s4; printf 'CC_VERSION=7.7.7\n' >> "$t/opencode/plugins/s-opencode/INSTALL.md"; go
    bad cc-line-without-binary "opencode/plugins/s-opencode/INSTALL.md: a line starts with CC_VERSION= but"
    fresh s5; rm "$t/claude-code/plugins/a-cc/.claude-plugin/plugin.json"; go
    bad missing-sibling-manifest "$A: needs a CC_VERSION= pin but the sibling manifest claude-code/plugins/a-cc/.claude-plugin/plugin.json does not exist"
    fresh s6; printf '{"version": "1.2"}\n' > "$t/opencode/plugins/a-opencode/package.json"; go
    bad non-semver-version "opencode/plugins/a-opencode/package.json: version '1.2'"
    fresh s7; printf '{"version": "v6"}\n' > "$t/qwen/plugins/a-qwen/qwen-extension.json"; go
    bad non-semver-version-qwen "qwen/plugins/a-qwen/qwen-extension.json: version 'v6'"
    fresh s8; printf 'Pick a version (`5.0.0` is current as of this writing).\n' >> "$t/antigravity/plugins/a-agy/README.md"; go
    bad stale-claim-guard "antigravity/plugins/a-agy/README.md: says \`current as of this writing\`"
    # the shape of opencode/plugins/repo-user-guide-opencode/INSTALL.md lines 9-10 before this change
    fresh s9; printf '> unpinned URLs. Agree the version with the user first (`0.2.0` is current as\n> of this writing; check the releases page)\n' >> "$t/$A"; go
    bad stale-claim-guard-wrapped "$A: says \`current as of this writing\`"
    fresh s10; printf 'https://github.com/ckir/aiplugins/releases/download/nightly/a-agy-agy-plugin.zip\n' >> "$t/antigravity/plugins/a-agy/README.md"; go
    bad unknown-tag-url "antigravity/plugins/a-agy/README.md: line 8: a release URL for a-agy is under the tag \`nightly\`"
    fresh s11; rm "$t/opencode/plugins/t-opencode/INSTALL.md"; go
    bad missing-document "opencode/plugins/t-opencode/INSTALL.md: the file does not exist"
    fresh s12; printf '{ not json\n' > "$t/opencode/plugins/a-opencode/opencode.jsonc"; go
    bad invalid-jsonc "opencode/plugins/a-opencode/opencode.jsonc: not valid JSON"

    # --- 6: --write is all-or-nothing, and a mixed tree lists the hand edits first
    t=$fx/t6; tree 0.1.0
    printf 'VERSION=0.1.0\n' >> "$t/opencode/plugins/t-opencode/INSTALL.md"
    snap
    go --write
    check write-all-or-nothing "$([ "$rc" -eq 1 ] && same && has 'Nothing was written.' && echo 0 || echo 1)" "exit $rc, or a file changed"
    check mixed-names-write-after "$(has 'After the edits above, run: bash scripts/check-install-pins.sh --write' && echo 0 || echo 1)" "no pointer to --write after the hand edits"

    # --- 7: CRLF survives, and a file with no final newline keeps none
    t=$fx/t7; tree 0.1.0
    awk '{ printf "%s\r\n", $0 }' "$t/$A" > "$t/x" && mv "$t/x" "$t/$A"
    printf '%s' "$(cat "$t/opencode/plugins/t-opencode/INSTALL.md")" > "$t/x" && mv "$t/x" "$t/opencode/plugins/t-opencode/INSTALL.md"
    lines=$(wc -l < "$t/$A")
    go --write
    ok=0
    [ "$rc" -eq 0 ] || ok=1
    [ "$(grep -c $'\r$' "$t/$A")" -eq "$lines" ] || ok=1
    grep -q $'^VERSION=1.2.3\r$' "$t/$A" || ok=1
    check crlf-kept "$ok" "a CRLF line ending was lost, or the pin did not move"
    check no-final-newline-kept "$([ -n "$(tail -c 1 "$t/opencode/plugins/t-opencode/INSTALL.md")" ] && grep -q '^VERSION=3.10.1$' "$t/opencode/plugins/t-opencode/INSTALL.md" && echo 0 || echo 1)" "a final newline was added"

    # --- 8: a tree with no pin anywhere is refused (a wrong --root must not pass)
    t=$fx/t8; mkdir -p "$t"
    printf '{"agents": [{"id": "claude-code", "plugins": [], "manifestPattern": "x/{plugin}"}, {"id": "opencode", "plugins": [], "manifestPattern": "x/{plugin}"}, {"id": "antigravity", "plugins": [], "manifestPattern": "x/{plugin}"}, {"id": "qwen", "plugins": [], "manifestPattern": "x/{plugin}"}]}\n' > "$t/agents.json"
    go
    check zero-census-refused "$([ "$rc" -eq 1 ] && has 'no pin found in any document' && echo 0 || echo 1)" "exit $rc"
    t=$fx/t9; mkdir -p "$t"
    go
    check missing-registry-refused "$([ "$rc" -eq 1 ] && has 'agents.json not found' && echo 0 || echo 1)" "exit $rc"

    echo "check-install-pins selftest: $n cases passed"
}

# --- main -------------------------------------------------------------------
mode="check"
root=$(cd "$here/.." && pwd -P)
while [ $# -gt 0 ]; do
    case $1 in
        --selftest) selftest; exit 0 ;;
        --write) mode="write"; shift ;;
        --root) root=${2:?--root needs a directory}; shift 2 ;;
        *) echo "usage: check-install-pins.sh [--write | --selftest] [--root DIR]" >&2; exit 2 ;;
    esac
done
run "$root" "$mode"
````

- [ ] **Step 2: Run the selftest**

Run: `bash scripts/check-install-pins.sh --selftest`
Expected: 30 lines starting `ok `, then `check-install-pins selftest: 30 cases passed`, exit 0.

- [ ] **Step 3: Run it against the real tree**

Run: `bash scripts/check-install-pins.sh; echo "exit $?"`
Expected: the first line is `check-install-pins: 14 of 18 document(s) carry a stale pin (22 pin(s) checked):`, then 14 lines starting `  stale    `, then `Fix: bash scripts/check-install-pins.sh --write`, then `exit 1`. No line starts with `  FAIL`. Do not run `--write` yet; Task 6 does.

If a `FAIL` line appears, Task 1 left a `current as of this writing` behind or the tree differs from the one the plan was written against: report it.

- [ ] **Step 4: Mutation run**

The spec asks for one line-level mutant per guard, each turning a named case red. Save the block below as `$SCRATCH/mutants.sh` and run `bash "$SCRATCH/mutants.sh"` from the repository root. It covers this script and the one from Task 3, so run it after Task 3 Step 2. It runs 25 selftests and takes about three and a half minutes: run it in the background, or with a command timeout of at least 300 seconds. A run cut off by a shorter timeout leaves `scripts/mut.sh` behind; delete it and run again.

````bash
# Run from the repository root. Each line applies one edit to a copy of a gate
# script and runs that copy's selftest; the selftest must go red.
m() {
    src=$1; name=$2; shift 2
    sed "$@" "$src" > scripts/mut.sh
    if cmp -s scripts/mut.sh "$src"; then
        echo "$name: MUTANT DID NOT APPLY"
    else
        r=$(bash scripts/mut.sh --selftest 2>&1 | grep -m1 '^FAIL' || echo SURVIVED)
        echo "$name: $r"
    fi
    rm -f scripts/mut.sh
}
P=scripts/check-install-pins.sh
R=scripts/check-readme-coverage.sh
m $P duplicate-check   -e 's/elif \[ "\$b" -gt 1 \]; then/elif [ "$b" -gt 99 ]; then/'
m $P sibling-check     -e 's/^        elif \[ "\$need_cc" -eq 1 \]; then$/        elif false; then/'
m $P anchor            -e 's|if (line ~ /^VERSION=/) {|if (line ~ /VERSION=/) {|'
m $P legacy-migration  -e '/^    line = relegacy(line)$/d'
m $P crlf              -e '/if (sub(\/\\r\$\/, "", line)) cr = "\\r"/d'
m $P default-writes    -e 's/^    if \[ "\$mode" = write \]; then$/    if true; then/'
m $P guard             -e 's/^    if has_stale_claim "\$doc"; then$/    if false; then/'
m $P guard-join        -e 's|sub(/^ \*>? ?/, "");||'
m $P cc-required       -e 's/if \[ "\$need_cc" -eq 1 \] && \[ "\$c" -eq 0 \]; then/if false; then/'
m $P cc-forbidden      -e 's/elif \[ "\$need_cc" -eq 0 \] && \[ "\$c" -gt 0 \]; then/elif false; then/'
m $P token-boundary    -e 's|n > 0 \&\& pch !~ /\[A-Za-z0-9_-\]/|n > 0|'
m $P sibling-retag     -e '/line = retag(line, cc "-v", ccver)/d'
m $P url-tag-check     -e '/^    urlcheck(line)$/d'
m $P all-or-nothing    -e '/Nothing was written/{n;d;}'
m $P zero-census       -e 's/if \[ "\$total_pins" -eq 0 \] \&\& /if false \&\& /'
m $P final-newline     -e 's/if \[ -s "\$doc" \] \&\& \[ -n "\$(tail -c 1 "\$doc")" \]; then/if false; then/'
m $R section-start     -e 's/^    if (!inside) next$//'
m $R section-end       -e '/if (inside \&\& index(line, "## ") == 1)/d'
m $R fence             -e 's/^    if (infence) next$//'
m $R row-anchor        -e 's/if (index(line, "|") == 1) names/if (1) names/'
m $R bullet-anchor     -e 's/if (index(line, "- ") == 1) names/if (1) names/'
m $R install-file      -e 's|if \[ ! -f "\$root/opencode/plugins/\$p/INSTALL.md" \]; then|if false; then|'
m $R reverse-check     -e 's/case \$kind in ROW | BULLET) ;; \*) continue ;; esac/continue/'
m $R empty-registry    -e 's/if \[ "\$n" -eq 0 \]; then/if false; then/'
m $R crlf              -e '/sub(\/\\r\$\/, "", line)/d'
````

Expected, exactly (no `SURVIVED`, no `MUTANT DID NOT APPLY`, and `git status --short` shows no `scripts/mut.sh` afterwards):

````text
duplicate-check: FAIL duplicate-version-line: exit 0
sibling-check: FAIL missing-sibling-manifest: exit 0
anchor: FAIL stale-check-fails: exit 1
legacy-migration: FAIL stale-check-fails: exit 1
crlf: FAIL crlf-kept: a CRLF line ending was lost, or the pin did not move
default-writes: FAIL stale-check-fails: exit 0
guard: FAIL stale-claim-guard: exit 0
guard-join: FAIL stale-claim-guard-wrapped: exit 0
cc-required: FAIL missing-cc-line: exit 0
cc-forbidden: FAIL cc-line-without-binary: exit 0
token-boundary: FAIL decoys-untouched: something that is not this plugin's pin was rewritten
sibling-retag: FAIL literal-in-install-fails: exit 1
url-tag-check: FAIL unknown-tag-url: exit 0
all-or-nothing: FAIL missing-version-line: exit 0
zero-census: FAIL zero-census-refused: exit 0
final-newline: FAIL no-final-newline-kept: a final newline was added
section-start: FAIL missing-row: exit 0
section-end: FAIL missing-row: exit 0
fence: FAIL fenced-lines-ignored: exit 0
row-anchor: FAIL link-outside-a-table-row: exit 0
bullet-anchor: FAIL bullet-not-at-line-start: exit 0
install-file: FAIL missing-install-file: exit 0
reverse-check: FAIL unregistered-row: exit 1
empty-registry: FAIL empty-registry: exit 1
crlf: FAIL crlf-readme-passes: exit 1
````

A mutant such as `legacy-migration` goes red at `stale-check-fails` because the unmigrated URL is then a structural problem and the stale listing never prints; that is the right case to go red.

---

### Task 3: `scripts/check-readme-coverage.sh` (part of commit 2; do not commit)

**Files:**
- Create: `scripts/check-readme-coverage.sh`

**Interfaces:**
- Consumes: `registry_plugins opencode` from `scripts/lib/registry.sh:18-20`, with `REGISTRY` pointed at `<root>/agents.json`.
- Produces: the command line `check-readme-coverage.sh [--root DIR] | --selftest`. Exit 0 when every OpenCode plugin has a row, a bullet and an `INSTALL.md`; 1 otherwise; 2 on a usage error. Task 4 calls `bash scripts/check-readme-coverage.sh` and `bash scripts/check-readme-coverage.sh --selftest`.

- [ ] **Step 0: Confirm the starting state**

Run: `ls scripts/check-readme-coverage.sh 2>&1; grep -n '^## ' README.md | sed -n '2,3p'`
Expected: `No such file or directory`, then `15:## Installing` and `63:## What each plugin costs a context window`: the section the script reads, and the heading that ends it.

- [ ] **Step 1: Create the script**

Create the file with exactly the content of the block below. Do not retype it:

```bash
awk '/^### Task 3:/ { t = 1 } t && /^````bash$/ { on = 1; next } on && /^````$/ { exit } on' \
  docs/superpowers/plans/2026-11-21-install-pins.md > scripts/check-readme-coverage.sh
chmod +x scripts/check-readme-coverage.sh
sha256sum scripts/check-readme-coverage.sh
```

Expected: `c035f6a6e29eda3d421f360397aba0cc5bfff279ff41eed8e7e458019aa32c48  scripts/check-readme-coverage.sh`. A different sum: stop and report it.

````bash
#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Check that the root README's install section covers every OpenCode plugin.
#
#   bash scripts/check-readme-coverage.sh [--root DIR]
#   bash scripts/check-readme-coverage.sh --selftest
#
# Why: the OpenCode table and the "ask an agent" URL list in README.md are
# written by hand, and three of seven plugins were missing from both with
# every check green.
#
# It reads only the `## Installing` section of ROOT/README.md: from that heading
# to the next line that starts with `## `. The README links every plugin again
# in its repository-structure list, so a link elsewhere in the file proves
# nothing. Lines inside a fenced code block are ignored. Inside the section,
# for every OpenCode plugin in ROOT/agents.json it requires:
#   a ROW     a line that starts with `|` and contains `](opencode/plugins/<plugin>/)`
#   a BULLET  a line that starts with `- ` and contains the raw INSTALL.md URL
#             https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/<plugin>/INSTALL.md
#   the file  opencode/plugins/<plugin>/INSTALL.md
# and it fails on a row or a bullet that names a plugin the registry does not have.
# Matching is by substring, so backticks and link text never matter. It proves
# coverage, not that the words in a row are true.
#
# POSIX awk only: this runs on macOS (BSD awk) and Git Bash too.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
self=$here/$(basename "${BASH_SOURCE[0]}")

RAW=https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/

# Emits, for the `## Installing` section outside fenced blocks:
#   SECTION                  the heading was found
#   ROW<TAB>plugin           once per `](opencode/plugins/<plugin>/)` on a line starting with `|`
#   BULLET<TAB>plugin        once per raw INSTALL.md URL on a line starting with `- `
AWK_SECTION=$(cat <<'AWK'
# names LINE LEAD STOP KIND: print KIND and the text between LEAD and STOP, for every LEAD.
function names(s, lead, stop, kind,   i, j, name) {
    while ((i = index(s, lead)) > 0) {
        s = substr(s, i + length(lead))
        j = index(s, stop)
        if (j == 0) return
        name = substr(s, 1, j - 1)
        if (name != "" && name !~ /[^A-Za-z0-9_.-]/) print kind "\t" name
        s = substr(s, j)
    }
}
BEGIN { bt = sprintf("%c", 96); fence3 = bt bt bt; inside = 0; infence = 0 }
{
    line = $0
    sub(/\r$/, "", line)
    probe = line
    sub(/^ */, "", probe)
    if (index(probe, fence3) == 1 || index(probe, "~~~") == 1) { infence = !infence; next }
    if (infence) next
    if (line == "## Installing") { inside = 1; print "SECTION"; next }
    if (inside && index(line, "## ") == 1) { inside = 0; next }
    if (!inside) next
    if (index(line, "|") == 1) names(line, "](opencode/plugins/", "/)", "ROW")
    if (index(line, "- ") == 1) names(line, raw, "/INSTALL.md", "BULLET")
}
AWK
)

TAB=$'\t'
failures=0
fail() { echo "  FAIL     $1" >&2; failures=$((failures + 1)); }

# run ROOT
run() {
    local root=$1 found p plugins n=0
    if [ ! -f "$root/README.md" ]; then
        fail "README.md: not found under $root"
        return 0
    fi
    REGISTRY=$root/agents.json
    if [ ! -f "$REGISTRY" ]; then
        fail "agents.json: not found under $root"
        return 0
    fi
    # shellcheck source=scripts/lib/registry.sh
    source "$here/lib/registry.sh"
    REGISTRY=$root/agents.json
    found=$(awk -v raw="$RAW" "$AWK_SECTION" "$root/README.md")
    if ! printf '%s\n' "$found" | grep -qx 'SECTION'; then
        fail "README.md: no line reads \`## Installing\`, so there is no section to check"
        return 0
    fi
    plugins=$(registry_plugins opencode)
    for p in $plugins; do
        n=$((n + 1))
        if ! printf '%s\n' "$found" | grep -qxF "ROW$TAB$p"; then
            fail "README.md: the OpenCode table in \`## Installing\` has no row linking opencode/plugins/$p/"
        fi
        if ! printf '%s\n' "$found" | grep -qxF "BULLET$TAB$p"; then
            fail "README.md: the URL list in \`## Installing\` has no bullet with ${RAW}$p/INSTALL.md"
        fi
        if [ ! -f "$root/opencode/plugins/$p/INSTALL.md" ]; then
            fail "opencode/plugins/$p/INSTALL.md: the file does not exist"
        fi
    done
    if [ "$n" -eq 0 ]; then
        fail "agents.json lists no OpenCode plugin; the registry or the layout has gone stale"
    fi
    while IFS=$'\t' read -r kind p; do
        case $kind in ROW | BULLET) ;; *) continue ;; esac
        if ! printf '%s\n' "$plugins" | grep -qxF "$p"; then
            if [ "$kind" = ROW ]; then
                fail "README.md: a table row in \`## Installing\` links opencode/plugins/$p/, which agents.json does not list"
            else
                fail "README.md: a bullet in \`## Installing\` names opencode/plugins/$p/INSTALL.md, which agents.json does not list"
            fi
        fi
    done <<< "$found"
    if [ "$failures" -eq 0 ]; then
        echo "check-readme-coverage: $n OpenCode plugin(s) each have a row, a bullet and an INSTALL.md."
    fi
}

# --- self-test --------------------------------------------------------------
# Every case is a seeded defect in a throwaway tree whose README copies the
# real shapes: backticks inside the link text, the URL in backticks, a fenced
# block inside the section, and a structure list that links every plugin again.
# shellcheck disable=SC2016  # the backticks are literal Markdown
selftest() {
    local n=0 out rc t
    # (global: the EXIT trap runs after this function's locals are gone)
    fx=$(mktemp -d)
    trap 'rm -rf "$fx"' EXIT

    check() { # check NAME RESULT(0 = pass) DETAIL
        n=$((n + 1))
        if [ "$2" -eq 0 ]; then echo "ok $1"; else echo "FAIL $1: $3" >&2; printf '%s\n' "$out" | sed 's/^/     /' >&2; exit 1; fi
    }
    go() {
        set +e
        out=$(bash "$self" --root "$t" 2>&1)
        rc=$?
        set -e
    }
    has() { printf '%s\n' "$out" | grep -qF -e "$1"; }
    # edit SED-ARGS...: rewrite the fixture README.
    edit() { sed "$@" "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; }

    # tree: two registered plugins, both fully covered.
    tree() {
        rm -rf "$t"
        mkdir -p "$t/opencode/plugins/a-opencode" "$t/opencode/plugins/b-opencode"
        : > "$t/opencode/plugins/a-opencode/INSTALL.md"
        : > "$t/opencode/plugins/b-opencode/INSTALL.md"
        printf '{"agents": [{"id": "opencode", "plugins": ["a-opencode", "b-opencode"], "manifestPattern": "opencode/plugins/{plugin}/package.json"}]}\n' > "$t/agents.json"
        cat > "$t/README.md" <<'MD'
# Fixture

## Installing

```bash
claude plugin marketplace add ckir/aiplugins
```

| Plugin | Copies | Binary staging |
|---|---|---|
| [`a-opencode`](opencode/plugins/a-opencode/) | `plugin.ts` → `.opencode/plugins/` | `just build-a-opencode` |
| [`b-opencode`](opencode/plugins/b-opencode/) | `skills/*` → `.opencode/skills/` | none |

Prefer to have an agent do it?

- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/a-opencode/INSTALL.md`
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/b-opencode/INSTALL.md`

## Repository structure

- `opencode/plugins/`
  - [`a-opencode`](opencode/plugins/a-opencode/) — one.
  - [`b-opencode`](opencode/plugins/b-opencode/) — two.

| Plugin | Note |
|---|---|
| [`a-opencode`](opencode/plugins/a-opencode/) | a row outside the section |
| [`b-opencode`](opencode/plugins/b-opencode/) | a row outside the section |

- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/a-opencode/INSTALL.md`
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/b-opencode/INSTALL.md`
MD
    }
    bad() { check "$1" "$([ "$rc" -eq 1 ] && has "$2" && echo 0 || echo 1)" "exit $rc"; }

    t=$fx/t; tree; go
    check covered-tree-passes "$([ "$rc" -eq 0 ] && has '2 OpenCode plugin(s) each have a row' && echo 0 || echo 1)" "exit $rc"

    tree; edit -e '1,/^## Repository structure$/{/^| \[`b-opencode`\]/d;}'; go
    bad missing-row 'no row linking opencode/plugins/b-opencode/'
    check missing-row-only "$(has 'no bullet' && echo 1 || echo 0)" "a bullet that is present was reported"

    tree; edit -e '1,/^## Repository structure$/{/^- .*b-opencode\/INSTALL.md/d;}'; go
    bad missing-bullet 'no bullet with https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/b-opencode/INSTALL.md'

    tree; edit -e '1,/^## Repository structure$/s|^\(- .*plugins/b-opencode/\)INSTALL.md|\1README.md|'; go
    bad bullet-wrong-file 'no bullet with'

    tree; edit -e '1,/^## Repository structure$/s|^\(- .*\)refs/heads/main\(/opencode/plugins/b-opencode/\)|\1main\2|'; go
    bad bullet-wrong-path 'no bullet with'

    tree; edit -e '1,/^## Repository structure$/s|^- \(.*b-opencode/INSTALL.md\)|  - \1|'; go
    bad bullet-not-at-line-start 'no bullet with https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/b-opencode/INSTALL.md'

    tree
    edit -e '1,/^## Repository structure$/s#^| \[`b-opencode`\].*#| [`z-opencode`](opencode/plugins/z-opencode/) | x | y |#'; go
    bad unregistered-row 'links opencode/plugins/z-opencode/, which agents.json does not list'

    tree
    edit -e '1,/^## Repository structure$/s|^\(- .*plugins/\)b-opencode\(/INSTALL.md.*\)|\1z-opencode\2|'; go
    bad unregistered-bullet 'names opencode/plugins/z-opencode/INSTALL.md, which agents.json does not list'

    tree; rm "$t/opencode/plugins/b-opencode/INSTALL.md"; go
    bad missing-install-file 'opencode/plugins/b-opencode/INSTALL.md: the file does not exist'

    # The case that proves the section scoping: b keeps its row and its bullet
    # in the section after `## Installing`, and loses them only inside it.
    tree
    awk '/^## Repository structure$/ { past = 1 }
         !past && (/^\| \[`b-opencode`\]/ || /^- .*b-opencode\/INSTALL.md/) { next }
         { print }' "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; go
    check linked-only-outside-installing "$([ "$rc" -eq 1 ] && has 'no row linking opencode/plugins/b-opencode/' && has 'no bullet with' && [ "$(grep -c '^| \[`b-opencode`\]' "$t/README.md")" -eq 1 ] && [ "$(grep -c '^- .*b-opencode/INSTALL.md' "$t/README.md")" -eq 1 ] && echo 0 || echo 1)" "exit $rc"

    # A link in a sentence is not a table row.
    tree
    awk '/^## Repository structure$/ { past = 1 }
         !past && /^\| \[`b-opencode`\]/ { print "See [`b-opencode`](opencode/plugins/b-opencode/) for the rest."; next }
         { print }' "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; go
    bad link-outside-a-table-row 'no row linking opencode/plugins/b-opencode/'

    # A row and a bullet inside a fenced block do not count.
    tree
    awk 'BEGIN { f = sprintf("%c%c%c", 96, 96, 96) }
         /^## Repository structure$/ { past = 1 }
         !past && (/^\| \[`b-opencode`\]/ || /^- .*b-opencode\/INSTALL.md/) { print f; print; print f; next }
         { print }' "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; go
    check fenced-lines-ignored "$([ "$rc" -eq 1 ] && has 'no row linking opencode/plugins/b-opencode/' && has 'no bullet with' && echo 0 || echo 1)" "exit $rc"

    tree; edit -e 's/^## Installing$/## Install/'; go
    bad no-installing-section 'no line reads `## Installing`'

    tree; printf '{"agents": [{"id": "opencode", "plugins": [], "manifestPattern": "x/{plugin}"}]}\n' > "$t/agents.json"; go
    bad empty-registry 'agents.json lists no OpenCode plugin'

    tree; awk '{ printf "%s\r\n", $0 }' "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; go
    check crlf-readme-passes "$([ "$rc" -eq 0 ] && echo 0 || echo 1)" "exit $rc"

    echo "check-readme-coverage selftest: $n cases passed"
}

# --- main -------------------------------------------------------------------
root=$(cd "$here/.." && pwd -P)
while [ $# -gt 0 ]; do
    case $1 in
        --selftest) selftest; exit 0 ;;
        --root) root=${2:?--root needs a directory}; shift 2 ;;
        *) echo "usage: check-readme-coverage.sh [--root DIR] | --selftest" >&2; exit 2 ;;
    esac
done
run "$root"
[ "$failures" -eq 0 ]
````

- [ ] **Step 2: Run the selftest**

Run: `bash scripts/check-readme-coverage.sh --selftest`
Expected: 16 lines starting `ok `, then `check-readme-coverage selftest: 16 cases passed`, exit 0.

- [ ] **Step 3: Run it against the real tree**

Run: `bash scripts/check-readme-coverage.sh; echo "exit $?"`
Expected: `check-readme-coverage: 7 OpenCode plugin(s) each have a row, a bullet and an INSTALL.md.` and `exit 0`.

To see it catch the defect it exists for, run it on the README as it was before Task 1, in a throwaway root:

```bash
mkdir -p "$SCRATCH/cov/opencode/plugins" && cp agents.json "$SCRATCH/cov/"
for p in opencode/plugins/*/; do mkdir -p "$SCRATCH/cov/$p" && cp "$p/INSTALL.md" "$SCRATCH/cov/$p"; done
git show HEAD~1:README.md > "$SCRATCH/cov/README.md"
bash scripts/check-readme-coverage.sh --root "$SCRATCH/cov"; echo "exit $?"
```

Expected: six `FAIL` lines (a missing row and a missing bullet for each of `repo-user-guide-opencode`, `opencode-ctl-mcp-opencode` and `repo-tasks-mcp-opencode`), then `exit 1`.

- [ ] **Step 4: Mutation run**

Run the mutant file from Task 2 Step 4 now, if it has not run yet. Its last nine lines cover this script.

---

### Task 4: Wiring (part of commit 2; do not commit)

**Files:**
- Modify: `Justfile`, `.github/workflows/ci.yml`, `.github/workflows/release-refresh.yml`

**Interfaces:**
- Consumes: the command lines Tasks 2 and 3 produce.
- Produces: the recipes `install-pins`, `install-pins-test`, `readme-coverage`, `readme-coverage-test`, all reached by `just check`; the CI step names "Check install-document pins are current" and "Check the root README covers every OpenCode plugin", which Task 5's text quotes; and the workflow step id `pins`.

- [ ] **Step 0: Confirm the starting state**

Run: `bash scripts/check-gate-wiring.sh; echo "exit $?"`
Expected: exit 1, with these four lines among the output (the two new scripts exist and nothing runs them yet):

```text
  FAIL     check-install-pins.sh: not run by any Justfile recipe
  FAIL     check-install-pins.sh: not run in CI (no workflow names scripts/check-install-pins.sh or runs a `just` recipe that reaches it)
  FAIL     check-readme-coverage.sh: not run by any Justfile recipe
  FAIL     check-readme-coverage.sh: not run in CI (no workflow names scripts/check-readme-coverage.sh or runs a `just` recipe that reaches it)
```

- [ ] **Step 1: `Justfile`**

The line numbers in every edit of this plan are those of the file before any edit to it.

**Edit 1.** `Justfile:110-112`

Find exactly:

````text
# Self-test of that check: seeded wiring defects in a throwaway tree.
gate-wiring-test:
    bash scripts/check-gate-wiring.sh --selftest
````

Replace with:

````text
# Self-test of that check: seeded wiring defects in a throwaway tree.
gate-wiring-test:
    bash scripts/check-gate-wiring.sh --selftest

# Check that the release pins in the install documents (OpenCode INSTALL.md, Antigravity and Qwen
# README.md) equal the plugin manifests. `bash scripts/check-install-pins.sh --write` fixes a stale
# pin; the release branch runs that itself (.github/workflows/release-refresh.yml).
install-pins:
    bash scripts/check-install-pins.sh

# Self-test of that check: seeded defects in throwaway trees.
install-pins-test:
    bash scripts/check-install-pins.sh --selftest

# Check that the root README's install section has a row and a URL for every OpenCode plugin.
readme-coverage:
    bash scripts/check-readme-coverage.sh

# Self-test of that check: seeded defects in throwaway trees.
readme-coverage-test:
    bash scripts/check-readme-coverage.sh --selftest
````

**Edit 2.** `Justfile:378`

Find exactly:

````text
doc-paths-test doc-paths gate-wiring-test gate-wiring agy-wiring
````

Replace with:

````text
doc-paths-test doc-paths gate-wiring-test gate-wiring install-pins-test install-pins readme-coverage-test readme-coverage agy-wiring
````


- [ ] **Step 2: `.github/workflows/ci.yml`**

The steps go in the `wiring` job, which runs on `ubuntu-latest` only, like the other script gates.

**Edit 3.** `.github/workflows/ci.yml:147-148`

Find exactly:

````text
      - name: Check generated marketplaces are current
        run: bash scripts/gen-marketplaces.sh --check
````

Replace with:

````text
      - name: Check generated marketplaces are current
        run: bash scripts/gen-marketplaces.sh --check

      # The install documents (each OpenCode INSTALL.md, the Antigravity and
      # Qwen READMEs) pin a release, and release-refresh.yml rewrites the pin
      # on the release branch. A pin that does not equal its manifest fails
      # here, and the self-test proves the check rejects each seeded defect.
      - name: Self-test the install-pins check
        run: bash scripts/check-install-pins.sh --selftest

      - name: Check install-document pins are current
        run: bash scripts/check-install-pins.sh

      # The root README's OpenCode table and URL list are written by hand;
      # three of seven plugins were once missing from both with CI green.
      - name: Self-test the README coverage check
        run: bash scripts/check-readme-coverage.sh --selftest

      - name: Check the root README covers every OpenCode plugin
        run: bash scripts/check-readme-coverage.sh
````


- [ ] **Step 3: `.github/workflows/release-refresh.yml`**

Four edits: the header comment, the new step, the `git add` list, and the last step. The pins step is `continue-on-error` so that a document the script cannot rewrite does not stop the marketplace refresh from committing; the script writes nothing in that case, so the commit never carries a half-rewritten tree. The last step's condition must read `steps.pins.outcome`, not `steps.pins.conclusion`: under `continue-on-error` the conclusion of a failed step is `success`.

**Edit 4.** `.github/workflows/release-refresh.yml:6-12`

Find exactly:

````text
# regenerate the tag-pinned source URLs in the generated marketplace
# manifests. Without a refresh, every release PR fails
# `gen-marketplaces.sh --check` on its first CI run - red through no fault of
# the release content. Footprint documents no longer carry a version stamp,
# so they need no refresh.
#
# This job runs the generator against the release branch and commits the
````

Replace with:

````text
# regenerate the tag-pinned source URLs in the generated marketplace
# manifests, nor the release pins in the install documents (each OpenCode
# INSTALL.md, the Antigravity and Qwen READMEs). Without a refresh, every
# release PR fails `gen-marketplaces.sh --check` and `check-install-pins.sh`
# on its first CI run - red through no fault of the release content.
# Footprint documents no longer carry a version stamp, so they need no refresh.
#
# This job runs both against the release branch and commits the
````

**Edit 5.** `.github/workflows/release-refresh.yml:54-56`

Find exactly:

````text
      - name: Regenerate marketplace manifests
        run: bash scripts/gen-marketplaces.sh
````

Replace with:

````text
      - name: Regenerate marketplace manifests
        run: bash scripts/gen-marketplaces.sh

      # continue-on-error: a document this cannot rewrite (it writes nothing
      # then) must not stop the marketplace refresh below from committing.
      # The last step of the job turns the run red and names this step.
      - name: Refresh install-document pins
        id: pins
        continue-on-error: true
        run: bash scripts/check-install-pins.sh --write
````

**Edit 6.** `.github/workflows/release-refresh.yml:64-72`

Find exactly:

````text
          # Only the generated marketplace manifests. A release branch never
          # carries hand-written changes, but an unrestricted `git add -A`
          # would commit them silently if it ever did - and a refresh commit
          # must never smuggle content into a release.
          git add \
            .claude-plugin/marketplace.json \
            .qwen-plugin/marketplace.json \
            .agy-plugin/marketplace.json \
            .opencode-plugin/marketplace.json
````

Replace with:

````text
          # Only the generated marketplace manifests and the install
          # documents whose pins the step above rewrites. A release branch never
          # carries hand-written changes, but an unrestricted `git add -A`
          # would commit them silently if it ever did - and a refresh commit
          # must never smuggle content into a release.
          git add \
            .claude-plugin/marketplace.json \
            .qwen-plugin/marketplace.json \
            .agy-plugin/marketplace.json \
            .opencode-plugin/marketplace.json \
            opencode/plugins/*/INSTALL.md \
            antigravity/plugins/*/README.md \
            qwen/plugins/*/README.md
````

**Edit 7.** `.github/workflows/release-refresh.yml:79-80`

Find exactly:

````text
            git push origin "HEAD:${HEAD_REF}"
          fi
````

Replace with:

````text
            git push origin "HEAD:${HEAD_REF}"
          fi

      # `outcome`, not `conclusion`: under continue-on-error the conclusion of
      # a failed step reads `success`.
      - name: Fail when the install-document pins could not be refreshed
        if: steps.pins.outcome == 'failure'
        run: |
          echo "::error::The step 'Refresh install-document pins' failed: scripts/check-install-pins.sh --write found a problem that needs a hand edit. Its log lists each one."
          exit 1
````


- [ ] **Step 4: Verify the wiring**

Run:

```bash
bash scripts/check-gate-wiring.sh | grep -e install-pins -e readme-coverage -e 'Every gate'
bash scripts/check-gate-wiring.sh --selftest | tail -1
just install-pins-test | tail -1 && just readme-coverage-test | tail -1 && just readme-coverage | tail -1
python3 -c "import yaml; [yaml.safe_load(open(f)) for f in ('.github/workflows/ci.yml', '.github/workflows/release-refresh.yml')]; print('yaml ok')"
```

Expected:

```text
  ok       check-install-pins.sh  (recipe: install-pins install-pins-test)
  ok       check-readme-coverage.sh  (recipe: readme-coverage readme-coverage-test)
Every gate script is wired.
All 27 self-test cases passed.
check-install-pins selftest: 30 cases passed
check-readme-coverage selftest: 16 cases passed
check-readme-coverage: 7 OpenCode plugin(s) each have a row, a bullet and an INSTALL.md.
yaml ok
```

If `actionlint` is installed, `actionlint .github/workflows/release-refresh.yml` prints nothing. (`just install-pins` still fails at this point, on the 14 stale documents; Task 6 fixes that.)

---

### Task 5: `docs/PLUGIN_GUIDELINES.md` (part of commit 2; do not commit)

**Files:**
- Modify: `docs/PLUGIN_GUIDELINES.md`

**Interfaces:**
- Consumes: the script paths from Tasks 2 and 3 and the CI step names from Task 4.
- Produces: nothing a later task reads.

Spec 4.3 asks for the pins rule in section 3.4 and "the sections for the other kinds", named in the "what fails if you skip it" tables. This task also adds the root README row to sections 3.4 and 3.5, because Task 3's check creates that obligation for every new OpenCode plugin and the spec's wording names only the pins; it is flagged in the hand-off so the owner can strike it.

`scripts/check-doc-paths.sh` reads this file. Two of its rules shaped the wording, so keep the text as written: every backticked path must exist, and a backticked bare name ending in `md` is looked up at the repository root, which is why the rows say "the install guide" and never put that file's bare name in backticks.

- [ ] **Step 0: Confirm the starting state**

Run: `grep -c 'check-install-pins\|check-readme-coverage' docs/PLUGIN_GUIDELINES.md; bash scripts/check-doc-paths.sh | tail -1`
Expected: `0`, then `All documented paths exist.`

- [ ] **Step 1: Apply the six edits**

Edits 1 and 2 add the same row to the tables of sections 3.2 and 3.3; edits 3 and 4 add two rows each to sections 3.4 and 3.5; edit 5 extends the "Generated files" bullet of section 4; edit 6 adds a row to the table of section 5.

**Edit 1.** `docs/PLUGIN_GUIDELINES.md:98`

Find exactly:

````text
| Antigravity configs | Antigravity only: binaries named in its configs must resolve. | no | `scripts/check-agy-wiring.sh`, and `just agy-validate` where `agy` is installed. |
````

Replace with:

````text
| Antigravity configs | Antigravity only: binaries named in its configs must resolve. | no | `scripts/check-agy-wiring.sh`, and `just agy-validate` where `agy` is installed. |
| `<plugin folder>/README.md` | Antigravity and Qwen only, and only if the README gives a download URL: write its tag as `releases/download/<plugin>-v<x.y.z>/<asset>`. Do not edit the number afterwards: `bash scripts/check-install-pins.sh --write` sets it from the manifest, and the release branch runs that itself. | the version in the URL | `bash scripts/check-install-pins.sh`, the CI step "Check install-document pins are current". |
````

**Edit 2.** `docs/PLUGIN_GUIDELINES.md:121`

Find exactly:

````text
| `scripts/check-package-versions.sh` | As in 3.1, with the manifest named twice. | no | Nothing. |
````

Replace with:

````text
| `scripts/check-package-versions.sh` | As in 3.1, with the manifest named twice. | no | Nothing. |
| `<plugin folder>/README.md` | Antigravity and Qwen only, and only if the README gives a download URL: write its tag as `releases/download/<plugin>-v<x.y.z>/<asset>`. Do not edit the number afterwards: `bash scripts/check-install-pins.sh --write` sets it from the manifest, and the release branch runs that itself. | the version in the URL | `bash scripts/check-install-pins.sh`, the CI step "Check install-document pins are current". |
````

**Edit 3.** `docs/PLUGIN_GUIDELINES.md:162-164`

Find exactly:

````text
| `scripts/check-package-versions.sh` | As in 3.1, with `package.json` named twice. | no | Nothing. |

No `Cargo.toml` edit is needed
````

Replace with:

````text
| `scripts/check-package-versions.sh` | As in 3.1, with `package.json` named twice. | no | Nothing. |
| The install guide | Give it exactly one line that starts with `VERSION=`, in the Step 2 code block, and write every download URL with `$VERSION`. If `opencode.jsonc` declares a local MCP server, add exactly one line that starts with `CC_VERSION=` (the version of the Claude Code sibling whose release carries the binary); otherwise have none. Do not edit the number afterwards: `bash scripts/check-install-pins.sh --write` sets it from `package.json`, and the release branch runs that itself. Do not write that a version is "current as of this writing"; the check rejects the phrase. | the version on those lines | `bash scripts/check-install-pins.sh`, the CI step "Check install-document pins are current". |
| `README.md` at the repository root | In its `## Installing` section, add a row to the OpenCode table that links the plugin folder, and a bullet with the raw URL of the install guide. | no | `bash scripts/check-readme-coverage.sh`, the CI step "Check the root README covers every OpenCode plugin". |

No `Cargo.toml` edit is needed
````

**Edit 4.** `docs/PLUGIN_GUIDELINES.md:188-190`

Find exactly:

````text
| `scripts/check-package-versions.sh` | As in 3.1, with `package.json` named twice. | no | Nothing. |

A new family needs the release plumbing described in 3.3.
````

Replace with:

````text
| `scripts/check-package-versions.sh` | As in 3.1, with `package.json` named twice. | no | Nothing. |
| The install guide | Give it exactly one line that starts with `VERSION=`, in the Step 2 code block, and write every download URL with `$VERSION`. Have no line that starts with `CC_VERSION=`: there is no binary. Do not edit the number afterwards: `bash scripts/check-install-pins.sh --write` sets it from `package.json`, and the release branch runs that itself. Do not write that a version is "current as of this writing"; the check rejects the phrase. | the version on those lines | `bash scripts/check-install-pins.sh`, the CI step "Check install-document pins are current". |
| `README.md` at the repository root | In its `## Installing` section, add a row to the OpenCode table that links the plugin folder, and a bullet with the raw URL of the install guide. | no | `bash scripts/check-readme-coverage.sh`, the CI step "Check the root README covers every OpenCode plugin". |

A new family needs the release plumbing described in 3.3.
````

**Edit 5.** `docs/PLUGIN_GUIDELINES.md:214-215`

Find exactly:

````text
  regions of the READMEs (`just footprint-regen`), and the skill copies emitted by
  `just emit-devtools-skills`.
````

Replace with:

````text
  regions of the READMEs (`just footprint-regen`), the skill copies emitted by
  `just emit-devtools-skills`, and the release version pinned in each OpenCode install guide and in the
  Antigravity and Qwen READMEs (`bash scripts/check-install-pins.sh --write`).
````

**Edit 6.** `docs/PLUGIN_GUIDELINES.md:231`

Find exactly:

````text
| Documents | `just links` (see below), `just spellcheck`, `bash scripts/check-doc-paths.sh` |
````

Replace with:

````text
| Documents | `just links` (see below), `just spellcheck`, `bash scripts/check-doc-paths.sh` |
| An install guide, a plugin README with a download URL, the root README's install section | `just install-pins`, `just readme-coverage` |
````


- [ ] **Step 2: Verify**

Run: `bash scripts/check-doc-paths.sh | tail -1 && bash scripts/check-doc-paths.sh --selftest | tail -1 && just spellcheck && grep -c 'check-install-pins\|check-readme-coverage' docs/PLUGIN_GUIDELINES.md`
Expected: `All documented paths exist.`, `All 61 self-test cases passed.`, the echoed command `typos` with nothing after it, and `7`.

---

### Task 6: First `--write`, the full gate, commit 2

**Files:**
- Modify (by the script, never by hand): the 7 `opencode/plugins/*/INSTALL.md`, `antigravity/plugins/{rtk-mcp-agy,re-ghidra-mcp-agy,aiplugins-devtools-agy,opencode-ctl-mcp-agy}/README.md`, `qwen/plugins/{rtk-mcp-qwen,re-ghidra-mcp-qwen,aiplugins-devtools-qwen}/README.md`

**Interfaces:**
- Consumes: everything from Tasks 2 to 5.
- Produces: commit 2.

- [ ] **Step 0: Confirm the starting state**

Run: `git status --short`
Expected: exactly these, and nothing else:

```text
 M .github/workflows/ci.yml
 M .github/workflows/release-refresh.yml
 M Justfile
 M docs/PLUGIN_GUIDELINES.md
?? scripts/check-install-pins.sh
?? scripts/check-readme-coverage.sh
```

- [ ] **Step 1: Rewrite the pins**

Run: `bash scripts/check-install-pins.sh --write`
Expected: 14 lines starting `  rewrote  `, then `check-install-pins: rewrote 14 of 18 document(s); 22 pin(s) are current.`

Run: `bash scripts/check-install-pins.sh`
Expected: `check-install-pins: 22 pin(s) in 18 document(s) are current.`, exit 0.

- [ ] **Step 2: Check the result against the spec tables**

Run: `git diff -U0 -- opencode antigravity qwen | grep '^+[^+]' | sed 's|https://github.com/ckir/aiplugins/releases/download/||'`

Expected: these 22 changed values, which are the "Current release" columns of spec section 1 (order may differ):

| Document | After `--write` |
|---|---|
| `opencode/plugins/repo-tasks-mcp-opencode/INSTALL.md` | `VERSION=0.2.0`, `CC_VERSION=0.2.0` |
| `opencode/plugins/opencode-ctl-mcp-opencode/INSTALL.md` | `VERSION=0.2.1`, `CC_VERSION=0.2.0` |
| `opencode/plugins/rtk-mcp-opencode/INSTALL.md` | `VERSION=0.10.1`, `CC_VERSION=0.8.1` |
| `opencode/plugins/re-ghidra-mcp-opencode/INSTALL.md` | `VERSION=0.10.1`, `CC_VERSION=0.9.1` |
| `opencode/plugins/todov2-opencode/INSTALL.md` | `VERSION=0.6.1` |
| `opencode/plugins/repo-user-guide-opencode/INSTALL.md` | `VERSION=0.3.0` |
| `opencode/plugins/aiplugins-devtools-opencode/INSTALL.md` | `VERSION=0.5.0` |
| `antigravity/plugins/rtk-mcp-agy/README.md` | `rtk-mcp-agy-v0.9.1` (one URL) |
| `antigravity/plugins/re-ghidra-mcp-agy/README.md` | `re-ghidra-mcp-agy-v0.11.0` (one URL) |
| `antigravity/plugins/opencode-ctl-mcp-agy/README.md` | `opencode-ctl-mcp-agy-v0.2.0` (one URL) |
| `antigravity/plugins/aiplugins-devtools-agy/README.md` | `aiplugins-devtools-agy-v0.5.0` (the prose tag and one URL) |
| `qwen/plugins/rtk-mcp-qwen/README.md` | `rtk-mcp-qwen-v0.8.0/rtk-mcp-qwen-extension.zip` (two URLs, one in a table cell) |
| `qwen/plugins/re-ghidra-mcp-qwen/README.md` | `re-ghidra-mcp-qwen-v0.8.0/re-ghidra-mcp-qwen-extension.zip` (two URLs) |
| `qwen/plugins/aiplugins-devtools-qwen/README.md` | `aiplugins-devtools-qwen-v0.4.0` (two URLs, one in a table cell) |

Also run `git diff --stat -- opencode antigravity qwen | tail -1`; expected `14 files changed, 22 insertions(+), 22 deletions(-)`. Any other changed line in those folders is a defect in the script: stop and report it. A manifest version that has moved since the plan was written (a release merged in between) changes the numbers, not the count; in that case the manifest wins and the table above is out of date.

Then `grep -n 'releases/download/v[0-9]' opencode/plugins/*/INSTALL.md antigravity/plugins/*/README.md qwen/plugins/*/README.md`; expected: no output and exit 1 (no legacy lockstep tag remains; before this step it prints four lines, two in each of the `rtk-mcp-qwen` and `re-ghidra-mcp-qwen` READMEs).

- [ ] **Step 3: Run the repository gate**

Run: `just check`
Expected: exit 0, and its output includes `check-install-pins selftest: 30 cases passed`, `check-install-pins: 22 pin(s) in 18 document(s) are current.`, `check-readme-coverage selftest: 16 cases passed` and the coverage pass line.

- [ ] **Step 4: Measure the footprint**

Spec section 5: the footprint figures live in README regions and version strings changed length. `just check` already runs both recipes; read their output specifically.

Run: `just footprint && just footprint-budgets && git status --short`
Expected: both exit 0, and `git status --short` lists no file under `docs/footprints/` and no README beyond the seven from Step 1. If a figure moved, do not regenerate or override anything: report the plugin and both figures to the owner (the override procedure in `HACK.md` is theirs to invoke).

- [ ] **Step 5: Commit**

```bash
git add scripts/check-install-pins.sh scripts/check-readme-coverage.sh Justfile \
  .github/workflows/ci.yml .github/workflows/release-refresh.yml docs/PLUGIN_GUIDELINES.md \
  opencode/plugins/*/INSTALL.md antigravity/plugins/*/README.md qwen/plugins/*/README.md
git status --short   # everything staged, nothing left over
git commit -m "chore: generate and gate the install-document release pins" \
  -m "scripts/check-install-pins.sh reads each plugin's version from its manifest and checks, or with --write rewrites, the release pins in the OpenCode INSTALL.md files and the Antigravity and Qwen READMEs. release-refresh.yml runs --write on the release branch; CI and just check run the check. scripts/check-readme-coverage.sh requires a row and a URL for every OpenCode plugin in the root README's install section. The first --write run brings 14 documents to the current releases and moves the Qwen READMEs off the retired v0.7.2 lockstep tag." \
  -m "<the Co-Authored-By line from the attribution reminder>"
```

The pre-commit hook runs `scripts/check-license-headers.sh` on the staged files; both scripts carry the header. Then update the execution index with the commit SHA and the resume point "Task 7".

---

### Task 7: Completion gates

**Files:** none.

- [ ] **Step 1: Confirm both commits pass the gate on their own**

Commit 2 was gated in Task 6 and commit 1 in Task 1. Confirm nothing else changed: `git log --oneline origin/main..HEAD` lists the two spec commits, the plan commit, and the two commits of this plan, and `git status --short` is empty.

- [ ] **Step 2: AGY-CAPSTONE**

Before calling the work complete, run the `clavity:agy-capstone` skill on the committed range (the two implementation commits). Every finding is measured before it is folded, and so is every fix the peer proposes. The owner's standing preference for long sessions is a fresh session for this step.

- [ ] **Step 3: AGY-TEST-AUDIT**

After the capstone is green, run the `clavity:agy-test-audit` skill on the two selftests.

- [ ] **Step 4: Hand back**

Use superpowers:finishing-a-development-branch. Pushing the branch and opening a pull request need the owner's go-ahead. Tell the owner what the first release PR after the merge will show: the release-refresh job gains the step "Refresh install-document pins", and its first live run is the first real test of that workflow change.
