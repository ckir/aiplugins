#!/usr/bin/env bash
# Print an agent's registry plugin keys (the folder names), one per line.
# Usage: registry-plugins.sh <agent-id> [--exclude-skills-only]
set -euo pipefail

cd "$(dirname "$0")/.."

# Capture the arguments and clear them BEFORE sourcing: scripts/lib/registry.sh
# reads ${1:-} at source time and would run its own --self-test.
args=("$@")
set --

# shellcheck source=/dev/null
source scripts/lib/registry.sh

usage() {
    echo "usage: $0 <agent-id> [--exclude-skills-only]" >&2
    exit 2
}

{ [ "${#args[@]}" -ge 1 ] && [ "${#args[@]}" -le 2 ]; } || usage
agent=${args[0]}
exclude=0
if [ "${#args[@]}" -eq 2 ]; then
    [ "${args[1]}" = "--exclude-skills-only" ] || usage
    exclude=1
fi
[ -n "$agent" ] && [ "$(registry_field "$agent" id)" = "$agent" ] || {
    echo "unknown agent id: $agent" >&2
    usage
}

while IFS= read -r plugin; do
    if [ "$exclude" -eq 1 ] && registry_is_skills_only "$agent" "$plugin"; then
        continue
    fi
    printf '%s\n' "$plugin"
done < <(registry_plugins "$agent")
