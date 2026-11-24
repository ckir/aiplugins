#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Copy the Rust toolchain scaffold into a repository without overwriting anything.
#
#   apply.sh <target-repo> <code-owner>     e.g. apply.sh . @octocat
#
# Every file under ../assets/template/ is stored as <name>.tmpl and is copied to
# <name>. A file that already exists in the target is never changed: it is listed
# as SKIPPED, marked identical to the template or different from it, so that
# running this again on a scaffolded repository doubles as a drift report. A
# justfile counts as existing under any spelling `just` accepts (justfile,
# Justfile, .justfile), because `just` refuses to run when two coexist.
# Nothing is committed. Run `git status` afterwards.
set -euo pipefail

target="${1:?usage: apply.sh <target-repo> <code-owner>}"
owner="${2:?usage: apply.sh <target-repo> <code-owner> (for example @octocat or @org/team)}"
case "$owner" in
  @*) ;;
  *) echo "apply.sh: the code owner must start with @ (got: $owner)" >&2; exit 2 ;;
esac
# The owner becomes sed replacement text, so allow only what a GitHub handle holds.
case "${owner#@}" in
  "" | *[!A-Za-z0-9_/-]* | /* | */ | */*/*)
    echo "apply.sh: '$owner' is not an @user or @org/team handle" >&2; exit 2 ;;
esac

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
template="$here/../assets/template"
[ -d "$template" ] || { echo "apply.sh: no template directory at $template" >&2; exit 2; }
target="$(cd -- "$target" && pwd -P)"
git -C "$target" rev-parse --git-dir >/dev/null 2>&1 || {
  echo "apply.sh: $target is not a git repository (run git init first)" >&2; exit 2; }
# The files belong at the top of the repository: hooks, CODEOWNERS and workflows are read from
# there only. --show-prefix is the target's path below the top level, empty exactly at the top, so
# no two spellings of a path are compared (on Windows, Git Bash and git can spell a temp directory
# differently).
prefix="$(git -C "$target" rev-parse --show-prefix)"
[ -z "$prefix" ] || {
  echo "apply.sh: $target is inside the repository at $(git -C "$target" rev-parse --show-toplevel), not its top level; run it on the top level" >&2; exit 2; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# `find -iname` is in both GNU and BSD find; `just` accepts the name in any case.
existing_justfile="$(find "$target" -maxdepth 1 \( -iname justfile -o -iname .justfile \) -print | head -n 1)"

copied="" identical="" differs=""
while IFS= read -r -d '' file; do
  rel="${file#"$template"/}"
  case "$rel" in
    *.tmpl) ;;
    *) echo "apply.sh: template file without the .tmpl suffix: $rel" >&2; exit 1 ;;
  esac
  dest="${rel%.tmpl}"
  rendered="$tmp/rendered"
  # The file is reused for every template; a read-only one left by the last round would refuse the next write.
  rm -f -- "$rendered"
  if [ "$dest" = ".github/CODEOWNERS" ]; then
    sed "s|@OWNER|$owner|g" "$file" > "$rendered"
  else
    cp -- "$file" "$rendered"
  fi

  existing="$target/$dest"
  if [ "$dest" = "justfile" ] && [ -n "$existing_justfile" ]; then
    existing="$existing_justfile"
    dest="${existing#"$target"/}"
  fi
  # A dangling symlink is not -e but is something at the destination: never write through it.
  if [ -e "$existing" ] || [ -L "$existing" ]; then
    if cmp -s -- "$rendered" "$existing"; then
      identical="$identical$dest"$'\n'
    else
      differs="$differs$dest"$'\n'
    fi
    continue
  fi
  mkdir -p -- "$(dirname -- "$target/$dest")"
  cp -- "$rendered" "$target/$dest"
  # A template installed read-only (a read-only skill directory) must not make the copy read-only.
  chmod u+w -- "$target/$dest"
  copied="$copied$dest"$'\n'
done < <(find "$template" -type f -print0 | sort -z)

lock_failed=0
if printf '%s' "$copied" | grep -qx 'toolchain-tests/Cargo.toml'; then
  if command -v cargo >/dev/null 2>&1; then
    # The lock is generated here, not shipped: a shipped one would be stale on arrival.
    (cd "$target" && cargo generate-lockfile --manifest-path toolchain-tests/Cargo.toml) || lock_failed=1
  else
    lock_failed=1
  fi
fi

count() { if [ -z "$1" ]; then echo 0; else printf '%s' "$1" | grep -c .; fi; }
list() { if [ -n "$1" ]; then printf '%s' "$1" | sed 's/^/  /'; fi; }

echo "COPIED ($(count "$copied")):"
list "$copied"
echo "SKIPPED, identical to the template ($(count "$identical")):"
list "$identical"
echo "SKIPPED, already present and DIFFERENT - merge by hand, keeping the user's content ($(count "$differs")):"
list "$differs"
echo "NOT TOUCHED: the agent instructions file, README.md - see assets/snippets/ in the skill."

if [ "$lock_failed" -ne 0 ]; then
  echo "ERROR: toolchain-tests/Cargo.lock was not generated. Run this, with network access, then commit it:" >&2
  echo "  cargo generate-lockfile --manifest-path toolchain-tests/Cargo.toml" >&2
fi
if printf '%s' "$differs" | grep -qx '.gitignore'; then
  echo "NOTE: .gitignore already existed. It needs /target, and toolchain-tests/.gitignore covers the test crate."
fi
if [ -f "$target/Cargo.toml" ] && grep -q '^\[workspace\]' "$target/Cargo.toml"; then
  echo "NOTE: the root Cargo.toml is a workspace. toolchain-tests/ has its own [workspace] table; if a members glob"
  echo "      could match it, add it to the root \`exclude\` list or cargo fails with 'multiple workspace roots'."
fi
if [ -n "$existing_justfile" ]; then
  case "$(basename -- "$existing_justfile")" in
    justfile | Justfile | .justfile) ;;
    *) echo "NOTE: the justfile is spelled $(basename -- "$existing_justfile"). just accepts that, but CODEOWNERS and the"
       echo "      tests know only justfile, Justfile and .justfile; rename it or extend both." ;;
  esac
fi
others="$(find "$target/.github/workflows" -maxdepth 1 -type f \( -name '*.yml' -o -name '*.yaml' \) ! -name ci.yml 2>/dev/null | sort || true)"
if [ -n "$others" ]; then
  echo "NOTE: other workflow files exist. The gate tests check every one of them for an env key from the"
  echo "      JUST_, GITLEAKS_, LEFTHOOK, HOOK_ARG_, NEXTEST_ and TYPOS_ families and for a job named like the gate job (check):"
  printf '%s\n' "$others" | sed "s|^$target/|  |"
fi
[ "$lock_failed" -eq 0 ] || exit 1
