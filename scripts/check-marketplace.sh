#!/usr/bin/env bash
#
# Verify the root marketplace manifest still describes the plugins in this repo.
#
# `.claude-plugin/marketplace.json` is what `claude plugin marketplace add
# ckir/aiplugins` reads, and every field in it is a copy of something that lives
# somewhere else: the plugin's own manifest, the release asset name the bundle
# workflow produces, the repository url. Nothing fails when a copy goes stale —
# the marketplace keeps installing, it just advertises the wrong thing, or
# fetches an asset nobody publishes any more.
#
# The one thing an entry must NOT carry is a version. The entries point at
# `releases/latest/download/...`, so the payload's own plugin.json is the only
# honest answer to "which version is this"; a version here would be a second
# answer that drifts the moment a release lands.
set -euo pipefail

cd "$(dirname "$0")/.."

manifest=.claude-plugin/marketplace.json

# claude-code/example is a reference implementation people read, not something
# anyone installs; it is deliberately absent from the marketplace.
not_published="example"

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
    echo "ERROR: $manifest lists no plugins — has the manifest shape changed?" >&2
    exit 1
}

checked=0
for name in $entries; do
    checked=$((checked + 1))
    plugin_json="claude-code/$name/.claude-plugin/plugin.json"

    if [ ! -f "$plugin_json" ]; then
        fail "$name: no such plugin ($plugin_json missing)"
        continue
    fi

    entry=$(jq --arg n "$name" '.plugins[] | select(.name == $n)' "$manifest")

    manifest_name=$(jqr '.name' "$plugin_json")
    [ "$manifest_name" = "$name" ] ||
        fail "$name: plugin.json calls itself '$manifest_name'"

    # The bundle ships this plugin.json alongside binaries built from the
    # workspace at that version, and with latest/download urls it is the only
    # version anyone sees. It sat three releases behind before this check.
    manifest_version=$(jqr '.version // ""' "$plugin_json")
    [ "$manifest_version" = "$workspace_version" ] ||
        fail "$name: plugin.json says $manifest_version, workspace is $workspace_version"

    for field in description license; do
        want=$(jqr --arg f "$field" '.[$f] // ""' "$plugin_json")
        got=$(jqr --arg f "$field" '.[$f] // ""' <<< "$entry")
        [ "$want" = "$got" ] || fail "$name: $field differs from $plugin_json"
    done

    want_url="$repo_url/releases/latest/download/$name-plugin.zip"
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

# The reverse direction: a plugin added to claude-code/ and never listed here is
# a plugin nobody can install, and nothing else in CI would notice.
for dir in claude-code/*/; do
    name=$(basename "$dir")
    [ -f "$dir.claude-plugin/plugin.json" ] || continue
    case " $not_published " in
        *" $name "*) continue ;;
    esac
    printf '%s\n' "$entries" | grep -qxF "$name" ||
        fail "$name: exists in claude-code/ but is not listed in $manifest"
done

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures marketplace problem(s)." >&2
    exit 1
fi

echo "Marketplace lists $checked plugin(s), all consistent with their manifests."
