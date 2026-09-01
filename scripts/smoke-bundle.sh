#!/usr/bin/env bash
#
# Assemble a host-only plugin bundle from locally built binaries and probe it.
#
# scripts/bundle-plugin.sh builds the real, five-platform article out of release
# archives, so it can only run once a release exists. This stages the same
# layout for the machine it runs on, which is enough to answer the question CI
# could not answer before: does the thing we ship actually start, by both the
# routes Claude Code starts it?
#
# Cheap on purpose — debug binaries, no zip, no cross-compilation — so it can
# sit on every pull request across ubuntu, macOS and Windows.
#
# Usage: scripts/smoke-bundle.sh <plugin-name>
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "usage: $0 <plugin-name>" >&2
    exit 2
fi

plugin=$1
repo=$(cd "$(dirname "$0")/.." && pwd)
src="$repo/claude-code/$plugin"

command -v jq > /dev/null 2>&1 || {
    echo "ERROR: jq is required and not on PATH." >&2
    exit 1
}

[ -f "$src/.claude-plugin/plugin.json" ] || {
    echo "ERROR: $src/.claude-plugin/plugin.json not found." >&2
    exit 1
}

binaries=$(
    for config in "$src/hooks/hooks.json" "$src/.mcp.json"; do
        [ -f "$config" ] || continue
        jq -r '
            [.. | objects | .command? // empty]
            | .[]
            | select(type == "string" and contains("CLAUDE_PLUGIN_ROOT"))
        ' "$config"
    done | sed 's#.*/bin/##; s#\.exe$##' | tr -d '\r' | sort -u
)
[ -n "$binaries" ] || {
    echo "ERROR: $plugin declares no \${CLAUDE_PLUGIN_ROOT}/bin/... commands." >&2
    exit 1
}

host=$(rustc -vV | sed -n 's/^host: //p')
[ -n "$host" ] || {
    echo "ERROR: could not read the host triple from rustc -vV." >&2
    exit 1
}

cargo_args=()
for name in $binaries; do
    cargo_args+=(--bin "$name")
done
(cd "$repo" && cargo build -p "$plugin" "${cargo_args[@]}")

stage="$repo/target/smoke/$plugin"
rm -rf "$stage"
mkdir -p "$stage/bin"

tar --force-local -c -C "$src" \
    --exclude=./bin \
    --exclude=./src \
    --exclude=./tests \
    --exclude=./Cargo.toml \
    --exclude=./.gitignore \
    . | tar --force-local -x -C "$stage"

# Order matters, and only on Windows. MSYS resolves a path with no extension to
# a sibling .exe, so writing `bin/<name>` AFTER staging `bin/<name>.exe` opens
# the .exe and truncates it. Writing the dispatcher first — when no .exe is
# there yet — creates a real extensionless file, and the copy that follows
# names its .exe explicitly. The PE check below is what keeps that ordering
# honest rather than merely intended.
for name in $binaries; do
    sed "s/__NAME__/$name/g" "$repo/scripts/plugin-bin-dispatch.sh.in" > "$stage/bin/$name"
    chmod 755 "$stage/bin/$name"
done

case $host in
    *windows*)
        for name in $binaries; do
            built="$repo/target/debug/$name.exe"
            [ -f "$built" ] || {
                echo "ERROR: cargo produced no $built" >&2
                exit 1
            }
            cp "$built" "$stage/bin/$name.exe"
            [ "$(head -c 2 "$stage/bin/$name.exe")" = "MZ" ] || {
                echo "ERROR: bin/$name.exe is not a PE image — the dispatcher overwrote it." >&2
                exit 1
            }
            head -n 1 "$stage/bin/$name" | grep -qx '#!/bin/sh' || {
                echo "ERROR: bin/$name is not the dispatcher — the binary overwrote it." >&2
                exit 1
            }
        done
        ;;
    *)
        mkdir -p "$stage/bin/$host"
        for name in $binaries; do
            built="$repo/target/debug/$name"
            [ -f "$built" ] || {
                echo "ERROR: cargo produced no $built" >&2
                exit 1
            }
            cp "$built" "$stage/bin/$host/$name"
            chmod 755 "$stage/bin/$host/$name"
        done
        ;;
esac

echo "Staged $plugin for $host in $stage"
bash "$repo/scripts/probe-plugin-bin.sh" "$stage"
