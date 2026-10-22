#!/usr/bin/env bash
# Checks the canonical aiplugins-devtools skill source under shared/devtools/.
# Section `source`: every skill is complete and self-contained; the vendored
# code-review-and-quality skill is also attributed.
# Section `copies`: each emitted per-agent copy equals its canonical source.
# `--mutation-test` seeds a throwaway copy of the tree, applies one defect at a
# time and requires this gate to fail with the message that names that defect.
set -euo pipefail
mode=${1:-}
# registry.sh reads $1 for its own --self-test, so clear the arguments first.
set --
cd "$(dirname "$0")/.."
# shellcheck source=lib/registry.sh disable=SC1091
source scripts/lib/registry.sh

fail=0
bad() {
    echo "  FAIL  $1" >&2
    fail=1
}

skills_root=shared/devtools/skills
commands_root=shared/devtools/commands
cc_plugin="$(registry_field claude-code dir)/aiplugins-devtools-cc"
qwen_plugin="$(registry_field qwen dir)/aiplugins-devtools-qwen"
agy_plugin="$(registry_field antigravity dir)/aiplugins-devtools-agy"
oc_plugin="$(registry_field opencode dir)/aiplugins-devtools-opencode"

# The frontmatter is the lines between the opening `---` on line 1 and the next
# `---`; the body is everything after that closer. (awk, not a sed range: a sed
# range would run on to EOF when line 2 is the closer.)
frontmatter() { awk 'NR == 1 { next } /^---$/ { exit } { print }' "$1" 2>/dev/null; }
body() { awk 'NR == 1 { next } done { print; next } /^---$/ { done = 1 }' "$1" 2>/dev/null; }
# Succeeds when line 1 opens a frontmatter block and a later line closes it.
frontmatter_closed() {
    [ "$(sed -n '1p' "$1" 2>/dev/null)" = '---' ] && [ "$(grep -cx -- '---' "$1")" -ge 2 ]
}

# One defect at a time on a throwaway copy of the tree; the gate must fail with
# the message that names it. Without this nothing re-runs the failure branches,
# so an edit could turn one into a no-op and the gate would stay green.
mutation_test() {
    local pristine work total=0 failed=0
    local sk=shared/devtools/skills/spec-panel-review/SKILL.md
    local cmd=shared/devtools/commands/spec-panel-review.md
    local occ="$oc_plugin/commands/spec-panel-review.md"
    local ocs="$oc_plugin/skills/aiplugins-devtools-spec-panel-review/SKILL.md"
    # (global: the EXIT trap runs after this function's locals are gone)
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    pristine=$tmp/pristine
    work=$tmp/work
    mkdir -p "$pristine"
    tar -cf - agents.json scripts shared/devtools "$cc_plugin" "$qwen_plugin" "$agy_plugin" "$oc_plugin" \
        | tar -xf - -C "$pristine"

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
        out=$(bash "$work/scripts/check-devtools-skills.sh" 2>&1) || rc=$?
        if [ "$rc" -ne "$want_rc" ] || ! grep -qF -- "$want_msg" <<<"$out"; then
            echo "  FAIL  mutation '$name': exit $rc (want $want_rc), wanted: $want_msg" >&2
            failed=$((failed + 1))
        fi
    }

    case_run "baseline is green" 0 "Devtools skills consistent." "true"
    case_run "skill name only in the body" 1 "lacks 'name: spec-panel-review'" \
        "{ printf -- '---\n---\nname: spec-panel-review\n'; sed '1,4d' $sk; } > t.tmp && mv t.tmp $sk"
    case_run "skill without description" 1 "lacks 'description:'" \
        "sed '3d' $sk > t.tmp && mv t.tmp $sk"
    case_run "skill frontmatter never closed" 1 "frontmatter is missing or never closed" \
        "sed '4d' $sk > t.tmp && mv t.tmp $sk"
    case_run "skill file missing" 1 "missing file:" "rm $sk"
    case_run "dangling references path" 1 "dangling reference references/nope.md" \
        "printf '%s\n' 'See \`references/nope.md\`.' >> $sk"
    case_run "command names no prefixed skill" 1 "names no aiplugins-devtools-" \
        "for f in $cmd $occ; do sed 's/aiplugins-devtools-spec-panel-review/spec-panel-review/' \$f > t.tmp && mv t.tmp \$f; done"
    case_run "command names another skill" 1 "body must name aiplugins-devtools-spec-panel-review" \
        "for f in $cmd $occ; do sed 's/aiplugins-devtools-spec-panel-review/aiplugins-devtools-code-review-and-quality/' \$f > t.tmp && mv t.tmp \$f; done"
    case_run "command frontmatter never closed" 1 "frontmatter is never closed" \
        "for f in $cmd $occ; do sed '3d' \$f > t.tmp && mv t.tmp \$f; done"
    case_run "command description only in the body" 1 "frontmatter lacks 'description:'" \
        "for f in $cmd $occ; do sed '2d' \$f > t.tmp && echo 'description: x' >> t.tmp && mv t.tmp \$f; done"
    # shellcheck disable=SC2016  # the literal text '$ARGUMENTS' is what is being moved
    case_run "ARGUMENTS only in the command frontmatter" 1 'body lacks $ARGUMENTS' \
        "for f in $cmd $occ; do sed '2s/\$/ \$ARGUMENTS/; 4s/\\\$ARGUMENTS//' \$f > t.tmp && mv t.tmp \$f; done"
    case_run "skill name only in the command frontmatter" 1 "names no aiplugins-devtools-" \
        "for f in $cmd $occ; do sed '2s/\$/ aiplugins-devtools-spec-panel-review/; 4s/aiplugins-devtools-spec-panel-review/the skill/' \$f > t.tmp && mv t.tmp \$f; done"
    case_run "no commands" 1 "no commands under" "rm $cmd $occ"
    case_run "stale OpenCode command copy" 1 "stale copy:" "echo extra >> $occ"
    case_run "stale per-agent skill copy" 1 "stale copy:" "echo extra >> $cc_plugin/skills/spec-panel-review/SKILL.md"
    case_run "extra skill folder in a copy" 1 "different set of skills" "mkdir $cc_plugin/skills/extra"
    case_run "OpenCode rewrite also hit a body line" 1 "stale copy:" \
        "echo 'name: spec-panel-review' >> $sk; for d in $cc_plugin $qwen_plugin $agy_plugin; do cp $sk \$d/skills/spec-panel-review/SKILL.md; done; sed 's/^name: spec-panel-review\$/name: aiplugins-devtools-spec-panel-review/' $sk > $ocs"
    case_run "attribution line removed" 1 "copyright line missing" \
        "n=$skills_root/code-review-and-quality/THIRD-PARTY-NOTICE.md; grep -v 'Copyright (c) 2025 Addy Osmani' \$n > t.tmp; mv t.tmp \$n"

    if [ "$failed" -ne 0 ]; then
        echo "  $failed of $total mutation cases failed" >&2
        return 1
    fi
    echo "Devtools skills gate fails on all $((total - 1)) seeded defects."
}

if [ "$mode" = "--mutation-test" ]; then
    mutation_test
    exit $?
fi
[ -z "$mode" ] || { echo "usage: $0 [--mutation-test]" >&2; exit 2; }

# --- source ---
skill_names=()
for dir in "$skills_root"/*/; do
    [ -d "$dir" ] || continue
    skill_names+=("$(basename "$dir")")
done
[ "${#skill_names[@]}" -gt 0 ] || bad "no skills under $skills_root"

for name in "${skill_names[@]}"; do
    s="$skills_root/$name"
    if [ ! -f "$s/SKILL.md" ]; then
        bad "missing file: $s/SKILL.md"
        continue
    fi
    frontmatter_closed "$s/SKILL.md" || bad "$s/SKILL.md: frontmatter is missing or never closed"
    fm=$(frontmatter "$s/SKILL.md")
    grep -qx "name: $name" <<<"$fm" \
        || bad "$s/SKILL.md: frontmatter lacks 'name: $name'"
    grep -q '^description: ' <<<"$fm" \
        || bad "$s/SKILL.md: frontmatter lacks 'description:'"

    # Every backticked references/<file> path must resolve under the skill dir.
    # shellcheck disable=SC2016  # the backticks are literal markdown, not a command
    while IFS= read -r ref; do
        [ -f "$s/$ref" ] || bad "$s/SKILL.md: dangling reference $ref"
    done < <(grep -o '`references/[^`]*`' "$s/SKILL.md" | tr -d '`' | sort -u)
done

# The vendored upstream skill: complete, attributed, and free of references to
# sibling skills of the upstream repo that this plugin does not ship.
skill=$skills_root/code-review-and-quality
files=(
    "$skill/SKILL.md"
    "$skill/references/security-checklist.md"
    "$skill/references/performance-checklist.md"
    "$skill/THIRD-PARTY-NOTICE.md"
)
for f in "${files[@]}"; do
    [ -f "$f" ] || bad "missing file: $f"
done

for f in "$skill/SKILL.md" "$skill/references/security-checklist.md" "$skill/references/performance-checklist.md"; do
    [ -f "$f" ] || continue
    for name in security-and-hardening performance-optimization constraint-driven-development; do
        if grep -q "$name" "$f"; then
            bad "$f: names missing sibling skill $name"
        fi
    done
done

notice="$skill/THIRD-PARTY-NOTICE.md"
if [ -f "$notice" ]; then
    grep -qF 'Copyright (c) 2025 Addy Osmani' "$notice" || bad "$notice: copyright line missing"
    grep -qF '28f435ea69a78531ec00779c7f3b78c64ce95896' "$notice" || bad "$notice: upstream commit missing"
    grep -qF 'Permission is hereby granted' "$notice" || bad "$notice: MIT permission text missing"
fi

# --- copies ---
# Each emitted per-agent copy must hold exactly the canonical skills, each
# equal to its canonical directory. A skill added to or removed from the source
# without re-emitting shows up as a missing or extra folder here.
for copy_root in "$cc_plugin/skills" "$qwen_plugin/skills" "$agy_plugin/skills"; do
    if [ ! -d "$copy_root" ]; then
        bad "stale copy: $copy_root"
        continue
    fi
    if [ "$(find "$copy_root" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort)" != "$(printf '%s\n' "${skill_names[@]}" | sort)" ]; then
        bad "stale copy: $copy_root holds a different set of skills than $skills_root"
        continue
    fi
    for name in "${skill_names[@]}"; do
        diff -r "$skills_root/$name" "$copy_root/$name" >/dev/null 2>&1 \
            || bad "stale copy: $copy_root/$name"
    done
done

# OpenCode keys skills by folder name, so its copies are renamed: the folder is
# prefixed and SKILL.md's frontmatter `name:` line is rewritten to match; nothing
# else may differ. The check does not repeat the emitter's rewrite: it requires
# the new name in the frontmatter, then the rest of the frontmatter, the body and
# every other file to equal the canonical ones.
oc_root="$oc_plugin/skills"
expected_oc=()
for name in "${skill_names[@]}"; do
    expected_oc+=("aiplugins-devtools-$name")
done
if [ ! -d "$oc_root" ] \
    || [ "$(find "$oc_root" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; | sort)" != "$(printf '%s\n' "${expected_oc[@]}" | sort)" ]; then
    bad "stale copy: $oc_root holds a different set of skills than $skills_root"
else
    for name in "${skill_names[@]}"; do
        canon="$skills_root/$name/SKILL.md"
        oc="$oc_root/aiplugins-devtools-$name"
        if ! frontmatter_closed "$oc/SKILL.md" \
            || ! grep -qx "name: aiplugins-devtools-$name" <<<"$(frontmatter "$oc/SKILL.md")" \
            || ! diff <(frontmatter "$canon" | grep -vx "name: $name") <(frontmatter "$oc/SKILL.md" | grep -vx "name: aiplugins-devtools-$name") >/dev/null 2>&1 \
            || ! diff <(body "$canon") <(body "$oc/SKILL.md") >/dev/null 2>&1 \
            || ! diff -r -x SKILL.md "$skills_root/$name" "$oc" >/dev/null 2>&1; then
            bad "stale copy: $oc"
        fi
    done
fi

# OpenCode lists no skill in its `/` menu, so each slash command is a command
# file copied unchanged into the plugin's commands/ folder. Every command must
# name the skill it is for, under its OpenCode (prefixed) name.
oc_cmds="$oc_plugin/commands"
if [ ! -d "$oc_cmds" ] || ! diff -r "$commands_root" "$oc_cmds" >/dev/null 2>&1; then
    bad "stale copy: $oc_cmds"
fi
cmd_files=("$commands_root"/*.md)
[ -f "${cmd_files[0]}" ] || bad "no commands under $commands_root"
for cmd in "${cmd_files[@]}"; do
    [ -f "$cmd" ] || continue
    sed -n '1p' "$cmd" | grep -qx -- '---' || bad "$cmd: no frontmatter"
    [ "$(grep -cx -- '---' "$cmd")" -ge 2 ] || bad "$cmd: frontmatter is never closed"
    # Frontmatter is line 2 to the closing `---`; the body is everything after it.
    fm=$(frontmatter "$cmd")
    cmd_body=$(body "$cmd")
    grep -q '^description: ' <<<"$fm" || bad "$cmd: frontmatter lacks 'description:'"
    # shellcheck disable=SC2016  # the literal text '$ARGUMENTS' is what is searched for
    grep -qF '$ARGUMENTS' <<<"$cmd_body" || bad "$cmd: body lacks \$ARGUMENTS, so the path would not be passed on"
    # A command naming no prefixed skill would load nothing under OpenCode, so
    # zero matches is a failure, not an empty loop.
    refs=$(grep -o 'aiplugins-devtools-[a-z0-9-]*' <<<"$cmd_body" | sort -u || true)
    [ -n "$refs" ] || bad "$cmd: names no aiplugins-devtools-<skill>, so OpenCode would load no skill"
    # <name>.md is the command for skill <name>; a copy pasted from another
    # command that still names the old skill would load the wrong one.
    cmd_skill="aiplugins-devtools-$(basename "$cmd" .md)"
    grep -qx "$cmd_skill" <<<"$refs" || bad "$cmd: body must name $cmd_skill, the skill this command is for"
    while IFS= read -r ref; do
        [ -n "$ref" ] || continue
        [ -d "$oc_root/$ref" ] || bad "$cmd: names skill '$ref' which $oc_root does not ship"
    done <<<"$refs"
done

# --- registry helpers ---
# Run as a child process: sourcing registry.sh would make it read this script's $1.
bash scripts/lib/registry.sh --self-test >/dev/null 2>&1 || bad "registry self-test failed: bash scripts/lib/registry.sh --self-test"

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "Devtools skills consistent."
