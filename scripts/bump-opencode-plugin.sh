#!/usr/bin/env bash
# Move the @opencode/plugin pin in every OpenCode plugin at once.
#
#   bash scripts/bump-opencode-plugin.sh [--root DIR] VERSION
#   bash scripts/bump-opencode-plugin.sh --selftest
#
# scripts/check-opencode-wiring.sh requires every plugin's package.json to pin
# the same @opencode/plugin version and every README/INSTALL.md claim to match.
# Dependabot edits only package.json, so it cannot satisfy that gate (and is
# told to ignore the package in .github/dependabot.yml). This script is the
# supported way to move the pin: it edits package.json and the doc claims the
# gate reads, nothing else.
#
# It does NOT touch bun.lock (the hashes need bun) or the OpenCode CLI version
# the repo is tested against (shared/opencode-ctl, .claude/recommended-tools.json).
# It prints the follow-up steps.
set -euo pipefail

# --- the work ---------------------------------------------------------------

# pin_of PACKAGE_JSON: the @opencode/plugin version it pins, or nothing.
pin_of() {
    sed -n 's/.*"@opencode\/plugin"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$1" | head -n 1
}

# rewrite FILE SED-ARGS...: edit through a temp file so no sed -i dialect matters.
rewrite() {
    local f=$1 tmp
    shift
    tmp=$(mktemp)
    sed "$@" "$f" > "$tmp"
    if cmp -s "$tmp" "$f"; then rm -f "$tmp"; else cat "$tmp" > "$f"; rm -f "$tmp"; echo "  edited $f"; fi
}

# bump ROOT VERSION: rewrite pins and doc claims under ROOT/opencode/plugins.
bump() {
    local root=$1 new=$2 dir pkg pin doc distinct n=0
    local pins=()
    if ! [[ $new =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "bump-opencode-plugin: '$new' is not an x.y.z version" >&2
        return 1
    fi
    for pkg in "$root"/opencode/plugins/*/package.json; do
        [ -f "$pkg" ] || continue
        pin=$(pin_of "$pkg")
        [ -n "$pin" ] || continue
        pins+=("$pin")
        n=$((n + 1))
    done
    if [ "$n" -eq 0 ]; then
        echo "bump-opencode-plugin: no package.json under $root/opencode/plugins pins @opencode/plugin" >&2
        return 1
    fi
    distinct=$(printf '%s\n' "${pins[@]}" | sort -u | tr '\n' ' ')
    if [ "$(printf '%s\n' "${pins[@]}" | sort -u | wc -l)" -gt 1 ]; then
        echo "bump-opencode-plugin: plugins disagree on the current pin ($distinct); moving all to $new" >&2
    fi
    for pkg in "$root"/opencode/plugins/*/package.json; do
        [ -f "$pkg" ] || continue
        [ -n "$(pin_of "$pkg")" ] || continue
        dir=$(dirname "$pkg")
        rewrite "$pkg" -e "s|\\(\"@opencode/plugin\"[[:space:]]*:[[:space:]]*\"\\)[^\"]*\"|\\1$new\"|"
        # The same three claim shapes the wiring gate reads, anchored on the
        # plugin token so the OpenCode release number on the same line stays.
        for doc in "$dir/README.md" "$dir/INSTALL.md"; do
            [ -f "$doc" ] || continue
            rewrite "$doc" \
                -e "s|\\(@opencode/plugin@\\)[0-9][0-9.]*|\\1$new|g" \
                -e "s|\\(@opencode/plugin\` \\*\\*\\)[0-9][0-9.]*\\(\\*\\*\\)|\\1$new\\2|g" \
                -e "s|\\(@opencode/plugin\` than \`\\)[0-9][0-9.]*\\(\`\\)|\\1$new\\2|g"
        done
    done
    echo "bump-opencode-plugin: $n plugin(s) now pin @opencode/plugin $new."
    echo "Still to do by hand:"
    for pkg in "$root"/opencode/plugins/*/bun.lock; do
        [ -f "$pkg" ] || continue
        echo "  (cd $(dirname "$pkg") && bun install)   # regenerates bun.lock; needs bun"
    done
    echo "  bash scripts/check-opencode-wiring.sh     # the gate that reads these pins"
    echo "  re-test each plugin against OpenCode $new (the plugin API is beta); the OpenCode CLI"
    echo "  version the repo is tested against (shared/opencode-ctl, .claude/recommended-tools.json) is separate."
}

# --- self-test --------------------------------------------------------------

# shellcheck disable=SC2016  # the backticks are literal Markdown, not substitutions
selftest() {
    local me
    # (global: the EXIT trap runs after this function's locals are gone)
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    me=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)/$(basename "${BASH_SOURCE[0]}")

    check() { # check NAME CONDITION-RESULT(0 = pass) DETAIL
        if [ "$2" -eq 0 ]; then echo "ok $1"; else echo "FAIL $1: $3" >&2; exit 1; fi
    }

    # The gate's own claim readers (scripts/check-opencode-wiring.sh doc_pins);
    # kept in step by hand, and the gate itself runs in CI after any real bump.
    doc_pins() {
        sed -n \
            -e 's/.*@opencode\/plugin@\([0-9][0-9.]*\).*/\1/p' \
            -e 's/.*@opencode\/plugin` \*\*\([0-9][0-9.]*\)\*\*.*/\1/p' \
            -e 's/.*@opencode\/plugin` than `\([0-9][0-9.]*\)`.*/\1/p' \
            "$1"
    }

    # make_tree DIR OLD_A OLD_B: two plugins with a pin, one skills-only plugin,
    # one plugin whose package.json has no @opencode/plugin pin.
    make_tree() {
        local d=$1 a=$2 b=$3 p
        for p in a b; do mkdir -p "$d/opencode/plugins/$p-opencode"; done
        mkdir -p "$d/opencode/plugins/c-opencode" "$d/opencode/plugins/d-opencode"
        local v
        for p in a b; do
            v=$a
            [ "$p" = b ] && v=$b
            printf '{\n  "dependencies": {\n    "@opencode/plugin": "%s",\n    "@opencode/other": "%s"\n  }\n}\n' \
                "$v" "$v" > "$d/opencode/plugins/$p-opencode/package.json"
            {
                printf 'Tested with OpenCode **v%s** and `@opencode/plugin` **%s**.\n' "$v" "$v"
                printf 'Run `bun add @opencode/plugin@%s` and see ticket 2.0.32 notes.\n' "$v"
                printf 'If your pins name a newer `@opencode/plugin` than `%s`, stop.\n' "$v"
            } > "$d/opencode/plugins/$p-opencode/README.md"
        done
        # INSTALL.md of b uses CRLF line endings; they must survive.
        printf 'cd x && bun add @opencode/plugin@%s\r\nnothing else\r\n' "$b" > "$d/opencode/plugins/b-opencode/INSTALL.md"
        printf 'skills only, mentions @opencode/plugin@9.9.9 nowhere real\n' > "$d/opencode/plugins/c-opencode/README.md"
        printf '{"dependencies": {"left-pad": "1.0.0"}}\n' > "$d/opencode/plugins/d-opencode/package.json"
    }

    # --- case 1: every pin and claim moves, nothing else does
    make_tree "$tmp/t1" 2.0.32 2.0.32
    bash "$me" --root "$tmp/t1" 2.1.2 > /dev/null 2> "$tmp/err1" || check bump-runs 1 "exit non-zero: $(cat "$tmp/err1")"
    ok=0
    for p in a b; do
        pin=$(sed -n 's/.*"@opencode\/plugin": "\([^"]*\)".*/\1/p' "$tmp/t1/opencode/plugins/$p-opencode/package.json")
        [ "$pin" = 2.1.2 ] || ok=1
    done
    check pins-moved "$ok" "a package.json still holds the old pin"
    ok=0
    for f in a-opencode/README.md b-opencode/README.md b-opencode/INSTALL.md; do
        claims=$(doc_pins "$tmp/t1/opencode/plugins/$f" | sort -u | tr -d '\r')
        [ "$claims" = 2.1.2 ] || ok=1
    done
    check claims-moved "$ok" "a doc claim does not read 2.1.2"
    ok=0
    grep -q '"@opencode/other": "2.0.32"' "$tmp/t1/opencode/plugins/a-opencode/package.json" || ok=1
    grep -q 'OpenCode \*\*v2.0.32\*\*' "$tmp/t1/opencode/plugins/a-opencode/README.md" || ok=1
    grep -q 'ticket 2.0.32 notes' "$tmp/t1/opencode/plugins/a-opencode/README.md" || ok=1
    grep -q 'left-pad": "1.0.0"' "$tmp/t1/opencode/plugins/d-opencode/package.json" || ok=1
    grep -q '9.9.9' "$tmp/t1/opencode/plugins/c-opencode/README.md" || ok=1
    check decoys-untouched "$ok" "a line that is not the plugin pin was rewritten"
    check crlf-kept "$(grep -c $'\r' "$tmp/t1/opencode/plugins/b-opencode/INSTALL.md" > /dev/null && echo 0 || echo 1)" "CRLF lost"

    # --- case 2: bumping to the version already in place changes nothing
    cp -R "$tmp/t1" "$tmp/t1.before"
    bash "$me" --root "$tmp/t1" 2.1.2 > /dev/null 2>&1 || check idempotent-runs 1 "second run failed"
    check idempotent "$(diff -r "$tmp/t1" "$tmp/t1.before" > /dev/null && echo 0 || echo 1)" "second run changed files"

    # --- case 3: a version that is not x.y.z is refused and nothing changes
    make_tree "$tmp/t3" 2.0.32 2.0.32
    cp -R "$tmp/t3" "$tmp/t3.before"
    ok=0
    for bad in v2.1.2 2.1 latest '' '2.1.2-beta'; do
        if bash "$me" --root "$tmp/t3" "$bad" > /dev/null 2>&1; then ok=1; fi
    done
    check bad-version-refused "$ok" "a non-x.y.z version was accepted"
    check bad-version-no-change "$(diff -r "$tmp/t3" "$tmp/t3.before" > /dev/null && echo 0 || echo 1)" "a refused run changed files"

    # --- case 4: a tree with no pin is refused (a typo in --root must not pass)
    mkdir -p "$tmp/t4/opencode/plugins/x-opencode"
    check no-pin-refused "$(bash "$me" --root "$tmp/t4" 2.1.2 > /dev/null 2>&1 && echo 1 || echo 0)" "ran on a tree without any pin"

    # --- case 5: plugins that already disagree are warned about, then converge
    make_tree "$tmp/t5" 2.0.32 2.0.30
    bash "$me" --root "$tmp/t5" 2.1.2 > /dev/null 2> "$tmp/err5" || check drift-runs 1 "exit non-zero"
    check drift-warned "$(grep -q 'disagree' "$tmp/err5" && echo 0 || echo 1)" "no warning that the old pins disagreed"
    ok=0
    for p in a b; do grep -q '"@opencode/plugin": "2.1.2"' "$tmp/t5/opencode/plugins/$p-opencode/package.json" || ok=1; done
    check drift-converged "$ok" "pins did not converge"
}

if [ "${1:-}" = "--selftest" ]; then
    selftest
    exit $?
fi
root=.
if [ "${1:-}" = "--root" ]; then
    root=${2:?--root needs a directory}
    shift 2
fi
[ $# -eq 1 ] || { echo "usage: bump-opencode-plugin.sh [--root DIR] VERSION | --selftest" >&2; exit 1; }
bump "$root" "$1"
