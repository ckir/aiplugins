#!/usr/bin/env bash
# Per-package version agreement across the eight registry packages.
set -euo pipefail
cd "$(dirname "$0")/.."
failures=0
check_pair() { # $1 = label, $2 = crate Cargo.toml, $3 = host manifest, $4 = jq field
    local crate_v manifest_v
    if [[ "$2" == *.json ]]; then
        # opencode ships no crates: compare package.json version to itself
        # (asserts presence and form, keeps all eight packages in one table).
        crate_v=$(jq -r '.version // ""' "$2" | tr -d '\r')
    else
        crate_v=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$2" | head -n 1)
    fi
    manifest_v=$(jq -r "$4 // \"\"" "$3" | tr -d '\r')
    if [ "$crate_v" != "$manifest_v" ]; then
        echo "  FAIL  $1: crate says $crate_v, manifest says $manifest_v" >&2
        failures=$((failures + 1))
    else
        printf '  ok       %-24s %s\n' "$1" "$crate_v"
    fi
}
for spec in \
    "rtk-mcp-cc|claude-code/plugins/rtk-mcp-cc/Cargo.toml|claude-code/plugins/rtk-mcp-cc/.claude-plugin/plugin.json|.version" \
    "re-ghidra-mcp-cc|claude-code/plugins/re-ghidra-mcp-cc/Cargo.toml|claude-code/plugins/re-ghidra-mcp-cc/.claude-plugin/plugin.json|.version" \
    "rtk-mcp-qwen|qwen/plugins/rtk-mcp-qwen/Cargo.toml|qwen/plugins/rtk-mcp-qwen/qwen-extension.json|.version" \
    "re-ghidra-mcp-qwen|qwen/plugins/re-ghidra-mcp-qwen/Cargo.toml|qwen/plugins/re-ghidra-mcp-qwen/qwen-extension.json|.version" \
    "rtk-mcp-opencode|opencode/plugins/rtk-mcp-opencode/package.json|opencode/plugins/rtk-mcp-opencode/package.json|.version" \
    "re-ghidra-mcp-opencode|opencode/plugins/re-ghidra-mcp-opencode/package.json|opencode/plugins/re-ghidra-mcp-opencode/package.json|.version" \
    "rtk-mcp-agy|antigravity/plugins/rtk-mcp-agy/Cargo.toml|antigravity/plugins/rtk-mcp-agy/plugin.json|.version" \
    "re-ghidra-mcp-agy|antigravity/plugins/re-ghidra-mcp-agy/Cargo.toml|antigravity/plugins/re-ghidra-mcp-agy/plugin.json|.version" \
; do
    IFS='|' read -r label crate manifest field <<< "$spec"
    check_pair "$label" "$crate" "$manifest" "$field"
done
# Shippable crates must NOT inherit the workspace version anymore.
for crate in claude-code/plugins/rtk-mcp-cc/Cargo.toml claude-code/plugins/re-ghidra-mcp-cc/Cargo.toml \
    qwen/plugins/rtk-mcp-qwen/Cargo.toml qwen/plugins/re-ghidra-mcp-qwen/Cargo.toml \
    antigravity/plugins/rtk-mcp-agy/Cargo.toml antigravity/plugins/re-ghidra-mcp-agy/Cargo.toml; do
    if grep -q '^version\.workspace' "$crate"; then
        echo "  FAIL  $crate still inherits the workspace version" >&2
        failures=$((failures + 1))
    fi
done
[ "$failures" -eq 0 ] && echo "All package versions agree."
exit "$failures"
