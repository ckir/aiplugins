#!/usr/bin/env bash
#
# Regenerate the `packages` table in release-please-config.json from agents.json.
#
# The package list is a copy of something owned elsewhere — the plugin registry
# (Plan A agents.json) — so the committed table is generated rather than
# hand-edited: `bash scripts/gen-release-please-packages.sh` rewrites ONLY the
# top-level `packages` key (every other top-level key keeps its value), and
# `--check` exits non-zero naming the drift, for CI.
#
# Enumeration comes from agents.json via scripts/lib/registry.sh, in registry
# agent/plugin order. Per package:
#   path         - "<agent dir>/<plugin>" (the release-please package path).
#   release-type - "rust" when the package ships a crate (Cargo.toml exists:
#     the rust strategy owns the crate version), else "simple" (the opencode
#     plugins ship no crates; the version lives in package.json alone).
#   extra-files  - the registry manifestPattern for that agent/plugin (the host
#     manifest whose `version` key release-please bumps alongside the release),
#     RELATIVE to the package directory: release-please resolves extra-files
#     against the package path, so emitting the repo-rooted manifest path would
#     make it look for <package>/<package>/... (observed live: every extra-file
#     fetched 404 on the first release-please run).
#
# Crate-vs-extra-files rule: the rust strategy owns the crate version and
# extra-files owns the host manifest; the two must never target the same file.
# The manifests are host JSONs by construction (registry manifestPattern), and
# the generator refuses to emit an extra-file pointing at a Cargo.toml.
set -euo pipefail

cd "$(dirname "$0")/.."

source scripts/lib/registry.sh

check_only=0
if [ "${1:-}" = "--check" ]; then
    check_only=1
fi

# Plan A (agents.json plugin registry) is the prerequisite input. Without it
# there is nothing to enumerate from, so stop instead of guessing.
[ -f agents.json ] || {
    echo "ERROR: agents.json not found. Plan A (plugin registry) must land first." >&2
    exit 1
}

# The jq on a Windows PATH emits CRLF; a stray carriage return turns every
# comparison below into a mismatch and every path into one that does not exist.
jqr() {
    jq -r "$@" | tr -d '\r'
}

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

: > "$tmpdir/entries.jsonl"
for agent in $(jqr '.agents[].id' agents.json); do
    agent_dir=$(registry_field "$agent" dir)
    for plugin in $(registry_plugins "$agent"); do
        key="$agent_dir/$plugin"
        manifest=$(registry_manifest_path "$agent" "$plugin")
        case "$manifest" in
            *Cargo.toml)
                echo "ERROR: manifestPattern for '$key' resolves to a Cargo.toml ($manifest): extra-files must list host manifests only, never Cargo.toml." >&2
                exit 1
                ;;
        esac
        case "$manifest" in
            "$key"/*)
                # release-please joins extra-files onto the package path.
                manifest="${manifest#"$key"/}"
                ;;
            *)
                echo "ERROR: manifestPattern for '$key' does not live under the package dir ($manifest): extra-files must be package-relative." >&2
                exit 1
                ;;
        esac
        if [ -f "$key/Cargo.toml" ]; then
            release_type="rust"
        else
            release_type="simple"
        fi
        jq -n \
            --arg key "$key" \
            --arg release_type "$release_type" \
            --arg manifest "$manifest" \
            '{"key": $key, "value": {"release-type": $release_type, "extra-files": [$manifest]}}' \
            >> "$tmpdir/entries.jsonl"
    done
done

jq -s 'from_entries' "$tmpdir/entries.jsonl" > "$tmpdir/expected.json"

config="release-please-config.json"
if [ ! -f "$config" ]; then
    if [ "$check_only" -eq 1 ]; then
        echo "DRIFT: $config does not exist. Run 'bash scripts/gen-release-please-packages.sh' and commit." >&2
        exit 1
    fi
    jq -n --arg schema "https://raw.githubusercontent.com/googleapis/release-please/main/schemas/config.json" \
        --slurpfile packages "$tmpdir/expected.json" \
        '{"$schema": $schema, "packages": $packages[0]}' > "$config"
    echo "$config created."
    exit 0
fi

jq -S '.packages' "$config" > "$tmpdir/actual.json"
jq -S '.' "$tmpdir/expected.json" > "$tmpdir/expected-sorted.json"

if cmp -s "$tmpdir/expected-sorted.json" "$tmpdir/actual.json"; then
    if [ "$check_only" -eq 1 ]; then
        echo "release-please packages match the registry."
    else
        echo "release-please packages already current."
    fi
    exit 0
fi

missing=$(jq -r -n --slurpfile e "$tmpdir/expected-sorted.json" --slurpfile a "$tmpdir/actual.json" '(($e[0] | keys) - ($a[0] | keys)) | .[]')
extra=$(jq -r -n --slurpfile e "$tmpdir/expected-sorted.json" --slurpfile a "$tmpdir/actual.json" '(($a[0] | keys) - ($e[0] | keys)) | .[]')

if [ "$check_only" -eq 1 ]; then
    {
        echo "DRIFT: $config packages do not match the registry in agents.json."
        [ -n "$missing" ] && printf '  missing: %s\n' "$missing"
        [ -n "$extra" ] && printf '  extra: %s\n' "$extra"
        echo "Run 'bash scripts/gen-release-please-packages.sh' and commit."
    } >&2
    exit 1
fi

jq --slurpfile packages "$tmpdir/expected.json" '.packages = $packages[0]' "$config" > "$tmpdir/config.json"
cp "$tmpdir/config.json" "$config"
echo "$config packages regenerated from the registry."
