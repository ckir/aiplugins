#!/usr/bin/env bash
#
# Assemble one self-contained, installable zip for a Claude Code plugin.
#
# `claude plugin install` fetches a marketplace entry's archive and extracts it
# as the plugin directory. Nothing in that flow compiles anything, so the zip
# has to carry the binaries `.mcp.json` and `hooks/hooks.json` point at — for
# every platform, because a marketplace entry is ONE url with no per-platform
# matrix.
#
# The layout that makes one zip work everywhere:
#
#   bin/<name>.exe            the Windows x64 binary
#   bin/<target>/<name>       the binary for each unix target
#   bin/<name>                a /bin/sh dispatcher that execs the right one
#
# Both configs keep naming `${CLAUDE_PLUGIN_ROOT}/bin/<name>` and never learn
# about platforms:
#
#   * On Windows a directly spawned `bin/<name>` resolves through PATHEXT and
#     lands on `bin/<name>.exe`, because libuv tries .com and .exe before the
#     bare name. A hook with no `args` instead runs through a shell, and where
#     Git Bash is installed that shell picks the extensionless dispatcher — so
#     the dispatcher has a MINGW/MSYS/CYGWIN branch that execs the .exe itself.
#   * On macOS/Linux the dispatcher IS `bin/<name>`, and execs the binary for
#     the running uname. Claude Code's zip installer reads unix modes out of the
#     zip central directory and chmods anything carrying an exec bit, so the
#     dispatcher and the binaries stay executable through install.
#
# That second point is why this script insists on an archive whose entries were
# recorded by a unix zip (host byte 3). Info-ZIP on Windows records FAT/NTFS
# attributes instead, the exec bits are lost, and the plugin installs onto
# macOS/Linux dead — with nothing to see until someone tries to use it. The
# verification pass at the end refuses to hand back such an archive.
#
# Usage: scripts/bundle-plugin.sh <plugin-name> <assets-dir> <out-dir>
#
#   <assets-dir>  holds the per-target release archives dist already publishes:
#                 <plugin>-<target>.tar.xz, and <plugin>-<target>.zip on Windows.
#   <out-dir>     receives <plugin>-plugin.zip, built from a staging tree of the
#                 same name that is left in place for inspection.
set -euo pipefail

if [ "$#" -ne 3 ]; then
    echo "usage: $0 <plugin-name> <assets-dir> <out-dir>" >&2
    exit 2
fi

plugin=$1
assets=$(cd "$2" && pwd)
outdir=$3

repo=$(cd "$(dirname "$0")/.." && pwd)
src="$repo/claude-code/$plugin"

for tool in jq tar unzip zip zipinfo; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "ERROR: $tool is required and not on PATH." >&2
        exit 1
    }
done

# Two independent reasons this cannot run on Windows, both silent if allowed:
#
#   * Info-ZIP under Windows records FAT attributes, so no entry can carry an
#     exec bit and the bundle installs dead on macOS/Linux.
#   * The MSYS/Cygwin runtime resolves a path with no extension to a sibling
#     .exe, so writing the `bin/<name>` dispatcher TRUNCATES the `bin/<name>.exe`
#     that was just staged next to it. The zip then contains a 700-byte "binary".
#
# WSL counts as Linux here, and is the way to run this on a Windows machine.
case $(uname -s) in
    MINGW* | MSYS* | CYGWIN*)
        echo "ERROR: plugin bundles cannot be built from a Windows shell." >&2
        echo "       Use WSL, Linux, or macOS — see the comments in this script." >&2
        exit 1
        ;;
esac

[ -f "$src/.claude-plugin/plugin.json" ] || {
    echo "ERROR: $src/.claude-plugin/plugin.json not found — is '$plugin' a Claude Code plugin?" >&2
    exit 1
}

# The targets come out of dist-workspace.toml rather than a list here: the
# release assets this script consumes are named after exactly those triples, so
# a target added or dropped there must not need a second edit to be honoured.
targets=$(sed -n 's/^targets = \[\(.*\)\]$/\1/p' "$repo/dist-workspace.toml" |
    tr ',' '\n' | tr -d ' "' | grep -v '^$')
[ -n "$targets" ] || {
    echo "ERROR: no targets parsed from dist-workspace.toml — has the [dist] targets line changed shape?" >&2
    exit 1
}

# The binaries to ship are the ones the plugin's own configs name. Deriving them
# from the configs, rather than from whatever the archives happen to contain,
# makes this the same contract scripts/check-plugin-wiring.sh enforces: every
# `${CLAUDE_PLUGIN_ROOT}/bin/X` reference has to end up as a real file in bin/.
binaries=$(
    for config in "$src/hooks/hooks.json" "$src/.mcp.json"; do
        [ -f "$config" ] || continue
        jq -r '
            [.. | objects | .command? // empty]
            | .[]
            | select(type == "string" and contains("CLAUDE_PLUGIN_ROOT"))
        ' "$config"
    done | sed 's#.*/bin/##; s#\.exe$##' | sort -u
)
[ -n "$binaries" ] || {
    echo "ERROR: $plugin declares no \${CLAUDE_PLUGIN_ROOT}/bin/... commands to bundle." >&2
    exit 1
}

mkdir -p "$outdir"
outdir=$(cd "$outdir" && pwd)
stage="$outdir/$plugin"
rm -rf "$stage"
mkdir -p "$stage/bin"

# Copy the plugin tree, minus what only the build needs. Excluding, rather than
# listing what to include, means a component directory added later — commands/,
# monitors/, .lsp.json — ships without anyone remembering to edit this script.
tar -c -C "$src" \
    --exclude=./bin \
    --exclude=./src \
    --exclude=./tests \
    --exclude=./Cargo.toml \
    --exclude=./.gitignore \
    . | tar -x -C "$stage"

# The plugin directories carry no LICENSE of their own; the repo's applies.
cp "$repo/LICENSE" "$stage/LICENSE"

for target in $targets; do
    case $target in
        *windows*) archive="$assets/$plugin-$target.zip" ;;
        *) archive="$assets/$plugin-$target.tar.xz" ;;
    esac

    [ -f "$archive" ] || {
        echo "ERROR: missing release archive $(basename "$archive") in $assets" >&2
        exit 1
    }

    extract="$outdir/.extract/$target"
    rm -rf "$extract"
    mkdir -p "$extract"

    case $target in
        *windows*)
            # dist's Windows zips are flat: the binaries sit at the archive root.
            unzip -q "$archive" -d "$extract"
            for name in $binaries; do
                [ -f "$extract/$name.exe" ] || {
                    echo "ERROR: $(basename "$archive") does not contain $name.exe" >&2
                    exit 1
                }
                cp "$extract/$name.exe" "$stage/bin/$name.exe"
            done
            ;;
        *)
            # dist's unix tarballs wrap everything in one <plugin>-<target>/ dir.
            tar -xf "$archive" -C "$extract"
            root="$extract/$plugin-$target"
            [ -d "$root" ] || {
                echo "ERROR: $(basename "$archive") has no $plugin-$target/ directory" >&2
                exit 1
            }
            mkdir -p "$stage/bin/$target"
            for name in $binaries; do
                [ -f "$root/$name" ] || {
                    echo "ERROR: $(basename "$archive") does not contain $name" >&2
                    exit 1
                }
                cp "$root/$name" "$stage/bin/$target/$name"
                chmod 755 "$stage/bin/$target/$name"
            done
            ;;
    esac
done

# One dispatcher per binary, instantiated from the template that
# scripts/check-bundle-dispatch.sh exercises on every CI run. `exec` keeps the
# pid and the stdio handles, which an MCP server speaking JSON-RPC over stdin
# and stdout depends on.
for name in $binaries; do
    sed "s/__NAME__/$name/g" "$repo/scripts/plugin-bin-dispatch.sh.in" > "$stage/bin/$name"
    chmod 755 "$stage/bin/$name"
done

# Never build a bundle around a dispatcher that routes wrong. The check fakes
# every platform, so the branch this machine cannot reach is covered too.
bash "$repo/scripts/check-bundle-dispatch.sh" > /dev/null || {
    echo "ERROR: bin/ dispatcher self-test failed; run scripts/check-bundle-dispatch.sh" >&2
    exit 1
}

zipfile="$outdir/$plugin-plugin.zip"
rm -f "$zipfile"
# Zip the staging tree's children by name rather than `.`: some zip
# implementations keep the "./" prefix, and the installer looks for
# `.claude-plugin/plugin.json` by exact entry name.
(cd "$stage" && zip -q -r "$zipfile" -- $(ls -A))
rm -rf "$outdir/.extract"

# Everything below is verification. A bundle wrong in any of these ways installs
# without complaint and fails later, on someone else's machine.
fail=0
report() {
    echo "  MISSING  $1" >&2
    fail=$((fail + 1))
}

listing=$(zipinfo -1 "$zipfile")
modes=$(zipinfo "$zipfile")

printf '%s\n' "$listing" | grep -qxF '.claude-plugin/plugin.json' ||
    report ".claude-plugin/plugin.json"

for name in $binaries; do
    printf '%s\n' "$listing" | grep -qxF "bin/$name.exe" || report "bin/$name.exe"
    printf '%s\n' "$listing" | grep -qxF "bin/$name" || report "bin/$name"

    # A dispatcher and a PE executable are both just files in the zip, and a
    # mixup between the two — one overwriting the other — costs nothing to check
    # and is invisible from the listing alone.
    [ "$(head -c 2 "$stage/bin/$name.exe")" = "MZ" ] ||
        report "bin/$name.exe is not a Windows executable"
    head -n 1 "$stage/bin/$name" | grep -qx '#!/bin/sh' ||
        report "bin/$name is not the /bin/sh dispatcher"

    # The exec bit has to survive the zip, and only a unix-made entry — "unx" in
    # zipinfo's host column — can carry one at all.
    printf '%s\n' "$modes" | grep -qE "^-rwxr-xr-x .* unx .* bin/$name\$" ||
        report "executable unix mode on bin/$name"

    for target in $targets; do
        case $target in
            *windows*) continue ;;
        esac
        printf '%s\n' "$listing" | grep -qxF "bin/$target/$name" || report "bin/$target/$name"
        printf '%s\n' "$modes" | grep -qE "^-rwxr-xr-x .* unx .* bin/$target/$name\$" ||
            report "executable unix mode on bin/$target/$name"
    done
done

if [ "$fail" -gt 0 ]; then
    echo >&2
    echo "ERROR: $(basename "$zipfile") is not installable — $fail problem(s) above." >&2
    echo "       Exec bits are lost when the zip is built by Info-ZIP on Windows;" >&2
    echo "       build the bundle on Linux or macOS." >&2
    rm -f "$zipfile"
    exit 1
fi

echo "Bundled $plugin -> $zipfile"
printf '  binaries: %s\n' "$(echo "$binaries" | tr '\n' ' ')"
printf '  targets:  %s\n' "$(echo "$targets" | tr '\n' ' ')"
