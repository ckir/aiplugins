#!/usr/bin/env bash
# Copyright 2026 AI Plugins Marketplace Contributors
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 together with the
# additional terms (evaluation grant, anti-circumvention) that apply to this
# repository. See the LICENSE file at the repository root.

# Fail if any in-scope source file lacks a license header (google/addlicense -check).
# Usage: check-license-headers.sh [FILE...]   (no args = all tracked files)
# Scope mirrors the header rollout: .rs .sh .ts .js .py .ps1, minus third-party
# skill copies, generated site output, and opencode plugin.ts (its byte size is the
# plugin's per-request footprint, so a header there fails the Plugin Footprint CI check). Install: go install github.com/google/addlicense@v1.2.0
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
command -v addlicense >/dev/null || { echo "addlicense not found: go install github.com/google/addlicense@v1.2.0" >&2; exit 2; }

if [[ $# -gt 0 ]]; then files=("$@"); else mapfile -t files < <(git ls-files); fi
mapfile -t files < <(printf '%s\n' "${files[@]}" \
  | grep -E '\.(rs|sh|ts|js|py|ps1)$' \
  | grep -vE '^(\.claude|\.agents)/skills/|^(docs|tests)/site/|^opencode/plugins/[^/]+/plugin\.ts$' || true)
[[ ${#files[@]} -eq 0 ]] && exit 0

# -check only tests that a copyright/SPDX header exists; -l/-f/-c are required args, not validated.
exec addlicense -check -f LICENSE-HEADER.tmpl -l PolyForm-Noncommercial-1.0.0 \
  -c "AI Plugins Marketplace Contributors" "${files[@]}"
