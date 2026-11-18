#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Wrapper around google/addlicense for custom (-f template) licenses.
# Usage: license-headers.sh apply|check --template F --spdx ID --holder NAME [--year Y] [--ignore GLOB]... PATH...
set -euo pipefail

mode="${1:-}"; shift || true
[[ "$mode" == apply || "$mode" == check ]] || { echo "usage: $0 apply|check --template F --spdx ID --holder NAME [--year Y] [--ignore GLOB]... PATH..." >&2; exit 2; }

template="" spdx="" holder="" year="$(date +%Y)"
ignores=('**/target/**' '**/node_modules/**' '**/.git/**' '**/dist/**' '**/generated/**' '**/*.lock')
paths=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --template) template="$2"; shift 2;;
    --spdx)     spdx="$2"; shift 2;;
    --holder)   holder="$2"; shift 2;;
    --year)     year="$2"; shift 2;;
    --ignore)   ignores+=("$2"); shift 2;;
    *)          paths+=("$1"); shift;;
  esac
done

command -v addlicense >/dev/null || { echo "addlicense not found: go install github.com/google/addlicense@v1.2.0" >&2; exit 2; }
[[ -f "$template" ]] || { echo "template not found: '$template'" >&2; exit 2; }
[[ -n "$spdx" ]]   || { echo "--spdx is required (addlicense defaults to Apache-2.0 otherwise)" >&2; exit 2; }
[[ -n "$holder" ]] || { echo "--holder is required" >&2; exit 2; }
[[ ${#paths[@]} -gt 0 ]] || { echo "no PATH given" >&2; exit 2; }

# addlicense treats a file as already licensed only if the first 1000 bytes contain one of these.
if ! grep -qiE 'copyright|spdx-license-identifier' "$template"; then
  echo "template has no 'Copyright' or 'SPDX-License-Identifier': addlicense would re-add the header on every run" >&2
  exit 2
fi

args=(-f "$template" -l "$spdx" -c "$holder" -y "$year")
for g in "${ignores[@]}"; do args+=(-ignore "$g"); done
[[ "$mode" == check ]] && args+=(-check) || args+=(-v)

exec addlicense "${args[@]}" "${paths[@]}"
