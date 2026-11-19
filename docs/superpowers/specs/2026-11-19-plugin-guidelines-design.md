# Plugin guidelines: keeping the README's promise

Status: design approved by the owner on 2026-11-19; spec awaiting owner review. No plan yet.

## 1. Problem

`README.md` ends with the line "More details and plugin guidelines will be added soon." It came in with the
scaffold commit `637037e`, before the repository grew its conventions. Nothing keeps it.

- No document tells a plugin author what a new plugin must touch outside its own folder. The only trace is a
  paragraph in `docs/superpowers/specs/2026-11-18-repo-tasks-design.md` (section 17.1), which lists the files
  the `opencode-ctl` pull request (merge `9f315e5`) changed: `agents.json`, the four marketplace manifests,
  `release-please-config.json` and `.release-please-manifest.json`, `Cargo.toml` and `Cargo.lock`, `Justfile`,
  `.github/dependabot.yml`, three workflows, `docs/footprints/`, `README.md` and `QWEN.md`.
- `HACK.md` at the root is titled "contributor notes". It covers adding or removing an agent, the footprint
  override, tags and releases, and marketplace URLs. It says nothing about adding a plugin. No Markdown link
  in the repository points to it, and the README does not mention it; only old plans refer to it by name.
- `HACK.md` has already drifted. Step 1 of "To add an agent" names `claude-code/example/`, which does not
  exist; the template is `claude-code/plugins/example/`. The same section says "the Task 6 gate command",
  leftover plan jargon. Nothing caught either, because the `links` recipe runs `mlc --offline`, which
  checks Markdown links and not paths in backticks.
- A prototype of the path check run over `HACK.md` as it stands (inline spans only, fences removed) found
  one more problem and one shape the contract must handle: the bare shorthand `check-package-versions.sh`
  (the file is `scripts/check-package-versions.sh`; a bare name does not resolve from the root), and a span
  carrying a line suffix, `tools/plugin-footprint/src/manifest.rs:16`, whose file does exist.

Three requirements that an author would otherwise find only by failing CI were found while preparing
this design and are the reason a plain "list the files" document is not enough:

- `docs/site/families.txt` must list a new plugin family; `scripts/check-site.sh` fails when the
  catalog's family keys and that file differ.
- A crate-less (skills-only) plugin must be listed in the root `Cargo.toml` `exclude` array, because the
  workspace members use wildcards.
- `scripts/check-package-versions.sh` carries a hardcoded table of plugin to manifest pairs. A plugin
  missing from it is not checked at all, and nothing says so.

## 2. Decisions already ruled

Each was put to the owner after a consult of the agy peer. Where agy and the driver differed, one
counter-turn was held and the result is recorded.

| # | Fork | Ruling |
|---|---|---|
| 1 | Location | The content goes in the new `docs/PLUGIN_GUIDELINES.md`. The README links it, and links `HACK.md`. No `.github/CONTRIBUTING.md` pointer. agy first proposed a root `CONTRIBUTING.md` and withdrew it, because two root contributor files would overlap with `HACK.md`. |
| 2 | Scope | The wiring checklist for each plugin type, plus the authoring standards. No new README content. |
| 3 | "More details" | The scaffold README was 25 lines with three sections. It now carries install steps, a footprint table, the `shared/` crate table and a documentation index, none of which the scaffold had. That half of the promise is treated as kept, and the pull request says so. |
| 4 | Drift | A new script checks that the paths named in the two contributor documents exist, and runs wherever the other checks run. agy first proposed prose only and withdrew it once shown that `mlc` skips code spans. |
| 5 | `HACK.md` | Fixed in the same change: the dead `claude-code/example/` path, the "Task 6" jargon, and the bare `check-package-versions.sh` written as its full path. Its other content is untouched. |

## 3. Out of scope

- New user-facing README content, and any rewrite of existing README sections beyond the promise line and
  one index entry.
- A `.github/CONTRIBUTING.md`. It can be added later without moving any content.
- Guidance on writing a plugin for a particular host (Claude Code, Qwen, Antigravity, OpenCode). The
  guidelines cover this repository's wiring and standards and point at each host's own documentation.
- Fixing any other document's drift, or checking paths in documents other than the two named above.
- Changing any existing check script.

## 4. Deliverables

New:
- `docs/PLUGIN_GUIDELINES.md`.
- `scripts/check-doc-paths.sh`, and a self-test for it (section 7).

Edited:
- `README.md`: the promise line becomes links to the guidelines and `HACK.md`; the documentation index gains
  an entry for the guidelines.
- `HACK.md`: the three fixes named in section 2, row 5.
- `Justfile`, `.github/workflows/ci.yml` and `lefthook.yml`: wiring for the new check, following how the
  neighbouring checks are wired (`Justfile` `check` recipe, `ci.yml` steps that run `bash scripts/...`,
  the `lefthook.yml` `pre-push` block).

## 5. The guidelines document

Audience: someone adding a plugin to this repository, human or agent. Tone and form follow the repository's
other documents: plain, specific, every behavioural statement traceable to a file.

Rules for the text:
- It cites files and scripts, never line numbers. The guard in section 7 can check that a file exists and
  cannot check a line.
- Paths in running text go in backticks, so the guard sees them. Illustrative trees go in fenced blocks,
  which the guard skips.
- Anything generated is said to be generated, with the command that regenerates it. A generated file is
  never described as hand-edited.
- A statement the author could not confirm is marked as unconfirmed, as the repo-tasks README marks
  Windows and macOS `not measured`. It is not softened into prose.

Structure:

1. **Orientation.** What a plugin is in this repository; the naming scheme `<name>-<suffix>` with the four
   suffixes; what a family is; how `shared/` engines differ from per-agent fronts; the published-name
   mapping in `agents.json` that lets a folder name and a published name differ.
2. **Which kind are you adding.** Three kinds, because their wiring differs: a Rust binary plugin, a
   skills-only plugin (no crate, flagged `skillsOnly` in `agents.json`), and an OpenCode TypeScript plugin.
   A short decision table, then one section per kind.
3. **The wiring checklist, per kind.** A table per kind. Each row: the file, what to put in it, whether it is
   generated, and which check fails if the row is skipped (or "nothing fails", where that is the truth, as
   with the versions table). Each kind names a live exemplar to copy.
4. **Authoring standards.** Naming and suffixes; license headers and their exemptions; versioning (start
   value, who mints tags, the rule against inheriting the workspace version); the footprint budget and a
   pointer to `HACK.md` for the override procedure; the shape of a plugin's README including its generated
   footprint region.
5. **Before you open a pull request.** The `just` recipes to run and what each proves; which files are
   generated and must not be hand-edited; where the maintainer procedures live (`HACK.md`).

Every row and standard is read from the source while the document is written (section 6). The list of
touched files in section 1 comes from a past pull request and is a starting point, not evidence.

## 6. How the guidelines are verified

Reading the scripts is necessary and not sufficient: a checklist can be consistent with every script and
still omit a file the scripts demand only together.

Acceptance oracle: a fresh-context agent, given only the finished document and the repository (not this
spec, not the author's notes), follows the document literally to add one throwaway skills-only plugin on a
scratch branch, then runs the gates the document names for that kind (wiring, marketplace manifests,
package versions, the site check, the skills bundle checks, the footprint checks that apply). The author
does not run it, because the author's own knowledge fills the document's gaps without noticing. Every file
or step a gate demands that the document did not name is a defect in the document. The scratch branch and
plugin are never committed or pushed.

The Rust binary and OpenCode TypeScript kinds are verified by reading the exemplar's wiring against the
checklist, because a full throwaway build is out of proportion. The guidelines say so themselves: each
kind's section is marked either "checked by a walkthrough" or "checked by reading only", and the plan
records which gates were run for each kind and which were not.

## 7. The path check

`scripts/check-doc-paths.sh` follows the conventions of the existing `scripts/check-*.sh`: runs from the
repository root, counts what it checked, prints an aligned status per item, exits non-zero on failure, and
fails when it checked nothing, so a broken extraction cannot pass silently.

Contract:
- Inputs: `docs/PLUGIN_GUIDELINES.md` and `HACK.md`. Each is read with carriage returns removed, as the
  other check scripts do for tool output.
- What counts as a path span: an inline backticked span with no whitespace that either contains `/`
  (checked relative to the repository root) or has no `/` and ends in one of the extensions `md`, `json`,
  `toml`, `sh`, `yml`, `yaml`, `lock`, `rs`, `ts`, `txt` (a bare file name, checked at the root only).
  A trailing `:<digits>` is removed before the check. Consequence for the text: a file that is not at the
  root is always written with its repository-relative path, never by its bare name.
- What is skipped: a span containing `<`, `>`, `{`, `}` or `*`; and everything inside fenced code blocks
  (both backtick and tilde fences).
- A span that git ignores is skipped only when its parent directory exists, so build output such as
  `bin/` or `target/`, absent from a fresh clone, does not fail, while a misspelt plugin directory
  above an ignored `bin/` still does (measured: `.gitignore:65` ignores `qwen/plugins/*/bin/`, so
  `qwen/plugins/typo-qwen/bin/` would otherwise pass).
- It fails, naming the file and the span, on a path that does not exist; on an unbalanced code fence
  (an unclosed fence would otherwise silently disable the check for the rest of the file); and when any
  one input file yields zero checked paths, so losing every path from `HACK.md` cannot hide behind the
  guidelines' count.

Known risk, named by the peer and accepted: a placeholder or illustrative path that the skip rules miss
becomes a false failure, and false failures train people to ignore the check. The skip rules therefore stay
narrow and explicit, and the self-test pins each one.

Self-test, following the repository's seeded-defect convention (the site check "proves the checker rejects
each seeded defect"): fixtures show a dead path failing; a bare name at the root passing and a bare name
not at the root failing; a `:<digits>` suffix handled; a placeholder skipped; a git-ignored path with an
existing parent skipped and with a missing parent failing; a fenced block skipped, tilde fences included;
an unbalanced fence failing; and an input file with no paths failing.

Wiring: the `check` recipe, a `ci.yml` step, and the `lefthook.yml` `pre-push` block.

## 8. Risks and what stays open

- The guidelines are only as good as their sources. The oracle in section 6 is the mitigation, and it does
  not cover the Rust binary and OpenCode kinds end to end.
- The check proves a path exists, not that the sentence around it is true.
- A third contributor-facing document set (`README.md`, `HACK.md`, the guidelines) needs a clear division.
  The guidelines own "adding a plugin"; `HACK.md` owns maintainer procedure. Where a topic could sit in
  either, it stays in `HACK.md` and the guidelines link to it.

Left for the plan, because they depend on code to be read then: the exact script name and shell idiom, how
it extracts spans, which `ci.yml` job carries the step, whether `scripts/check-license-headers.sh` and the
spell check apply to the new files, the self-test's file layout and recipe name, and the final wording of
every checklist row.
