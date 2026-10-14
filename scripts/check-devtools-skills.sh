#!/usr/bin/env bash
# Checks the canonical aiplugins-devtools skill source under shared/devtools/.
# Section `source`: the vendored skill is complete, self-contained and attributed.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=lib/registry.sh disable=SC1091
source scripts/lib/registry.sh

fail=0
bad() {
    echo "  FAIL  $1" >&2
    fail=1
}

skill=shared/devtools/skills/code-review-and-quality
files=(
    "$skill/SKILL.md"
    "$skill/references/security-checklist.md"
    "$skill/references/performance-checklist.md"
    "$skill/THIRD-PARTY-NOTICE.md"
)

# --- source ---
for f in "${files[@]}"; do
    [ -f "$f" ] || bad "missing file: $f"
done

if [ -f "$skill/SKILL.md" ]; then
    grep -qx 'name: code-review-and-quality' "$skill/SKILL.md" \
        || bad "$skill/SKILL.md: frontmatter lacks 'name: code-review-and-quality'"

    # Every backticked references/<file> path must resolve under the skill dir.
    # shellcheck disable=SC2016  # the backticks are literal markdown, not a command
    while IFS= read -r ref; do
        [ -f "$skill/$ref" ] || bad "$skill/SKILL.md: dangling reference $ref"
    done < <(grep -o '`references/[^`]*`' "$skill/SKILL.md" | tr -d '`' | sort -u)
fi

# Sibling skills of the upstream repo that this plugin does not ship.
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
# Each emitted per-agent copy must equal the canonical skill directory.
for copy in \
    "$(registry_field claude-code dir)/aiplugins-devtools-cc/skills/code-review-and-quality" \
    "$(registry_field qwen dir)/aiplugins-devtools-qwen/skills/code-review-and-quality" \
    "$(registry_field antigravity dir)/aiplugins-devtools-agy/skills/code-review-and-quality" \
; do
    if [ ! -d "$copy" ] || ! diff -r "$skill" "$copy" >/dev/null 2>&1; then
        bad "stale copy: $copy"
    fi
done

# OpenCode keys skills by folder name, so its copy is renamed: the folder is
# prefixed and SKILL.md's frontmatter `name:` line is rewritten to match; nothing
# else may differ. Compare the renamed canonical SKILL.md, then everything else.
oc="$(registry_field opencode dir)/aiplugins-devtools-opencode/skills/aiplugins-devtools-code-review-and-quality"
if [ ! -d "$oc" ] \
    || ! diff <(sed 's/^name: code-review-and-quality$/name: aiplugins-devtools-code-review-and-quality/' "$skill/SKILL.md") "$oc/SKILL.md" >/dev/null 2>&1 \
    || ! diff -r -x SKILL.md "$skill" "$oc" >/dev/null 2>&1; then
    bad "stale copy: $oc"
fi

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "Devtools skills consistent."
