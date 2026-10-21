#!/usr/bin/env bash
#
# Verify the antigravity/ plugins still describe things this repo builds.
#
# `hooks.json` and `mcp_config.json` name their binaries as plain strings, and
# nothing else checks them. A stale copy installs happily while advertising
# or launching the wrong thing. That is the exact failure this catches.
set -euo pipefail

cd "$(dirname "$0")/.."

source scripts/lib/registry.sh

jqr() {
    jq -r "$@" | tr -d '\r'
}

all_plugins() {
    registry_plugins "$1"
    rq --arg a "$1" '.agents[] | select(.id == $a) | .notPublished[]?'
}

known=$(cargo metadata --no-deps --format-version 1 |
    jq -r '.packages[].targets[] | select(.kind[] == "bin") | .name' | sort -u)

failures=0
checked=0
fail() { echo "  FAIL  $1" >&2; failures=$((failures + 1)); }

for name in $(all_plugins antigravity | sort -u); do
    dir="antigravity/plugins/$name"
    plugin_json="$dir/plugin.json"
    hooks_json="$dir/hooks.json"
    mcp_config="$dir/mcp_config.json"
    cargo_toml="$dir/Cargo.toml"

    # 1. plugin.json exists and is valid JSON.
    [ -f "$plugin_json" ] || { fail "$name: $plugin_json missing"; continue; }
    jqr -e . "$plugin_json" > /dev/null || { fail "$name: $plugin_json is not valid JSON"; continue; }
    checked=$((checked + 1))

    # 2. plugin.json version is valid semver.
    pkg_version=$(jqr '.version // ""' "$plugin_json")
    [[ "$pkg_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]] ||
        fail "$name: plugin.json version '$pkg_version' is not semver"
    checked=$((checked + 1))

    # 3. If Cargo.toml exists, pkg_version must match the crate version.
    if [ -f "$cargo_toml" ]; then
        crate_version=$(tr -d '\r' < "$cargo_toml" | sed -n 's/^version = "\(.*\)"$/\1/p' | head -n 1)
        if [ -n "$crate_version" ]; then
            checked=$((checked + 1))
            [ "$pkg_version" = "$crate_version" ] ||
                fail "$name: plugin.json version '$pkg_version' does not match crate version '$crate_version'"
        fi
    fi

    # 4. If hooks.json exists, it is valid JSON and every command resolves to a built binary.
    if [ -f "$hooks_json" ]; then
        jqr -e . "$hooks_json" > /dev/null || fail "$name: $hooks_json is not valid JSON"
        while IFS= read -r command; do
            [ -n "$command" ] || continue
            bin=$(basename "$command")
            bin=${bin%.exe}
            checked=$((checked + 1))
            if printf '%s\n' "$known" | grep -qxF "$bin"; then
                printf '  ok       %-20s <- %s\n' "$bin" "$hooks_json"
            else
                fail "$name: hooks.json command '$command' names no built binary"
            fi
        done < <(jqr '[.. | objects | .command? // empty] | .[] | select(type == "string")' "$hooks_json")
    fi

    # 5. If mcp_config.json exists, it is valid JSON and every command resolves to a built binary.
    if [ -f "$mcp_config" ]; then
        jqr -e . "$mcp_config" > /dev/null || fail "$name: $mcp_config is not valid JSON"
        while IFS= read -r command; do
            [ -n "$command" ] || continue
            bin=$(basename "$command")
            bin=${bin%.exe}
            checked=$((checked + 1))
            if printf '%s\n' "$known" | grep -qxF "$bin"; then
                printf '  ok       %-20s <- %s\n' "$bin" "$mcp_config"
            else
                fail "$name: mcp_config.json command '$command' names no built binary"
            fi
        done < <(jqr '[.. | objects | .command? // empty] | .[] | select(type == "string")' "$mcp_config")
    fi

    # 6. For every skills/*/SKILL.md: name matches the parent folder name.
    for skill in "$dir"/skills/*/SKILL.md; do
        [ -f "$skill" ] || continue
        sname=$(basename "$(dirname "$skill")")
        front=$(sed -n 's/^name:[[:space:]]*\(.*\)$/\1/p' "$skill" | head -n 1 | tr -d '\r')
        [ "$front" = "$sname" ] || fail "$name: $skill frontmatter names '$front'"
        checked=$((checked + 1))
    done

    # 7. For every agents/*.md: description: is present.
    for agent in "$dir"/agents/*.md; do
        [ -f "$agent" ] || continue
        grep -q '^description:' "$agent" || fail "$name: $agent has no description:"
        checked=$((checked + 1))
    done
done

# 8. Copy-staleness: verbatim copies must still be byte-identical to sources.
driver_src="claude-code/plugins/re-ghidra-mcp-cc/skills/ghidra-re-driver/SKILL.md"
driver_dst="antigravity/plugins/re-ghidra-mcp-agy/skills/ghidra-re-driver/SKILL.md"
checked=$((checked + 1))
if [ ! -f "$driver_dst" ]; then
    fail "copy missing: $driver_dst"
elif ! cmp -s "$driver_src" "$driver_dst"; then
    fail "stale copy: $driver_dst differs from $driver_src"
else
    printf '  ok       copy %-40s\n' "$driver_dst"
fi

rug_src="claude-code/plugins/repo-user-guide-cc/skills"
rug_dst="antigravity/plugins/repo-user-guide-agy/skills"
checked=$((checked + 1))
if [ ! -d "$rug_dst" ]; then
    fail "copy missing: $rug_dst"
elif ! diff -r -q "$rug_src" "$rug_dst" >/dev/null 2>&1; then
    fail "stale copy: $rug_dst differs from $rug_src"
else
    printf '  ok       copy %-40s\n' "$rug_dst"
fi

if [ "$checked" -eq 0 ]; then
    echo "ERROR: found no antigravity references to check." >&2
    echo "       Either the registry plugin list or the jq filters have gone stale." >&2
    exit 1
fi

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures antigravity wiring problem(s)." >&2
    exit 1
fi

echo "All $checked antigravity reference(s) resolve."
