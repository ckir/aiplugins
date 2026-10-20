#!/usr/bin/env bash
# Shared core for per-host wiring checks: every agent/plugin enumeration
# comes from agents.json. No host names are hardcoded below this line.
set -euo pipefail

REGISTRY="${REGISTRY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/agents.json}"

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

registry_published_name() {
    # shellcheck disable=SC2016  # jq program, not shell
    rq --arg a "$1" --arg p "$2" '.agents[] | select(.id == $a) | .publishedNames[$p] // $p'
}

registry_plugin_for_published() {
    # shellcheck disable=SC2016  # jq program, not shell
    rq --arg a "$1" --arg n "$2" '.agents[] | select(.id == $a)
        | ((.publishedNames // {}) | to_entries | map(select(.value == $n)) | .[0].key) // $n'
}

registry_is_skills_only() {
    jq -e --arg a "$1" --arg p "$2" \
        '.agents[] | select(.id == $a) | .skillsOnly[]? | select(. == $p)' \
        "$REGISTRY" > /dev/null
}

registry_field() {
    rq --arg a "$1" --arg f "$2" '.agents[] | select(.id == $a) | .[$f] // ""'
}

if [ "${1:-}" = "--self-test" ]; then
    failures=0
    [ "$(registry_plugins claude-code | tr '\n' ' ')" = "rtk-mcp-cc re-ghidra-mcp-cc aiplugins-devtools-cc repo-user-guide-cc opencode-ctl-mcp-cc " ] \
        || { echo "FAIL plugins claude-code"; failures=$((failures + 1)); }
    [ "$(registry_plugins opencode | tr '\n' ' ')" = "rtk-mcp-opencode re-ghidra-mcp-opencode todov2-opencode aiplugins-devtools-opencode repo-user-guide-opencode " ] \
        || { echo "FAIL plugins opencode"; failures=$((failures + 1)); }
    [ "$(registry_plugins antigravity | tr '\n' ' ')" = "rtk-mcp-agy re-ghidra-mcp-agy aiplugins-devtools-agy repo-user-guide-agy opencode-ctl-mcp-agy " ] \
        || { echo "FAIL plugins antigravity"; failures=$((failures + 1)); }
    [ "$(registry_manifest_path qwen rtk-mcp-qwen)" = "qwen/plugins/rtk-mcp-qwen/qwen-extension.json" ] \
        || { echo "FAIL manifest path"; failures=$((failures + 1)); }
    registry_is_excluded qwen example \
        || { echo "FAIL exclusion"; failures=$((failures + 1)); }
    ! registry_is_excluded qwen rtk-mcp-qwen \
        || { echo "FAIL non-exclusion"; failures=$((failures + 1)); }
    [ "$(registry_field claude-code artifactSuffix)" = "-plugin.zip" ] \
        || { echo "FAIL field"; failures=$((failures + 1)); }
    # Published names / skills-only: identity against the real registry.
    [ "$(registry_published_name claude-code rtk-mcp-cc)" = "rtk-mcp-cc" ] \
        || { echo "FAIL published name identity"; failures=$((failures + 1)); }
    [ "$(registry_plugin_for_published claude-code rtk-mcp-cc)" = "rtk-mcp-cc" ] \
        || { echo "FAIL plugin-for-published identity"; failures=$((failures + 1)); }
    ! registry_is_skills_only claude-code rtk-mcp-cc \
        || { echo "FAIL skills-only absent"; failures=$((failures + 1)); }
    ! registry_is_skills_only claude-code no-such-plugin \
        || { echo "FAIL skills-only unknown plugin"; failures=$((failures + 1)); }
    real_registry=$REGISTRY
    [ "$(registry_published_name claude-code aiplugins-devtools-cc)" = "aiplugins-devtools" ] \
        || { echo "FAIL published name devtools"; failures=$((failures + 1)); }
    [ "$(registry_plugin_for_published claude-code aiplugins-devtools)" = "aiplugins-devtools-cc" ] \
        || { echo "FAIL plugin-for-published devtools"; failures=$((failures + 1)); }
    [ "$(bash "$(dirname "${BASH_SOURCE[0]}")/../registry-plugins.sh" claude-code | tr '\n' ' ')" = "rtk-mcp-cc re-ghidra-mcp-cc aiplugins-devtools-cc repo-user-guide-cc opencode-ctl-mcp-cc " ] \
        || { echo "FAIL registry-plugins real"; failures=$((failures + 1)); }
    # registry-plugins.sh argument validation: usage + exit 2, and no self-test leak.
    rp="$(dirname "${BASH_SOURCE[0]}")/../registry-plugins.sh"
    rc=0; bash "$rp" no-such-agent >/dev/null 2>&1 || rc=$?
    [ "$rc" -eq 2 ] || { echo "FAIL registry-plugins unknown agent (rc=$rc)"; failures=$((failures + 1)); }
    rc=0; bash "$rp" claude-code --bogus >/dev/null 2>&1 || rc=$?
    [ "$rc" -eq 2 ] || { echo "FAIL registry-plugins bogus flag (rc=$rc)"; failures=$((failures + 1)); }
    rc=0; bash "$rp" --self-test >/dev/null 2>&1 || rc=$?
    [ "$rc" -eq 2 ] || { echo "FAIL registry-plugins --self-test leak (rc=$rc)"; failures=$((failures + 1)); }
    # Mapped cases run against a scratch copy that adds the two optional fields.
    scratch=$(mktemp)
    jq '(.agents[] | select(.id == "claude-code")) += {publishedNames: {"rtk-mcp-cc": "demo"}, skillsOnly: ["rtk-mcp-cc"]}' \
        "$real_registry" > "$scratch"
    REGISTRY=$scratch
    [ "$(registry_published_name claude-code rtk-mcp-cc)" = "demo" ] \
        || { echo "FAIL published name mapped"; failures=$((failures + 1)); }
    [ "$(registry_published_name claude-code re-ghidra-mcp-cc)" = "re-ghidra-mcp-cc" ] \
        || { echo "FAIL published name unmapped sibling"; failures=$((failures + 1)); }
    [ "$(registry_plugin_for_published claude-code demo)" = "rtk-mcp-cc" ] \
        || { echo "FAIL plugin-for-published mapped"; failures=$((failures + 1)); }
    registry_is_skills_only claude-code rtk-mcp-cc \
        || { echo "FAIL skills-only listed"; failures=$((failures + 1)); }
    ! registry_is_skills_only claude-code re-ghidra-mcp-cc \
        || { echo "FAIL skills-only unlisted sibling"; failures=$((failures + 1)); }
    [ "$(REGISTRY=$scratch bash "$(dirname "${BASH_SOURCE[0]}")/../registry-plugins.sh" claude-code --exclude-skills-only | tr '\n' ' ')" = "re-ghidra-mcp-cc aiplugins-devtools-cc repo-user-guide-cc opencode-ctl-mcp-cc " ] \
        || { echo "FAIL registry-plugins exclude"; failures=$((failures + 1)); }
    REGISTRY=$real_registry
    rm -f "$scratch"
    [ "$failures" -eq 0 ] && echo "registry self-test ok"
    exit "$failures"
fi
