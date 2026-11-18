#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 together with the
# additional terms (evaluation grant, anti-circumvention) that apply to this
# repository. See the LICENSE file at the repository root.

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
