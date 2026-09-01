#!/usr/bin/env bash
#
# Verify the Qwen marketplace manifest still describes the extensions in this repo.
#
# `.qwen-plugin/marketplace.json` is the Qwen equivalent of the Claude Code
# marketplace, and every field in it is a copy of something that lives somewhere
# else: the extension's own manifest, the release asset name the bundle workflow
# produces, the repository url. Nothing fails when a copy goes stale — the
# marketplace keeps installing, it just advertises the wrong thing.
set -euo pipefail

cd "$(dirname "$0")/.."

manifest=.qwen-plugin/marketplace.json

# The jq on a Windows PATH emits CRLF; a stray carriage return turns every
# comparison below into a mismatch and every path into one that does not exist.
jqr() {
    jq -r "$@" | tr -d '\r'
}

failures=0
fail() {
    echo "  FAIL  $1" >&2
    failures=$((failures + 1))
}

[ -f "$manifest" ] || {
    echo "ERROR: $manifest not found." >&2
    exit 1
}
jq -e . "$manifest" > /dev/null || {
    echo "ERROR: $manifest is not valid JSON." >&2
    exit 1
}

# The repository the release assets come from, taken from the workspace manifest
# so the url in the marketplace cannot quietly point at a different repo.
repo_url=$(sed -n 's/^repository = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ -n "$repo_url" ] || {
    echo "ERROR: no [workspace.package] repository found in Cargo.toml." >&2
    exit 1
}
repo_url=${repo_url%.git}

workspace_version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ -n "$workspace_version" ] || {
    echo "ERROR: no [workspace.package] version found in Cargo.toml." >&2
    exit 1
}

entries=$(jqr '.plugins[].name' "$manifest")
[ -n "$entries" ] || {
    echo "ERROR: $manifest lists no extensions — has the manifest shape changed?" >&2
    exit 1
}

checked=0
for name in $entries; do
    checked=$((checked + 1))
    ext_json="qwen/$name/qwen-extension.json"

    if [ ! -f "$ext_json" ]; then
        fail "$name: no such extension ($ext_json missing)"
        continue
    fi

    entry=$(jq --arg n "$name" '.plugins[] | select(.name == $n)' "$manifest")

    manifest_name=$(jqr '.name // ""' "$ext_json")
    [ "$manifest_name" = "$name" ] ||
        fail "$name: qwen-extension.json calls itself '$manifest_name'"

    # The bundle ships this qwen-extension.json alongside binaries built from the
    # workspace at that version, and with latest/download urls it is the only
    # version anyone sees.
    manifest_version=$(jqr '.version // ""' "$ext_json")
    [ "$manifest_version" = "$workspace_version" ] ||
        fail "$name: qwen-extension.json says $manifest_version, workspace is $workspace_version"

    for field in description license; do
        want=$(jqr --arg f "$field" '.[$f] // ""' "$ext_json")
        got=$(jqr --arg f "$field" '.[$f] // ""' <<< "$entry")
        [ "$want" = "$got" ] || fail "$name: $field differs from $ext_json"
    done

    want_url="$repo_url/releases/latest/download/$name-extension.zip"
    got_url=$(jqr '.source.url // ""' <<< "$entry")
    [ "$want_url" = "$got_url" ] ||
        fail "$name: source url is '$got_url', expected '$want_url'"

    got_source=$(jqr '.source.source // ""' <<< "$entry")
    [ "$got_source" = "archive" ] ||
        fail "$name: source type is '$got_source', expected 'archive'"

    if jq -e 'has("version")' <<< "$entry" > /dev/null 2>&1; then
        fail "$name: entry declares a version; the latest/download payload owns that"
    fi
done

# The reverse direction: an extension added to qwen/ and never listed here is
# an extension nobody can install, and nothing else in CI would notice.
for dir in qwen/*/; do
    name=$(basename "$dir")
    [ -f "$dir/qwen-extension.json" ] || continue
    printf '%s\n' "$entries" | grep -qxF "$name" ||
        fail "$name: exists in qwen/ but is not listed in $manifest"
done

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures Qwen marketplace problem(s)." >&2
    exit 1
fi

echo "Qwen marketplace lists $checked extension(s), all consistent with their manifests."
