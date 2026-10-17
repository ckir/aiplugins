#!/usr/bin/env bash
#
# Assemble every OpenCode plugin.ts bundle and check the zip contents — the
# PR-time counterpart to opencode-bundles.yml's tag-time verify job.
#
# Skills-only plugins are NOT covered here: check-skills-bundles.sh already
# assembles and checks those (they go through bundle-skills-plugin.sh, a
# different script). This covers the plugin.ts plugins through
# bundle-opencode-plugin.sh — the same script the tag workflow runs — so "the
# zip we would publish carries the file set an install needs" is checked on
# every PR instead of only when a tag lands.
#
# Why Ubuntu only in CI: both bundler scripts refuse Windows shells (FAT
# attributes, MSYS path translation) — see their headers. macOS would be
# redundant: the file set is platform-independent.
set -euo pipefail
cd "$(dirname "$0")/.."

# Deviation from the task brief: the brief's loop called the lib function
# `registry_plugins opencode --exclude-skills-only`, but lib/registry.sh's
# registry_plugins takes only the agent id and would silently ignore the flag.
# The exclusion lives in scripts/registry-plugins.sh (matching `just smoke`'s
# pattern for claude-code), so iterate the same list through that CLI.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail=0
bad() { echo "  FAIL  $1" >&2; fail=$((fail + 1)); }

for name in $(bash scripts/registry-plugins.sh opencode --exclude-skills-only | sort); do
    zip="$tmp/$name-opencode.zip"
    if ! bash scripts/bundle-opencode-plugin.sh "opencode/plugins/$name" "$tmp" >/dev/null; then
        bad "$name: bundler failed"
        continue
    fi
    [ -f "$zip" ] || { bad "$name: $zip missing after bundling"; continue; }

    out="$tmp/extract-$name"
    mkdir -p "$out"
    unzip -q "$zip" -d "$out"

    # The exact set bundle-opencode-plugin.sh stages as required, plus what the
    # sources actually ship (the tag workflow's verify step checks the same).
    for f in plugin.ts opencode.jsonc package.json README.md INSTALL.md; do
        [ -f "$out/$f" ] || bad "$name: $f missing from the bundle"
    done
    jq -e . "$out/opencode.jsonc" > /dev/null 2>&1 ||
        bad "$name: bundled opencode.jsonc is not valid JSON"

    src="opencode/plugins/$name"
    for optional in skills agents tool tui.tsx tsconfig.json; do
        if [ -e "$src/$optional" ]; then
            [ -e "$out/$optional" ] || bad "$name: $optional missing from the bundle"
        fi
    done

    # The zip is versioned source only: build inputs must never leak in.
    if unzip -Z1 "$zip" | grep -q '^bin/\|^tests/\|^node_modules/\|^bun\.lock$'; then
        bad "$name: bundle carries build inputs (bin/, tests/, node_modules/ or bun.lock)"
    fi
done

[ "$fail" -eq 0 ] || exit 1
echo "All OpenCode plugin bundles assemble with the expected contents."
