#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Checks the rust-toolchain-scaffold skill source under shared/devtools/.
# The generic devtools gate (check-devtools-skills.sh) checks every skill's frontmatter,
# its references/ links, its OpenCode command and the emitted copies. This one checks what
# is particular to the Rust scaffold: every template is a .tmpl file (so this repository's
# own gates, cargo and lefthook never treat a template as a live file), every file the
# skill names exists, the CI template pins its actions, the test crate stays outside the
# workspace of the repository it guards, and this repository keeps pinning LF for the
# skill's files (the `.gitattributes` line that covers every devtools skill). The end-to-end test (scripts/test-rust-scaffold.sh) proves the scaffold works.
# `--mutation-test` seeds a throwaway copy of the tree, applies one defect at a time and
# requires this gate to fail with the message that names that defect.
set -euo pipefail
mode=${1:-}
# Clear the arguments so nothing sourced later reads $1.
set --
cd "$(dirname "$0")/.."

fail=0
bad() {
    echo "  FAIL  $1" >&2
    fail=1
}

skill=shared/devtools/skills/rust-toolchain-scaffold
cmd=shared/devtools/commands/rust-toolchain-scaffold.md
tpl=$skill/assets/template
pin='shared/devtools/skills/** text eol=lf'

# The templates the skill cannot work without, relative to $tpl.
REQUIRED=(
    .gitattributes.tmpl
    .github/CODEOWNERS.tmpl
    .github/workflows/ci.yml.tmpl
    .gitignore.tmpl
    .lefthook/pre-push/check.sh.tmpl
    .lefthook/rc.sh.tmpl
    deny.toml.tmpl
    docs/accepted-boundaries.md.tmpl
    justfile.tmpl
    lefthook.yml.tmpl
    rust-toolchain.toml.tmpl
    toolchain-tests/.gitignore.tmpl
    toolchain-tests/Cargo.toml.tmpl
    toolchain-tests/src/lib.rs.tmpl
    toolchain-tests/tests/common/mod.rs.tmpl
    toolchain-tests/tests/gate.rs.tmpl
    toolchain-tests/tests/toolchain.rs.tmpl
    typos.toml.tmpl
)

# The frontmatter is the lines between the opening `---` on line 1 and the next `---`.
frontmatter() { awk 'NR == 1 { next } /^---$/ { exit } { print }' "$1" 2>/dev/null; }

# One defect at a time on a throwaway copy of the tree; the gate must fail with the
# message that names it. Without this nothing re-runs the failure branches, so an edit
# could turn one into a no-op and the gate would stay green.
mutation_test() {
    local pristine work total=0 failed=0
    local sk=$skill/SKILL.md
    # (global: the EXIT trap runs after this function's locals are gone)
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    pristine=$tmp/pristine
    work=$tmp/work
    mkdir -p "$pristine"
    tar -cf - scripts shared/devtools .gitattributes | tar -xf - -C "$pristine"

    # case_run <name> <want-rc> <want-message-substring> <mutation-shell>
    case_run() {
        local name=$1 want_rc=$2 want_msg=$3 mutation=$4 rc=0 out
        total=$((total + 1))
        rm -rf "$work"
        mkdir -p "$work"
        tar -cf - -C "$pristine" . | tar -xf - -C "$work"
        if ! (cd "$work" && eval "$mutation") >/dev/null 2>&1; then
            echo "  FAIL  mutation '$name': the mutation itself did not apply" >&2
            failed=$((failed + 1))
            return
        fi
        out=$(bash "$work/scripts/check-rust-scaffold.sh" 2>&1) || rc=$?
        if [ "$rc" -ne "$want_rc" ] || ! grep -qF -- "$want_msg" <<<"$out"; then
            echo "  FAIL  mutation '$name': exit $rc (want $want_rc), wanted: $want_msg" >&2
            failed=$((failed + 1))
        fi
    }

    case_run "baseline is green" 0 "Rust toolchain scaffold consistent." "true"
    case_run "a template without the .tmpl suffix" 1 "template file without the .tmpl suffix" \
        "cp $tpl/justfile.tmpl $tpl/justfile"
    case_run "a dangling assets reference" 1 "dangling reference assets/nope.md" \
        "printf '%s\n' 'See \`assets/nope.md\`.' >> $sk"
    case_run "a dangling scripts reference" 1 "dangling reference scripts/nope.sh" \
        "printf '%s\n' 'See \`scripts/nope.sh\`.' >> $sk"
    case_run "an action pinned to a tag" 1 "is not pinned to a 40-hex SHA" \
        "sed -E 's/@[0-9a-f]{40}/@v7/' $tpl/.github/workflows/ci.yml.tmpl > t.tmp && mv t.tmp $tpl/.github/workflows/ci.yml.tmpl"
    case_run "a required template missing" 1 "missing template: lefthook.yml.tmpl" \
        "rm $tpl/lefthook.yml.tmpl"
    case_run "the LF pin removed" 1 "the LF pin for the devtools skill trees is gone" \
        "grep -vxF '$pin' .gitattributes > t.tmp; mv t.tmp .gitattributes"
    case_run "apply.sh missing" 1 "missing file: $skill/scripts/apply.sh" \
        "rm $skill/scripts/apply.sh"
    case_run "the test crate loses its workspace table" 1 "lacks its own [workspace] table" \
        "grep -vx '\\[workspace\\]' $tpl/toolchain-tests/Cargo.toml.tmpl > t.tmp; mv t.tmp $tpl/toolchain-tests/Cargo.toml.tmpl"
    case_run "the test crate becomes publishable" 1 "lacks publish = false" \
        "grep -vx 'publish = false' $tpl/toolchain-tests/Cargo.toml.tmpl > t.tmp; mv t.tmp $tpl/toolchain-tests/Cargo.toml.tmpl"
    case_run "the description stops naming Rust" 1 "does not name Rust and Cargo" \
        "sed 's/^description: .*/description: Scaffold a tested toolchain into a repository./' $sk > t.tmp && mv t.tmp $sk"
    case_run "the OpenCode command missing" 1 "missing file: $cmd" "rm $cmd"

    if [ "$failed" -ne 0 ]; then
        echo "  $failed of $total mutation cases failed" >&2
        return 1
    fi
    echo "Rust scaffold gate fails on all $((total - 1)) seeded defects."
}

if [ "$mode" = "--mutation-test" ]; then
    mutation_test
    exit $?
fi
[ -z "$mode" ] || { echo "usage: $0 [--mutation-test]" >&2; exit 2; }

# --- the skill ---
for f in "$skill/SKILL.md" "$skill/scripts/apply.sh" "$cmd" \
    "$skill/references/pitfalls.md" "$skill/references/adapting.md" \
    "$skill/references/github-settings.md" "$skill/references/review-workflow.md" \
    "$skill/assets/snippets/agent-instructions.md" "$skill/assets/snippets/README.development.md"; do
    [ -f "$f" ] || bad "missing file: $f"
done

if [ -f "$skill/SKILL.md" ]; then
    fm=$(frontmatter "$skill/SKILL.md")
    grep -qx 'name: rust-toolchain-scaffold' <<<"$fm" || bad "$skill/SKILL.md: frontmatter lacks 'name: rust-toolchain-scaffold'"
    desc=$(grep '^description: ' <<<"$fm" || true)
    { grep -q 'Rust' <<<"$desc" && grep -q 'Cargo' <<<"$desc"; } \
        || bad "$skill/SKILL.md: the description does not name Rust and Cargo"
    # Every backticked assets/, references/ or scripts/ path must resolve under the skill.
    # shellcheck disable=SC2016  # the backticks are literal markdown, not a command
    while IFS= read -r ref; do
        [ -e "$skill/$ref" ] || bad "$skill/SKILL.md: dangling reference $ref"
    done < <(grep -oE '`(assets|references|scripts)/[^`]*`' "$skill/SKILL.md" | tr -d '`' | sort -u)
fi

# The same for the references/ files, which the SKILL.md sends the agent to read.
for doc in "$skill"/references/*.md; do
    [ -f "$doc" ] || continue
    # shellcheck disable=SC2016  # the backticks are literal markdown, not a command
    while IFS= read -r ref; do
        [ -e "$skill/$ref" ] || bad "$doc: dangling reference $ref"
    done < <(grep -oE '`(assets|references|scripts)/[^`]*`' "$doc" | tr -d '`' | sort -u)
done

# --- the templates ---
while IFS= read -r f; do
    case "$f" in
        *.tmpl) ;;
        *) bad "template file without the .tmpl suffix: $f" ;;
    esac
done < <(find "$tpl" -type f | sort)

for t in "${REQUIRED[@]}"; do
    [ -f "$tpl/$t" ] || bad "missing template: $t"
done

ci=$tpl/.github/workflows/ci.yml.tmpl
if [ -f "$ci" ]; then
    uses=0
    while IFS= read -r line; do
        uses=$((uses + 1))
        ref=${line#*uses:}
        ref=${ref# }
        ref=${ref%% *}
        sha=${ref##*@}
        grep -qE '^[0-9a-f]{40}$' <<<"$sha" || bad "$ci: action $ref is not pinned to a 40-hex SHA"
    done < <(grep -E '^[[:space:]-]*uses:' "$ci" || true)
    [ "$uses" -gt 0 ] || bad "$ci: no action is used, so the pin check checked nothing"
fi

crate=$tpl/toolchain-tests/Cargo.toml.tmpl
if [ -f "$crate" ]; then
    grep -qx '\[workspace\]' "$crate" || bad "$crate: lacks its own [workspace] table"
    grep -qx 'publish = false' "$crate" || bad "$crate: lacks publish = false"
fi

# --- this repository ---
grep -qxF "$pin" .gitattributes || bad ".gitattributes: the LF pin for the devtools skill trees is gone ($pin)"

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "Rust toolchain scaffold consistent."
