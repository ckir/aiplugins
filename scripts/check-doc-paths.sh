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
# FILE paths are read relative to ROOT (default: the repository holding this
# script), not to the caller's directory. Git's global ignore files are switched
# off for the ignore test (core.excludesFile=/dev/null), so a run on a machine
# with a global ignore decides as CI does; .gitignore files and
# .git/info/exclude still apply.
#
# Rules, in the order they are applied to each token of each backticked span
# (a span is split on whitespace, so `bash scripts/x.sh` checks scripts/x.sh):
#   1. A trailing :<digits> or :<digits>-<digits> is removed first
#      (README.md:164 and README.md:164-170 -> README.md).
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
# outside a fence (a stray backtick shifts every span after it), a run of two or
# more adjacent backticks outside a fence (a double-backtick span is not
# parsed, and its path would go unchecked), and an input file in which no path
# was checked.
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
    if (index(line, bt bt) > 0) { print "ERR\t" NR "\ta run of two or more backticks outside a fence; write each path in single backticks"; next }
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
            if [[ $t =~ ^(.*):[0-9]+(-[0-9]+)?$ ]]; then t=${BASH_REMATCH[1]}; fi
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
            ignored=$(printf '%s' "$probes" | git -c core.excludesFile=/dev/null check-ignore --stdin 2>/dev/null | tr -d '\r' || true)
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
    local fx am n=0 bad=0 out rc
    fx=$(mktemp -d)
    am=$(mktemp -d)   # a user's global git configuration, as the ambient-ignore cases fake it
    # shellcheck disable=SC2064  # expand now: $fx and $am are local and gone when the trap fires
    trap "rm -rf '$fx' '$am'" EXIT
    (
        cd "$fx"
        git init -q .
        printf 'target\nplugins/*/bin/\n' > .gitignore
        mkdir -p scripts docs plugins/real x
        : > README.md; : > Justfile; : > LICENSE; : > scripts/ok.sh; : > scripts/check.sh; : > x/y.sh
        : > scripts/a..b.sh   # a .. inside a name, which is not a .. segment
    )
    local bt='`'

    indent() { sed 's/^/             /'; }
    # judge NAME WANT(pass|fail) RC OUTPUT PATTERN...: the exit class must match, every
    # PATTERN must appear in the output, and a PATTERN written !TEXT must NOT appear.
    # An empty PATTERN asserts nothing.
    judge() {
        local name=$1 want=$2 jrc=$3 jout=$4 pat
        shift 4
        if { [ "$want" = pass ] && [ "$jrc" -ne 0 ]; } || { [ "$want" = fail ] && [ "$jrc" -eq 0 ]; }; then
            echo "  FAIL     $name: wanted $want, exit was $jrc"; printf '%s\n' "$jout" | indent
            bad=$((bad + 1)); return
        fi
        for pat in "$@"; do
            [ -n "$pat" ] || continue
            if [ "${pat:0:1}" = '!' ]; then
                if printf '%s\n' "$jout" | grep -qF -e "${pat:1}"; then
                    echo "  FAIL     $name: output has: ${pat:1}"; printf '%s\n' "$jout" | indent
                    bad=$((bad + 1)); return
                fi
            elif ! printf '%s\n' "$jout" | grep -qF -e "$pat"; then
                echo "  FAIL     $name: output lacks: $pat"; printf '%s\n' "$jout" | indent
                bad=$((bad + 1)); return
            fi
        done
        echo "  ok       $name"
    }

    # case_run NAME WANT PATTERN...: the document is read from stdin.
    case_run() {
        local name=$1 want=$2
        shift 2
        n=$((n + 1))
        cat > "$fx/docs/case.md"
        set +e
        out=$(bash "$self" --root "$fx" docs/case.md 2>&1)
        rc=$?
        set -e
        judge "$name" "$want" "$rc" "$out" "$@"
    }

    # cli_run NAME WANT ARG...: the script is run with exactly these arguments, from
    # $cli_cwd (default: the fixture) and as $cli_script (default: this script); the
    # patterns to judge are taken from the array cli_pats.
    cli_pats=()
    cli_run() {
        local name=$1 want=$2
        shift 2
        n=$((n + 1))
        set +e
        out=$(cd "${cli_cwd:-$fx}" && bash "${cli_script:-$self}" "$@" 2>&1)
        rc=$?
        set -e
        judge "$name" "$want" "$rc" "$out" ${cli_pats[@]+"${cli_pats[@]}"}
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
    case_run "line range on a nested file"      pass "scripts/ok.sh" <<< "${bt}scripts/ok.sh:12-15${bt}"
    case_run "line range does not hide a dead path" fail "docs/nope.md does not exist" <<< "${bt}docs/nope.md:3-5${bt} ${bt}README.md${bt}"
    case_run "path inside a command line is checked" pass "scripts/ok.sh" <<< "Run ${bt}bash scripts/ok.sh --flag${bt}."
    case_run "dead path inside a command line fails" fail "scripts/gone.sh does not exist" <<< "Run ${bt}bash scripts/gone.sh${bt} ${bt}README.md${bt}"
    case_run "placeholder, URL, git ref and flag skipped" pass "" <<< "${bt}<plugin dir>/bin/${bt} ${bt}https://example.com/a/b${bt} ${bt}origin/main${bt} ${bt}@types/node${bt} ${bt}--out=x/y${bt} ${bt}\$HOME/x${bt} ${bt}a/*${bt} ${bt}README.md${bt}"
    case_run "ignored build output, parent present" pass "git-ignored build output" <<< "${bt}target/release/${bt} ${bt}README.md${bt}"
    case_run "ignored dir under a real plugin"   pass "git-ignored build output" <<< "${bt}plugins/real/bin/${bt}"
    case_run "ignored dir without its trailing slash" pass "git-ignored build output" <<< "${bt}plugins/real/bin${bt}"
    case_run "misspelt dir above an ignored bin/ fails" fail "above its ignored part" <<< "${bt}plugins/typo/bin/${bt} ${bt}README.md${bt}"
    # One case per position: in one shared document the first failing token satisfies
    # the exit code and the message, and a later position could be dropped unseen.
    case_run ".. alone fails"                  fail "docs/case.md:1: .. has a .. segment" <<< "${bt}..${bt} ${bt}README.md${bt}"
    case_run "leading ../ fails"               fail "docs/case.md:1: ../x has a .. segment" <<< "${bt}../x${bt} ${bt}README.md${bt}"
    case_run "inner /../ fails"                fail "docs/case.md:1: x/../y has a .. segment" <<< "${bt}x/../y${bt} ${bt}README.md${bt}"
    case_run "trailing /.. fails even when it resolves" fail "docs/case.md:1: x/.. has a .. segment" <<< "${bt}x/..${bt} ${bt}README.md${bt}"
    # The sole token counts as checked: the run fails once, for the .., not twice.
    case_run ".. that would resolve to a real file fails" fail "docs/../README.md has a .. segment" "!no repository path was checked" <<< "${bt}docs/../README.md${bt}"
    # Each shape the skip rule drops is shown to be dropped: were it checked, the
    # token (absent from the fixture) would fail the run.
    case_run "refs/ token skipped"             pass "" <<< "${bt}refs/heads/main${bt} ${bt}README.md${bt}"
    case_run "~ token skipped"                 pass "" <<< "${bt}~/gone/x.md${bt} ${bt}README.md${bt}"
    case_run "absolute path skipped, not tested against the host" pass "" <<< "${bt}/gone-by-design/x.md${bt} ${bt}README.md${bt}"
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
    case_run "a double-backtick span fails rather than hiding its path" fail "docs/case.md:1: a run of two or more backticks" <<< "${bt}README.md${bt} and ${bt}${bt}scripts/gone.sh${bt}${bt}"

    # A user's global git ignore must not excuse a dead path: with it, the check would
    # pass on that machine and fail in CI. Two routes: core.excludesFile in the global
    # configuration, and git's default $XDG_CONFIG_HOME/git/ignore.
    printf '*.md\n' > "$am/ignore"
    printf '[core]\n\texcludesFile = %s\n' "$am/ignore" > "$am/gitconfig"
    mkdir -p "$am/xdg/git"; printf '*.md\n' > "$am/xdg/git/ignore"
    GIT_CONFIG_GLOBAL="$am/gitconfig" case_run "a global core.excludesFile does not excuse a dead path" fail "docs/nope.md does not exist" <<< "See ${bt}docs/nope.md${bt}."
    GIT_CONFIG_GLOBAL=/dev/null XDG_CONFIG_HOME="$am/xdg" case_run "the default global ignore does not excuse a dead path" fail "docs/nope.md does not exist" <<< "See ${bt}docs/nope.md${bt}."
    case_run "a file with no checked path fails" fail "no repository path was checked" <<< "Only prose and ${bt}just check${bt}."
    case_run "CRLF input is read like LF" pass "scripts/ok.sh" < <(printf '%s\r\n%s\r\n%s\r\n%s\r\n' '```' 'docs/nope.md' '```' "${bt}scripts/ok.sh${bt}")

    # Added by the test audit. Each case below fails against a one-line mutant of the
    # rule it pins (the mutants are listed in docs/agy-test-audit-ledger.md's row).
    case_run "an existing directory passes" pass "docs/case.md:1  scripts/" "docs/case.md:1  plugins/real" "!git-ignored" <<< "${bt}scripts/${bt} ${bt}plugins/real${bt}"
    case_run "an unlisted or uppercase bare extension is not checked" pass "" <<< "${bt}v1.2.3${bt} ${bt}script.py${bt} ${bt}GONE.MD${bt} ${bt}README.md${bt}"
    case_run "a multi-dot bare root name is checked" fail "foo.bar.md does not exist" <<< "${bt}foo.bar.md${bt} ${bt}README.md${bt}"
    # Each of < > { } skips on its own, so each token carries exactly one of them.
    case_run "each placeholder bracket skips alone" pass "" <<< "${bt}x/<y${bt} ${bt}x/y>${bt} ${bt}x/{y${bt} ${bt}x/y}${bt} ${bt}plugins/{name}/plugin.json${bt} ${bt}README.md${bt}"
    case_run "a .. inside a name is not a .. segment" pass "scripts/a..b.sh" <<< "${bt}main..HEAD${bt} ${bt}...${bt} ${bt}scripts/a..b.sh${bt}"
    case_run "a fence indented up to three spaces is a fence" pass "" <<EOF
   ${bt}${bt}${bt}
   ${bt}docs/nope.md${bt}
   ${bt}${bt}${bt}
${bt}README.md${bt}
EOF
    case_run "four spaces of indent is not a fence" fail "docs/case.md:2: odd number of backticks" <<EOF
${bt}README.md${bt}
    ${bt}${bt}${bt}
EOF
    case_run "a line-start triple-backtick span is not a fence" fail "a run of two or more backticks" <<< "${bt}${bt}${bt}scripts/gone.sh${bt}${bt}${bt} ${bt}README.md${bt}"
    case_run "two tildes do not open a fence" fail "docs/nope.md does not exist" "!unclosed code fence" <<EOF
~~old~~
${bt}docs/nope.md${bt}
${bt}README.md${bt}
EOF
    case_run "a longer closing fence with trailing spaces closes" pass "" < <(printf '%s\n' '```' "${bt}docs/nope.md${bt}" '````  ' "${bt}README.md${bt}")
    case_run "a fence line with an info string does not close" pass "" < <(printf '%s\n' '```' '```bash' "${bt}docs/nope.md${bt}" '```' "${bt}README.md${bt}")
    # Every missing path is judged on its own: an ignored one, a dead one, and a dead
    # one that is the tail of an ignored one (plugins/real/bin/ ends in bin/).
    case_run "missing paths in one file are judged independently" fail "target/release/ (git-ignored build output)" "plugins/real/bin/ (git-ignored build output)" "docs/nope.md does not exist" "bin/ does not exist" <<< "${bt}target/release/${bt} ${bt}plugins/real/bin/${bt} ${bt}docs/nope.md${bt} ${bt}bin/${bt}"

    # A document that names the whole closed list of bare extensions and names, in a
    # repository holding none of them: each one must be reported.
    local em="$am/empty"
    mkdir -p "$em/docs"; git init -q "$em"
    printf '%s\n' "${bt}gone.md${bt} ${bt}gone.json${bt} ${bt}gone.toml${bt} ${bt}gone.sh${bt} ${bt}gone.yml${bt} ${bt}gone.yaml${bt} ${bt}gone.lock${bt} ${bt}gone.rs${bt} ${bt}gone.ts${bt} ${bt}gone.txt${bt}" > "$em/docs/ext.md"
    printf '%s\n' "${bt}Justfile${bt} ${bt}LICENSE${bt} ${bt}LICENSE-ADDENDUM${bt} ${bt}.gitignore${bt} ${bt}.gitattributes${bt}" > "$em/docs/names.md"
    cli_pats=("gone.md does not exist" "gone.json does not exist" "gone.toml does not exist" "gone.sh does not exist" "gone.yml does not exist" "gone.yaml does not exist" "gone.lock does not exist" "gone.rs does not exist" "gone.ts does not exist" "gone.txt does not exist")
    cli_run "every listed bare extension is checked" fail --root "$em" docs/ext.md
    cli_pats=("Justfile does not exist" "LICENSE does not exist" "LICENSE-ADDENDUM does not exist" ".gitignore does not exist" ".gitattributes does not exist")
    cli_run "every listed bare name is checked" fail --root "$em" docs/names.md

    # The command line.
    cli_pats=("usage:")
    cli_run "an unknown flag prints usage" fail --bogus
    cli_run "--root without a directory prints usage" fail --root
    printf '%s\n' "${bt}docs/nope.md${bt} ${bt}README.md${bt}" > "$fx/docs/dead.md"
    cli_pats=("docs/not-there.md: input file does not exist" "docs/dead.md:1: docs/nope.md does not exist")
    cli_run "a missing first file does not stop the second being checked" fail --root "$fx" docs/not-there.md docs/dead.md
    # The missing list is per file: a path found missing in the first file is not
    # reported again under the second.
    printf '%s\n' "${bt}target/release/${bt}" > "$fx/docs/first.md"
    printf '%s\n' "${bt}README.md${bt}" > "$fx/docs/second.md"
    cli_pats=("docs/first.md:1  target/release/ (git-ignored build output)" "!docs/second.md:1  target/release/")
    cli_run "a missing path does not carry into the next file" pass --root "$fx" docs/first.md docs/second.md

    # With no FILE and no --root the script checks its own repository's two documents,
    # whatever the caller's directory: a copy placed in the fixture, run from outside it.
    cp "$self" "$fx/scripts/check-doc-paths.sh"
    printf '%s\n' "${bt}README.md${bt}" > "$fx/docs/PLUGIN_GUIDELINES.md"
    printf '%s\n' "${bt}Justfile${bt}" > "$fx/HACK.md"
    cli_pats=("docs/PLUGIN_GUIDELINES.md:1  README.md" "HACK.md:1  Justfile")
    cli_cwd=$am cli_script="$fx/scripts/check-doc-paths.sh" cli_run "the default FILE list and ROOT are the script's own repository" pass

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
