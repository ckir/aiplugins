#!/usr/bin/env bash
#
# Assemble a host-only Antigravity plugin bundle from locally built debug
# binaries and probe its entry points.
#
# Usage: scripts/smoke-agy-bundle.sh [plugin-name]
#
# If plugin-name is provided, smoke that plugin; if omitted, smoke every
# published Antigravity binary plugin from the registry.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)

command -v jq > /dev/null 2>&1 || {
    echo "ERROR: jq is required and not on PATH." >&2
    exit 1
}

host=$(rustc -vV | sed -n 's/^host: //p' | tr -d '\r')
[ -n "$host" ] || {
    echo "ERROR: could not read the host triple from rustc -vV." >&2
    exit 1
}

if [ "$#" -gt 1 ]; then
    echo "usage: $0 [plugin-name]" >&2
    exit 2
fi

plugins=()
if [ "$#" -eq 1 ]; then
    plugins+=("$(printf '%s' "$1" | tr -d '\r')")
else
    for plugin in $(bash "$repo/scripts/registry-plugins.sh" antigravity --exclude-skills-only | tr -d '\r'); do
        plugins+=("$plugin")
    done
fi

[ "${#plugins[@]}" -gt 0 ] || {
    echo "ERROR: no Antigravity plugins to smoke." >&2
    exit 1
}

for plugin in "${plugins[@]}"; do
    src="$repo/antigravity/plugins/$plugin"

    [ -f "$src/plugin.json" ] || {
        echo "ERROR: $src/plugin.json not found." >&2
        exit 1
    }

    binaries=$(
        for config in "$src/hooks.json" "$src/mcp_config.json"; do
            [ -f "$config" ] || continue
            jq -r '
                [.. | objects | .command? // empty]
                | .[]
                | select(type == "string")
            ' "$config"
        done | sed 's#.*/bin/##; s#\.exe$##' | tr -d '\r' | sort -u
    )
    [ -n "$binaries" ] || {
        echo "ERROR: $plugin declares no commands in hooks.json or mcp_config.json." >&2
        exit 1
    }

    cargo_args=()
    for name in $binaries; do
        cargo_args+=(--bin "$name")
    done
    (cd "$repo" && cargo build -p "$plugin" "${cargo_args[@]}")

    stage="$repo/target/smoke-agy/$plugin"
    rm -rf "$stage"
    mkdir -p "$stage/bin"

    tar -c -C "$src" \
        --exclude=./bin \
        --exclude=./src \
        --exclude=./tests \
        --exclude=./Cargo.toml \
        --exclude=./.gitignore \
        . | tar -x -C "$stage"

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
done
