#!/usr/bin/env bash
#
# Verify the release-please manifest still tracks the plugin registry.
#
# `.release-please-manifest.json` maps each release-please package path to its
# last released version, and every key in it is a copy of something owned
# elsewhere: the package paths enumerated from agents.json. Nothing fails when
# a key goes stale — a move that repoints the registry (and the generated
# release-please-config.json) without renaming the manifest keys leaves
# release-please facing configured packages with no recorded version, which it
# treats as brand-new packages and re-releases. That is the exact failure this
# catches.
#
# Keys only, never values: the recorded versions are release-please's own
# state, and asserting them here would fight the tool that owns them.
set -euo pipefail

cd "$(dirname "$0")/.."

source scripts/lib/registry.sh

manifest=.release-please-manifest.json

# The jq on a Windows PATH emits CRLF; strip it exactly like the other checks.
jqr() {
    jq -r "$@" | tr -d '\r'
}

failures=0
checked=0
fail() { echo "  FAIL  $1" >&2; failures=$((failures + 1)); }

[ -f "$manifest" ] || {
    echo "ERROR: $manifest not found." >&2
    exit 1
}
jqr -e . "$manifest" > /dev/null || {
    echo "ERROR: $manifest is not valid JSON." >&2
    exit 1
}

# Every registry package must have a manifest entry: a configured path with
# no recorded version reads as a new package on the next release-please run.
for agent in claude-code qwen opencode antigravity; do
    dir=$(registry_field "$agent" dir)
    [ -n "$dir" ] || { fail "$agent: registry has no dir"; continue; }
    for name in $(registry_plugins "$agent" | sort); do
        path="$dir/$name"
        checked=$((checked + 1))
        if jqr -e --arg p "$path" '.[$p] | strings' "$manifest" > /dev/null; then
            printf '  ok       %-44s %s\n' "$path" "$(jqr -r --arg p "$path" '.[$p]' "$manifest")"
        else
            fail "$path: no entry in $manifest (release-please would treat it as a new package)"
        fi
    done
done

# The reverse direction: a manifest entry for a path nothing configures is a
# package that was moved or removed without renaming its recorded version.
# release-please prunes such entries on its next run, silently dropping the
# version history that pins the old release tags.
for key in $(jqr -r 'keys[]' "$manifest" | sort); do
    found=0
    for agent in claude-code qwen opencode antigravity; do
        dir=$(registry_field "$agent" dir)
        for name in $(registry_plugins "$agent"); do
            [ "$key" = "$dir/$name" ] && found=1
        done
    done
    checked=$((checked + 1))
    [ "$found" -eq 1 ] || fail "$key: in $manifest but no registry package lives there"
done

# A check that silently checks nothing is worse than no check at all: it reports
# success forever. If the registry or the manifest shape changes so the loops
# above stop matching, say so and fail.
if [ "$checked" -eq 0 ]; then
    echo "ERROR: found no manifest entries to check." >&2
    echo "       Either the registry queries or the manifest shape has gone stale." >&2
    exit 1
fi

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures release-manifest problem(s)." >&2
    echo "       Rename the manifest keys to the registry paths (versions untouched)." >&2
    exit 1
fi

echo "Release manifest tracks $checked registry path(s)."
