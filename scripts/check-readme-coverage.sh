#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Check that the root README's install section covers every OpenCode plugin.
#
#   bash scripts/check-readme-coverage.sh [--root DIR]
#   bash scripts/check-readme-coverage.sh --selftest
#
# Why: the OpenCode table and the "ask an agent" URL list in README.md are
# written by hand, and three of seven plugins were missing from both with
# every check green.
#
# It reads only the `## Installing` section of ROOT/README.md: from that heading
# to the next line that starts with `## `. The README links every plugin again
# in its repository-structure list, so a link elsewhere in the file proves
# nothing. Lines inside a fenced code block are ignored. Inside the section,
# for every OpenCode plugin in ROOT/agents.json it requires:
#   a ROW     a line that starts with `|` and contains `](opencode/plugins/<plugin>/)`
#   a BULLET  a line that starts with `- ` and contains the raw INSTALL.md URL
#             https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/<plugin>/INSTALL.md
#   the file  opencode/plugins/<plugin>/INSTALL.md
# and it fails on a row or a bullet that names a plugin the registry does not have.
# Matching is by substring, so backticks and link text never matter. It proves
# coverage, not that the words in a row are true.
#
# POSIX awk only: this runs on macOS (BSD awk) and Git Bash too.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
self=$here/$(basename "${BASH_SOURCE[0]}")

RAW=https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/

# Emits, for the `## Installing` section outside fenced blocks:
#   SECTION                  the heading was found
#   ROW<TAB>plugin           once per `](opencode/plugins/<plugin>/)` on a line starting with `|`
#   BULLET<TAB>plugin        once per raw INSTALL.md URL on a line starting with `- `
AWK_SECTION=$(cat <<'AWK'
# names LINE LEAD STOP KIND: print KIND and the text between LEAD and STOP, for every LEAD.
function names(s, lead, stop, kind,   i, j, name) {
    while ((i = index(s, lead)) > 0) {
        s = substr(s, i + length(lead))
        j = index(s, stop)
        if (j == 0) return
        name = substr(s, 1, j - 1)
        if (name != "" && name !~ /[^A-Za-z0-9_.-]/) print kind "\t" name
        s = substr(s, j)
    }
}
BEGIN { bt = sprintf("%c", 96); fence3 = bt bt bt; inside = 0; infence = 0 }
{
    line = $0
    sub(/\r$/, "", line)
    probe = line
    sub(/^ */, "", probe)
    if (index(probe, fence3) == 1 || index(probe, "~~~") == 1) { infence = !infence; next }
    if (infence) next
    if (line == "## Installing") { inside = 1; print "SECTION"; next }
    if (inside && index(line, "## ") == 1) { inside = 0; next }
    if (!inside) next
    if (index(line, "|") == 1) names(line, "](opencode/plugins/", "/)", "ROW")
    if (index(line, "- ") == 1) names(line, raw, "/INSTALL.md", "BULLET")
}
AWK
)

TAB=$'\t'
failures=0
fail() { echo "  FAIL     $1" >&2; failures=$((failures + 1)); }

# run ROOT
run() {
    local root=$1 found p plugins n=0
    if [ ! -f "$root/README.md" ]; then
        fail "README.md: not found under $root"
        return 0
    fi
    REGISTRY=$root/agents.json
    if [ ! -f "$REGISTRY" ]; then
        fail "agents.json: not found under $root"
        return 0
    fi
    # shellcheck source=scripts/lib/registry.sh
    source "$here/lib/registry.sh"
    REGISTRY=$root/agents.json
    found=$(awk -v raw="$RAW" "$AWK_SECTION" "$root/README.md")
    if ! printf '%s\n' "$found" | grep -qx 'SECTION'; then
        fail "README.md: no line reads \`## Installing\`, so there is no section to check"
        return 0
    fi
    plugins=$(registry_plugins opencode)
    for p in $plugins; do
        n=$((n + 1))
        if ! printf '%s\n' "$found" | grep -qxF "ROW$TAB$p"; then
            fail "README.md: the OpenCode table in \`## Installing\` has no row linking opencode/plugins/$p/"
        fi
        if ! printf '%s\n' "$found" | grep -qxF "BULLET$TAB$p"; then
            fail "README.md: the URL list in \`## Installing\` has no bullet with ${RAW}$p/INSTALL.md"
        fi
        if [ ! -f "$root/opencode/plugins/$p/INSTALL.md" ]; then
            fail "opencode/plugins/$p/INSTALL.md: the file does not exist"
        fi
    done
    if [ "$n" -eq 0 ]; then
        fail "agents.json lists no OpenCode plugin; the registry or the layout has gone stale"
    fi
    while IFS=$'\t' read -r kind p; do
        case $kind in ROW | BULLET) ;; *) continue ;; esac
        if ! printf '%s\n' "$plugins" | grep -qxF "$p"; then
            if [ "$kind" = ROW ]; then
                fail "README.md: a table row in \`## Installing\` links opencode/plugins/$p/, which agents.json does not list"
            else
                fail "README.md: a bullet in \`## Installing\` names opencode/plugins/$p/INSTALL.md, which agents.json does not list"
            fi
        fi
    done <<< "$found"
    if [ "$failures" -eq 0 ]; then
        echo "check-readme-coverage: $n OpenCode plugin(s) each have a row, a bullet and an INSTALL.md."
    fi
}

# --- self-test --------------------------------------------------------------
# Every case is a seeded defect in a throwaway tree whose README copies the
# real shapes: backticks inside the link text, the URL in backticks, a fenced
# block inside the section, and a structure list that links every plugin again.
# shellcheck disable=SC2016  # the backticks are literal Markdown
selftest() {
    local n=0 out rc t
    # (global: the EXIT trap runs after this function's locals are gone)
    fx=$(mktemp -d)
    trap 'rm -rf "$fx"' EXIT

    check() { # check NAME RESULT(0 = pass) DETAIL
        n=$((n + 1))
        if [ "$2" -eq 0 ]; then echo "ok $1"; else echo "FAIL $1: $3" >&2; printf '%s\n' "$out" | sed 's/^/     /' >&2; exit 1; fi
    }
    go() {
        set +e
        out=$(bash "$self" --root "$t" 2>&1)
        rc=$?
        set -e
    }
    has() { printf '%s\n' "$out" | grep -qF -e "$1"; }
    # edit SED-ARGS...: rewrite the fixture README.
    edit() { sed "$@" "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; }

    # tree: two registered plugins, both fully covered.
    tree() {
        rm -rf "$t"
        mkdir -p "$t/opencode/plugins/a-opencode" "$t/opencode/plugins/b-opencode"
        : > "$t/opencode/plugins/a-opencode/INSTALL.md"
        : > "$t/opencode/plugins/b-opencode/INSTALL.md"
        printf '{"agents": [{"id": "opencode", "plugins": ["a-opencode", "b-opencode"], "manifestPattern": "opencode/plugins/{plugin}/package.json"}]}\n' > "$t/agents.json"
        cat > "$t/README.md" <<'MD'
# Fixture

## Installing

```bash
claude plugin marketplace add ckir/aiplugins
```

| Plugin | Copies | Binary staging |
|---|---|---|
| [`a-opencode`](opencode/plugins/a-opencode/) | `plugin.ts` → `.opencode/plugins/` | `just build-a-opencode` |
| [`b-opencode`](opencode/plugins/b-opencode/) | `skills/*` → `.opencode/skills/` | none |

Prefer to have an agent do it?

- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/a-opencode/INSTALL.md`
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/b-opencode/INSTALL.md`

## Repository structure

- `opencode/plugins/`
  - [`a-opencode`](opencode/plugins/a-opencode/) — one.
  - [`b-opencode`](opencode/plugins/b-opencode/) — two.

| Plugin | Note |
|---|---|
| [`a-opencode`](opencode/plugins/a-opencode/) | a row outside the section |
| [`b-opencode`](opencode/plugins/b-opencode/) | a row outside the section |

- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/a-opencode/INSTALL.md`
- `https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/b-opencode/INSTALL.md`
MD
    }
    bad() { check "$1" "$([ "$rc" -eq 1 ] && has "$2" && echo 0 || echo 1)" "exit $rc"; }

    t=$fx/t; tree; go
    check covered-tree-passes "$([ "$rc" -eq 0 ] && has '2 OpenCode plugin(s) each have a row' && echo 0 || echo 1)" "exit $rc"

    tree; edit -e '1,/^## Repository structure$/{/^| \[`b-opencode`\]/d;}'; go
    bad missing-row 'no row linking opencode/plugins/b-opencode/'
    check missing-row-only "$(has 'no bullet' && echo 1 || echo 0)" "a bullet that is present was reported"

    tree; edit -e '1,/^## Repository structure$/{/^- .*b-opencode\/INSTALL.md/d;}'; go
    bad missing-bullet 'no bullet with https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/b-opencode/INSTALL.md'

    tree; edit -e '1,/^## Repository structure$/s|^\(- .*plugins/b-opencode/\)INSTALL.md|\1README.md|'; go
    bad bullet-wrong-file 'no bullet with'

    tree; edit -e '1,/^## Repository structure$/s|^\(- .*\)refs/heads/main\(/opencode/plugins/b-opencode/\)|\1main\2|'; go
    bad bullet-wrong-path 'no bullet with'

    tree; edit -e '1,/^## Repository structure$/s|^- \(.*b-opencode/INSTALL.md\)|  - \1|'; go
    bad bullet-not-at-line-start 'no bullet with https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main/opencode/plugins/b-opencode/INSTALL.md'

    tree
    edit -e '1,/^## Repository structure$/s#^| \[`b-opencode`\].*#| [`z-opencode`](opencode/plugins/z-opencode/) | x | y |#'; go
    bad unregistered-row 'links opencode/plugins/z-opencode/, which agents.json does not list'

    tree
    edit -e '1,/^## Repository structure$/s|^\(- .*plugins/\)b-opencode\(/INSTALL.md.*\)|\1z-opencode\2|'; go
    bad unregistered-bullet 'names opencode/plugins/z-opencode/INSTALL.md, which agents.json does not list'

    tree; rm "$t/opencode/plugins/b-opencode/INSTALL.md"; go
    bad missing-install-file 'opencode/plugins/b-opencode/INSTALL.md: the file does not exist'

    # The case that proves the section scoping: b keeps its row and its bullet
    # in the section after `## Installing`, and loses them only inside it.
    tree
    awk '/^## Repository structure$/ { past = 1 }
         !past && (/^\| \[`b-opencode`\]/ || /^- .*b-opencode\/INSTALL.md/) { next }
         { print }' "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; go
    check linked-only-outside-installing "$([ "$rc" -eq 1 ] && has 'no row linking opencode/plugins/b-opencode/' && has 'no bullet with' && [ "$(grep -c '^| \[`b-opencode`\]' "$t/README.md")" -eq 1 ] && [ "$(grep -c '^- .*b-opencode/INSTALL.md' "$t/README.md")" -eq 1 ] && echo 0 || echo 1)" "exit $rc"

    # A link in a sentence is not a table row.
    tree
    awk '/^## Repository structure$/ { past = 1 }
         !past && /^\| \[`b-opencode`\]/ { print "See [`b-opencode`](opencode/plugins/b-opencode/) for the rest."; next }
         { print }' "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; go
    bad link-outside-a-table-row 'no row linking opencode/plugins/b-opencode/'

    # A row and a bullet inside a fenced block do not count.
    tree
    awk 'BEGIN { f = sprintf("%c%c%c", 96, 96, 96) }
         /^## Repository structure$/ { past = 1 }
         !past && (/^\| \[`b-opencode`\]/ || /^- .*b-opencode\/INSTALL.md/) { print f; print; print f; next }
         { print }' "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; go
    check fenced-lines-ignored "$([ "$rc" -eq 1 ] && has 'no row linking opencode/plugins/b-opencode/' && has 'no bullet with' && echo 0 || echo 1)" "exit $rc"

    tree; edit -e 's/^## Installing$/## Install/'; go
    bad no-installing-section 'no line reads `## Installing`'

    tree; printf '{"agents": [{"id": "opencode", "plugins": [], "manifestPattern": "x/{plugin}"}]}\n' > "$t/agents.json"; go
    bad empty-registry 'agents.json lists no OpenCode plugin'

    tree; awk '{ printf "%s\r\n", $0 }' "$t/README.md" > "$t/x" && mv "$t/x" "$t/README.md"; go
    check crlf-readme-passes "$([ "$rc" -eq 0 ] && echo 0 || echo 1)" "exit $rc"

    echo "check-readme-coverage selftest: $n cases passed"
}

# --- main -------------------------------------------------------------------
root=$(cd "$here/.." && pwd -P)
while [ $# -gt 0 ]; do
    case $1 in
        --selftest) selftest; exit 0 ;;
        --root) root=${2:?--root needs a directory}; shift 2 ;;
        *) echo "usage: check-readme-coverage.sh [--root DIR] | --selftest" >&2; exit 2 ;;
    esac
done
run "$root"
[ "$failures" -eq 0 ]
