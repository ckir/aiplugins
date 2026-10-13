#!/usr/bin/env bash
# Print an agent's registry plugin keys (the folder names), one per line.
# Usage: registry-plugins.sh <agent-id> [--exclude-skills-only]
set -euo pipefail

cd "$(dirname "$0")/.."

source scripts/lib/registry.sh

[ $# -ge 1 ] || {
    echo "usage: $0 <agent-id> [--exclude-skills-only]" >&2
    exit 2
}
agent=$1
exclude=0
[ "${2:-}" = "--exclude-skills-only" ] && exclude=1

while IFS= read -r plugin; do
    if [ "$exclude" -eq 1 ] && registry_is_skills_only "$agent" "$plugin"; then
        continue
    fi
    printf '%s\n' "$plugin"
done < <(registry_plugins "$agent")
