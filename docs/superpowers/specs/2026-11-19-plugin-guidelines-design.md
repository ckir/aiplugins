# Plugin guidelines: keeping the README's promise

Status: design approved by the owner on 2026-11-19; implemented on branch docs/plugin-guidelines per
docs/superpowers/plans/2026-11-19-plugin-guidelines.md. The adversarial panel (four rounds) read documents and
did not run code; the path check's contract is proven by its self-test and mutation run (Task 2), not by the
panel.

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
- The first real run of the script (the plan's Task 2) over `HACK.md` found a third bare script name that the
  prototype missed, because the prototype looked only at spans that end in an extension:
  `gen-marketplaces.sh --check`, in the paragraph about the release-please bump pull request.

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
| 5 | `HACK.md` | Fixed in the same change: the dead `claude-code/example/` path, the "Task 6" jargon, and the two bare script names `check-package-versions.sh` and `gen-marketplaces.sh`, each written as `scripts/<name>`. Its other content is untouched. |

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
- `scripts/check-doc-paths.sh`, which also carries its own `--selftest` mode (section 7).

Edited:
- `README.md`: the promise line becomes links to the guidelines and `HACK.md`; the documentation index gains
  an entry for the guidelines.
- `HACK.md`: the four fixes named in section 2, row 5.
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
2. **Which kind are you adding.** The kinds follow what the checks key on, which is two properties:
   whether the host's plugin folders are Cargo workspace members (Claude Code, Qwen and Antigravity are;
   OpenCode is not), and whether `agents.json` flags the plugin `skillsOnly`. That gives four kinds,
   measured against the repository today:
   - a Rust front on a Cargo host (a workspace member crate with `[[bin]]` targets);
   - a skills-only plugin on a Cargo host (no crate, listed in the root `Cargo.toml` `exclude` and flagged
     `skillsOnly`);
   - an OpenCode plugin with a `plugin.ts` and a `package.json` (never a Cargo member);
   - an OpenCode skills-only plugin (a `package.json` and no `plugin.ts`, flagged `skillsOnly`, and no
     `Cargo.toml` edit at all).

   A short decision table, then one section per kind. The plan confirms the kind list against
   `agents.json` and the root `Cargo.toml` before the text is written.
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
or step a gate demands that the document did not name is a defect in the document. The scratch branch may
carry local commits, because the footprint gate fails on uncommitted footprint documents (`Justfile`,
the `footprint` recipe) and the license-header check reads only tracked files; it is never merged or
pushed, and is deleted afterwards.

A second throwaway covers the Rust front kind, since the repository's main archetype would otherwise pass
unexercised: a minimal crate with one `[[bin]]` target, followed through the same document, running only
the gates that need no build (`scripts/check-plugin-wiring.sh` reads `cargo metadata --no-deps`, not a
compiled binary). The gates that need a built MCP server, such as the footprint probe and the bundle
smoke, are not run and are reported as not run. The two OpenCode kinds are verified by reading an
exemplar's wiring against the checklist.

The guidelines say so themselves: each kind's section is marked either "checked by a walkthrough" or
"checked by reading only", and the plan records which gates were run for each kind and which were not.

## 7. The path check

`scripts/check-doc-paths.sh` follows the conventions of the existing `scripts/check-*.sh`: runs from the
repository root, counts what it checked, prints an aligned status per item, exits non-zero on failure, and
fails when it checked nothing, so a broken extraction cannot pass silently.

Contract:
- Inputs: `docs/PLUGIN_GUIDELINES.md` and `HACK.md`. Each is read with carriage returns removed, as the
  other check scripts do for tool output.
- Candidates: every inline backticked span is split on whitespace and each token is judged on its own,
  so `bash scripts/gen-marketplaces.sh` checks `scripts/gen-marketplaces.sh` (measured: a whole-span rule
  skips every command line, where the paths most likely to rot are). A trailing `:<digits>` is removed
  from the token FIRST, before anything else is decided (measured: `README.md:164` and `Justfile:360`
  match neither the extension list nor the root list until the suffix is gone). Then a token is a path
  candidate when it contains `/` (checked relative to the repository root), or has no `/` and either ends
  in one of the extensions `md`, `json`, `toml`, `sh`, `yml`, `yaml`, `lock`, `rs`, `ts`, `txt`, or is one
  of the extensionless or dotted root files `Justfile`, `LICENSE`, `LICENSE-ADDENDUM`, `.gitignore`,
  `.gitattributes` (a bare name, checked at the root only). Consequence for the text: a file that is not
  at the root is always written with its repository-relative path, never by its bare name, and the
  extensionless list is closed: a name outside it is not checked.
- What is skipped: a token containing `<`, `>`, `{`, `}` or `*`; a token that is not a repository path by
  shape, namely one containing `://`, starting with `@`, `~`, `$`, `/` or `-`, or starting with
  `origin/` or `refs/`; and everything inside fenced code blocks. Fences follow the CommonMark rule: a
  fence opens on a line of at least three backticks or tildes and closes only on a line of the same
  character that is at least as long, so a four-backtick fence can hold a three-backtick one (this
  repository's Markdown already uses four-backtick fences, in the skill documents). Any other token with
  a `/` is asserted to be a repository path, so text such as a MIME type has to be written outside
  backticks or reworded; the skip list stays short on purpose.
- A token with a `..` segment (`..`, `../x`, `x/../y`, `x/..`) fails outright, whether or not it contains a
  `/` and before it is classified as a candidate, and is never tested for
  existence: it is not a path written from the repository root, and `test -e` would follow it out of the
  repository and pass on a file that is not the repository's (measured from the repository root:
  `test -e ../../../../../etc/passwd` succeeds, and `test -e ../aiplugins/README.md` succeeds through the
  checkout directory's own name). Backslashes and a `.`-only token are treated the same way as any other
  token: judged by the rules above, with no special case.
- A token that git ignores is skipped only when the shallowest ignored prefix of it has an existing
  parent. Build output absent from a fresh clone (`target/release/` has the ignored prefix `target`,
  whose parent is the root) does not fail; a misspelt directory above an ignored `bin/` does (measured:
  `.gitignore:65` ignores `qwen/plugins/*/bin/`, so `qwen/plugins/typo-qwen/bin/` is ignored, and its
  shallowest ignored prefix has a parent that does not exist). The root `bin/` is not ignored
  (`.gitignore` ignores `bin/` only under three plugin trees), so the text writes `<plugin dir>/bin/`,
  which the placeholder rule skips.
- It fails, naming the file, the line and the span (`file:line: ...`), on a path that does not exist; on
  an unbalanced code fence, naming the line where the unclosed fence opened (an unclosed fence would
  otherwise silently disable the check for the rest of the file); on a line outside a fence with an odd
  number of backticks (a stray backtick would shift every span after it on that line); and when any one
  input file yields zero
  checked paths, so losing every path from `HACK.md` cannot hide behind the guidelines' count.

Known risk, named by the peer and accepted: a placeholder or illustrative path that the skip rules miss
becomes a false failure, and false failures train people to ignore the check. The skip rules therefore stay
narrow and explicit, and the self-test pins each one.

Self-test, following the repository's seeded-defect convention (the site check "proves the checker rejects
each seeded defect"): fixtures show a dead path failing; a bare name at the root passing and a bare name
not at the root failing; an extensionless root file such as `Justfile` checked, and a listed name whose file is
absent failing, and an unlisted name not checked at all; a `:<digits>` suffix handled; a path inside a command line
checked; a placeholder, a URL and a git ref skipped; an ignored path whose shallowest ignored prefix has an
existing parent skipped (`target/release/`) and one with a missing parent failing (a misspelt plugin
directory above `bin/`); an ignored directory written without its trailing slash, accepted like the same path with it; a fenced block skipped,
tilde fences and a three-backtick fence nested in a four-backtick one included; an unbalanced fence
failing with the line where it opened; a bare root file cited with a line suffix (`README.md:164`)
checked; a token with a `..` segment failing, in each of the four positions listed above, including one
that would resolve to a file that exists; and an input file with no paths failing.

Self-test wiring follows the neighbours (measured: five scripts take a `--selftest` flag; of those,
`bump-opencode-plugin.sh` and `rerun-runner-failures.sh` each have a `Justfile` recipe listed in `check`
and a `ci.yml` step): the new script takes `--selftest`, which runs the fixtures, and that has its own
recipe in `check` and its own `ci.yml` step, separate from the run over the two documents. A self-test
nothing runs would prove nothing.

Wiring of the check itself: the `check` recipe, a `ci.yml` step, and the `lefthook.yml` `pre-push` block.
The `wiring` job (`Plugin Wiring`) in `ci.yml` is the natural home, being the job that already runs the
other static checks; the plan confirms it.

Licensing: `scripts/check-license-headers.sh` covers `.sh` files (its scope is `.rs .sh .ts .js .py .ps1`)
and not Markdown, so the new script carries the repository's license header and the two documents do not.
The check reads tracked files only, so it sees the script once it is added.

## 8. Risks and what stays open

- The guidelines are only as good as their sources. The oracle in section 6 is the mitigation. It runs
  both throwaways without a build, so the Rust front is checked on its static gates only, and the two
  OpenCode kinds are checked by reading.
- The check proves a path exists, not that the sentence around it is true.
- The extensionless root-file list is closed, so a misspelt `Justfile` is caught only because it is on the
  list; any other bare extensionless name goes unchecked. Accepted: widening it means guessing at prose.
- Three contributor-facing documents (`README.md`, `HACK.md`, the guidelines) need a clear division. The
  guidelines own "adding a plugin"; `HACK.md` owns maintainer procedure. Where a topic could sit in
  either, it stays in `HACK.md` and the guidelines link to it.

Left for the plan, because they depend on code to be read then: the exact script name and shell idiom, the
extraction technique (the contract in section 7 is fixed; only how it is implemented is open), whether the
spell check applies to the new files, the self-test's file layout and recipe name, and the final wording of
every checklist row.

One constraint on that implementation: the check runs in the pre-push hook on Windows hosts, where every
external process is slow, so the number of processes it starts must not grow with the number of tokens
(for example one `git check-ignore --stdin` call for all ignored-path questions, not one per token). No
timing for this has been measured, and none can be from this Linux host.

## 9. Stand-downs

Recorded from the panel (agy, 2026-11-19). Rounds 2, 3 and 4 raised nothing that was stood down: each
finding was measured and folded, and round 4 returned no findings. Round 1:

- `REJECTED: the peer's proposed fix for the ignored-path rule` (check whether any leading component is
  ignored): measured against `.gitignore:65`, `qwen/plugins/typo-qwen/bin/` is ignored through its `bin`
  component alone, so that fix lets the misspelt directory pass. Section 7 uses the shallowest ignored
  prefix and its parent instead. The finding behind it (nested build paths such as `target/release/`
  failing on a fresh clone) was real and is folded.
- `DISCARDED-BELOW-FLOOR: a failing new check halts "just check" before later diagnostics` unreachable as
  a distinct defect because `Justfile:360` is a plain dependency list in which any existing check can
  already fail first; the new check adds no new ordering behaviour.
