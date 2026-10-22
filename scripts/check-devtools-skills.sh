#!/usr/bin/env bash
# Checks the canonical aiplugins-devtools skill source under shared/devtools/.
# Section `source`: every skill is complete and self-contained; the vendored
# code-review-and-quality skill is also attributed.
# Section `copies`: each emitted per-agent copy equals its canonical source.
set -euo pipefail
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
    grep -qx "name: $name" "$s/SKILL.md" \
        || bad "$s/SKILL.md: frontmatter lacks 'name: $name'"

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
for copy_root in \
    "$(registry_field claude-code dir)/aiplugins-devtools-cc/skills" \
    "$(registry_field qwen dir)/aiplugins-devtools-qwen/skills" \
    "$(registry_field antigravity dir)/aiplugins-devtools-agy/skills" \
; do
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
# else may differ. Compare the renamed canonical SKILL.md, then everything else.
oc_plugin="$(registry_field opencode dir)/aiplugins-devtools-opencode"
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
        oc="$oc_root/aiplugins-devtools-$name"
        if ! diff <(sed "s/^name: $name\$/name: aiplugins-devtools-$name/" "$skills_root/$name/SKILL.md") "$oc/SKILL.md" >/dev/null 2>&1 \
            || ! diff -r -x SKILL.md "$skills_root/$name" "$oc" >/dev/null 2>&1; then
            bad "stale copy: $oc"
        fi
    done
fi

# OpenCode lists no skill in its `/` menu, so each slash command is a command
# file copied unchanged into the plugin's commands/ folder. Every command must
# name a skill that the same plugin ships, under its OpenCode (prefixed) name.
oc_cmds="$oc_plugin/commands"
if [ ! -d "$oc_cmds" ] || ! diff -r "$commands_root" "$oc_cmds" >/dev/null 2>&1; then
    bad "stale copy: $oc_cmds"
fi
for cmd in "$commands_root"/*.md; do
    [ -f "$cmd" ] || continue
    sed -n '1p' "$cmd" | grep -qx -- '---' || bad "$cmd: no frontmatter"
    grep -q '^description: ' "$cmd" || bad "$cmd: frontmatter lacks 'description:'"
    grep -qF '$ARGUMENTS' "$cmd" || bad "$cmd: body lacks \$ARGUMENTS, so the path would not be passed on"
    while IFS= read -r ref; do
        [ -d "$oc_root/$ref" ] || bad "$cmd: names skill '$ref' which $oc_root does not ship"
    done < <(grep -o 'aiplugins-devtools-[a-z0-9-]*' "$cmd" | sort -u)
done

# --- registry helpers ---
# Run as a child process: sourcing registry.sh would make it read this script's $1.
bash scripts/lib/registry.sh --self-test >/dev/null 2>&1 || bad "registry self-test failed: bash scripts/lib/registry.sh --self-test"

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "Devtools skills consistent."
