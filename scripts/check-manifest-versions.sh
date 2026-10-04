#!/usr/bin/env bash
#
# Verify that every plugin.json, qwen-extension.json, and package.json across the
# workspace specifies the exact same version as the workspace Cargo.toml.
#
# Manually bumping the version in Cargo.toml without using cargo-release or
# a dedicated bumping script bypasses `pre-release-replacements`. This script
# prevents a release from shipping with outdated manifest versions.
set -euo pipefail

cd "$(dirname "$0")/.."

workspace_version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ -n "$workspace_version" ] || {
    echo "ERROR: no [workspace.package] version found in Cargo.toml." >&2
    exit 1
}

failures=0
checked=0

check_file() {
    local file="$1"
    local field="$2"
    [ -f "$file" ] || return 0
    checked=$((checked + 1))
    local got
    got=$(jq -r ".$field // \"\"" "$file" | tr -d '\r')
    if [ "$got" != "$workspace_version" ]; then
        echo "  FAIL  $file: says $got, expected $workspace_version" >&2
        failures=$((failures + 1))
    else
        printf '  ok       %-16s -> %s\n' "$workspace_version" "$file"
    fi
}

for file in antigravity/*/plugin.json claude-code/*/.claude-plugin/plugin.json; do
    check_file "$file" "version"
done

for file in qwen/*/qwen-extension.json; do
    check_file "$file" "version"
done

for file in opencode/*/package.json; do
    check_file "$file" "version"
done

if [ "$checked" -eq 0 ]; then
    echo "ERROR: found no manifest files to check." >&2
    exit 1
fi

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures manifest(s) have an outdated version." >&2
    echo "       Fix the version string in the manifests, or use cargo-release." >&2
    exit 1
fi

echo "All $checked manifest(s) match workspace version $workspace_version."
