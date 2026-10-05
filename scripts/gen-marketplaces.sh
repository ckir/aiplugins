#!/usr/bin/env bash
#
# Regenerate the host marketplace manifests from the per-plugin manifests.
#
# Each entry in .claude-plugin/marketplace.json / .qwen-plugin/marketplace.json
# is a copy of something owned elsewhere, so the committed copies are generated
# rather than hand-edited: `bash scripts/gen-marketplaces.sh` rewrites them,
# and `--check` fails when they are stale (the same way a stale footprint
# fails). The committed files are the oracle: this script must reproduce them
# byte-for-byte, so field order and whitespace are load-bearing below (jq with
# --sort-keys off, explicit field construction, 2-space indent).
#
# Enumeration comes from agents.json via scripts/lib/registry.sh: every agent
# with a non-null `marketplace` gets its manifest regenerated, in registry
# plugin order. Agents with null marketplace (opencode, antigravity) are
# skipped, so a future agent only needs a registry row to join in.
#
# Field provenance per entry (key order: name, source, description, author,
# homepage, license, category, keywords):
#   name/description/license - the plugin's own manifest, the same
#     fields the marketplace checks verify.
#   source.url - registry sourceUrlBase + plugin name + artifactSuffix.
#   author.name - the manifest's own author when its schema has one, else the
#     workspace owner from Cargo.toml (a marketplace-wide fact).
#   homepage - $repo/tree/main/$agent_dir/$plugin; the manifest homepage is
#     only the repo root, so the per-plugin tree URL is derived (repo URL via
#     the same sed extraction the checks use).
#   category - the literal "development": no manifest schema carries it, and it
#     is the only value ever published.
#   keywords - the manifest's own keywords when its schema has them (only the
#     Claude plugin schema does); otherwise the keywords of the same plugin
#     family's manifest from any registered agent that declares them. The top
#     level $schema/name/owner/metadata blocks are preserved verbatim from the
#     committed file: they describe the marketplace itself, not the plugins.
set -euo pipefail

cd "$(dirname "$0")/.."

source scripts/lib/registry.sh

check_only=0
if [ "${1:-}" = "--check" ]; then
    check_only=1
fi

# The jq on a Windows PATH emits CRLF; a stray carriage return turns every
# comparison below into a mismatch and every path into one that does not exist.
jqr() {
    jq -r "$@" | tr -d '\r'
}

# The repository the release assets come from, taken from the workspace
# manifest (same sed extraction the marketplace checks use).
repo_url=$(sed -n 's/^repository = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ -n "$repo_url" ] || {
    echo "ERROR: no [workspace.package] repository found in Cargo.toml." >&2
    exit 1
}
repo_url=${repo_url%.git}

workspace_author=$(sed -n 's/^authors = \["\(.*\)"\]$/\1/p' Cargo.toml | head -n 1)
[ -n "$workspace_author" ] || {
    echo "ERROR: no [workspace.package] authors found in Cargo.toml." >&2
    exit 1
}

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

# Keywords live only in the Claude plugin manifest schema. For a plugin whose
# own manifest has none, fall back to the same family's manifest from any
# registered agent that declares them (family = name minus its agent suffix:
# rtk-mcp-qwen shares rtk-mcp-cc's keywords). Empty array when nobody has any.
family_keywords() {
    local stem=${1%-*}
    local a2 p2 m2 kw
    for a2 in $(jqr '.agents[].id' agents.json); do
        for p2 in $(registry_plugins "$a2"); do
            case "$p2" in
                "$stem"-*)
                    m2=$(registry_manifest_path "$a2" "$p2")
                    [ -f "$m2" ] || continue
                    kw=$(jqr -c '.keywords // empty' "$m2")
                    if [ -n "$kw" ] && [ "$kw" != "[]" ]; then
                        printf '%s\n' "$kw"
                        return 0
                    fi
                    ;;
            esac
        done
    done
    printf '[]\n'
}

failures=0

for agent in $(jqr '.agents[] | select(.marketplace != null) | .id' agents.json); do
    marketplace=$(jqr --arg a "$agent" '.agents[] | select(.id == $a) | .marketplace' agents.json)
    agent_dir=$(registry_field "$agent" dir)
    source_base=$(registry_field "$agent" sourceUrlBase)
    artifact_suffix=$(registry_field "$agent" artifactSuffix)

    [ -f "$marketplace" ] || {
        echo "ERROR: $marketplace not found." >&2
        exit 1
    }

    : > "$tmpdir/entries.jsonl"
    for plugin in $(registry_plugins "$agent"); do
        plugin_json=$(registry_manifest_path "$agent" "$plugin")
        [ -f "$plugin_json" ] || {
            echo "ERROR: $plugin_json not found." >&2
            exit 1
        }

        name=$(jqr '.name' "$plugin_json")
        description=$(jqr '.description' "$plugin_json")
        license=$(jqr '.license' "$plugin_json")
        author=$(jqr '.author.name // ""' "$plugin_json")
        [ -n "$author" ] || author=$workspace_author
        keywords=$(jqr -c '.keywords // empty' "$plugin_json")
        if [ -z "$keywords" ] || [ "$keywords" = "[]" ]; then
            keywords=$(family_keywords "$plugin")
        fi

        jq -n \
            --arg name "$name" \
            --arg url "$source_base/$plugin$artifact_suffix" \
            --arg description "$description" \
            --arg author "$author" \
            --arg homepage "$repo_url/tree/main/$agent_dir/$plugin" \
            --arg license "$license" \
            --argjson keywords "$keywords" \
            '{"name": $name, "source": {"source": "archive", "url": $url}, "description": $description, "author": {"name": $author}, "homepage": $homepage, "license": $license, "category": "development", "keywords": $keywords}' \
            >> "$tmpdir/entries.jsonl"
    done

    jq -s '.' "$tmpdir/entries.jsonl" > "$tmpdir/plugins.json"
    jq --slurpfile plugins "$tmpdir/plugins.json" \
        '{"$schema": .["$schema"], "name": .name, "owner": .owner, "metadata": .metadata, "plugins": $plugins[0]}' \
        "$marketplace" > "$tmpdir/marketplace.json"

    if [ "$check_only" -eq 1 ]; then
        if ! cmp -s "$tmpdir/marketplace.json" "$marketplace"; then
            echo "STALE: $marketplace does not match regenerated output for agent '$agent'. Run 'bash scripts/gen-marketplaces.sh' and commit." >&2
            failures=$((failures + 1))
        fi
    else
        cp "$tmpdir/marketplace.json" "$marketplace"
    fi
done

if [ "$failures" -gt 0 ]; then
    exit 1
fi

if [ "$check_only" -eq 1 ]; then
    echo "Marketplaces are current."
else
    echo "Marketplaces regenerated."
fi
