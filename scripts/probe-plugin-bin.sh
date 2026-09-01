#!/usr/bin/env bash
#
# Run an assembled plugin's binaries the two ways Claude Code starts them, and
# fail if the packaging — not the program — is what answers.
#
# A plugin's entry points are reached by two different routes, and they resolve
# `bin/<name>` differently on Windows:
#
#   * an MCP server is spawned directly, and CreateProcess appends .exe to a
#     path with no extension, so it lands on the real binary;
#   * a hook declared without `args` runs through a shell, and where Git Bash is
#     installed that shell executes the extensionless dispatcher instead.
#
# 0.6.0 shipped a dispatcher that rejected MINGW, which the direct route never
# noticed. So both routes are exercised here, against the same tree.
#
# What is asserted is the packaging contract, not program behaviour — the Rust
# suites own that:
#
#   1. the dispatcher hands off rather than refusing (its refusals are
#      recognisable, and are the failure this exists to catch);
#   2. the process exits 0;
#   3. a binary that prints a version prints ITS OWN name and the version its
#      plugin.json declares — which is how a bundle carrying binaries from one
#      release and a manifest from another gets caught.
#
# Usage: scripts/probe-plugin-bin.sh <plugin-root>
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "usage: $0 <plugin-root>" >&2
    exit 2
fi

root=$(cd "$1" && pwd)

command -v jq > /dev/null 2>&1 || {
    echo "ERROR: jq is required and not on PATH." >&2
    exit 1
}

# node models the host's own spawn: Claude Code starts an MCP server through
# libuv, and no shell is involved.
node=$(command -v node) || {
    echo "ERROR: node is required to spawn a binary without a shell." >&2
    exit 1
}

manifest="$root/.claude-plugin/plugin.json"
version=""
if [ -f "$manifest" ]; then
    version=$(jq -r '.version // ""' "$manifest" | tr -d '\r')
fi

names=$(
    for config in "$root/hooks/hooks.json" "$root/.mcp.json"; do
        [ -f "$config" ] || continue
        jq -r '
            [.. | objects | .command? // empty]
            | .[]
            | select(type == "string" and contains("CLAUDE_PLUGIN_ROOT"))
        ' "$config"
    done | sed 's#.*/bin/##; s#\.exe$##' | tr -d '\r' | sort -u
)
[ -n "$names" ] || {
    echo "ERROR: $root declares no \${CLAUDE_PLUGIN_ROOT}/bin/... commands to probe." >&2
    exit 1
}

# Under Git Bash the shell's own paths are MSYS-style (/c/...), which a native
# Windows program cannot open. Convert for anything that is not the shell.
native_path() {
    if command -v cygpath > /dev/null 2>&1; then
        cygpath -m "$1"
    else
        printf '%s' "$1"
    fi
}

failures=0
checked=0

# A dispatcher that declines to dispatch says so in a recognisable way. Anything
# matching this came from the shell script, never from the program.
dispatcher_refusal='(unsupported operating system|unsupported architecture|ships no binary for)'

judge() {
    # judge <route> <name> <status> <output>
    checked=$((checked + 1))
    local route=$1 name=$2 status=$3 output=$4

    if printf '%s' "$output" | grep -qE "$dispatcher_refusal"; then
        echo "  FAIL  $name via $route: the dispatcher refused — $output" >&2
        failures=$((failures + 1))
        return
    fi
    if [ "$status" -ne 0 ]; then
        echo "  FAIL  $name via $route: exit $status — ${output:-no output}" >&2
        failures=$((failures + 1))
        return
    fi
    # Only a binary that answers --version with a version line gets its version
    # checked. `re-ghidra-cc-hook` has no --version: it reads stdin and replies
    # with its SessionStart preflight, which is a perfectly good sign of life and
    # not a version claim to hold against the manifest.
    case $output in
        "$name "*)
            if [ -n "$version" ] && [ "$output" != "$name $version" ]; then
                echo "  FAIL  $name via $route: reported '$output', manifest says '$name $version'" >&2
                failures=$((failures + 1))
                return
            fi
            ;;
    esac
    echo "  ok    $name via $route${output:+ -> $output}"
}

for name in $names; do
    # Route 1: through a shell. On Windows with Git Bash this is the dispatcher.
    #
    # stdin comes from /dev/null in both routes: a hook reads stdin, and one that
    # blocked waiting for input would hang the run rather than fail it.
    #
    # The whole output is captured and trimmed afterwards, never piped through
    # `head`: `re-ghidra-cc-mcp --version` prints its version and then three
    # lines of usage, and a reader that closes after the first line leaves the
    # writer with EPIPE — which Rust turns into a panic and exit 101. That is a
    # race between the two processes, so it failed only on macOS, and only
    # sometimes.
    raw=$("$root/bin/$name" --version < /dev/null 2>&1) && status=0 || status=$?
    output=$(printf '%s\n' "$raw" | sed -n '1p' | tr -d '\r')
    judge shell "$name" "$status" "$output"

    # Route 2: spawned directly, no shell — how an MCP server is started.
    #
    # node, not python: the host spawns through libuv, which tries .com and .exe
    # before the bare name and so reaches the real binary. Python's CreateProcess
    # call finds the extensionless dispatcher instead and dies with WinError 193,
    # which would make this probe test python rather than the packaging.
    raw=$("$node" -e '
const { spawnSync } = require("child_process");
const r = spawnSync(process.argv[1], ["--version"], { encoding: "utf8", input: "" });
if (r.error) { console.error(r.error.code || String(r.error)); process.exit(1); }
const line = ((r.stdout || "") + (r.stderr || "")).split(/\r?\n/).find((l) => l.trim()) || "";
process.stdout.write(line);
process.exit(r.status === null ? 1 : r.status);
' "$(native_path "$root/bin/$name")" 2>&1) && status=0 || status=$?
    output=$(printf '%s\n' "$raw" | sed -n '$p' | tr -d '\r')
    judge direct "$name" "$status" "$output"
done

if [ "$failures" -gt 0 ]; then
    echo >&2
    echo "ERROR: $failures of $checked probe(s) failed against $root." >&2
    exit 1
fi

# The plugin's own name, not `basename "$root"`: an installed plugin lives in a
# directory named after its VERSION, so that read "in 0.6.1 0.6.1".
label=$(jq -r '.name // ""' "$manifest" 2>/dev/null | tr -d '\r')
[ -n "$label" ] || label=$(basename "$root")
echo "Probed $checked entry point route(s) in $label${version:+ $version}, all sound."
