#!/usr/bin/env bash
#
# Assemble one installable zip for a skills-only plugin (any agent).
#
# Skills-only plugins carry no binaries and no plugin.ts: the zip is the
# plugin directory's versioned source (manifest, README.md, skills/...) plus
# the repo LICENSE. The top level of the zip IS the plugin directory — no
# wrapper folder.
#
# The bundle is staged in a script-owned mktemp dir inside <out-dir> that is
# removed on exit (it is not left in place); only the zip remains. The staging
# dir's children are zipped by name rather than `.`: some zip
# implementations keep the `./` prefix, and installers look for the manifest
# by exact entry name.
#
# Usage: scripts/bundle-skills-plugin.sh <agent-id> <plugin-key> <out-dir> [<plugin-dir>]
#
#   <agent-id>    agent id in agents.json (e.g. claude-code)
#   <plugin-key>  plugin key; must be listed in that agent's skillsOnly
#   <out-dir>     receives <plugin-key><artifactSuffix> only; the staging
#                 dir (.stage.XXXXXX) is temporary and removed on exit
#   <plugin-dir>  optional source override (default <dir>/<plugin-key>), so
#                 tests can bundle a throwaway copy
set -euo pipefail

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
    echo "usage: $0 <agent-id> <plugin-key> <out-dir> [<plugin-dir>]" >&2
    exit 2
fi

agent=$1
key=$2
outdir=$3

repo=$(cd "$(dirname "$0")/.." && pwd)

for tool in jq tar unzip zip zipinfo; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "ERROR: $tool is required and not on PATH." >&2
        exit 1
    }
done

# Info-ZIP under Windows records FAT attributes (entries lose unix modes), and
# MSYS/Cygwin translation can corrupt the source tree. WSL counts as Linux.
case $(uname -s) in
    MINGW* | MSYS* | CYGWIN*)
        echo "ERROR: skills-only bundles cannot be built from a Windows shell." >&2
        echo "       Use WSL, Linux, or macOS — see the comments in this script." >&2
        exit 1
        ;;
esac

# shellcheck source=lib/registry.sh disable=SC1091
source "$repo/scripts/lib/registry.sh"

registry_is_skills_only "$agent" "$key" || {
    echo "ERROR: '$key' is not a skills-only plugin of agent '$agent' in agents.json." >&2
    exit 1
}

dir=$(registry_field "$agent" dir)
suffix=$(registry_field "$agent" artifactSuffix)
manifest=$(registry_manifest_path "$agent" "$key")
manifest_rel=${manifest#"$dir/$key/"}

src=${4:-"$repo/$dir/$key"}
[ -d "$src" ] || {
    echo "ERROR: plugin directory $src not found." >&2
    exit 1
}
src=$(cd -P -- "$src" && pwd -P)

# Physical absolute path of a possibly-not-yet-existing directory: resolve the
# nearest existing ancestor with `pwd -P`, then re-append the missing tail.
physpath() {
    local p=$1 tail=""
    while [ ! -d "$p" ]; do
        tail="/$(basename -- "$p")$tail"
        p=$(dirname -- "$p")
    done
    printf '%s%s\n' "$(cd -P -- "$p" && pwd -P)" "$tail"
}

# Defense in depth: refuse an out-dir that overlaps the source tree. The
# structural protection is that the script only ever deletes a staging dir it
# created itself with mktemp (below), never a pre-existing path.
check_overlap() {
    local out_phys=$1
    case "$out_phys/" in
        "$src"/*)
            echo "ERROR: out-dir $out_phys overlaps the source $src (inside or equal)." >&2
            exit 1
            ;;
    esac
    case "$src/" in
        "$out_phys"/*)
            echo "ERROR: out-dir $out_phys overlaps the source $src (contains it)." >&2
            exit 1
            ;;
    esac
}
check_overlap "$(physpath "$outdir")"

mkdir -p -- "$outdir"
outdir=$(cd -P -- "$outdir" && pwd -P)
# Re-check on the real path: spellings such as `missing/..` only resolve to
# their true location once the directory exists.
check_overlap "$outdir"
zipfile="$outdir/$key$suffix"
if [ -e "$zipfile" ] && [ ! -f "$zipfile" ]; then
    echo "ERROR: $zipfile exists and is not a regular file." >&2
    exit 1
fi

# The staging dir is a fresh mktemp dir owned by this script, removed on every
# exit. Nothing that pre-existed is ever rm -rf'd.
stage=$(mktemp -d "$outdir/.stage.XXXXXX")
trap 'rm -rf -- "$stage"' EXIT
rm -f -- "$zipfile"

# Copy the source minus tests/ and .gitignore.
tar -C "$src" --exclude=./tests --exclude=./.gitignore -cf - . | tar -C "$stage" -xf -
cp "$repo/LICENSE" "$stage/LICENSE"

# `ls -A` emits children in sorted order, keeping entry order deterministic.
mapfile -t children < <(cd "$stage" && ls -A)
(cd "$stage" && zip -q -r "$zipfile" -- "${children[@]}")

# Everything below is verification. A bundle wrong in any of these ways
# installs without complaint and fails later, on someone else's machine.
fail=0
report() {
    echo "  MISSING  $1" >&2
    fail=$((fail + 1))
}

listing=$(zipinfo -1 "$zipfile")
has() { printf '%s\n' "$listing" | grep -qxF -- "$1"; }

has "$manifest_rel" || report "$manifest_rel"
has README.md || report "README.md"
printf '%s\n' "$listing" | grep -q '^skills/[^/]*/SKILL\.md$' ||
    report "skills/*/SKILL.md"
printf '%s\n' "$listing" | grep -q '^skills/.*THIRD-PARTY-NOTICE\.md$' ||
    report "skills/**/THIRD-PARTY-NOTICE.md"
has LICENSE || report "LICENSE"
if [ "$agent" = opencode ]; then
    has INSTALL.md || report "INSTALL.md"
fi
if printf '%s\n' "$listing" | grep -q '^bin/'; then
    echo "  FORBIDDEN  bin/ (skills-only plugins ship no binaries)" >&2
    fail=$((fail + 1))
fi

if [ "$fail" -gt 0 ]; then
    echo >&2
    echo "ERROR: $(basename "$zipfile") is not installable — $fail problem(s) above." >&2
    rm -f "$zipfile"
    exit 1
fi

echo "Bundled $key -> $zipfile"
