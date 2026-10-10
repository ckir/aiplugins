#!/usr/bin/env bash
# Shared core for per-host wiring checks: every agent/plugin enumeration
# comes from agents.json. No host names are hardcoded below this line.
set -euo pipefail

REGISTRY="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/agents.json"

rq() { jq -r "$@" "$REGISTRY" | tr -d '\r'; }

registry_plugins() {
    rq --arg a "$1" '.agents[] | select(.id == $a) | .plugins[]'
}

registry_manifest_path() {
    local pattern
    pattern=$(rq --arg a "$1" '.agents[] | select(.id == $a) | .manifestPattern')
    printf '%s\n' "${pattern//\{plugin\}/$2}"
}

registry_is_excluded() {
    local name=$2
    rq --arg a "$1" '.agents[] | select(.id == $a) | .notPublished[]' \
        | grep -qxF "$name"
}

registry_field() {
    rq --arg a "$1" --arg f "$2" '.agents[] | select(.id == $a) | .[$f] // ""'
}

if [ "${1:-}" = "--self-test" ]; then
    failures=0
    [ "$(registry_plugins claude-code | tr '\n' ' ')" = "rtk-mcp-cc re-ghidra-mcp-cc " ] \
        || { echo "FAIL plugins claude-code"; failures=$((failures + 1)); }
    [ "$(registry_manifest_path qwen rtk-mcp-qwen)" = "qwen/plugins/rtk-mcp-qwen/qwen-extension.json" ] \
        || { echo "FAIL manifest path"; failures=$((failures + 1)); }
    registry_is_excluded qwen example \
        || { echo "FAIL exclusion"; failures=$((failures + 1)); }
    ! registry_is_excluded qwen rtk-mcp-qwen \
        || { echo "FAIL non-exclusion"; failures=$((failures + 1)); }
    [ "$(registry_field claude-code artifactSuffix)" = "-plugin.zip" ] \
        || { echo "FAIL field"; failures=$((failures + 1)); }
    [ "$failures" -eq 0 ] && echo "registry self-test ok"
    exit "$failures"
fi
