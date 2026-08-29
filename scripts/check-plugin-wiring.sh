#!/usr/bin/env bash
#
# Verify that every binary a plugin config points at actually exists as a
# [[bin]] target in this workspace.
#
# `hooks.json` and `.mcp.json` name their binaries as plain strings, and nothing
# else checks them. Rename a [[bin]] and the whole test suite still passes while
# the installed plugin silently does nothing: Claude Code cannot spawn a hook
# that is not there, and a hook that fails to spawn is indistinguishable from a
# hook that decided to stay quiet. That is the exact failure this catches.
set -euo pipefail

cd "$(dirname "$0")/.."

known=$(cargo metadata --no-deps --format-version 1 |
    jq -r '.packages[].targets[] | select(.kind[] == "bin") | .name' | sort -u)

failures=0
checked=0

while IFS= read -r reference; do
    file=${reference%%::*}
    command=${reference#*::}
    name=$(basename "$command")
    name=${name%.exe}
    checked=$((checked + 1))

    if printf '%s\n' "$known" | grep -qxF "$name"; then
        printf '  ok       %-16s <- %s\n' "$name" "$file"
    else
        printf '  MISSING  %-16s <- %s\n' "$name" "$file"
        failures=$((failures + 1))
    fi
done < <(
    for config in claude-code/*/hooks/hooks.json claude-code/*/.mcp.json; do
        [ -f "$config" ] || continue
        jq -r --arg file "$config" '
            [.. | objects | .command? // empty]
            | .[]
            | select(type == "string" and contains("CLAUDE_PLUGIN_ROOT"))
            | "\($file)::\(.)"
        ' "$config"
    done
)

# A check that silently checks nothing is worse than no check at all: it reports
# success forever. If the globs or the jq filter stop matching — a plugin moves,
# a config is renamed, the manifest shape changes — say so and fail.
if [ "$checked" -eq 0 ]; then
    echo "ERROR: found no plugin binary references to check." >&2
    echo "       Either the config globs or the jq filter has gone stale." >&2
    exit 1
fi

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures plugin reference(s) name a binary this workspace does not build." >&2
    echo "       Fix the [[bin]] name, or the path in the plugin config." >&2
    exit 1
fi

echo "All $checked plugin binary reference(s) resolve to real [[bin]] targets."
