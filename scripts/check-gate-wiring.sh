#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

#
# Check that every gate script stays wired in.
#
# A gate that nothing runs fails nothing. Dropping `doc-paths` from the `check`
# recipe, from ci.yml or from lefthook.yml leaves every other check green, so the
# gate is lost without anyone seeing it go. This script fails when that happens.
#
# Usage: check-gate-wiring.sh [--root DIR] | --selftest
#
# A GATE is every scripts/check-*.sh under ROOT (default: the repository holding
# this script). Each one must be:
#   1. run by a Justfile recipe, and that recipe must be reached by `just check`
#      (named in its dependencies, directly or through other recipes);
#   2. run in CI: a workflow names `scripts/<gate>` in a run line, or runs a
#      `just <recipe>` that reaches the gate (`just check` reaches all of them).
# A line that is only a comment never counts, in the Justfile or in a workflow.
#
# Two lists below are closed and checked both ways:
#   EXCEPTIONS      a gate that deliberately skips `just` or `ci`, with its reason.
#                   An exception for a gate that IS wired there is stale and fails.
#   LEFTHOOK_GATES  gates lefthook.yml must also run, by `scripts/<gate>`.
#
# Limits, by design: it proves a gate is named where it runs, not that the step
# runs on the right branch or that its flags are right. A recipe header with a
# parameter default that contains a colon is not understood. Only scripts named
# check-*.sh are gates.
set -euo pipefail

# GATE|SKIPS|REASON, SKIPS a comma list of: just, ci
EXCEPTIONS=(
    "check-license-headers.sh|just,ci|enforced by the lefthook pre-commit and pre-push hooks; it needs addlicense, which CI does not install"
)
LEFTHOOK_GATES=(check-doc-paths.sh check-license-headers.sh)

self=$(cd "$(dirname "$0")" && pwd)/$(basename "$0")

failures=0
fail() { echo "  FAIL     $1" >&2; failures=$((failures + 1)); }

# Comment-only lines removed: a mention in a comment must not count as wiring.
no_comments() { sed -e '/^[[:space:]]*#/d' "$1"; }

# Justfile as TAB-separated records: "H<TAB>recipe<TAB>deps" for a header and
# "B<TAB>recipe<TAB>line" for each body line. Comments, variable assignments
# (:=), attributes and blank lines are dropped.
AWK_JUST=$(cat <<'AWK'
/^[[:space:]]*$/ { next }
/^[[:space:]]*#/ { next }
/^[^[:space:]]/ {
    line = $0
    if (line ~ /:=/) { cur = ""; next }
    if (line ~ /^\[/) { next }
    i = index(line, ":")
    if (i == 0) { cur = ""; next }
    split(substr(line, 1, i - 1), h, /[ \t]+/)
    cur = h[1]
    sub(/^@/, "", cur)
    print "H\t" cur "\t" substr(line, i + 1)
    next
}
{ if (cur != "") print "B\t" cur "\t" $0 }
AWK
)

# closure FILE SEED...: SEED recipes plus everything they depend on, one per line.
closure() {
    local tsv=$1 seen="" todo r d
    shift
    todo=$*
    while [ -n "${todo# }" ]; do
        r=${todo%% *}
        todo=${todo#"$r"}; todo=${todo# }
        [ -n "$r" ] || continue
        case " $seen " in *" $r "*) continue ;; esac
        seen="$seen $r"
        for d in $(awk -F'\t' -v r="$r" '$1 == "H" && $2 == r { print $3 }' "$tsv" | tr -d '&' ); do
            case $d in -* | *[!A-Za-z0-9_-]*) continue ;; esac
            todo="$todo $d"
        done
    done
    # shellcheck disable=SC2086  # word splitting is the point: one recipe per line
    printf '%s\n' $seen
}

# Recipes named after `just` on any non-comment workflow line.
ci_recipes() {
    local f
    for f in "$1"/.github/workflows/*.yml; do
        [ -e "$f" ] || continue
        no_comments "$f"
    done | awk '{
        seen = 0
        for (i = 1; i <= NF; i++) {
            if (seen) print $i
            if ($i == "just") seen = 1
        }
    }'
}

in_list() { case " $2 " in *" $1 "*) return 0 ;; esac; return 1; }

run_check() {
    local root=$1 tsv ci_text
    tsv=$(mktemp)
    ci_text=$(mktemp)
    check_files "$root" "$tsv" "$ci_text"
    rm -f "$tsv" "$ci_text"
}

check_files() {
    local root=$1 tsv=$2 ci_text=$3 f gate g_re recipes r skips entry egate eskips ereason
    local check_set ci_set ci_seed gates=0 in_just in_check in_ci listed gate_names=""

    if [ ! -f "$root/Justfile" ]; then
        fail "Justfile: not found under $root"
        return 0
    fi
    awk "$AWK_JUST" "$root/Justfile" > "$tsv"
    for f in "$root"/.github/workflows/*.yml; do
        [ -e "$f" ] || continue
        no_comments "$f"
    done > "$ci_text"

    if ! awk -F'\t' '$1 == "H" && $2 == "check" { f = 1 } END { exit !f }' "$tsv"; then
        fail "Justfile: no \`check\` recipe, so nothing can be reached by \`just check\`"
        return 0
    fi
    check_set=" $(closure "$tsv" check | tr '\n' ' ')"
    ci_seed=$(ci_recipes "$root" | tr '\n' ' ')
    # shellcheck disable=SC2086  # the seed is a space-separated list of recipe names
    ci_set=" $(closure "$tsv" $ci_seed | tr '\n' ' ')"

    for entry in ${EXCEPTIONS[@]+"${EXCEPTIONS[@]}"}; do
        egate=${entry%%|*}
        ereason=${entry#*|*|}
        if [ ! -f "$root/scripts/$egate" ]; then
            fail "exception for $egate: there is no scripts/$egate"
        fi
        if [ -z "$ereason" ]; then
            fail "exception for $egate has no reason"
        fi
    done

    for f in "$root"/scripts/check-*.sh; do
        [ -e "$f" ] || continue
        gate=$(basename "$f")
        gates=$((gates + 1))
        gate_names="$gate_names $gate"
        g_re="scripts/${gate//./\\.}([^A-Za-z0-9_.-]|\$)"

        skips=""
        for entry in ${EXCEPTIONS[@]+"${EXCEPTIONS[@]}"}; do
            if [ "${entry%%|*}" = "$gate" ]; then
                eskips=${entry#*|}; eskips=${eskips%%|*}
                skips=",$eskips,"
            fi
        done

        recipes=$(g_re="$g_re" awk -F'\t' '$1 == "B" && $3 ~ ENVIRON["g_re"] { print $2 }' "$tsv" | sort -u | tr '\n' ' ')
        in_just=0; in_check=0; in_ci=0
        [ -n "${recipes// /}" ] && in_just=1
        for r in $recipes; do in_list "$r" "$check_set" && in_check=1; done
        if grep -Eq "$g_re" "$ci_text"; then
            in_ci=1
        else
            for r in $recipes; do in_list "$r" "$ci_set" && in_ci=1; done
        fi

        case $skips in
            *,just,*)
                if [ "$in_just" -eq 1 ] && [ "$in_check" -eq 1 ]; then
                    fail "$gate: the exception skipping \`just\` is stale; it runs in a recipe reached by \`just check\`, so delete the skip"
                fi ;;
            *)
                if [ "$in_just" -eq 0 ]; then
                    fail "$gate: not run by any Justfile recipe"
                elif [ "$in_check" -eq 0 ]; then
                    fail "$gate: runs only in recipe(s) ${recipes% }, none of which \`just check\` reaches"
                fi ;;
        esac
        case $skips in
            *,ci,*)
                if [ "$in_ci" -eq 1 ]; then
                    fail "$gate: the exception skipping \`ci\` is stale; CI already runs it, so delete the skip"
                fi ;;
            *)
                if [ "$in_ci" -eq 0 ]; then
                    fail "$gate: not run in CI (no workflow names scripts/$gate or runs a \`just\` recipe that reaches it)"
                fi ;;
        esac
        if [ "$in_just" -eq 1 ] && [ "$in_check" -eq 1 ] && [ "$in_ci" -eq 1 ]; then
            printf '  ok       %s  (recipe: %s)\n' "$gate" "${recipes% }"
        fi
    done

    if [ "$gates" -eq 0 ]; then
        fail "no scripts/check-*.sh found under $root; the gate list or the layout has gone stale"
    fi

    for listed in ${LEFTHOOK_GATES[@]+"${LEFTHOOK_GATES[@]}"}; do
        if ! in_list "$listed" "$gate_names"; then
            fail "LEFTHOOK_GATES names $listed, but there is no scripts/$listed"
        elif [ ! -f "$root/lefthook.yml" ] || ! no_comments "$root/lefthook.yml" | grep -Eq "scripts/${listed//./\\.}([^A-Za-z0-9_.-]|\$)"; then
            fail "lefthook.yml does not run scripts/$listed"
        fi
    done
}

# --- the self-test ----------------------------------------------------------
# Every case is a seeded defect (or a deliberate non-defect) in a throwaway tree
# of two gates, so an edit that turns a failure branch into a no-op turns a case red.
selftest() {
    local fx n=0 bad=0 out rc
    fx=$(mktemp -d)
    # shellcheck disable=SC2064  # expand now: $fx is local and gone when the trap fires
    trap "rm -rf '$fx'" EXIT

    # tree: a good tree with two gates, both fully wired. Cases overwrite one file.
    tree() {
        rm -rf "$fx/t"; mkdir -p "$fx/t/scripts" "$fx/t/.github/workflows"
        : > "$fx/t/scripts/check-a.sh"; : > "$fx/t/scripts/check-b.sh"
        printf 'check: a-test b\n\na-test:\n    bash scripts/check-a.sh\n\nb:\n    bash scripts/check-b.sh\n' > "$fx/t/Justfile"
        printf 'jobs:\n  x:\n    steps:\n      - run: bash scripts/check-a.sh\n      - run: just b\n' > "$fx/t/.github/workflows/ci.yml"
        printf 'pre-push:\n  commands:\n    a:\n      run: bash scripts/check-a.sh\n' > "$fx/t/lefthook.yml"
    }
    # case NAME WANT(pass|fail) PATTERN...: judge run_check against $fx/t. A PATTERN
    # written !TEXT must NOT appear. EXCEPTIONS and LEFTHOOK_GATES come from the caller.
    case_run() {
        local name=$1 want=$2 pat
        shift 2
        n=$((n + 1))
        set +e
        out=$( ( run_check "$fx/t"; [ "$failures" -eq 0 ] ) 2>&1 )
        rc=$?
        set -e
        if { [ "$want" = pass ] && [ "$rc" -ne 0 ]; } || { [ "$want" = fail ] && [ "$rc" -eq 0 ]; }; then
            echo "  FAIL     $name: wanted $want, exit was $rc"; printf '%s\n' "$out" | sed 's/^/             /'
            bad=$((bad + 1)); return
        fi
        for pat in "$@"; do
            if [ "${pat:0:1}" = '!' ]; then
                if printf '%s\n' "$out" | grep -qF -e "${pat:1}"; then
                    echo "  FAIL     $name: output has: ${pat:1}"; printf '%s\n' "$out" | sed 's/^/             /'
                    bad=$((bad + 1)); return
                fi
            elif ! printf '%s\n' "$out" | grep -qF -e "$pat"; then
                echo "  FAIL     $name: output lacks: $pat"; printf '%s\n' "$out" | sed 's/^/             /'
                bad=$((bad + 1)); return
            fi
        done
        echo "  ok       $name"
    }
    EXCEPTIONS=()
    LEFTHOOK_GATES=(check-a.sh)

    tree
    case_run "a fully wired tree passes" pass "ok       check-a.sh" "ok       check-b.sh"

    tree; printf 'check: a-test\n\na-test:\n    bash scripts/check-a.sh\n' > "$fx/t/Justfile"
    case_run "a gate no Justfile recipe runs fails" fail "check-b.sh: not run by any Justfile recipe"

    tree; printf 'check: a-test\n\na-test:\n    bash scripts/check-a.sh\n\nb:\n    bash scripts/check-b.sh\n' > "$fx/t/Justfile"
    case_run "a recipe outside the check aggregate fails" fail "check-b.sh: runs only in recipe(s) b, none of which \`just check\` reaches"

    tree; printf 'check: a-test group\n\ngroup: b\n\na-test:\n    bash scripts/check-a.sh\n\nb:\n    bash scripts/check-b.sh\n' > "$fx/t/Justfile"
    case_run "a recipe reached through another recipe passes" pass "ok       check-b.sh"

    tree; printf 'check: a-test b\n\na-test:\n    bash scripts/check-a.sh\n\nb:\n    # bash scripts/check-b.sh\n    echo nothing\n' > "$fx/t/Justfile"
    case_run "a Justfile comment is not a run" fail "check-b.sh: not run by any Justfile recipe"

    tree; printf 'jobs:\n  x:\n    steps:\n      - run: bash scripts/check-a.sh\n' > "$fx/t/.github/workflows/ci.yml"
    case_run "a gate no workflow runs fails" fail "check-b.sh: not run in CI"

    tree; printf 'jobs:\n  x:\n    steps:\n      - run: bash scripts/check-a.sh\n      # - run: bash scripts/check-b.sh\n' > "$fx/t/.github/workflows/ci.yml"
    case_run "a workflow comment is not a run" fail "check-b.sh: not run in CI"

    tree; printf 'jobs:\n  x:\n    steps:\n      - run: bash scripts/check-a.sh\n      - run: just b2\n' > "$fx/t/.github/workflows/ci.yml"
    case_run "a recipe whose name only starts like ours does not count" fail "check-b.sh: not run in CI"

    tree; printf 'jobs:\n  x:\n    steps:\n      - run: bash scripts/check-a.sh\n      - run: bash scripts/check-b.sh.old\n' > "$fx/t/.github/workflows/ci.yml"
    case_run "a script name that only starts like ours does not count" fail "check-b.sh: not run in CI"

    tree; printf 'jobs:\n  x:\n    steps:\n      - run: just check\n' > "$fx/t/.github/workflows/ci.yml"
    case_run "just check in CI reaches every gate in the aggregate" pass "ok       check-a.sh" "ok       check-b.sh"

    tree; printf 'jobs:\n  x:\n    steps:\n      - run: just --quiet b\n      - run: bash scripts/check-a.sh\n' > "$fx/t/.github/workflows/ci.yml"
    case_run "just with a flag before the recipe passes" pass "ok       check-b.sh"

    tree; rm "$fx/t/lefthook.yml"
    case_run "a lefthook gate with no lefthook.yml fails" fail "lefthook.yml does not run scripts/check-a.sh"

    tree; printf 'pre-push:\n  commands:\n    a:\n      # run: bash scripts/check-a.sh\n      run: echo hi\n' > "$fx/t/lefthook.yml"
    case_run "a lefthook comment is not a run" fail "lefthook.yml does not run scripts/check-a.sh"

    tree; printf 'pre-push:\n  commands:\n    a:\n      run: bash scripts/check-a.sh.old\n' > "$fx/t/lefthook.yml"
    case_run "a lefthook script name that only starts like ours does not count" fail "lefthook.yml does not run scripts/check-a.sh"

    tree; LEFTHOOK_GATES=(check-gone.sh)
    case_run "a LEFTHOOK_GATES entry with no script fails" fail "LEFTHOOK_GATES names check-gone.sh"
    LEFTHOOK_GATES=(check-a.sh)

    tree; : > "$fx/t/scripts/check-c.sh"
    EXCEPTIONS=("check-c.sh|just,ci|reason given")
    case_run "an exception excuses a gate wired nowhere" pass "!FAIL"
    EXCEPTIONS=()
    case_run "the same gate without the exception fails twice" fail "check-c.sh: not run by any Justfile recipe" "check-c.sh: not run in CI"

    tree; EXCEPTIONS=("check-b.sh|ci|already in CI")
    case_run "an exception for a gate CI already runs is stale" fail "check-b.sh: the exception skipping \`ci\` is stale"
    EXCEPTIONS=("check-b.sh|just|already in the aggregate")
    case_run "an exception for a gate the aggregate already reaches is stale" fail "check-b.sh: the exception skipping \`just\` is stale"

    tree; EXCEPTIONS=("check-gone.sh|ci|no such gate")
    case_run "an exception for a missing gate fails" fail "exception for check-gone.sh: there is no scripts/check-gone.sh"

    tree; EXCEPTIONS=("check-b.sh|ci|")
    case_run "an exception with no reason fails" fail "exception for check-b.sh has no reason"
    EXCEPTIONS=()

    tree; rm "$fx/t/scripts/check-a.sh" "$fx/t/scripts/check-b.sh"; LEFTHOOK_GATES=()
    case_run "a tree with no gate fails closed" fail "no scripts/check-*.sh found"
    LEFTHOOK_GATES=(check-a.sh)

    tree; printf 'a-test:\n    bash scripts/check-a.sh\n' > "$fx/t/Justfile"
    case_run "a Justfile with no check recipe fails" fail "no \`check\` recipe"

    tree; rm "$fx/t/Justfile"
    case_run "no Justfile fails" fail "Justfile: not found"

    # The exit code of the real script, not just the verdict run_check reaches: a copy
    # with this repository's two lists swapped for the fixture's, run against the tree.
    awk '/^EXCEPTIONS=\(/ { print "EXCEPTIONS=()"; skip = 1; next }
         skip && /^\)/ { skip = 0; next }
         skip { next }
         /^LEFTHOOK_GATES=/ { print "LEFTHOOK_GATES=(check-a.sh)"; next }
         { print }' "$self" > "$fx/entry.sh"
    entry_case() {
        local name=$1 want_rc=$2 want_text=$3
        n=$((n + 1))
        set +e
        out=$(bash "$fx/entry.sh" --root "$fx/t" 2>&1)
        rc=$?
        set -e
        if [ "$rc" -ne "$want_rc" ] || ! printf '%s\n' "$out" | grep -qF -e "$want_text"; then
            echo "  FAIL     $name: wanted exit $want_rc and \"$want_text\", exit was $rc"; printf '%s\n' "$out" | sed 's/^/             /'
            bad=$((bad + 1)); return
        fi
        echo "  ok       $name"
    }
    tree
    entry_case "the script exits 0 on a wired tree" 0 "Every gate script is wired."
    tree; printf 'jobs:\n  x:\n    steps:\n      - run: bash scripts/check-a.sh\n' > "$fx/t/.github/workflows/ci.yml"
    entry_case "the script exits 1 on an unwired tree and counts the problem" 1 "1 wiring problem(s)."
    entry_case_usage() {
        n=$((n + 1))
        set +e
        out=$(bash "$fx/entry.sh" --bogus 2>&1); rc=$?
        set -e
        if [ "$rc" -ne 1 ] || ! printf '%s\n' "$out" | grep -qF "usage:"; then
            echo "  FAIL     an unknown flag prints usage: exit was $rc"; bad=$((bad + 1)); return
        fi
        echo "  ok       an unknown flag prints usage"
    }
    entry_case_usage

    echo
    if [ "$bad" -ne 0 ]; then
        echo "$bad of $n self-test cases failed." >&2
        exit 1
    fi
    echo "All $n self-test cases passed."
}

# --- entry ------------------------------------------------------------------

root=$(cd "$(dirname "$0")/.." && pwd)
while [ $# -gt 0 ]; do
    case $1 in
        --selftest) selftest; exit 0 ;;
        --root) [ $# -ge 2 ] || { echo "usage: check-gate-wiring.sh [--root DIR] | --selftest" >&2; exit 1; }
                root=$(cd "$2" && pwd); shift 2 ;;
        *) echo "usage: check-gate-wiring.sh [--root DIR] | --selftest" >&2; exit 1 ;;
    esac
done

run_check "$root"

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "$failures wiring problem(s)." >&2
    exit 1
fi
echo "Every gate script is wired."
