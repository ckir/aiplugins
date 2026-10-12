#!/usr/bin/env bash
#
# Assemble one self-contained, installable zip for an opencode plugin.
#
# `opencode` loads a plugin directory containing `plugin.ts` + `opencode.jsonc`
# directly, so unlike the Claude Code bundler there are no per-target binaries
# to ship: the zip carries versioned source only (plugin.ts, opencode.jsonc,
# package.json, README.md, INSTALL.md, plus skills/ and agents/ when present,
# plus tool/ and tui.tsx when the plugin is multi-file TypeScript rather than
# a single plugin.ts).
#
# The layout that makes the zip installable:
#
#   plugin.ts opencode.jsonc package.json README.md INSTALL.md [skills/...] [agents/...] [tool/...] [tui.tsx]
#
# i.e. the top level of the zip IS the plugin directory — extracting yields
# `plugin.ts`, `opencode.jsonc`, … directly, with no wrapper folder.
#
# That last point is why this script zips the staging tree's children by name
# rather than `.`: some zip implementations keep the `./` prefix and the
# installer looks for `opencode.jsonc` by exact entry name.
#
# Usage: scripts/bundle-opencode-plugin.sh <plugin-dir> <out-dir>
#
#   <plugin-dir>  the opencode plugin source tree (e.g. opencode/rtk-mcp-opencode)
#   <out-dir>     receives <name>-opencode.zip, built from a staging tree of the
#                 same name that is left in place for inspection.
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "usage: $0 <plugin-dir> <out-dir>" >&2
    exit 2
fi

plugindir=$(cd "$1" && pwd)
outdir=$2

repo=$(cd "$(dirname "$0")/.." && pwd)

for tool in jq tar unzip zip zipinfo; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "ERROR: $tool is required and not on PATH." >&2
        exit 1
    }
done

# Two independent reasons this cannot run on Windows, both silent if allowed:
#
#   * Info-ZIP under Windows records FAT attributes, so entries lose their unix
#     modes and the bundle installs subtly wrong on Linux/macOS.
#   * Line-ending and path translation under MSYS/Cygwin can corrupt the
#     versioned source tree the zip is meant to carry verbatim.
#
# WSL counts as Linux here, and is the way to run this on a Windows machine.
case $(uname -s) in
    MINGW* | MSYS* | CYGWIN*)
        echo "ERROR: opencode plugin bundles cannot be built from a Windows shell." >&2
        echo "       Use WSL, Linux, or macOS — see the comments in this script." >&2
        exit 1
        ;;
esac

name=$(basename "$plugindir")

for required in plugin.ts opencode.jsonc package.json README.md INSTALL.md; do
    [ -e "$plugindir/$required" ] || {
        echo "ERROR: $plugindir/$required not found — is '$name' an opencode plugin?" >&2
        exit 1
    }
done

mkdir -p "$outdir"
outdir=$(cd "$outdir" && pwd)
stage="$outdir/$name"
rm -rf "$stage"
mkdir -p "$stage"

# Copy exactly the versioned source the install needs — no binaries, no build
# inputs. Listing what to include (rather than excluding what to drop) means
# node_modules/, bin/, tests/, examples/, and bun.lock can never leak in.
cp "$plugindir/plugin.ts" "$plugindir/opencode.jsonc" \
    "$plugindir/package.json" "$plugindir/README.md" \
    "$plugindir/INSTALL.md" "$stage/"
# skills/ is required for hook/MCP plugins but not for pure-TypeScript ones
# (todov2-opencode ships no skills); copy it only when the source has it.
if [ -d "$plugindir/skills" ]; then
    cp -r "$plugindir/skills" "$stage/skills"
fi
if [ -d "$plugindir/agents" ]; then
    cp -r "$plugindir/agents" "$stage/agents"
fi
# Multi-file TypeScript plugins keep their imports beside the entry points
# (todov2-opencode's plugin.ts imports ./tool/*, and tui.tsx is its second
# entry). A single-file copy would break those imports at install time.
if [ -d "$plugindir/tool" ]; then
    cp -r "$plugindir/tool" "$stage/tool"
    # Test files are not needed at install time (the top-level tests/ dir is
    # forbidden for the same reason); the tool/ sources themselves must stay.
    find "$stage/tool" -name '*.test.ts' -delete
fi
if [ -f "$plugindir/tui.tsx" ]; then
    cp "$plugindir/tui.tsx" "$stage/tui.tsx"
fi

zipfile="$outdir/$name-opencode.zip"
rm -f "$zipfile"
# Zip the staging tree's children by name rather than `.`: some zip
# implementations keep the "./" prefix, and the installer looks for
# `opencode.jsonc` by exact entry name. `ls -A` emits children in sorted
# order, keeping the archive entry order deterministic.
(cd "$stage" && zip -q -r "$zipfile" -- $(ls -A))

# Everything below is verification. A bundle wrong in any of these ways
# installs without complaint and fails later, on someone else's machine.
fail=0
report() {
    echo "  MISSING  $1" >&2
    fail=$((fail + 1))
}

listing=$(zipinfo -1 "$zipfile")

for required in plugin.ts opencode.jsonc package.json README.md INSTALL.md; do
    printf '%s\n' "$listing" | grep -qxF "$required" ||
        report "$required"
done
if [ -d "$plugindir/skills" ]; then
    printf '%s\n' "$listing" | grep -q '^skills/' ||
        report "skills/"
else
    printf '%s\n' "$listing" | grep -q '^skills/' &&
        { echo "  FORBIDDEN  skills/ (source has none)" >&2; fail=$((fail + 1)); } || true
fi

for forbidden in bin/ node_modules/ tests/ examples/ bun.lock; do
    printf '%s\n' "$listing" | grep -q "^$forbidden" &&
        { echo "  FORBIDDEN  $forbidden" >&2; fail=$((fail + 1)); } || true
done
if [ -d "$plugindir/agents" ]; then
    printf '%s\n' "$listing" | grep -q '^agents/' ||
        report "agents/"
else
    printf '%s\n' "$listing" | grep -q '^agents/' &&
        { echo "  FORBIDDEN  agents/ (source has none)" >&2; fail=$((fail + 1)); } || true
fi
if [ -d "$plugindir/tool" ]; then
    printf '%s\n' "$listing" | grep -q '^tool/' ||
        report "tool/"
else
    printf '%s\n' "$listing" | grep -q '^tool/' &&
        { echo "  FORBIDDEN  tool/ (source has none)" >&2; fail=$((fail + 1)); } || true
fi
if [ -f "$plugindir/tui.tsx" ]; then
    printf '%s\n' "$listing" | grep -qxF "tui.tsx" ||
        report "tui.tsx"
else
    printf '%s\n' "$listing" | grep -qxF "tui.tsx" &&
        { echo "  FORBIDDEN  tui.tsx (source has none)" >&2; fail=$((fail + 1)); } || true
fi

if [ "$fail" -gt 0 ]; then
    echo >&2
    echo "ERROR: $(basename "$zipfile") is not installable — $fail problem(s) above." >&2
    echo "       Build the bundle on Linux or macOS." >&2
    rm -f "$zipfile"
    exit 1
fi

echo "Bundled $name -> $zipfile"
