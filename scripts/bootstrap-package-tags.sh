#!/usr/bin/env bash
# One-time bootstrap: tag every package <name>-v0.7.2 so release-please has a
# base. Dry-run unless --execute. Base = the v0.7.2 tag's commit.
set -euo pipefail
cd "$(dirname "$0")/.."
BASE=$(git rev-list -n 1 v0.7.2)
declare -A VERSION_FILE=(
  [rtk-mcp-cc]=claude-code/rtk-mcp-cc/.claude-plugin/plugin.json
  [re-ghidra-mcp-cc]=claude-code/re-ghidra-mcp-cc/.claude-plugin/plugin.json
  [rtk-mcp-qwen]=qwen/rtk-mcp-qwen/qwen-extension.json
  [re-ghidra-mcp-qwen]=qwen/re-ghidra-mcp-qwen/qwen-extension.json
  [rtk-mcp-opencode]=opencode/rtk-mcp-opencode/package.json
  [re-ghidra-mcp-opencode]=opencode/re-ghidra-mcp-opencode/package.json
  [rtk-mcp-agy]=antigravity/rtk-mcp-agy/plugin.json
  [re-ghidra-mcp-agy]=antigravity/re-ghidra-mcp-agy/plugin.json
)
for name in "${!VERSION_FILE[@]}"; do
  v=$(git show "$BASE:${VERSION_FILE[$name]}" | jq -r .version | tr -d '\r')
  [ "$v" = "0.7.2" ] || { echo "ABORT: $name is $v at base $BASE" >&2; exit 1; }
done
echo "base $BASE verified: all eight packages read 0.7.2"
for name in "${!VERSION_FILE[@]}"; do
  if git ls-remote origin "refs/tags/$name-v0.7.2" | grep -q .; then
    echo "exists, skipping $name-v0.7.2 (resume-safe)"
    continue
  fi
  if [ "${1:-}" = "--execute" ]; then
    git tag -a "$name-v0.7.2" "$BASE" -m "Bootstrap $name at 0.7.2"
    git push origin "$name-v0.7.2"
    git ls-remote origin "refs/tags/$name-v0.7.2" | grep -q . \
      || { echo "ABORT: tag $name-v0.7.2 did not land" >&2; exit 1; }
    echo "pushed $name-v0.7.2"
  else
    echo "would tag $name-v0.7.2 -> $BASE"
  fi
done
