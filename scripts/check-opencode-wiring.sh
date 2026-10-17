#!/usr/bin/env bash
#
# Verify the opencode/ plugins still describe things this repo builds.
#
# An opencode.jsonc names its MCP binary as a plain string, a plugin.ts id is
# free text, and a skill copy is just a file: nothing else checks any of them.
# A stale copy installs happily while advertising — or launching — the wrong
# thing. That is the exact failure this catches.
set -euo pipefail

cd "$(dirname "$0")/.."

source scripts/lib/registry.sh

# The jq on a Windows PATH emits CRLF; strip it exactly like the other checks.
jqr() {
    jq -r "$@" | tr -d '\r'
}

known=$(cargo metadata --no-deps --format-version 1 |
    jq -r '.packages[].targets[] | select(.kind[] == "bin") | .name' | sort -u)

failures=0
checked=0
fail() { echo "  FAIL  $1" >&2; failures=$((failures + 1)); }

# Sorted: the old opencode/plugins/*/ glob expanded alphabetically.
for name in $(registry_plugins opencode | sort); do
    dir="opencode/plugins/$name/"
    plugin_ts="$dir/plugin.ts"
    jsonc="$dir/opencode.jsonc"
    pkg="$dir/package.json"

    # A skills-only plugin ships no plugin.ts, opencode.jsonc or MCP command, so
    # checks 1, 3 and 4 (and the pin check 8) skip it; 2 and 5 still apply.
    skills_only=0
    registry_is_skills_only opencode "$name" && skills_only=1

    if [ "$skills_only" -eq 0 ]; then
        [ -f "$plugin_ts" ] || { fail "$name: $plugin_ts missing"; continue; }
        [ -f "$jsonc" ] || { fail "$name: $jsonc missing"; continue; }

        # 1. The Plugin.define id matches the directory.
        id=$(sed -n 's/^[[:space:]]*id:[[:space:]]*"\([^"]*\)".*$/\1/p' "$plugin_ts" | head -n 1)
        [ "$id" = "$name" ] || fail "$name: plugin.ts id is '$id'"
    fi

    # 2. package.json carries a well-formed semver version. Per-package
    # releases bump it independently (the workspace version is not any
    # plugin's release version); crate/manifest agreement for opencode lives
    # in check-package-versions.sh, so form is all this check owns.
    pkg_version=$(jqr '.version // ""' "$pkg" 2>/dev/null || echo "")
    [[ "$pkg_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]] ||
        fail "$name: package.json version '$pkg_version' is not semver"

    if [ "$skills_only" -eq 0 ]; then
        # 3. opencode.jsonc parses (kept comment-free so jq reads it) and carries
        #    V2-native shapes only.
        jqr -e . "$jsonc" > /dev/null || { fail "$name: $jsonc is not valid JSON"; continue; }
        for legacy in '"plugin"' '"mcpServers"' '"enabled":' '"attachment"'; do
            if grep -q "$legacy" "$jsonc"; then
                fail "$name: $jsonc carries a V1/legacy key ($legacy)"
            fi
        done

        # 4. Every local MCP command resolves to a [[bin]] this workspace builds.
        while IFS= read -r command; do
            [ -n "$command" ] || continue
            bin=$(basename "$command")
            checked=$((checked + 1))
            if printf '%s\n' "$known" | grep -qxF "$bin"; then
                printf '  ok       %-20s <- %s\n' "$bin" "$jsonc"
            else
                fail "$name: mcp command '$command' names no built binary"
            fi
        done < <(jqr '.mcp.servers // {} | .[] | select(.type == "local") | .command[0] // empty' "$jsonc")
    fi

    # 5. Skill frontmatter names match their directories.
    for skill in "$dir"skills/*/SKILL.md; do
        [ -f "$skill" ] || continue
        sname=$(basename "$(dirname "$skill")")
        front=$(sed -n 's/^name:[[:space:]]*\(.*\)$/\1/p' "$skill" | head -n 1 | tr -d '\r')
        [ "$front" = "$sname" ] || fail "$name: $skill frontmatter names '$front'"
        checked=$((checked + 1))
    done

    # 6. Agent frontmatter uses native V2 keys (no `prompt:`, has `description:`).
    for agent in "$dir"agents/*.md; do
        [ -f "$agent" ] || continue
        grep -q '^prompt:' "$agent" && fail "$name: $agent carries legacy prompt:"
        grep -q '^description:' "$agent" || fail "$name: $agent has no description:"
        checked=$((checked + 1))
    done
done

# 7. Copy-staleness: verbatim copies must still be byte-identical to sources.
#
# NOTE (Task 5 deviation from the plan): the pair does NOT point at the
# canonical `shared/ghidra-mcp/skill/SKILL.md`. That file carries a 9-line HTML
# license header that `emit-skill` strips, so every committed plugin copy is 261
# lines against the canonical 270 and a raw `cmp` against it fails on a correct
# tree (verified byte-for-byte before writing this). Both plugin copies are
# emit outputs of the same binary via the same `just emit-ghidra-skill`
# recipe, so the sibling Claude Code copy is the correct oracle here: it
# catches a forgotten opencode regen (or a hand-edit) with no build, while the
# generator side stays pinned by `skill_emit.rs` (which covers both copies).
#
# Only `ghidra-re-driver` is pinned here. `rtk-policy`, `gain` and `doctor`
# used to be verbatim Claude copies too, but they carried Claude-only names
# (`RTK_CC_DISABLE`, `/rtk-mcp-cc:gain`, `${CLAUDE_PLUGIN_ROOT}`, `pwsh`-only
# commands) that are wrong on the OpenCode host — so the opencode variants are
# deliberately host-ported docs now, not copies, and must NOT be cmp'd.
for pair in \
    "claude-code/plugins/re-ghidra-mcp-cc/skills/ghidra-re-driver/SKILL.md:opencode/plugins/re-ghidra-mcp-opencode/skills/ghidra-re-driver/SKILL.md" \
; do
    src=${pair%%:*}; dst=${pair#*:}
    checked=$((checked + 1))
    if [ ! -f "$dst" ]; then
        fail "copy missing: $dst"
    elif ! cmp -s "$src" "$dst"; then
        fail "stale copy: $dst differs from $src"
    else
        printf '  ok       copy %-40s\n' "$dst"
    fi
done

# The repo-user-guide skill is a byte-copy of the Claude original: host-portable,
# unlike the rtk/doctor skills above, which carry host-specific names and so are
# compared per-content instead. A drift here means the port drifted apart.
src_skills="claude-code/plugins/repo-user-guide-cc/skills"
dst_skills="opencode/plugins/repo-user-guide-opencode/skills"
checked=$((checked + 1))
if [ ! -d "$dst_skills" ]; then
    fail "copy missing: $dst_skills"
elif ! diff -r -q "$src_skills" "$dst_skills" >/dev/null 2>&1; then
    fail "stale copy: $dst_skills differs from $src_skills"
else
    printf '  ok       copy %-40s\n' "$dst_skills"
fi

# 8. Pin-consistency: every opencode plugin pins the same @opencode/plugin
# version, so one plugin drifting ahead of (or behind) the other fails loudly
# instead of passing silently.
pin_ref=""
for name in $(registry_plugins opencode | sort); do
    pkg="opencode/plugins/$name/package.json"
    [ -f "$pkg" ] || continue
    registry_is_skills_only opencode "$name" && continue
    pin=$(jqr '.dependencies["@opencode/plugin"] // ""' "$pkg" 2>/dev/null || echo "")
    if [ -z "$pin" ]; then
        fail "$pkg pins no @opencode/plugin version"
        continue
    fi
    checked=$((checked + 1))
    if [ -z "$pin_ref" ]; then
        pin_ref=$pin
        printf '  ok       @opencode/plugin %-16s <- %s\n' "$pin" "$pkg"
    elif [ "$pin" != "$pin_ref" ]; then
        fail "@opencode/plugin pin drift: $pkg pins $pin, expected $pin_ref"
    else
        printf '  ok       @opencode/plugin %-16s <- %s\n' "$pin" "$pkg"
    fi
done

if [ "$checked" -eq 0 ]; then
    echo "ERROR: found no opencode references to check." >&2
    echo "       Either the registry plugin list or the jq filters have gone stale." >&2
    exit 1
fi

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures opencode wiring problem(s)." >&2
    exit 1
fi

echo "All $checked opencode reference(s) resolve."
