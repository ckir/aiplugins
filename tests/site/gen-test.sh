#!/usr/bin/env bash
#
# Cases for scripts/gen-site.sh against the fixture root in tests/site/root.
# Prints `ok <name>` per passing case; exits non-zero on the first failure.
set -euo pipefail
cd "$(dirname "$0")/../.."

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL $1: $2" >&2
    exit 1
}

# Run the generator on ROOT; leaves status in $status and stderr in $err.
run_gen() {
    status=0
    bash scripts/gen-site.sh --root "$1" "$2" 2> "$tmp/err" > /dev/null || status=$?
    err=$(cat "$tmp/err")
}

# expect_refusal NAME ROOT NEEDLE
expect_refusal() {
    run_gen "$2" "$tmp/refused.json"
    [ "$status" -eq 1 ] || fail "$1" "expected exit 1, got $status: $err"
    case "$err" in
        *"$3"*) ;;
        *) fail "$1" "message lacks '$3': $err" ;;
    esac
    echo "ok $1"
}

fresh_root() {
    rm -rf "${tmp:?}/$1"
    cp -r tests/site/root "$tmp/$1"
}

# --- matches-expected -------------------------------------------------------
run_gen tests/site/root "$tmp/out.json"
[ "$status" -eq 0 ] || fail matches-expected "exit $status: $err"
[ "$(jq -S . "$tmp/out.json")" = "$(jq -S . tests/site/expected/catalog.json)" ] \
    || fail matches-expected "output differs from tests/site/expected/catalog.json"
echo "ok matches-expected"

# --- no-install-shape -------------------------------------------------------
fresh_root zeta
jq '.agents += [{id: "zeta", dir: "zeta/plugins", plugins: ["beta-mcp-cc"],
        marketplace: ".zeta-plugin/marketplace.json", publishedNames: {}, skillsOnly: []}]' \
    tests/site/root/agents.json > "$tmp/zeta/agents.json"
mkdir -p "$tmp/zeta/.zeta-plugin"
cat > "$tmp/zeta/.zeta-plugin/marketplace.json" <<'JSON'
{"plugins": [{"name": "beta-mcp-cc",
  "source": {"url": "https://github.com/ckir/aiplugins/releases/download/beta-mcp-cc-v0.1.0/beta-mcp-cc-plugin.zip"},
  "description": "Fixture beta",
  "homepage": "https://github.com/ckir/aiplugins/tree/main/zeta/plugins/beta-mcp-cc"}]}
JSON
expect_refusal no-install-shape "$tmp/zeta" "install table in scripts/gen-site.sh"

# --- no-version-tag ---------------------------------------------------------
fresh_root notag
jq '(.plugins[] | select(.name == "alpha-mcp-cc") | .source.url) |= sub("-v0\\.1\\.0"; "")' \
    tests/site/root/.claude-plugin/marketplace.json > "$tmp/notag/.claude-plugin/marketplace.json"
expect_refusal no-version-tag "$tmp/notag" "-v<semver>"

# --- unknown-suffix ---------------------------------------------------------
fresh_root suffix
sed 's/"alpha-mcp-cc"/"alpha-mcp-zzz"/' tests/site/root/agents.json > "$tmp/suffix/agents.json"
expect_refusal unknown-suffix "$tmp/suffix" "agent suffix"

# --- crlf-input -------------------------------------------------------------
fresh_root crlf
cr=$(printf '\r')
while IFS= read -r f; do
    while IFS= read -r line || [ -n "$line" ]; do
        printf '%s%s\n' "$line" "$cr"
    done < "$f" > "$f.crlf"
    mv "$f.crlf" "$f"
done < <(find "$tmp/crlf" -name '*.json')
grep -q $'\r' "$tmp/crlf/agents.json" || fail crlf-input "fixture was not converted to CRLF"
run_gen "$tmp/crlf" "$tmp/crlf-out.json"
[ "$status" -eq 0 ] || fail crlf-input "exit $status: $err"
cmp -s "$tmp/out.json" "$tmp/crlf-out.json" || fail crlf-input "output differs from the LF run"
if grep -q $'\r' "$tmp/crlf-out.json"; then fail crlf-input "output contains a carriage return"; fi
echo "ok crlf-input"

# --- crlf-shim --------------------------------------------------------------
# A jq that appends a carriage return to every output line (as the jq on a
# Windows PATH does). The generator must still emit a byte-identical catalog
# with no carriage return, which only holds while it strips CR from jq output.
real_jq=$(command -v jq)
mkdir -p "$tmp/shim"
{
    echo '#!/usr/bin/env bash'
    printf 'real_jq=%q\n' "$real_jq"
    cat <<'SHIM'
rc=0
"$real_jq" "$@" > "${TMPDIR:-/tmp}/jqshim.$$" || rc=$?
while IFS= read -r line || [ -n "$line" ]; do
    printf '%s\r\n' "$line"
done < "${TMPDIR:-/tmp}/jqshim.$$"
rm -f "${TMPDIR:-/tmp}/jqshim.$$"
exit "$rc"
SHIM
} > "$tmp/shim/jq"
chmod +x "$tmp/shim/jq"
shim_rc=0
PATH="$tmp/shim:$PATH" bash scripts/gen-site.sh --root tests/site/root "$tmp/shim-out.json" 2> "$tmp/err" > /dev/null || shim_rc=$?
[ "$shim_rc" -eq 0 ] || fail crlf-shim "exit $shim_rc: $(cat "$tmp/err")"
cmp -s "$tmp/out.json" "$tmp/shim-out.json" || fail crlf-shim "output differs from the normal run"
if grep -q $'\r' "$tmp/shim-out.json"; then fail crlf-shim "output contains a carriage return"; fi
echo "ok crlf-shim"
