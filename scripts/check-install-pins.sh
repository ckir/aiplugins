#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Keep the release pins in the install documents equal to the plugin manifests.
#
#   bash scripts/check-install-pins.sh [--root DIR]           check (the default)
#   bash scripts/check-install-pins.sh --write [--root DIR]   rewrite stale pins
#   bash scripts/check-install-pins.sh --selftest
#
# Why: users install from a document on `main` (an OpenCode plugin from its
# INSTALL.md, an Antigravity or Qwen plugin from its README.md). Those documents
# pin a release by hand and nothing moved the pin on release, so they went
# stale. .github/workflows/release-refresh.yml runs this with --write on the
# release branch, and CI runs the check.
#
# The version comes from the plugin's own manifest (agents.json names it).
# Two pin styles:
#   variable-line (opencode/plugins/<plugin>/INSTALL.md)
#     Exactly one line starting `VERSION=`. Exactly one line starting
#     `CC_VERSION=` if and only if the plugin's opencode.jsonc declares a local
#     MCP server; its value is the version of the Claude Code sibling
#     (<plugin minus -opencode>-cc), whose release carries the binary.
#   literal-tag (antigravity and qwen README.md, and INSTALL.md too)
#     Every `<plugin>-v<x.y.z>` token carries the current version. The legacy
#     form `releases/download/v<x.y.z>/<plugin>-...` becomes
#     `releases/download/<plugin>-v<version>/<plugin>-...`. A release URL for
#     this plugin's asset under any other tag is a structural problem.
# `$VERSION` and `$CC_VERSION` in a URL are not literals and are left alone.
#
# A stale pin is fixed by --write. A structural problem (a missing or duplicated
# line, a bad manifest version, a pin line whose value is not x.y.z, the phrase `current as of this writing`) needs a
# hand edit, and --write writes nothing while one exists.
#
# POSIX awk and sed only: this runs on macOS (BSD awk) and Git Bash too.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
self=$here/$(basename "${BASH_SOURCE[0]}")

# --- the rewriter -----------------------------------------------------------
# Reads one document, prints it with every pin set to the current version, and
# writes to the file named by `report`:
#   ERR<TAB>message              a structural problem on a line
#   COUNT<TAB>pins<TAB>VERSION= lines<TAB>CC_VERSION= lines
# Variables: style (var|lit), plugin, ver, cc (sibling name), ccver ("" = none).
AWK_PINS=$(cat <<'AWK'
function verlen(s, pos,   rest) {
    rest = substr(s, pos)
    if (match(rest, /^[0-9]+\.[0-9]+\.[0-9]+/)) return RLENGTH
    return 0
}
# A pin line: only the x.y.z right after NAME= is replaced; the rest of the line stays.
function repin(s, name, want,   n) {
    n = verlen(s, length(name) + 1)
    if (n == 0) {
        print "ERR\tline " NR ": the value after " name " is not an x.y.z version; write `" name want "`" > report
        return s
    }
    return name want substr(s, length(name) + 1 + n)
}
# Every `prefix<x.y.z>` that starts a token becomes `prefix want`.
function retag(s, prefix, want,   out, i, n, pch, start) {
    out = ""
    while ((i = index(s, prefix)) > 0) {
        start = i + length(prefix)
        n = verlen(s, start)
        pch = (i > 1) ? substr(s, i - 1, 1) : substr(out, length(out), 1)
        if (n > 0 && pch !~ /[A-Za-z0-9_-]/) {
            out = out substr(s, 1, i - 1) prefix want
            s = substr(s, start + n)
            pins++
        } else {
            out = out substr(s, 1, start - 1)
            s = substr(s, start)
        }
    }
    return out s
}
# releases/download/v<x.y.z>/<plugin>-...  ->  releases/download/<plugin>-v<ver>/<plugin>-...
function relegacy(s,   out, i, n, p, tail, start) {
    p = "releases/download/v"
    tail = "/" plugin "-"
    out = ""
    while ((i = index(s, p)) > 0) {
        start = i + length(p)
        n = verlen(s, start)
        if (n > 0 && substr(s, start + n, length(tail)) == tail) {
            out = out substr(s, 1, i - 1) "releases/download/" plugin "-v" ver
            s = substr(s, start + n)
            pins++
        } else {
            out = out substr(s, 1, start - 1)
            s = substr(s, start)
        }
    }
    return out s
}
# A release URL for this plugin's asset (or its sibling's) must sit under the right tag.
function urlcheck(s,   p, i, j, tag, asset) {
    p = "releases/download/"
    while ((i = index(s, p)) > 0) {
        s = substr(s, i + length(p))
        j = index(s, "/")
        if (j == 0) break
        tag = substr(s, 1, j - 1)
        asset = substr(s, j + 1)
        if (index(asset, plugin "-") == 1 && tag != plugin "-v" ver && !(style == "var" && tag == plugin "-v$VERSION"))
            print "ERR\tline " NR ": a release URL for " plugin " is under the tag `" tag "`; write the tag as " plugin (style == "var" ? "-v$VERSION" : "-v<x.y.z>") > report
        if (style == "var" && ccver != "" && index(asset, cc "-") == 1 && tag != cc "-v" ccver && tag != cc "-v$CC_VERSION")
            print "ERR\tline " NR ": a release URL for " cc " is under the tag `" tag "`; write the tag as " cc "-v$CC_VERSION" > report
    }
}
BEGIN { pins = 0; vlines = 0; cclines = 0 }
{
    line = $0
    cr = ""
    if (sub(/\r$/, "", line)) cr = "\r"
    if (style == "var") {
        if (line ~ /^VERSION=/) {
            vlines++; pins++
            line = repin(line, "VERSION=", ver)
        } else if (line ~ /^CC_VERSION=/) {
            cclines++; pins++
            if (ccver != "") line = repin(line, "CC_VERSION=", ccver)
        }
    }
    line = retag(line, plugin "-v", ver)
    if (style == "var" && ccver != "") line = retag(line, cc "-v", ccver)
    line = relegacy(line)
    urlcheck(line)
    printf "%s%s\n", line, cr
}
END { print "COUNT\t" pins "\t" vlines "\t" cclines > report }
AWK
)

# has_stale_claim FILE: the phrase `current as of this writing`, also when it
# wraps across lines or across the lines of a blockquote.
has_stale_claim() {
    awk '{ sub(/\r$/, ""); sub(/^ *>? ?/, ""); printf "%s ", $0 }' "$1" \
        | tr -s '[:space:]' ' ' \
        | grep -qF 'current as of this writing'
}

is_semver() { [[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; }

# manifest_version FILE: its .version, or nothing.
manifest_version() {
    [ -f "$1" ] || return 0
    jq -r '.version // empty' "$1" 2> /dev/null | tr -d '\r' || true
}

structural=()
stale=()
outs=()
total_pins=0
documents=0

problem() { structural+=("$1"); }

# scan ROOT REL STYLE PLUGIN VER CC CCVER NEED_CC: one document.
scan() {
    local root=$1 rel=$2 style=$3 plugin=$4 ver=$5 cc=$6 ccver=$7 need_cc=$8
    local doc=$root/$rel out rep kind a b c
    if [ ! -f "$doc" ]; then
        problem "$rel: the file does not exist; every registry plugin on this host needs it"
        return 0
    fi
    documents=$((documents + 1))
    out=$work/$documents.out
    rep=$work/$documents.rep
    : > "$rep"
    awk -v style="$style" -v plugin="$plugin" -v ver="$ver" -v cc="$cc" -v ccver="$ccver" \
        -v report="$rep" "$AWK_PINS" "$doc" > "$out"
    # awk ends its last line with a newline; a file that had none keeps none.
    if [ -s "$doc" ] && [ -n "$(tail -c 1 "$doc")" ]; then
        printf '%s' "$(cat "$out")" > "$out.n" && mv "$out.n" "$out"
    fi
    while IFS=$'\t' read -r kind a b c; do
        case $kind in
            ERR) problem "$rel: $a" ;;
            COUNT)
                total_pins=$((total_pins + a))
                if [ "$style" = var ]; then
                    if [ "$b" -eq 0 ]; then
                        problem "$rel: no line starts with VERSION=; add \`VERSION=$ver\` to the Step 2 code block"
                    elif [ "$b" -gt 1 ]; then
                        problem "$rel: $b lines start with VERSION=; keep one and rename the others"
                    fi
                    if [ "$need_cc" -eq 1 ] && [ "$c" -eq 0 ]; then
                        problem "$rel: opencode.jsonc declares a local MCP server but no line starts with CC_VERSION=; add \`CC_VERSION=$ccver\` under the VERSION= line"
                    elif [ "$need_cc" -eq 1 ] && [ "$c" -gt 1 ]; then
                        problem "$rel: $c lines start with CC_VERSION=; keep one and rename the others"
                    elif [ "$need_cc" -eq 0 ] && [ "$c" -gt 0 ]; then
                        problem "$rel: a line starts with CC_VERSION= but opencode.jsonc declares no local MCP server; delete the line"
                    fi
                fi ;;
        esac
    done < "$rep"
    if has_stale_claim "$doc"; then
        problem "$rel: says \`current as of this writing\`; reword it to point at the pinned line or URL instead of repeating a version"
    fi
    if ! cmp -s "$out" "$doc"; then
        stale+=("$rel")
        outs+=("$out")
    fi
}

# run ROOT MODE(check|write)
run() {
    local root=$1 mode=$2 p dir ver cc ccman ccver need_cc n manifest agent i
    REGISTRY=$root/agents.json
    if [ ! -f "$REGISTRY" ]; then
        echo "check-install-pins: $REGISTRY not found" >&2
        return 1
    fi
    # shellcheck source=scripts/lib/registry.sh
    source "$here/lib/registry.sh"
    REGISTRY=$root/agents.json
    work=$(mktemp -d)
    # shellcheck disable=SC2064  # expand now: the directory name is fixed here
    trap "rm -rf '$work'" EXIT

    for p in $(registry_plugins opencode); do
        dir=opencode/plugins/$p
        manifest=$(registry_manifest_path opencode "$p")
        ver=$(manifest_version "$root/$manifest")
        if ! is_semver "$ver"; then
            problem "$manifest: version '$ver' is missing or is not x.y.z"
            continue
        fi
        need_cc=0
        if [ -f "$root/$dir/opencode.jsonc" ]; then
            if ! n=$(jq '[.mcp.servers // {} | .[] | select(.type == "local")] | length' "$root/$dir/opencode.jsonc" 2> /dev/null | tr -d '\r') || [ -z "$n" ]; then
                problem "$dir/opencode.jsonc: not valid JSON, so the script cannot tell whether a binary is needed"
                continue
            fi
            [ "$n" -gt 0 ] && need_cc=1
        fi
        cc=${p%-opencode}-cc
        ccman=$(registry_manifest_path claude-code "$cc")
        ccver=""
        if [ -f "$root/$ccman" ]; then
            ccver=$(manifest_version "$root/$ccman")
            if ! is_semver "$ccver"; then
                problem "$ccman: version '$ccver' is missing or is not x.y.z"
                continue
            fi
        elif [ "$need_cc" -eq 1 ]; then
            problem "$dir/INSTALL.md: needs a CC_VERSION= pin but the sibling manifest $ccman does not exist"
            continue
        fi
        scan "$root" "$dir/INSTALL.md" var "$p" "$ver" "$cc" "$ccver" "$need_cc"
    done
    for agent in antigravity qwen; do
        for p in $(registry_plugins "$agent"); do
            manifest=$(registry_manifest_path "$agent" "$p")
            ver=$(manifest_version "$root/$manifest")
            if ! is_semver "$ver"; then
                problem "$manifest: version '$ver' is missing or is not x.y.z"
                continue
            fi
            scan "$root" "$(dirname "$manifest")/README.md" lit "$p" "$ver" "" "" 0
        done
    done

    if [ "$total_pins" -eq 0 ] && [ ${#structural[@]} -eq 0 ]; then
        problem "no pin found in any document under $root; the registry or the layout has gone stale"
    fi

    if [ ${#structural[@]} -gt 0 ]; then
        echo "check-install-pins: ${#structural[@]} structural problem(s); each needs a hand edit:" >&2
        for p in "${structural[@]}"; do echo "  FAIL     $p" >&2; done
        if [ ${#stale[@]} -gt 0 ]; then
            echo "${#stale[@]} document(s) also carry a stale pin. After the edits above, run: bash scripts/check-install-pins.sh --write" >&2
        fi
        [ "$mode" = write ] && echo "Nothing was written." >&2
        return 1
    fi
    if [ ${#stale[@]} -eq 0 ]; then
        echo "check-install-pins: $total_pins pin(s) in $documents document(s) are current."
        return 0
    fi
    if [ "$mode" = write ]; then
        for i in "${!stale[@]}"; do
            cat "${outs[$i]}" > "$root/${stale[$i]}"
            echo "  rewrote  ${stale[$i]}"
        done
        echo "check-install-pins: rewrote ${#stale[@]} of $documents document(s); $total_pins pin(s) are current."
        return 0
    fi
    echo "check-install-pins: ${#stale[@]} of $documents document(s) carry a stale pin ($total_pins pin(s) checked):" >&2
    for p in "${stale[@]}"; do echo "  stale    $p" >&2; done
    echo "Fix: bash scripts/check-install-pins.sh --write" >&2
    return 1
}

# --- self-test --------------------------------------------------------------
# Every case runs the real script against a throwaway tree. The documents are
# copies of the real shapes: a blockquoted header, a URL in a table cell, a
# backticked prose tag followed by a full stop, a `-v$CC_VERSION` prose token.
# shellcheck disable=SC2016  # the backticks and $VERSION are literal document text
selftest() {
    local n=0 out rc t
    # (global: the EXIT trap runs after this function's locals are gone)
    fx=$(mktemp -d)
    trap 'rm -rf "$fx"' EXIT

    check() { # check NAME RESULT(0 = pass) DETAIL
        n=$((n + 1))
        if [ "$2" -eq 0 ]; then echo "ok $1"; else echo "FAIL $1: $3" >&2; printf '%s\n' "$out" | sed 's/^/     /' >&2; exit 1; fi
    }
    # go ARGS...: run the script on $t, keep its output and exit code.
    go() {
        set +e
        out=$(bash "$self" "$@" --root "$t" 2>&1)
        rc=$?
        set -e
    }
    has() { printf '%s\n' "$out" | grep -qF -e "$1"; }
    same() { diff -r "$t" "$t.before" > /dev/null; }
    snap() { rm -rf "$t.before"; cp -R "$t" "$t.before"; }

    # tree OLD: a stale tree whose every pin reads OLD (manifests are ahead of it).
    #   a-opencode  local MCP server, sibling a-cc      s-opencode  skills-only, sibling s-cc, no opencode.jsonc
    #   t-opencode  opencode.jsonc, no local server    a-agy  URL and prose tag    n-agy  no URL
    #   a-qwen      legacy lockstep tag in a command line and in a table cell
    tree() {
        local d=$t old=$1
        rm -rf "$d"
        mkdir -p "$d/opencode/plugins/a-opencode" "$d/opencode/plugins/s-opencode" "$d/opencode/plugins/t-opencode" \
            "$d/claude-code/plugins/a-cc/.claude-plugin" "$d/claude-code/plugins/s-cc/.claude-plugin" \
            "$d/antigravity/plugins/a-agy" "$d/antigravity/plugins/n-agy" "$d/qwen/plugins/a-qwen"
        cat > "$d/agents.json" <<'JSON'
{"agents": [
  {"id": "claude-code", "plugins": ["a-cc", "s-cc"], "manifestPattern": "claude-code/plugins/{plugin}/.claude-plugin/plugin.json"},
  {"id": "opencode", "plugins": ["a-opencode", "s-opencode", "t-opencode"], "manifestPattern": "opencode/plugins/{plugin}/package.json"},
  {"id": "antigravity", "plugins": ["a-agy", "n-agy"], "manifestPattern": "antigravity/plugins/{plugin}/plugin.json"},
  {"id": "qwen", "plugins": ["a-qwen"], "manifestPattern": "qwen/plugins/{plugin}/qwen-extension.json"}
]}
JSON
        printf '{"name": "a-opencode", "version": "1.2.3"}\n' > "$d/opencode/plugins/a-opencode/package.json"
        printf '{"name": "s-opencode", "version": "2.0.0"}\n' > "$d/opencode/plugins/s-opencode/package.json"
        printf '{"name": "t-opencode", "version": "3.10.1"}\n' > "$d/opencode/plugins/t-opencode/package.json"
        printf '{"version": "4.5.6"}\n' > "$d/claude-code/plugins/a-cc/.claude-plugin/plugin.json"
        printf '{"version": "7.7.7"}\n' > "$d/claude-code/plugins/s-cc/.claude-plugin/plugin.json"
        printf '{"version": "5.0.0"}\n' > "$d/antigravity/plugins/a-agy/plugin.json"
        printf '{"version": "5.1.0"}\n' > "$d/antigravity/plugins/n-agy/plugin.json"
        printf '{"version": "6.0.0"}\n' > "$d/qwen/plugins/a-qwen/qwen-extension.json"
        printf '{"mcp": {"servers": {"a": {"type": "local", "command": ["./bin/a-cc-mcp"]}}}}\n' > "$d/opencode/plugins/a-opencode/opencode.jsonc"
        printf '{"mcp": {"servers": {"r": {"type": "remote", "url": "https://example.com"}}}}\n' > "$d/opencode/plugins/t-opencode/opencode.jsonc"
        cat > "$d/opencode/plugins/a-opencode/INSTALL.md" <<MD
> Everything below comes from versioned release assets. Agree the versions
> with the user first; Step 2 pins them.

## Step 2

\`\`\`bash
VERSION=$old
CC_VERSION=$old
curl -fsSL -o /tmp/a.zip \\
  "https://github.com/ckir/aiplugins/releases/download/a-opencode-v\$VERSION/a-opencode-opencode.zip"
\`\`\`

The binary ships in asset \`a-cc-plugin.zip\` on release \`a-cc-v\$CC_VERSION\` - reuse that zip:

     "https://github.com/ckir/aiplugins/releases/download/a-cc-v\$CC_VERSION/a-cc-plugin.zip"

Run \`bun add @opencode/plugin@2.0.32\` (a pin this script does not own).
Another plugin: https://github.com/ckir/aiplugins/releases/download/z-opencode-v0.0.1/z-opencode-opencode.zip
Not a release: https://github.com/ckir/aiplugins/tree/a-opencode-vNEXT/docs
A longer name that only ends like this one: pre-a-opencode-v0.0.1
MD
        cat > "$d/opencode/plugins/s-opencode/INSTALL.md" <<MD
Set the agreed version once:

\`\`\`bash
VERSION=$old
\`\`\`

on release \`s-opencode-v\$VERSION\`.
MD
        cat > "$d/opencode/plugins/t-opencode/INSTALL.md" <<MD
\`\`\`bash
VERSION=$old
\`\`\`
MD
        cat > "$d/antigravity/plugins/a-agy/README.md" <<MD
1. **Download the plugin bundle** for tag \`a-agy-v$old\`. The
   asset name is \`<package>-agy-plugin.zip\`:

   \`\`\`bash
   curl -fsSL -o /tmp/a-agy-agy-plugin.zip \\
     https://github.com/ckir/aiplugins/releases/download/a-agy-v$old/a-agy-agy-plugin.zip
   \`\`\`
MD
        printf '# n-agy\n\nBuild it from source; there is no release URL here.\n' > "$d/antigravity/plugins/n-agy/README.md"
        cat > "$d/qwen/plugins/a-qwen/README.md" <<MD
As of Qwen Code v0.22.3 the installer accepts a URL:

qwen extensions install https://github.com/ckir/aiplugins/releases/download/v$old/a-qwen-extension.zip

   | Platform | URL |
   |---|---|
   | All | https://github.com/ckir/aiplugins/releases/download/v$old/a-qwen-extension.zip |
MD
    }
    A=opencode/plugins/a-opencode/INSTALL.md

    # --- 1: the default mode fails on a stale tree, names --write, writes nothing
    t=$fx/t1; tree 0.1.0; snap
    go
    check stale-check-fails "$([ "$rc" -eq 1 ] && has "stale    $A" && has 'Fix: bash scripts/check-install-pins.sh --write' && echo 0 || echo 1)" "exit $rc"
    check check-writes-nothing "$(same && echo 0 || echo 1)" "the default mode changed a file"

    # --- 2: --write brings every pin to its manifest
    go --write
    check write-runs "$([ "$rc" -eq 0 ] && echo 0 || echo 1)" "exit $rc"
    ok=0
    grep -qx 'VERSION=1.2.3' "$t/$A" || ok=1
    grep -qx 'CC_VERSION=4.5.6' "$t/$A" || ok=1
    grep -qx 'VERSION=2.0.0' "$t/opencode/plugins/s-opencode/INSTALL.md" || ok=1
    grep -qx 'VERSION=3.10.1' "$t/opencode/plugins/t-opencode/INSTALL.md" || ok=1
    check variable-pins-moved "$ok" "a VERSION= or CC_VERSION= line is not at its manifest version"
    ok=0
    grep -qF 'for tag `a-agy-v5.0.0`. The' "$t/antigravity/plugins/a-agy/README.md" || ok=1
    grep -qF 'download/a-agy-v5.0.0/a-agy-agy-plugin.zip' "$t/antigravity/plugins/a-agy/README.md" || ok=1
    check literal-tag-moved "$ok" "the agy prose tag or URL is not at 5.0.0"
    ok=0
    [ "$(grep -c 'download/a-qwen-v6.0.0/a-qwen-extension.zip' "$t/qwen/plugins/a-qwen/README.md")" -eq 2 ] || ok=1
    grep -q 'download/v0' "$t/qwen/plugins/a-qwen/README.md" && ok=1
    grep -qF '| All | https://github.com/ckir/aiplugins/releases/download/a-qwen-v6.0.0/a-qwen-extension.zip |' "$t/qwen/plugins/a-qwen/README.md" || ok=1
    check legacy-tag-migrated "$ok" "a legacy v<x.y.z> URL was not migrated in both places"
    ok=0
    grep -qF 'download/a-opencode-v$VERSION/a-opencode-opencode.zip' "$t/$A" || ok=1
    grep -qF 'on release `a-cc-v$CC_VERSION` - reuse' "$t/$A" || ok=1
    grep -qF 'download/a-cc-v$CC_VERSION/a-cc-plugin.zip' "$t/$A" || ok=1
    grep -qF '@opencode/plugin@2.0.32' "$t/$A" || ok=1
    grep -qF 'download/z-opencode-v0.0.1/z-opencode-opencode.zip' "$t/$A" || ok=1
    grep -qF 'tree/a-opencode-vNEXT/docs' "$t/$A" || ok=1
    grep -qF 'pre-a-opencode-v0.0.1' "$t/$A" || ok=1
    grep -qF 'Qwen Code v0.22.3' "$t/qwen/plugins/a-qwen/README.md" || ok=1
    grep -q 'CC_VERSION=' "$t/opencode/plugins/s-opencode/INSTALL.md" && ok=1
    cmp -s "$t/antigravity/plugins/n-agy/README.md" "$t.before/antigravity/plugins/n-agy/README.md" || ok=1
    check decoys-untouched "$ok" "something that is not this plugin's pin was rewritten"

    # --- 3: a fresh tree passes the check, and a second --write changes nothing
    snap
    go
    check fresh-check-passes "$([ "$rc" -eq 0 ] && has '8 pin(s) in 6 document(s) are current' && echo 0 || echo 1)" "exit $rc"
    go --write
    check idempotent "$([ "$rc" -eq 0 ] && same && echo 0 || echo 1)" "a second --write changed files"

    # --- 4: a hardcoded stale URL beside a correct VERSION= line
    t=$fx/t4; tree 0.1.0; go --write
    printf 'Mirror: https://github.com/ckir/aiplugins/releases/download/a-opencode-v0.9.9/a-opencode-opencode.zip\n' >> "$t/$A"
    printf 'Binary mirror: https://github.com/ckir/aiplugins/releases/download/a-cc-v0.9.9/a-cc-plugin.zip\n' >> "$t/$A"
    go
    check literal-in-install-fails "$([ "$rc" -eq 1 ] && has "stale    $A" && echo 0 || echo 1)" "exit $rc"
    go --write
    check literal-in-install-rewritten "$(grep -qF 'download/a-opencode-v1.2.3/a-opencode-opencode.zip' "$t/$A" && echo 0 || echo 1)" "the literal URL was not rewritten"
    check literal-sibling-tag-rewritten "$(grep -qF 'download/a-cc-v4.5.6/a-cc-plugin.zip' "$t/$A" && echo 0 || echo 1)" "the literal sibling URL was not rewritten"
    # only the version on a pin line moves; whatever follows it stays
    t=$fx/t4b; tree 0.1.0
    sed 's/^VERSION=0.1.0$/VERSION=0.1.0; echo "using $VERSION"/' "$t/opencode/plugins/t-opencode/INSTALL.md" > "$t/x" && mv "$t/x" "$t/opencode/plugins/t-opencode/INSTALL.md"
    go --write
    check pin-line-tail-kept "$([ "$rc" -eq 0 ] && grep -qxF 'VERSION=3.10.1; echo "using $VERSION"' "$t/opencode/plugins/t-opencode/INSTALL.md" && echo 0 || echo 1)" "the text after the version on the VERSION= line was lost"

    # --- 5: structural problems. Each fails, names the file, and does not offer --write.
    # bad NAME FILE-NAMED: judge the last `go`.
    bad() {
        check "$1" "$([ "$rc" -eq 1 ] && has "$2" && ! has '--write' && echo 0 || echo 1)" "exit $rc"
    }
    fresh() { t=$fx/$1; tree 0.1.0; go --write; }

    fresh s1; sed '/^VERSION=/d' "$t/$A" > "$t/x" && mv "$t/x" "$t/$A"; go
    bad missing-version-line "$A: no line starts with VERSION="
    fresh s2; printf 'VERSION=1.2.3\n' >> "$t/$A"; go
    bad duplicate-version-line "$A: 2 lines start with VERSION="
    fresh s3; sed '/^CC_VERSION=/d' "$t/$A" > "$t/x" && mv "$t/x" "$t/$A"; go
    bad missing-cc-line "$A: opencode.jsonc declares a local MCP server but no line starts with CC_VERSION="
    fresh s4; printf 'CC_VERSION=7.7.7\n' >> "$t/opencode/plugins/s-opencode/INSTALL.md"; go
    bad cc-line-without-binary "opencode/plugins/s-opencode/INSTALL.md: a line starts with CC_VERSION= but"
    fresh s5; rm "$t/claude-code/plugins/a-cc/.claude-plugin/plugin.json"; go
    bad missing-sibling-manifest "$A: needs a CC_VERSION= pin but the sibling manifest claude-code/plugins/a-cc/.claude-plugin/plugin.json does not exist"
    fresh s6; printf '{"version": "1.2"}\n' > "$t/opencode/plugins/a-opencode/package.json"; go
    bad non-semver-version "opencode/plugins/a-opencode/package.json: version '1.2'"
    fresh s7; printf '{"version": "v6"}\n' > "$t/qwen/plugins/a-qwen/qwen-extension.json"; go
    bad non-semver-version-qwen "qwen/plugins/a-qwen/qwen-extension.json: version 'v6'"
    fresh s8; printf 'Pick a version (`5.0.0` is current as of this writing).\n' >> "$t/antigravity/plugins/a-agy/README.md"; go
    bad stale-claim-guard "antigravity/plugins/a-agy/README.md: says \`current as of this writing\`"
    # the shape of opencode/plugins/repo-user-guide-opencode/INSTALL.md lines 9-10 before this change
    fresh s9; printf '> unpinned URLs. Agree the version with the user first (`0.2.0` is current as\n> of this writing; check the releases page)\n' >> "$t/$A"; go
    bad stale-claim-guard-wrapped "$A: says \`current as of this writing\`"
    fresh s10; printf 'https://github.com/ckir/aiplugins/releases/download/nightly/a-agy-agy-plugin.zip\n' >> "$t/antigravity/plugins/a-agy/README.md"; go
    bad unknown-tag-url "antigravity/plugins/a-agy/README.md: line 8: a release URL for a-agy is under the tag \`nightly\`"
    fresh s14; printf 'https://github.com/ckir/aiplugins/releases/download/a-opencode-v${VERSION}/a-opencode-opencode.zip\n' >> "$t/opencode/plugins/a-opencode/INSTALL.md"; go
    bad unknown-tag-url-install 'write the tag as a-opencode-v$VERSION'
    fresh s11; rm "$t/opencode/plugins/t-opencode/INSTALL.md"; go
    bad missing-document "opencode/plugins/t-opencode/INSTALL.md: the file does not exist"
    fresh s12; printf '{ not json\n' > "$t/opencode/plugins/a-opencode/opencode.jsonc"; go
    bad invalid-jsonc "opencode/plugins/a-opencode/opencode.jsonc: not valid JSON"
    # the sibling paths: its release URL under a tag that is not a version, a second CC_VERSION= line,
    # and a sibling manifest whose version is not x.y.z
    fresh s15; printf 'https://github.com/ckir/aiplugins/releases/download/a-cc-vNEXT/a-cc-plugin.zip\n' >> "$t/$A"; go
    bad sibling-url-wrong-tag "$A: line 21: a release URL for a-cc is under the tag \`a-cc-vNEXT\`"
    fresh s16; printf 'CC_VERSION=4.5.6\n' >> "$t/$A"; go
    bad duplicate-cc-version-line "$A: 2 lines start with CC_VERSION="
    fresh s17; printf '{"version": "4.5"}\n' > "$t/claude-code/plugins/a-cc/.claude-plugin/plugin.json"; go
    bad non-semver-sibling-version "claude-code/plugins/a-cc/.claude-plugin/plugin.json: version '4.5'"
    fresh s13; sed 's/^VERSION=.*/VERSION="2.0.0"/' "$t/opencode/plugins/s-opencode/INSTALL.md" > "$t/x" && mv "$t/x" "$t/opencode/plugins/s-opencode/INSTALL.md"; go
    bad version-line-not-semver 'opencode/plugins/s-opencode/INSTALL.md: line 4: the value after VERSION= is not an x.y.z version'

    # --- 6: --write is all-or-nothing, and a mixed tree lists the hand edits first
    t=$fx/t6; tree 0.1.0
    printf 'VERSION=0.1.0\n' >> "$t/opencode/plugins/t-opencode/INSTALL.md"
    snap
    go --write
    check write-all-or-nothing "$([ "$rc" -eq 1 ] && same && has 'Nothing was written.' && echo 0 || echo 1)" "exit $rc, or a file changed"
    check mixed-names-write-after "$(has 'After the edits above, run: bash scripts/check-install-pins.sh --write' && echo 0 || echo 1)" "no pointer to --write after the hand edits"

    # --- 7: CRLF survives, and a file with no final newline keeps none
    t=$fx/t7; tree 0.1.0
    awk '{ printf "%s\r\n", $0 }' "$t/$A" > "$t/x" && mv "$t/x" "$t/$A"
    printf '%s' "$(cat "$t/opencode/plugins/t-opencode/INSTALL.md")" > "$t/x" && mv "$t/x" "$t/opencode/plugins/t-opencode/INSTALL.md"
    lines=$(wc -l < "$t/$A")
    crs_before=$(tr -cd '\r' < "$t/$A" | wc -c)
    go --write
    ok=0
    [ "$rc" -eq 0 ] || ok=1
    # Carriage returns are counted with tr, byte for byte, so the count does not
    # depend on how this system's grep treats a carriage return.
    crs=$(tr -cd '\r' < "$t/$A" | wc -c)
    [ "$crs_before" -eq "$lines" ] || ok=1
    [ "$crs" -eq "$lines" ] || ok=1
    tr -d '\r' < "$t/$A" | grep -qx 'VERSION=1.2.3' || ok=1
    check crlf-kept "$ok" "a CRLF line ending was lost, or the pin did not move ($lines lines; carriage returns before: $crs_before, after: $crs)"
    check no-final-newline-kept "$([ -n "$(tail -c 1 "$t/opencode/plugins/t-opencode/INSTALL.md")" ] && grep -q '^VERSION=3.10.1$' "$t/opencode/plugins/t-opencode/INSTALL.md" && echo 0 || echo 1)" "a final newline was added"

    # --- 8: a tree with no pin anywhere is refused (a wrong --root must not pass)
    t=$fx/t8; mkdir -p "$t"
    printf '{"agents": [{"id": "claude-code", "plugins": [], "manifestPattern": "x/{plugin}"}, {"id": "opencode", "plugins": [], "manifestPattern": "x/{plugin}"}, {"id": "antigravity", "plugins": [], "manifestPattern": "x/{plugin}"}, {"id": "qwen", "plugins": [], "manifestPattern": "x/{plugin}"}]}\n' > "$t/agents.json"
    go
    check zero-census-refused "$([ "$rc" -eq 1 ] && has 'no pin found in any document' && echo 0 || echo 1)" "exit $rc"
    t=$fx/t9; mkdir -p "$t"
    go
    check missing-registry-refused "$([ "$rc" -eq 1 ] && has 'agents.json not found' && echo 0 || echo 1)" "exit $rc"

    echo "check-install-pins selftest: $n cases passed"
}

# --- main -------------------------------------------------------------------
mode="check"
root=$(cd "$here/.." && pwd -P)
while [ $# -gt 0 ]; do
    case $1 in
        --selftest) selftest; exit 0 ;;
        --write) mode="write"; shift ;;
        --root) root=${2:?--root needs a directory}; shift 2 ;;
        *) echo "usage: check-install-pins.sh [--write | --selftest] [--root DIR]" >&2; exit 2 ;;
    esac
done
run "$root" "$mode"
