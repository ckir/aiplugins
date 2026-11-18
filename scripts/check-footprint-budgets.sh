#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 together with the
# additional terms (evaluation grant, anti-circumvention) that apply to this
# repository. See the LICENSE file at the repository root.

# Every key in docs/footprints/budgets.json must name a live plugin.
#
# The ratchet only inserts and updates keys, so a renamed or removed plugin
# leaves its entry behind, and a later plugin reusing that name would silently
# inherit its ceiling. Pruning automatically would put the ratchet history at
# the mercy of a plugin that is briefly unbuildable, so this fails instead and
# leaves the deletion to the commit that renames or removes the plugin.
#
# Reads the working tree, not a base ref: footprint-gate compares against the
# base ref's budgets, and a check there would fail every PR that removes a plugin.
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=lib/registry.sh disable=SC1091
source scripts/lib/registry.sh

budgets=${1:-docs/footprints/budgets.json}

# Live plugins: every registry key on both fronts. Skills-only plugins are
# measured too — a footprint doc without a budget key is unguarded.
live=$(
    bash scripts/registry-plugins.sh claude-code | tr -d '\r'
    bash scripts/registry-plugins.sh opencode | tr -d '\r'
    bash scripts/registry-plugins.sh antigravity | tr -d '\r'
)

failures=0
while IFS= read -r key; do
    key=${key%$'\r'}
    [ -n "$key" ] || continue
    if ! grep -qxF -- "$key" <<<"$live"; then
        echo "  FAIL  $budgets has the key '$key', which names no live plugin; delete it by hand if the plugin was renamed or removed" >&2
        failures=$((failures + 1))
    fi
done < <(jq -r 'keys[]' "$budgets")

[ "$failures" -eq 0 ] && echo "Footprint budgets name only live plugins."
exit "$failures"
