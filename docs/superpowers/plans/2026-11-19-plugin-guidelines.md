# Plugin Guidelines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the README promise "More details and plugin guidelines will be added soon." by adding `docs/PLUGIN_GUIDELINES.md`, a script that stops its paths (and `HACK.md`'s) from rotting, and the fixes `HACK.md` needs to pass that script.

**Architecture:** A bash script `scripts/check-doc-paths.sh` extracts the inline backticked tokens of two Markdown files with a POSIX awk program, applies a fixed rule order, and tests existence (one `git check-ignore --stdin` call per file for build output). It carries a `--selftest` mode of 31 seeded-defect cases. The guidelines document is checklist tables per plugin kind, written from the sources and then proved by a fresh-context walkthrough.

**Tech Stack:** bash, POSIX awk, git, `just`, GitHub Actions YAML, lefthook, Markdown.

**Spec:** `docs/superpowers/specs/2026-11-19-plugin-guidelines-design.md` (committed; owner-approved 2026-11-19; agy panel closed clean at round 4). Read it before any task. This plan amends it in Task 1 where measurement found a detail wrong.

## Global Constraints

Copied from the spec; every task includes these.

- The guidelines document cites files and scripts, never line numbers. Paths in running text go in backticks. A file inside a plugin folder is written `<plugin folder>/...`; illustrative trees go in fenced blocks, which the path check skips.
- `scripts/check-doc-paths.sh` is bash plus POSIX awk, follows the existing `scripts/check-*.sh` conventions (runs from the repository root, fails when it checked nothing, exits non-zero on failure), carries the repository license header, and is wired into the `check` recipe, a `ci.yml` step and the `lefthook.yml` `pre-push` block. Its self-test is a `--selftest` flag with its own recipe in `check` and its own `ci.yml` step.
- The acceptance oracle is run by a fresh-context agent given only the finished document and the repository, never by the author, and never shown the spec.
- No existing check script is changed. `HACK.md` gets exactly four fixes (Task 3). The README gets exactly two edits (Task 4). No new user-facing README content.
- Markdown files carry no license header; the new `.sh` file does.
- Every new or edited statement about a check names the script that enforces it, or says "nothing checks it" where that was measured.

## Plan decisions beyond the spec

Small implementation choices the spec left to the plan, listed so the owner can strike any of them:

- **P1.** A line outside a fence with an odd number of backticks fails with `odd number of backticks on the line`. A stray backtick would otherwise shift every span after it on that line. Neither `HACK.md` nor `README.md` has such a line today (measured).
- **P2.** The `..` rule applies to every token that survives the skip rules, before candidate classification, so a bare `..` fails. The spec's own list names `..` as failing; this fixes the order that makes it so.
- **P3.** An ignored directory written without its trailing slash (`plugins/real/bin`) is accepted exactly like the same path with it, because git cannot tell a not-yet-built directory from a file.
- **P4.** The script prints one `ok` line per checked path and one `FAIL` line per defect, and takes `--root DIR` so its self-test can run the real script against a throwaway repository.
- **P5.** The check reads files given on the command line, defaulting to `docs/PLUGIN_GUIDELINES.md` and `HACK.md`. A named file that does not exist is a failure, not a skip.

## Shared commands

**LINKS** is the link check in the form lefthook's `pre-push` block runs. `just links` is not used because its `--ignore-path` list omits `./opencode/plugins/repo-tasks-mcp-opencode/node_modules` (the lefthook list has it), so it exits 1 wherever that plugin's dependencies are installed; that defect is recorded separately and is out of scope here.

```bash
mlc --offline --ignore-path "./node_modules,./opencode/plugins/re-ghidra-mcp-opencode/node_modules,./opencode/plugins/opencode-ctl-mcp-opencode/node_modules,./opencode/plugins/repo-tasks-mcp-opencode/node_modules,./opencode/plugins/rtk-mcp-opencode/node_modules,./opencode/plugins/todov2-opencode/node_modules,./target,./docs/site,./.clavity"
```

Expected: exit 0 (measured on this branch before any task: exit 0).

## Dispatch rules (apply to every implementation dispatch, Tasks 1 to 5)

The two walkthrough agents of Task 6 are the exception: they must not see this plan, the spec or these rules, so they get only the standalone briefs written in Task 6. Their scope is enforced by the driver afterwards (Task 6, Steps 2, 3 and 7), not by the file list in rule 8, because they must edit whatever files the document under test tells them to.

Each implementation dispatch prompt MUST carry all of these, pasted whole:

1. **Step 0, state verification.** Open the files the task names and confirm the context described matches reality before editing. If it differs, STOP and report `STATE_MISMATCH: <what>`; do not adapt.
2. **Shape-divergence stop.** If making anything work would change the shape, type or encoding of a value in the content pasted in this plan, STOP and report `[original] -> [yours] because <reason>`. "It runs" is not a justification.
3. **Name the oracle.** Task 2's oracle is the 31 self-test cases and the mutation run; Task 3 and 4's oracle is `bash scripts/check-doc-paths.sh`; Task 5's is the commands in its steps. If a value looks wrong, the oracle wins: surface the conflict, never edit a test to fit the code.
4. **No elided enumerations.** Everything is pasted in full in this plan. Do not invent content for anything marked "unchanged".
5. **Exact commands and the expected pass line** are in each task's steps. Use the repository's own gates as they are; do not add stricter flags.
6. **Tests are already written; implement until they pass.** (Task 2: the cases are inside the script text.)
7. **ANOMALIES.** If you notice something wrong that is NOT part of this task, report it under `## Anomalies noticed` at the end of your final message, one line each: `- [defect|tool|process] one line stating the fact * path/file.ext:LINE or n/a * what you were doing`. State it as an observed fact with whatever makes it checkable. Do not judge severity, do not stop your task, and do not write any anomalies file. If nothing, write `none`.
8. **FILES.** The dispatch lists the only paths the subagent may create or modify. Anything else is out of bounds; if the task cannot be done within the list, STOP and report `SCOPE: needs <path> because <reason>`. After the subagent returns, the driver records the SHA before the dispatch and checks BOTH `git status --short` and `git log --stat <sha-before>..HEAD`.

Model tier per the owner's rules: Tasks 1 to 5 are applying content that is already written and verified, so **Haiku**, with the driver verifying the oracle output itself (lower tier, so check harder). Task 6 is an independent reader whose capability matters: **Sonnet**. Tasks 7 and 8 are the main thread. Do not use Fable without asking.

## File structure

| File | Change | Responsibility |
|---|---|---|
| `docs/superpowers/specs/2026-11-19-plugin-guidelines-design.md` | modify (Task 1) | amended where measurement found a detail wrong; status line updated (Task 7) |
| `scripts/check-doc-paths.sh` | create (Task 2) | the path check and its `--selftest` |
| `HACK.md` | modify (Task 3) | four fixes so it passes the check |
| `docs/PLUGIN_GUIDELINES.md` | create (Task 4), amend (Task 6) | the guidelines |
| `README.md` | modify (Task 4) | the promise line becomes links; one index entry |
| `Justfile`, `.github/workflows/ci.yml`, `lefthook.yml` | modify (Task 5) | wiring for the check and its self-test |

---

### Task 1: Amend the spec for four measured details

**Tier:** Haiku. **Files:** Modify only `docs/superpowers/specs/2026-11-19-plugin-guidelines-design.md`.

**Why:** The first real run of the script over `HACK.md` found a third bare script name that the spec's prototype missed, and a few contract details were fixed while writing the script. The spec must say what the code does.

**Interfaces:** Produces the amended contract that Task 2's script implements.

- [ ] **Step 1: State verification.** Run `git branch --show-current` (expect `docs/plugin-guidelines`) and `git status --short` (expect no output). Run `grep -c 'the three fixes named in section 2, row 5' docs/superpowers/specs/2026-11-19-plugin-guidelines-design.md` (expect `1`).

- [ ] **Step 2: Apply these six edits** with the Edit tool. Each `old_string` must match exactly once; if one does not, STOP with `STATE_MISMATCH`.

Edit 1, section 1 (append a bullet after the prototype bullet). Old:

```
  carrying a line suffix, `tools/plugin-footprint/src/manifest.rs:16`, whose file does exist.
```

New:

```
  carrying a line suffix, `tools/plugin-footprint/src/manifest.rs:16`, whose file does exist.
- The first real run of the script (the plan's Task 2) over `HACK.md` found a third bare script name that the
  prototype missed, because the prototype looked only at spans that end in an extension:
  `gen-marketplaces.sh --check`, in the paragraph about the release-please bump pull request.
```

Edit 2, section 2 row 5. Old:

```
Fixed in the same change: the dead `claude-code/example/` path, the "Task 6" jargon, and the bare `check-package-versions.sh` written as its full path. Its other content is untouched. |
```

New:

```
Fixed in the same change: the dead `claude-code/example/` path, the "Task 6" jargon, and the two bare script names `check-package-versions.sh` and `gen-marketplaces.sh`, each written as `scripts/<name>`. Its other content is untouched. |
```

Edit 3, section 4. Old:

```
- `HACK.md`: the three fixes named in section 2, row 5.
```

New:

```
- `HACK.md`: the four fixes named in section 2, row 5.
```

Edit 4, section 7, the `..` rule. Old:

```
- A token with a `..` segment (`..`, `../x`, `x/../y`, `x/..`) fails outright, and is never tested for
```

New:

```
- A token with a `..` segment (`..`, `../x`, `x/../y`, `x/..`) fails outright, whether or not it contains a
  `/` and before it is classified as a candidate, and is never tested for
```

Edit 5, section 7, the failure rule. Old:

```
  otherwise silently disable the check for the rest of the file); and when any one input file yields zero
```

New:

```
  otherwise silently disable the check for the rest of the file); on a line outside a fence with an odd
  number of backticks (a stray backtick would shift every span after it on that line); and when any one
  input file yields zero
```

Edit 6, section 7, the self-test list (two replacements; each old string is on its own lines in the file). Old:

```
and a misspelling of it
failing only when it is on the closed list;
```

New:

```
and a listed name whose file is
absent failing, and an unlisted name not checked at all;
```

Then old:

```
an ignored directory written without its trailing slash;
```

New:

```
an ignored directory written without its trailing slash, accepted like the same path with it;
```

- [ ] **Step 3: Verify.** Run `grep -c "four fixes named in section 2" docs/superpowers/specs/2026-11-19-plugin-guidelines-design.md` (expect `1`) and `typos docs/superpowers/specs/2026-11-19-plugin-guidelines-design.md` (expect no output, exit 0).

- [ ] **Step 4: Commit.**

```bash
git add docs/superpowers/specs/2026-11-19-plugin-guidelines-design.md
git commit -m "docs(spec): amend the plugin guidelines design for four measured details

HACK.md has two bare script names, not one; the .. rule applies before
classification; a line with an odd backtick count fails; two self-test
fixture descriptions corrected.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The path check, its self-test and its mutation run

**Tier:** Haiku. **Files:** Create only `scripts/check-doc-paths.sh`. (The mutation harness runs from the session scratchpad and is not committed.)

**Why:** The script is the drift guard. It was written and verified in the planning session: the self-test passes 31 cases under gawk, mawk, nawk, busybox awk and `gawk --posix`; 18 single-rule mutants all fail it; `shellcheck` and `addlicense` pass. The text below is that verified file, unchanged.

**Interfaces:**
- Produces: `bash scripts/check-doc-paths.sh [--root DIR] [FILE...]` (exit 0 and the line `All documented paths exist.`, or exit 1 with `FAIL` lines) and `bash scripts/check-doc-paths.sh --selftest` (exit 0 and the line `All 31 self-test cases passed.`).
- Consumed by: Task 3 and 4 (the oracle), Task 5 (wiring).

- [ ] **Step 1: State verification.** `git branch --show-current` is `docs/plugin-guidelines`; `ls scripts/check-doc-paths.sh` fails (the file does not exist yet); `git status --short` prints nothing.

- [ ] **Step 2: Create `scripts/check-doc-paths.sh` with exactly this content.**

`````bash
#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Check that the repository paths named in the contributor documents exist.
#
#   bash scripts/check-doc-paths.sh [--root DIR] [FILE...]
#   bash scripts/check-doc-paths.sh --selftest
#
# Why: `mlc` (the `links` recipe) checks Markdown links, not paths written in
# backticks, so a document naming a path that was later renamed fails nothing.
# HACK.md named `claude-code/example/` for a long time after it stopped
# existing. This reads the inline backticked spans of the documents it is given
# (default: docs/PLUGIN_GUIDELINES.md and HACK.md) and requires every path in
# them to exist. It proves a path exists, not that the sentence around it is
# true.
#
# Rules, in the order they are applied to each token of each backticked span
# (a span is split on whitespace, so `bash scripts/x.sh` checks scripts/x.sh):
#   1. A trailing :<digits> is removed first (README.md:164 -> README.md).
#   2. SKIP a token with < > { } or *; a token that is not a repository path by
#      shape (contains ://, starts with @ ~ $ / or -, or starts with origin/ or
#      refs/); and everything inside fenced code blocks (CommonMark fences: the
#      closing fence uses the same character and is at least as long).
#   3. FAIL a token with a .. segment (.., ../x, x/../y, x/..). It is never
#      tested for existence: `test -e` would follow it out of the repository.
#   4. A token is a CANDIDATE when it contains /, or has no / and either ends
#      in md json toml sh yml yaml lock rs ts txt or is one of the closed list
#      Justfile LICENSE LICENSE-ADDENDUM .gitignore .gitattributes (a bare name
#      is checked at the repository root only). Anything else is not checked.
#   5. A candidate must exist. A missing one that git ignores is accepted only
#      when the shallowest ignored prefix of it has an existing parent, so
#      build output absent from a fresh clone (target/release/) passes while a
#      misspelt directory above an ignored bin/ still fails.
# Also FAIL: an unclosed code fence, a line with an odd number of backticks
# outside a fence (a stray backtick shifts every span after it), and an input
# file in which no path was checked.
set -euo pipefail

self=$(cd "$(dirname "$0")" && pwd)/$(basename "$0")

# --- the extractor ----------------------------------------------------------
# Emits "TOK<TAB>line<TAB>token" for every whitespace-separated token of every
# inline backticked span outside a fenced block, and "ERR<TAB>line<TAB>why".
# POSIX awk only: no intervals, no gensub, no match() arrays.
AWK_EXTRACT=$(cat <<'AWK'
function leading_spaces(s,   i) { i = 0; while (substr(s, i + 1, 1) == " ") i++; return i }
function run_len(s, pos, ch,   k) { k = 0; while (substr(s, pos + k, 1) == ch) k++; return k }
BEGIN { bt = sprintf("%c", 96); infence = 0 }
{
    line = $0
    sp = leading_spaces(line)
    if (sp <= 3) {
        ch = substr(line, sp + 1, 1)
        if (ch == bt || ch == "~") {
            k = run_len(line, sp + 1, ch)
            if (k >= 3) {
                rest = substr(line, sp + 1 + k)
                if (infence) {
                    if (ch == fch && k >= flen && rest ~ /^[ \t]*$/) infence = 0
                    next
                }
                if (ch == "~" || index(rest, bt) == 0) {
                    infence = 1; fch = ch; flen = k; fline = NR
                    next
                }
            }
        }
    }
    if (infence) next
    tmp = line
    n = gsub(bt, "", tmp)
    if (n % 2 == 1) { print "ERR\t" NR "\todd number of backticks on the line"; next }
    m = split(line, parts, bt)
    for (i = 2; i <= m; i += 2) {
        nt = split(parts[i], toks, /[ \t]+/)
        for (j = 1; j <= nt; j++) if (toks[j] != "") print "TOK\t" NR "\t" toks[j]
    }
}
END { if (infence) print "ERR\t" fline "\tunclosed code fence" }
AWK
)

failures=0
fail() { echo "  FAIL     $1" >&2; failures=$((failures + 1)); }

# --- the check --------------------------------------------------------------

# run_check ROOT FILE...: print one line per checked path, fail on a defect.
run_check() {
    local root=$1 file ext checked want probe probes ignored prefix rest s parent shallowest
    shift
    cd "$root"
    for file in "$@"; do
        if [ ! -f "$file" ]; then
            fail "$file: input file does not exist"
            continue
        fi
        checked=0
        # Pass 1: extract, classify, and test existence; collect the missing.
        local missing=()      # entries "line<TAB>token"
        local kind ln tok t
        while IFS=$'\t' read -r kind ln tok; do
            if [ "$kind" = ERR ]; then
                fail "$file:$ln: $tok"
                continue
            fi
            t=$tok
            if [[ $t =~ ^(.*):[0-9]+$ ]]; then t=${BASH_REMATCH[1]}; fi
            [ -n "$t" ] || continue
            case $t in
                *'<'* | *'>'* | *'{'* | *'}'* | *'*'*) continue ;;
                *://* | @* | '~'* | '$'* | /* | -* | origin/* | refs/*) continue ;;
            esac
            if [[ $t == .. || $t == ../* || $t == */../* || $t == */.. ]]; then
                checked=$((checked + 1))
                fail "$file:$ln: $t has a .. segment; write the path from the repository root"
                continue
            fi
            case $t in
                */*) ;;
                Justfile | LICENSE | LICENSE-ADDENDUM | .gitignore | .gitattributes) ;;
                *.*)
                    ext=${t##*.}
                    case $ext in
                        md | json | toml | sh | yml | yaml | lock | rs | ts | txt) ;;
                        *) continue ;;
                    esac
                    ;;
                *) continue ;;
            esac
            checked=$((checked + 1))
            if [ -e "$t" ]; then
                printf '  ok       %s:%s  %s\n' "$file" "$ln" "$t"
            else
                missing+=("$ln"$'\t'"$t")
            fi
        done < <(tr -d '\r' < "$file" | awk "$AWK_EXTRACT")

        # Pass 2: one git call decides which of the missing paths are ignored.
        if [ ${#missing[@]} -gt 0 ]; then
            probes=""
            for want in "${missing[@]}"; do
                t=${want#*$'\t'}
                prefix=""
                rest=$t
                while [[ $rest == */* ]]; do
                    prefix="$prefix${rest%%/*}/"
                    rest=${rest#*/}
                    probes="$probes$prefix"$'\n'
                done
                probes="$probes$t"$'\n'"${t%/}/"$'\n'
            done
            ignored=$(printf '%s' "$probes" | git check-ignore --stdin 2>/dev/null | tr -d '\r' || true)
            for want in "${missing[@]}"; do
                ln=${want%%$'\t'*}
                t=${want#*$'\t'}
                shallowest=""
                prefix=""
                rest=$t
                while [[ $rest == */* ]]; do
                    prefix="$prefix${rest%%/*}/"
                    rest=${rest#*/}
                    case $'\n'"$ignored"$'\n' in *$'\n'"$prefix"$'\n'*) shallowest=$prefix; break ;; esac
                done
                if [ -z "$shallowest" ]; then
                    for probe in "$t" "${t%/}/"; do
                        case $'\n'"$ignored"$'\n' in *$'\n'"$probe"$'\n'*) shallowest=$probe; break ;; esac
                    done
                fi
                if [ -z "$shallowest" ]; then
                    fail "$file:$ln: $t does not exist"
                    continue
                fi
                s=${shallowest%/}
                case $s in */*) parent=${s%/*} ;; *) parent=. ;; esac
                if [ -d "$parent" ]; then
                    printf '  ok       %s:%s  %s (git-ignored build output)\n' "$file" "$ln" "$t"
                else
                    fail "$file:$ln: $t does not exist, and $parent, above its ignored part, does not either"
                fi
            done
        fi

        if [ "$checked" -eq 0 ]; then
            fail "$file: no repository path was checked; the extraction rules or the file have gone stale"
        fi
    done
}

# --- the self-test ----------------------------------------------------------
# Every case is a seeded defect (or a deliberate non-defect) run through the
# real script against a throwaway repository, so an edit that turns a failure
# branch into a no-op turns a case red.
selftest() {
    local fx n=0 bad=0 out rc
    fx=$(mktemp -d)
    # shellcheck disable=SC2064  # expand now: $fx is local and gone when the trap fires
    trap "rm -rf '$fx'" EXIT
    (
        cd "$fx"
        git init -q .
        printf 'target\nplugins/*/bin/\n' > .gitignore
        mkdir -p scripts docs plugins/real x
        : > README.md; : > Justfile; : > LICENSE; : > scripts/ok.sh; : > scripts/check.sh; : > x/y.sh
    )
    local bt='`'

    # case NAME WANT(pass|fail) PATTERN: the document is read from stdin.
    indent() { sed 's/^/             /'; }
    case_run() {
        local name=$1 want=$2 pat=$3
        n=$((n + 1))
        cat > "$fx/docs/case.md"
        set +e
        out=$(bash "$self" --root "$fx" docs/case.md 2>&1)
        rc=$?
        set -e
        if { [ "$want" = pass ] && [ "$rc" -ne 0 ]; } || { [ "$want" = fail ] && [ "$rc" -eq 0 ]; }; then
            echo "  FAIL     $name: wanted $want, exit was $rc"; printf '%s\n' "$out" | indent
            bad=$((bad + 1)); return
        fi
        if [ -n "$pat" ] && ! printf '%s\n' "$out" | grep -qF -e "$pat"; then
            echo "  FAIL     $name: output lacks: $pat"; printf '%s\n' "$out" | indent
            bad=$((bad + 1)); return
        fi
        echo "  ok       $name"
    }

    case_run "existing path passes"            pass "docs/case.md:1" <<< "See ${bt}scripts/ok.sh${bt}."
    case_run "dead path fails"                 fail "docs/case.md:1: docs/nope.md does not exist" <<< "See ${bt}docs/nope.md${bt}."
    case_run "bare name at the root passes"    pass "" <<< "${bt}README.md${bt}"
    case_run "bare name not at the root fails" fail "check.sh does not exist" <<< "${bt}check.sh${bt} and ${bt}README.md${bt}"
    case_run "listed extensionless name checked" pass "Justfile" <<< "${bt}Justfile${bt}"
    case_run "listed extensionless name, absent file, fails" fail "LICENSE-ADDENDUM does not exist" <<< "${bt}LICENSE-ADDENDUM${bt} ${bt}README.md${bt}"
    case_run "unlisted extensionless name not checked" pass "" <<< "${bt}Justfiel${bt} ${bt}README.md${bt}"
    case_run "line suffix on a bare root file"   pass "README.md" <<< "${bt}README.md:164${bt}"
    case_run "line suffix on a nested file"      pass "scripts/ok.sh" <<< "${bt}scripts/ok.sh:12${bt}"
    case_run "line suffix does not hide a dead path" fail "docs/nope.md does not exist" <<< "${bt}docs/nope.md:3${bt} ${bt}README.md${bt}"
    case_run "path inside a command line is checked" pass "scripts/ok.sh" <<< "Run ${bt}bash scripts/ok.sh --flag${bt}."
    case_run "dead path inside a command line fails" fail "scripts/gone.sh does not exist" <<< "Run ${bt}bash scripts/gone.sh${bt} ${bt}README.md${bt}"
    case_run "placeholder, URL, git ref and flag skipped" pass "" <<< "${bt}<plugin dir>/bin/${bt} ${bt}https://example.com/a/b${bt} ${bt}origin/main${bt} ${bt}@types/node${bt} ${bt}--out=x/y${bt} ${bt}\$HOME/x${bt} ${bt}a/*${bt} ${bt}README.md${bt}"
    case_run "ignored build output, parent present" pass "git-ignored build output" <<< "${bt}target/release/${bt} ${bt}README.md${bt}"
    case_run "ignored dir under a real plugin"   pass "git-ignored build output" <<< "${bt}plugins/real/bin/${bt}"
    case_run "ignored dir without its trailing slash" pass "git-ignored build output" <<< "${bt}plugins/real/bin${bt}"
    case_run "misspelt dir above an ignored bin/ fails" fail "above its ignored part" <<< "${bt}plugins/typo/bin/${bt} ${bt}README.md${bt}"
    case_run ".. in each of the four positions fails" fail "has a .. segment" <<< "${bt}..${bt} ${bt}../x${bt} ${bt}x/../y${bt} ${bt}x/..${bt} ${bt}README.md${bt}"
    case_run ".. that would resolve to a real file fails" fail "docs/../README.md has a .. segment" <<< "${bt}docs/../README.md${bt}"
    case_run "backtick fence skipped" pass "" <<EOF
${bt}${bt}${bt}
docs/nope.md
${bt}${bt}${bt}
${bt}README.md${bt}
EOF
    case_run "tilde fence skipped" pass "" <<EOF
~~~
${bt}docs/nope.md${bt}
~~~
${bt}README.md${bt}
EOF
    case_run "three-backtick fence nested in four" pass "" <<EOF
${bt}${bt}${bt}${bt}markdown
${bt}${bt}${bt}
${bt}docs/nope.md${bt}
${bt}${bt}${bt}
${bt}docs/also-nope.md${bt}
${bt}${bt}${bt}${bt}
${bt}README.md${bt}
EOF
    case_run "a tilde line does not close a backtick fence" pass "" <<EOF
${bt}${bt}${bt}
~~~
${bt}docs/nope.md${bt}
${bt}${bt}${bt}
${bt}README.md${bt}
EOF
    case_run "a backtick line does not close a tilde fence" pass "" <<EOF
~~~
${bt}${bt}${bt}
${bt}docs/nope.md${bt}
~~~
${bt}README.md${bt}
EOF
    case_run "a path after a closed fence is checked" fail "docs/nope.md does not exist" <<EOF
${bt}${bt}${bt}
x
${bt}${bt}${bt}
${bt}docs/nope.md${bt}
EOF
    case_run "unclosed fence fails with its opening line" fail "docs/case.md:2: unclosed code fence" <<EOF
${bt}README.md${bt}
${bt}${bt}${bt}
${bt}docs/nope.md${bt}
EOF
    case_run "odd backtick count fails" fail "docs/case.md:1: odd number of backticks" <<< "${bt}README.md${bt} and a stray ${bt}"
    case_run "a file with no checked path fails" fail "no repository path was checked" <<< "Only prose and ${bt}just check${bt}."
    case_run "CRLF input is read like LF" pass "scripts/ok.sh" < <(printf '%s\r\n%s\r\n%s\r\n%s\r\n' '```' 'docs/nope.md' '```' "${bt}scripts/ok.sh${bt}")

    # Two input files: one clean, one with nothing checked, must fail as a whole.
    n=$((n + 1))
    printf '%s\n' "${bt}README.md${bt}" > "$fx/docs/a.md"
    printf '%s\n' "prose only" > "$fx/docs/b.md"
    set +e
    out=$(bash "$self" --root "$fx" docs/a.md docs/b.md 2>&1); rc=$?
    set -e
    if [ "$rc" -ne 0 ] && printf '%s\n' "$out" | grep -qF "docs/b.md: no repository path was checked"; then
        echo "  ok       one empty file fails the run"
    else
        echo "  FAIL     one empty file fails the run (exit $rc)"; printf '%s\n' "$out" | indent
        bad=$((bad + 1))
    fi

    # A missing input file fails rather than being skipped.
    n=$((n + 1))
    set +e
    out=$(bash "$self" --root "$fx" docs/not-there.md 2>&1); rc=$?
    set -e
    if [ "$rc" -ne 0 ] && printf '%s\n' "$out" | grep -qF "input file does not exist"; then
        echo "  ok       a missing input file fails"
    else
        echo "  FAIL     a missing input file fails (exit $rc)"; bad=$((bad + 1))
    fi

    echo
    if [ "$bad" -ne 0 ]; then
        echo "$bad of $n self-test cases failed." >&2
        exit 1
    fi
    echo "All $n self-test cases passed."
}

# --- entry ------------------------------------------------------------------

root=$(cd "$(dirname "$0")/.." && pwd)
files=()
while [ $# -gt 0 ]; do
    case $1 in
        --selftest) selftest; exit 0 ;;
        --root) [ $# -ge 2 ] || { echo "usage: check-doc-paths.sh [--root DIR] [FILE...] | --selftest" >&2; exit 1; }
                root=$(cd "$2" && pwd); shift 2 ;;
        -*) echo "usage: check-doc-paths.sh [--root DIR] [FILE...] | --selftest" >&2; exit 1 ;;
        *) files+=("$1"); shift ;;
    esac
done
[ ${#files[@]} -gt 0 ] || files=(docs/PLUGIN_GUIDELINES.md HACK.md)

run_check "$root" "${files[@]}"

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "$failures problem(s) in the documents' paths." >&2
    exit 1
fi
echo "All documented paths exist."
`````

- [ ] **Step 2b: Byte-identity check.** The file ends with exactly one newline. `sha256sum scripts/check-doc-paths.sh` must print `1ff46758aafebf60148a1926ac09ff9f8ab0070eab2f08465f0d9228ca48ad59`. If it does not, `diff` your file against the block above and fix the transcription; do not edit the block.

- [ ] **Step 3: Mark it executable and run the self-test (the oracle).**

```bash
chmod +x scripts/check-doc-paths.sh
bash scripts/check-doc-paths.sh --selftest
```

Expected: 31 lines starting `  ok       `, a blank line, then `All 31 self-test cases passed.`, exit 0. If any case prints `FAIL`, STOP and report the output; do not edit a case.

- [ ] **Step 4: Run the repository's own gates on the new file.**

```bash
shellcheck scripts/check-doc-paths.sh
addlicense -check -f LICENSE-HEADER.tmpl -l PolyForm-Noncommercial-1.0.0 -c "Costas Kirgoussios" scripts/check-doc-paths.sh
```

Expected: both print nothing and exit 0.

- [ ] **Step 5: Run the mutation harness, which proves each self-test case would go red.** Save the harness below as `mutate.py` in the directory `${TMPDIR:-/tmp}/check-doc-paths-mutation` (expand the variable to an absolute path first, for example with `echo "${TMPDIR:-/tmp}/check-doc-paths-mutation"`, and give the file-writing tool that absolute path), copy the script next to it, and run it there (it rewrites a copy named `mut.sh`, never the repository file).

`````python
import subprocess, sys, shutil, os
src = open('check-doc-paths.sh').read()
M = [
 ("no :digits strip",            'if [[ $t =~ ^(.*):[0-9]+$ ]]; then t=${BASH_REMATCH[1]}; fi', ':'),
 ("no .. rule",                  'if [[ $t == .. || $t == ../* || $t == */../* || $t == */.. ]]; then', 'if false; then'),
 ("ignored: any parent ok",      'if [ -d "$parent" ]; then', 'if true; then'),
 ("no closed name list",         'Justfile | LICENSE | LICENSE-ADDENDUM | .gitignore | .gitattributes) ;;', 'NOSUCHNAME) ;;'),
 ("fences never open",           'infence = 1; fch = ch; flen = k; fline = NR', 'fch = ch; flen = k; fline = NR'),
 ("closing fence ignores length",'k >= flen && rest', '1 && rest'),
 ("closing fence ignores char",  'ch == fch && k >= flen', '1 && k >= flen'),
 ("no zero-checked guard",       'if [ "$checked" -eq 0 ]; then', 'if false; then'),
 ("no URL/ref/flag skip",        '*://* | @* | \'~\'* | \'$\'* | /* | -* | origin/* | refs/*) continue ;;', 'NOSUCH) continue ;;'),
 ("no placeholder-char skip",    "*'<'* | *'>'* | *'{'* | *'}'* | *'*'*) continue ;;", 'NOSUCH) continue ;;'),
 ("span not split on whitespace",'split(parts[i], toks, /[ \\t]+/)', 'split(parts[i], toks, /\\001/)'),
 ("no odd-backtick error",       'if (n % 2 == 1) { print', 'if (0) { print'),
 ("no CR strip",                 "tr -d '\\r' < \"$file\" | awk", 'cat "$file" | awk'),
 ("no unclosed-fence error",     'END { if (infence) print', 'END { if (0) print'),
 ("tokens with / not candidates",'                */*) ;;\n                Justfile', '                */*) continue ;;\n                Justfile'),
 ("no ext list (md only)",       'md | json | toml | sh | yml | yaml | lock | rs | ts | txt) ;;', 'md) ;;'),
 ("missing input not an error",  'fail "$file: input file does not exist"', ': "$file"'),
 ("one git call result ignored", "ignored=$(printf '%s' \"$probes\" | git check-ignore --stdin 2>/dev/null | tr -d '\\r' || true)", 'ignored='),
]
survived = 0
for name, old, new in M:
    if src.count(old) != 1:
        print(f"BAD MUTANT (pattern occurs {src.count(old)}x): {name}"); survived += 1; continue
    open('mut.sh','w').write(src.replace(old, new))
    r = subprocess.run(['bash','mut.sh','--selftest'], capture_output=True, text=True)
    red = [l.strip() for l in (r.stdout+r.stderr).splitlines() if l.strip().startswith('FAIL')]
    status = "KILLED " if r.returncode != 0 else "SURVIVED"
    if r.returncode == 0: survived += 1
    print(f"{status} {name:34s} -> {len(red)} red case(s)" + (f": {red[0][:90]}" if red else ""))
os.remove('mut.sh')
print("\nsurvivors:", survived)
sys.exit(1 if survived else 0)
`````

```bash
M="${TMPDIR:-/tmp}/check-doc-paths-mutation"
mkdir -p "$M"
# ... save the harness above as "$M/mutate.py" here ...
cp scripts/check-doc-paths.sh "$M/check-doc-paths.sh"
(cd "$M" && python3 mutate.py)
rm -rf "$M"
```

Expected: 18 lines starting `KILLED`, then `survivors: 0`, exit 0. A `SURVIVED` or `BAD MUTANT` line means a self-test case no longer pins its rule: STOP and report it.

- [ ] **Step 6: Show the script failing on the real `HACK.md` (the pre-fix state).**

```bash
bash scripts/check-doc-paths.sh HACK.md
```

Expected: exit 1, exactly three `FAIL` lines: `HACK.md:17: claude-code/example/ does not exist`, `HACK.md:32: check-package-versions.sh does not exist`, `HACK.md:78: gen-marketplaces.sh does not exist`, and the line `3 problem(s) in the documents' paths.` (Line numbers match the file as it is before Task 3.)

- [ ] **Step 7: Commit.**

```bash
git add scripts/check-doc-paths.sh
git update-index --chmod=+x scripts/check-doc-paths.sh
git commit -m "feat(scripts): check-doc-paths, a check that documented paths exist

mlc checks Markdown links and not paths written in backticks, so HACK.md named
a directory that no longer exists. This reads the inline backticked tokens of
the contributor documents and requires each path to exist. Carries a
--selftest of 31 seeded-defect cases.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Fix `HACK.md`

**Tier:** Haiku. **Files:** Modify only `HACK.md`.

**Interfaces:** Consumes the script from Task 2. Produces a `HACK.md` that passes it.

- [ ] **Step 1: State verification.** `grep -n 'claude-code/example/' HACK.md` prints one line (17); `grep -n 'Task 6' HACK.md` prints one line (28); `bash scripts/check-doc-paths.sh HACK.md` exits 1 with three `FAIL` lines.

- [ ] **Step 2: Apply four edits** with the Edit tool; each `old_string` occurs exactly once.

Edit 1. Old: `` `claude-code/example/` is the template `` New: `` `claude-code/plugins/example/` is the template ``

Edit 2. Old: `4. Run the Task 6 gate command — ` New: `4. Run the gate command — `

Edit 3. Old: `` add its line to `check-package-versions.sh` and its footprint/smoke `` New: `` add its line to `scripts/check-package-versions.sh` and its footprint/smoke ``

Edit 4. Old: `` `gen-marketplaces.sh --check` plus the host wiring checks `` New: `` `bash scripts/gen-marketplaces.sh --check` plus the host wiring checks ``

- [ ] **Step 3: Run the oracle.**

```bash
bash scripts/check-doc-paths.sh HACK.md
```

Expected: exit 0, one `ok` line per path, ending `All documented paths exist.`

- [ ] **Step 4: Verify nothing else changed.** `git diff --stat` shows `HACK.md | 8 ++++----` or similar: four changed lines, nothing else. `typos HACK.md` prints nothing.

- [ ] **Step 5: Commit.**

```bash
git add HACK.md
git commit -m "docs(hack): fix a dead path, plan jargon and two bare script names

claude-code/example/ is claude-code/plugins/example/; 'Task 6 gate command'
is just the gate command; check-package-versions.sh and gen-marketplaces.sh
are written with their scripts/ path so scripts/check-doc-paths.sh can check
them.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The guidelines document and the README links

**Tier:** Haiku. **Files:** Create `docs/PLUGIN_GUIDELINES.md`; modify only `README.md`.

**Why:** Every statement below was read from the sources during planning (the exemplar scripts, workflows and manifests; see the spec's sections 1 and 5). The two `Verification status: not yet walked through.` lines are deliberate: Task 6 replaces them after the walkthrough, and Task 7 fails the branch if any remain.

**Interfaces:** Consumes the script (Task 2) and the fixed `HACK.md` (Task 3). Produces the document that Task 6 tests.

- [ ] **Step 1: State verification.** `ls docs/PLUGIN_GUIDELINES.md` fails; `grep -n 'More details and plugin guidelines' README.md` prints line 164; `bash scripts/check-doc-paths.sh HACK.md` exits 0.

- [ ] **Step 2: Create `docs/PLUGIN_GUIDELINES.md` with exactly this content.**

`````markdown
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
`````

- [ ] **Step 2b: Byte-identity check.** The file ends with exactly one newline. `sha256sum docs/PLUGIN_GUIDELINES.md` must print `c9d7ad410f6b265b6339ef253251a432b343614c01e48c18d2d0b9994d0af063`. If it does not, `diff` against the block above and fix the transcription.

- [ ] **Step 3: Edit `README.md`** (two edits).

Edit 1, the documentation index. Old:

```
- [User Guide (PDF)](docs/USER_GUIDE.pdf) — install and use the marketplace and its plugins, with verified examples.
```

New:

```
- [User Guide (PDF)](docs/USER_GUIDE.pdf) — install and use the marketplace and its plugins, with verified examples.
- [Plugin guidelines](docs/PLUGIN_GUIDELINES.md) — add a plugin: the wiring checklist for each kind and the standards the checks enforce.
- [`HACK.md`](HACK.md) — maintainer notes: releases and tags, the footprint override, marketplace URLs, adding an agent.
```

Edit 2, the promise. Old:

```
*More details and plugin guidelines will be added soon.*
```

New:

```
Adding a plugin? Start with the [plugin guidelines](docs/PLUGIN_GUIDELINES.md). Releases, tags and the footprint override are in [`HACK.md`](HACK.md).
```

- [ ] **Step 4: Run the oracles.**

```bash
bash scripts/check-doc-paths.sh
# then LINKS (see "Shared commands")
typos docs/PLUGIN_GUIDELINES.md README.md
```

Expected: the first prints `ok` lines for both documents and ends `All documented paths exist.` (exit 0); LINKS exits 0 (mlc reports no broken link); `typos` prints nothing.

- [ ] **Step 5: Commit.**

```bash
git add docs/PLUGIN_GUIDELINES.md README.md
git commit -m "docs: add the plugin guidelines and link them from the README

Wiring checklists for the four plugin kinds plus the standards the checks
enforce. Replaces the README's 'will be added soon' line with links to the
guidelines and to HACK.md. Two kinds are not yet walked through; the status
lines say so until the walkthrough.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Wire the check and its self-test

**Tier:** Haiku. **Files:** Modify only `Justfile`, `.github/workflows/ci.yml`, `lefthook.yml`.

**Interfaces:** Consumes the script (Task 2) and both documents (Tasks 3, 4), which must exist before the check is wired, because it fails on a missing input.

- [ ] **Step 1: State verification.** `bash scripts/check-doc-paths.sh` exits 0; `grep -c doc-paths Justfile .github/workflows/ci.yml lefthook.yml` prints `0` for each.

- [ ] **Step 2: `Justfile`, two edits.**

Edit 1, add two recipes after the runner-failure self-test. Old:

```
# Self-test of the CI runner-failure re-run decision against a stubbed gh.
rerun-runner-failures-test:
    bash scripts/rerun-runner-failures.sh --selftest
```

New:

```
# Self-test of the CI runner-failure re-run decision against a stubbed gh.
rerun-runner-failures-test:
    bash scripts/rerun-runner-failures.sh --selftest

# Check that every repository path the contributor documents name exists.
# `links` (mlc) checks Markdown links only, not paths written in backticks.
doc-paths:
    bash scripts/check-doc-paths.sh

# Self-test of that check: seeded defects run through the real script against a throwaway repository.
doc-paths-test:
    bash scripts/check-doc-paths.sh --selftest
```

Edit 2, add both to `check`. Old: `bump-opencode-plugin-test rerun-runner-failures-test agy-wiring` New: `bump-opencode-plugin-test rerun-runner-failures-test doc-paths-test doc-paths agy-wiring`

- [ ] **Step 3: `.github/workflows/ci.yml`, one edit** in the `wiring` job. Old:

```
      - name: Self-test the runner-failure re-run decision
        run: bash scripts/rerun-runner-failures.sh --selftest
```

New:

```
      - name: Self-test the runner-failure re-run decision
        run: bash scripts/rerun-runner-failures.sh --selftest

      # mlc checks Markdown links, not paths written in backticks, so a document
      # could name a path that no longer exists and fail nothing. This requires
      # every path the contributor documents name to exist, and the self-test
      # proves the check rejects each seeded defect.
      - name: Self-test the documented-paths check
        run: bash scripts/check-doc-paths.sh --selftest

      - name: Check the paths named in the contributor documents
        run: bash scripts/check-doc-paths.sh
```

- [ ] **Step 4: `lefthook.yml`, one edit** in the `pre-push` block. Old:

```
    license-headers:
      run: scripts/check-license-headers.sh
```

New:

```
    license-headers:
      run: scripts/check-license-headers.sh
    doc-paths:
      run: bash scripts/check-doc-paths.sh
```

- [ ] **Step 5: Run the oracles.**

```bash
just doc-paths-test
just doc-paths
python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in ('.github/workflows/ci.yml','lefthook.yml')]; print('yaml ok')"
just --list | grep -c 'doc-paths'
```

Expected: `All 31 self-test cases passed.`; `All documented paths exist.`; `yaml ok`; `2`. Then `just --dry-run check 2>&1 | grep -c doc-paths` prints a number of at least `2`.

- [ ] **Step 6: Commit.**

```bash
git add Justfile .github/workflows/ci.yml lefthook.yml
git commit -m "ci: run the documented-paths check and its self-test

New just recipes doc-paths and doc-paths-test, both in the check recipe, a
step for each in the Plugin Wiring job, and a pre-push hook.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The acceptance oracle, then fix the guidelines

**Tier:** Sonnet for the two walkthrough agents (an independent reader; this is where capability matters). The driver (main thread) judges the reports and edits the document, never the agents.

**Files:** The walkthroughs run on scratch branches and change nothing that is kept. The driver modifies only `docs/PLUGIN_GUIDELINES.md` and, if the document needs it, nothing else.

**Why:** The document is a claim about what a new plugin must touch. Reading scripts is not proof; following the document and watching the gates is. Do NOT show the walkthrough agents the spec, this plan or any author notes: they get the brief below and the repository only.

**Interfaces:** Consumes the committed document (Task 4) and the wired check (Task 5). Produces a list of gaps, the fixed document, and the final `Verification status:` lines.

- [ ] **Step 1: Record state.** `git rev-parse HEAD` (note it as `BASE`), `git status --short` empty. Record the long-running dependency: the document's footprint row needs the plugin binaries built first. Measure what is already staged with `ls claude-code/plugins/*/bin antigravity/plugins/*/bin opencode/plugins/*/bin 2>/dev/null` and note which plugins lack a binary; walkthrough A's brief caps the whole footprint step at 40 minutes, so a missing binary costs at most that.

- [ ] **Step 2: Dispatch walkthrough A (kind B, a skills-only plugin on a Cargo host, in a NEW family, which exercises the hardest rows).** Fresh Sonnet subagent, foreground, with exactly this brief:

> You are testing a document, not writing code. Work in `/home/user/Development/Rust/aiplugins`. First run `git switch -c scratch/oracle-skills-only` from the current commit; you may make local commits on that branch; commit everything you create before you finish and leave the working tree clean; NEVER push, merge, or switch away from it, and NEVER touch the branch you started on.
>
> Read `docs/PLUGIN_GUIDELINES.md` and nothing else in `docs/`. Your task: add one throwaway skills-only plugin for Claude Code named `zz-oracle-cc`, in a new family `zz-oracle`, with a single skill `zz-oracle-skill` whose `SKILL.md` has valid frontmatter and two lines of body. Do exactly what the document tells you for that kind, in the order it gives, and use ONLY the document for knowledge of what to change; do not use any prior knowledge of this repository's layout.
>
> After the edits, run the gates the document names for what you changed (including the ones in its section 5), except `just check` as a whole. Run `just footprint-regen` only after building the binaries the document says to build first. Budget: if a staged binary already exists under `<plugin folder>/bin/` for a plugin, do not rebuild it; run each long command in the background and wait; if the builds and the footprint run have not finished within 40 minutes of wall-clock time, stop, leave them unfinished, and report `footprint run: not completed` under part (6) with the last output line. An unfinished footprint run is an acceptable result; a hung one is not.
>
> Report in this exact shape: (1) a numbered list of every file you created or changed, each tagged `DOC` if the document named it or `GATE` if you had to change it because a gate or error told you to; (2) for every `GATE` item, the command, the exact failing output line, and what you changed; (3) every command you ran that the document did not name; (4) every sentence of the document that was wrong, ambiguous, or that you could not do as written, quoted; (5) the final result of every gate you ran, with its exit code; (6) the gates you could not run and why. Do not summarise away failures. Do not fix the document.

- [ ] **Step 2b: Driver check between the walkthroughs.** Run `git branch --show-current` (expect `scratch/oracle-skills-only`), `git branch --list scratch/oracle-skills-only` (expect one line), `git ls-files claude-code/plugins/zz-oracle-cc | head -3` (expect at least one tracked file) and `git status --short` (expect no output). If agent A returned an error, or the tree is not clean, do not dispatch B: read what A reported, then on `scratch/oracle-skills-only` run `git status --short`; commit whatever is there with `git add -A && git commit -m "oracle: leftovers"` (this branch is thrown away in Step 7, and the commit leaves the tree clean for B). If A did not create the plugin at all, record that as a gap in the document (the brief or a row failed it) and still continue with B.

- [ ] **Step 3: Dispatch walkthrough B (kind A, a Rust front, no-build gates only).** After A has returned and its branch exists, a second fresh Sonnet subagent with this brief:

> You are testing a document, not writing code. Work in `/home/user/Development/Rust/aiplugins`. Run `git switch -c scratch/oracle-rust-front docs/plugin-guidelines` (the working tree is clean, so this works from whichever branch you find yourself on); commit everything you create before you finish and leave the working tree clean; local commits are allowed on it; NEVER push, merge, or touch any other branch.
>
> Read `docs/PLUGIN_GUIDELINES.md` and nothing else in `docs/`. Add one throwaway Rust front for Claude Code named `zz-rust-oracle-cc`: a crate with one `[[bin]]` target `zz-rust-oracle-cc-mcp` whose `main` prints one line and exits, an `.mcp.json` that names that binary as `${CLAUDE_PLUGIN_ROOT}/bin/zz-rust-oracle-cc-mcp`, and the host manifest. Do exactly what the document says for a Rust front, using ONLY the document for what to change.
>
> Run only the gates that do not build or run a plugin binary and do not need a footprint measurement: the registry and marketplace generators and their `--check` modes, `just marketplace`, `just wiring`, `just versions`, `bash scripts/check-release-manifest.sh`, `just doc-paths`. Do NOT run `just footprint-regen`, `just footprint`, `just smoke`, or any `build-*` recipe, and say so in the report as not run.
>
> Report in the same six-part shape as the first brief. Do not fix the document.

- [ ] **Step 4: Spot-check three "what fails" claims by reversing one edit at a time** on `scratch/oracle-skills-only` (the driver runs these, not an agent). Walkthrough B left the working tree on its own branch, so first run `git switch scratch/oracle-skills-only` and confirm with `git branch --show-current`. For each, make the single reversal, run the command, record the exit code and the first failing line, then `git checkout -- .` to restore:
  1. Remove the new folder from the `exclude` array in the root `Cargo.toml`; run `cargo metadata --no-deps --format-version 1 >/dev/null`. Record what happens (the document says "Cargo cannot load the workspace").
  2. Remove the `zz-oracle` line from `docs/site/families.txt`; run `just site-check`. The document says `scripts/check-site.sh` fails on a family-set difference.
  3. Remove the `zz-oracle-cc` row from `scripts/check-package-versions.sh`; run `bash scripts/check-package-versions.sh`. The document says nothing fails.

- [ ] **Step 5: Judge and fix.** First run `git checkout -- .` (restore the last spot-check), then `git switch docs/plugin-guidelines` and confirm with `git branch --show-current`; the document fixes must be committed here, never on a scratch branch, which Step 7 deletes. For every item in the two reports under (1) `GATE`, (3) and (4), every non-zero exit in (5), every gate listed in (6) except those a brief told the agent not to run, and for any spot-check whose result differs from the document, decide: the document is wrong or incomplete (fix it), or the agent erred (note why). Edit `docs/PLUGIN_GUIDELINES.md` only. For each wording that a spot-check measured, replace the unmeasured wording with the measured one. Then replace the two lines `Verification status: not yet walked through.` (kinds A and B) with the truth, in this form and no other claims:
  - kind A: `Verification status: checked by a walkthrough of the gates that need no build; the footprint probe, the bundle smoke and the binary build were not run.`
  - kind B: `Verification status: checked by a walkthrough, including the footprint run and the release-plumbing rows for a new family.` (Adjust to what was actually run; if the footprint run or a row was not run, say that instead.)

- [ ] **Step 6: Re-run the document's own gates.**

```bash
bash scripts/check-doc-paths.sh
# then LINKS (see "Shared commands")
typos docs/PLUGIN_GUIDELINES.md
grep -c 'not yet walked through' docs/PLUGIN_GUIDELINES.md
```

Expected: `All documented paths exist.`; LINKS exit 0; no typos output; `0`.

- [ ] **Step 7: Delete the scratch branches** (`git branch -D scratch/oracle-skills-only scratch/oracle-rust-front`) after confirming neither was pushed (`git branch -r | grep scratch` prints nothing) and that `git worktree list` shows only the main worktree. Build output under `target/` is gitignored and may stay.

- [ ] **Step 8: Commit** (only if the document changed).

```bash
git add docs/PLUGIN_GUIDELINES.md
git commit -m "docs(guidelines): fold the walkthrough results

Gaps the fresh-context walkthroughs found, wording replaced by what the
spot-checks measured, and the verification status of kinds A and B.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Whole-branch verification and hand-off

**Tier:** main thread. **Files:** Modify only the spec's status line.

- [ ] **Step 1: Run the branch's gates.**

```bash
just doc-paths-test
just doc-paths
just spellcheck
bash scripts/check-license-headers.sh
shellcheck scripts/check-doc-paths.sh
# and LINKS (see "Shared commands")
git status --short
git diff --stat main...HEAD
```

Expected: `All 31 self-test cases passed.`; `All documented paths exist.`; LINKS and the others exit 0 with no findings; `git status --short` prints nothing (a walkthrough that left files behind shows here). The diff stat names exactly: the spec, this plan, `scripts/check-doc-paths.sh`, `HACK.md`, `docs/PLUGIN_GUIDELINES.md`, `README.md`, `Justfile`, `.github/workflows/ci.yml`, `lefthook.yml`. Any other file is a stop-and-explain.

- [ ] **Step 2: Run what `just check` runs that is cheap here,** and say which parts were not run: `just versions`, `just wiring`, `just marketplace`, `just qwen-marketplace`, `just agy-marketplace`, `just opencode-wiring`, `just agy-wiring`, `just footprint-budgets`, `just skills-bundles`, `just skills-release-wiring`, `just site-check`. Expected: each exits 0, because this branch changes no plugin. Measured before any task: `versions`, `wiring`, `marketplace`, `qwen-marketplace`, `agy-marketplace`, `opencode-wiring`, `agy-wiring`, `footprint-budgets` and `skills-release-wiring` each exit 0; `skills-bundles` and `site-check` were not measured in advance. Record any failure with its output; a failure on an untouched area is an anomaly to capture, not to fix here.

- [ ] **Step 3: Update the spec's status line** to say the plan was executed: Old `Status: design approved by the owner on 2026-11-19; spec awaiting owner review. No plan yet.` New `Status: design approved by the owner on 2026-11-19; implemented on branch docs/plugin-guidelines per docs/superpowers/plans/2026-11-19-plugin-guidelines.md.` Commit as `docs(spec): record that the plan was executed`.

- [ ] **Step 4: Update the memory index** `project_plugin_guidelines_execution.md` with every commit SHA of this plan's tasks and the resume point `Task 8`.

- [ ] **Step 5: Report to the owner and STOP.** Do not push, open a pull request, or merge. Say what was run and what was not (Windows and macOS are `not measured`; the Rust front walkthrough ran without a build). Ask whether to open the pull request.

---

### Task 8: AGY-CAPSTONE and AGY-TEST-AUDIT

**Tier:** main thread. **Why:** the owner's rules require a convergent adversarial review of the COMMITTED implementation (executable code and tests) before the plan is declared complete, then an audit of the test safety net. The owner has previously chosen to run these in a fresh session; do not run them inline unless the owner says so.

- [ ] **Step 1: Record the resume point in memory** (`Task 8: capstone then test-audit on the range main..HEAD of docs/plugin-guidelines`) so a fresh session can start from it.
- [ ] **Step 2: In a fresh session, invoke `clavity:agy-capstone`** on the committed range `main..docs/plugin-guidelines`, rounds until green. The reviewable code is `scripts/check-doc-paths.sh` and its 31 cases; the documents are context. Verify every finding and every suggested fix by measurement before folding.
- [ ] **Step 3: Invoke `clavity:agy-test-audit`** once the capstone is green, on the same range: would the 31 cases catch the next regression? Ask for the covered set and compute the complement yourself; settle each claimed gap by mutating the script (the harness in Task 2 is the tool).
- [ ] **Step 4: Add the ledger rows** to `docs/agy-capstone-ledger.md` and `docs/agy-test-audit-ledger.md` in the format the previous rows use, as the earlier branches did, and commit them.
- [ ] **Step 5: Ask the owner** whether to open the pull request.

---

## Self-review

- **Spec coverage.** Deliverables (spec section 4): the guidelines (Task 4, proved in 6), the script and self-test (2), the README edit (4), the `HACK.md` fixes (3), the wiring (5). Verification oracle with two throwaways and the "checked by walkthrough or reading" markings (6). Licensing (2, Global Constraints). The Windows process-count constraint (the script makes one `git check-ignore` call per input file, in the code of Task 2). Out-of-scope items are not touched: no `.github/CONTRIBUTING.md`, no new README content, no change to any existing check script.
- **Placeholder scan.** The only deliberate temporary text is `Verification status: not yet walked through.`, which Task 6 Step 5 replaces and Task 6 Step 6 counts to zero.
- **Type and name consistency.** The script is `scripts/check-doc-paths.sh` everywhere; recipes are `doc-paths` and `doc-paths-test`; the case count is 31 in Tasks 2, 5 and 7; the HACK.md fix count is four in the spec amendment and in Task 3.
- **Dry run.** Tasks 1 to 5 were executed literally in a throwaway worktree during planning, by extracting this plan's blocks and applying its Old/New edits with a script: all 7 + 4 + 2 + 4 edit pairs matched exactly one place; the script and the guidelines were byte-identical to their stated checksums; the self-test printed 31 `ok` lines and `All 31 self-test cases passed.`; shellcheck and addlicense exited 0; the mutation harness printed 18 `KILLED` and `survivors: 0`; the pre-fix `HACK.md` run printed exactly the three `FAIL` lines and `3 problem(s)`; after the fixes `HACK.md | 8 ++++----`; the link check exited 0; `just doc-paths-test` and `just doc-paths` passed; both new CI steps landed in the `wiring` job right after the runner-failure self-test; `lefthook validate` printed `All good`. The worktree and its branch were then removed. Tasks 6, 7 and 8 were not dry-run.
- **Not proved by this plan:** Windows and macOS behaviour of the script (the self-test passes under four awk implementations on Linux only); the footprint, bundle-smoke and binary-build parts of the Rust front walkthrough; the OpenCode kinds, which are checked by reading only and say so.
