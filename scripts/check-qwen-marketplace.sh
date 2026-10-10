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

source scripts/lib/registry.sh

agent=qwen
agent_dir=$(registry_field "$agent" dir)
source_base=$(registry_field "$agent" sourceUrlBase)
artifact_suffix=$(registry_field "$agent" artifactSuffix)

manifest=.qwen-plugin/marketplace.json

# qwen/plugins/example is a reference implementation people read, not something
# anyone installs; it is deliberately absent from the marketplace. The
# exclusion list lives in agents.json (notPublished); registry_is_excluded
# below is the only read path.

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

# The registry pins the release-asset base url; it must stay the workspace
# repository's releases/download endpoint so the marketplace url cannot quietly
# point at a different repo.
expected_base="$repo_url/releases/download"
[ "$source_base" = "$expected_base" ] || {
    echo "ERROR: registry sourceUrlBase for $agent ($source_base) is not $expected_base." >&2
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
    ext_json=$(registry_manifest_path "$agent" "$name")

    if [ ! -f "$ext_json" ]; then
        fail "$name: no such extension ($ext_json missing)"
        continue
    fi

    entry=$(jq --arg n "$name" '.plugins[] | select(.name == $n)' "$manifest")

    manifest_name=$(jqr '.name // ""' "$ext_json")
    [ "$manifest_name" = "$name" ] ||
        fail "$name: qwen-extension.json calls itself '$manifest_name'"

    # The bundle ships this qwen-extension.json alongside binaries built from
    # the package's own crate at that version, and with tag-pinned urls the
    # manifest version selects the release. Per-package releases bump crates
    # independently, so the crate — never the workspace version — is the
    # oracle here.
    crate_version=$(tr -d '\r' < "$agent_dir/$name/Cargo.toml" | sed -n 's/^version = "\(.*\)"$/\1/p' | head -n 1)
    [ -n "$crate_version" ] || fail "$name: no version in $agent_dir/$name/Cargo.toml"
    manifest_version=$(jqr '.version // ""' "$ext_json")
    [ "$manifest_version" = "$crate_version" ] ||
        fail "$name: qwen-extension.json says $manifest_version, crate is $crate_version"

    for field in description license; do
        want=$(jqr --arg f "$field" '.[$f] // ""' "$ext_json")
        got=$(jqr --arg f "$field" '.[$f] // ""' <<< "$entry")
        [ "$want" = "$got" ] || fail "$name: $field differs from $ext_json"
    done

    want_url="$source_base/$name-v$manifest_version/$name$artifact_suffix"
    got_url=$(jqr '.source.url // ""' <<< "$entry")
    [ "$want_url" = "$got_url" ] ||
        fail "$name: source url is '$got_url', expected '$want_url'"

    got_source=$(jqr '.source.source // ""' <<< "$entry")
    [ "$got_source" = "archive" ] ||
        fail "$name: source type is '$got_source', expected 'archive'"

    if jq -e 'has("version")' <<< "$entry" > /dev/null 2>&1; then
        fail "$name: entry declares a version; the tag-pinned release payload owns that"
    fi
done

# The reverse direction: an extension added to qwen/ and never listed here is
# an extension nobody can install, and nothing else in CI would notice.
for dir in "$agent_dir"/*/; do
    name=$(basename "$dir")
    [ -f "$(registry_manifest_path "$agent" "$name")" ] || continue
    if registry_is_excluded "$agent" "$name"; then continue; fi
    printf '%s\n' "$entries" | grep -qxF "$name" ||
        fail "$name: exists in $agent_dir/ but is not listed in $manifest"
done

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures Qwen marketplace problem(s)." >&2
    exit 1
fi

echo "Qwen marketplace lists $checked extension(s), all consistent with their manifests."
