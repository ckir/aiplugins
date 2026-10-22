#!/usr/bin/env bash
#
# Check the storefront catalog (catalog.json) against the repository inputs.
#
#   bash scripts/check-site.sh [--root DIR] CATALOG
#   bash scripts/check-site.sh --selftest
#
# --root DIR  repository root to read (default "."; relative paths resolve
#             against the repository root, so fixtures can stand in for it).
#             Read under it: agents.json, each agent's marketplace file,
#             docs/footprints/*.json (with budgets.json) and
#             docs/site/families.txt.
# --selftest  run tests/site/gen-test.sh, require the generator's fixture
#             catalog to pass, then require one mutated copy of the oracle
#             catalog per invariant to fail with its `FAIL <id>` line.
#
# Every failure is one line `FAIL <id>: <plugin>: <detail>` (<plugin> is the
# dirKey, or `-` when catalog-wide). All checks run, so one defect shows up
# next to any others. This script never calls gen-marketplaces.sh; the
# `just site-check` recipe does.
# shellcheck disable=SC2016  # every single-quoted string here is a jq program, not shell
set -euo pipefail
cd "$(dirname "$0")/.."

selftest() {
    local case_n=0 rc out
    # (global: the EXIT trap runs after this function's locals are gone)
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT

    # --- (a) the generator's own cases, then its fixture catalog is accepted
    bash tests/site/gen-test.sh
    bash scripts/gen-site.sh --root tests/site/root "$tmp/gen.json"
    rc=0
    out=$(bash scripts/check-site.sh --root tests/site/root "$tmp/gen.json" 2>&1) || rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "FAIL fixture-accepted: checker rejected the generated fixture catalog (exit $rc): $out" >&2
        exit 1
    fi
    echo "ok fixture-accepted"

    # --- (b) one mutated copy of the oracle per case, fed straight to the checker
    # mutate NAME ID JQ-PROGRAM
    mutate() {
        local name=$1 id=$2 prog=$3 rc=0 out
        case_n=$((case_n + 1))
        jq "$prog" tests/site/expected/catalog.json > "$tmp/case.json" \
            || { echo "FAIL $name: mutation program failed" >&2; exit 1; }
        if cmp -s "$tmp/case.json" tests/site/expected/catalog.json; then
            echo "FAIL $name: mutation changed nothing" >&2
            exit 1
        fi
        out=$(bash scripts/check-site.sh --root tests/site/root "$tmp/case.json" 2>&1) || rc=$?
        if [ "$rc" -ne 1 ]; then
            echo "FAIL $name: expected exit 1, got $rc: $out" >&2
            exit 1
        fi
        case "$out" in
            *"FAIL $id"*) ;;
            *)
                echo "FAIL $name: output lacks 'FAIL $id': $out" >&2
                exit 1
                ;;
        esac
        echo "ok $name"
    }

    local alpha='.families[] | select(.key == "alpha-mcp") | .plugins'
    local rug='.families[] | select(.key == "repo-user-guide") | .plugins'
    mutate plugin-set-delete plugin-set "del(${alpha}[\"claude-code\"])"
    mutate plugin-set-duplicate plugin-set \
        "(${alpha}[\"claude-code\"]) as \$p | (.families[] | select(.key == \"gamma-skills\") | .plugins[\"claude-code\"]) = \$p"
    mutate tier-totals tier-totals "(${alpha}[\"claude-code\"].footprint.resident) += 1"
    mutate footprint-null footprint-null \
        "($alpha.qwen.footprint) = {\"resident\":1,\"invocation\":1,\"setup\":null,\"budget\":{}}"
    mutate name name "(${rug}[\"claude-code\"].name) = \"repo-user-guide-cc\""
    mutate install-shape install-shape "($alpha.qwen.install[0].command) = \"agy plugin install /tmp/x\""
    mutate install-count install-count "del(${alpha}[\"claude-code\"].install[1])"
    mutate null-fields null-fields "(${alpha}[\"claude-code\"].description) = \"\""
    mutate url-prefix url-prefix "($alpha.antigravity.install[0].command) = \"curl https://example.com/x\""
    mutate version version "(${alpha}[\"claude-code\"].version) = \"v0.1.0\""
    mutate readme readme \
        "(${alpha}[\"claude-code\"].readme) = \"https://github.com/ckir/aiplugins/tree/main/other\""
    mutate budget budget "($alpha.antigravity.footprint.budget.deltaBytes) = 500"
    mutate skills-only skills-only "(${rug}[\"claude-code\"].skillsOnly) = false"
    mutate family-set family-set "(.families[] | select(.key == \"alpha-mcp\") | .key) = \"alpha-mcpx\""
    mutate install-label-claude-code install-shape "(${alpha}[\"claude-code\"].install[0].label) = \"Foo\""
    mutate install-label-opencode install-shape "(${alpha}.opencode.install[0].label) = \"Foo\""
    mutate install-label-antigravity install-shape "(${alpha}.antigravity.install[0].label) = \"Foo\""
    mutate install-label-agy-override install-shape "(${rug}.antigravity.install[0].label) = \"Foo\""
    [ "$case_n" -eq 18 ] || { echo "FAIL selftest: ran $case_n cases, expected 18" >&2; exit 1; }

    # --- (c) staging and the staged-tree gate over the synthetic doc tree
    # stage-site.sh only removes an --out strictly inside the repository, so the
    # staging runs write under the git-ignored target/ (still a mktemp -d dir).
    mkdir -p target
    stg=$(mktemp -d target/selftest-stage.XXXXXX)
    sent_out='' sent_nt='' sent_ln='' link=''
    trap 'rm -rf "$tmp" "$stg" "$sent_out" "$sent_nt" "$sent_ln" "$link"' EXIT
    local good="$stg/out" s_rc s_out

    # stage_case NAME DOCDIR OUTDIR: sets s_rc and s_out
    stage_case() {
        s_rc=0
        s_out=$(bash scripts/stage-site.sh --root tests/site/root --doc-dir "$2" --out "$3" 2>&1) || s_rc=$?
    }
    # require NAME CONDITION-DESCRIPTION: fail the selftest unless the preceding test passed
    s_fail() {
        echo "FAIL $1: $2" >&2
        exit 1
    }

    stage_case stages-and-gates tests/site/doc "$good"
    [ "$s_rc" -eq 0 ] || s_fail stages-and-gates "expected exit 0, got $s_rc: $s_out"
    echo "ok stages-and-gates"

    if grep -qF '../alpha_crate/index.html' "$good/api/index.html" \
        && grep -qF '../rtk_mcp_cc/index.html' "$good/api/index.html" \
        && ! grep -qF 'mock_rtk' "$good/api/index.html" \
        && [ -f "$good/mock_rtk/index.html" ]; then
        echo "ok deny-filter"
    else
        s_fail deny-filter "api/index.html must list alpha_crate and rtk_mcp_cc, omit mock_rtk, and mock_rtk/index.html must still be staged"
    fi

    if grep -qF 'Fixture plugin A &amp; B' "$good/index.html" \
        && ! grep -qF 'Fixture plugin A & B' "$good/index.html"; then
        echo "ok escapes-ampersand"
    else
        s_fail escapes-ampersand "staged index.html lacks 'A &amp; B' or still has the raw ampersand"
    fi

    cp -R tests/site/doc "$stg/doc-collide"
    echo '<!doctype html><title>stale</title>' > "$stg/doc-collide/index.html"
    stage_case collision-refused "$stg/doc-collide" "$stg/out-collide"
    if [ "$s_rc" -ne 0 ] && [[ $s_out == *"cargo clean --doc"* ]]; then
        echo "ok collision-refused"
    else
        s_fail collision-refused "expected non-zero exit with 'cargo clean --doc', got exit $s_rc: $s_out"
    fi

    mkdir -p "$stg/doc-empty/static.files"
    echo 'a{}' > "$stg/doc-empty/static.files/a.css"
    stage_case gate-catches-empty-set "$stg/doc-empty" "$stg/out-empty"
    [ "$s_rc" -ne 0 ] || s_fail gate-catches-empty-set "staging a tree with no crate directory exited 0: $s_out"
    echo "ok gate-catches-empty-set"

    # gate_case NAME MUTATION-COMMAND...: mutate a fresh copy of the good output
    # (cwd is the copy during the mutation) and require --gate-only to reject it.
    gate_case() {
        local name=$1 copy="$stg/gate-$1" g_rc=0 g_out
        shift
        cp -R "$good" "$copy"
        (cd "$copy" && "$@") || s_fail "$name" "mutation command failed"
        g_out=$(bash scripts/stage-site.sh --gate-only --out "$copy" 2>&1) || g_rc=$?
        [ "$g_rc" -ne 0 ] || s_fail "$name" "gate accepted the mutated tree: $g_out"
        echo "ok $name"
    }
    # The unmutated copy must pass, so a rejection cannot come from a broken harness.
    cp -R "$good" "$stg/gate-control"
    s_out=$(bash scripts/stage-site.sh --gate-only --out "$stg/gate-control" 2>&1) \
        || s_fail gate-control "the unmutated staged copy was rejected: $s_out"
    gate_case gate-rejects-missing-styles rm styles.css
    gate_case gate-rejects-placeholder-left bash -c 'echo "<!-- NOSCRIPT -->" >> index.html'
    gate_case gate-rejects-denied-crate-listed bash -c 'echo "<a href=\"../mock_rtk/index.html\">mock_rtk</a>" >> api/index.html'
    gate_case gate-rejects-root-absolute-href bash -c 'echo "<a href=\"/x\">x</a>" >> index.html'
    gate_case gate-rejects-innerhtml-in-app bash -c 'echo "x.innerHTML = 1;" >> app.js'
    gate_case gate-rejects-footer-link-missing sed -i 's|href="api/index.html"|href="api/"|' index.html
    # --- (c) confinement: --out must be strictly inside <repo>/target/. Each case
    # points --out at a throwaway directory holding a sentinel, so even a broken
    # guard could only remove that throwaway directory.
    # refuse_case NAME OUTDIR SENTINEL
    refuse_case() {
        local name=$1 r_rc=0 r_out
        r_out=$(bash scripts/stage-site.sh --root tests/site/root --doc-dir tests/site/doc --out "$2" 2>&1) || r_rc=$?
        [ "$r_rc" -ne 0 ] || s_fail "$name" "staging into $2 exited 0: $r_out"
        [[ $r_out == *"refusing to remove"* ]] || s_fail "$name" "output lacks 'refusing to remove': $r_out"
        [ -e "$3" ] || s_fail "$name" "the sentinel $3 was removed"
        echo "ok $name"
    }
    # (globals: the EXIT trap runs after this function's locals are gone)
    sent_out=$(mktemp -d)
    echo keep > "$sent_out/sentinel"
    refuse_case out-outside-repo-refused "$sent_out" "$sent_out/sentinel"
    sent_nt=$(mktemp -d "$(pwd -P)/selftest-notarget.XXXXXX")
    echo keep > "$sent_nt/sentinel"
    refuse_case out-in-repo-not-target-refused "$sent_nt" "$sent_nt/sentinel"
    sent_ln=$(mktemp -d)
    echo keep > "$sent_ln/sentinel"
    link=$(mktemp -u "$(pwd -P)/target/selftest-link.XXXXXX")
    if ln -s "$sent_ln" "$link" 2> /dev/null && [ -L "$link" ]; then
        refuse_case out-symlink-escape-refused "$link" "$sent_ln/sentinel"
    else
        echo "ok out-symlink-escape-refused (skipped: no symlinks)"
    fi
    rm -rf "$stg" "$sent_out" "$sent_nt" "$sent_ln" "$link"
}

if [ "${1:-}" = "--selftest" ]; then
    [ $# -eq 1 ] || { echo "check-site: --selftest takes no other argument" >&2; exit 1; }
    selftest
    exit 0
fi

root=.
catalog=
while [ $# -gt 0 ]; do
    case "$1" in
        --root)
            [ $# -ge 2 ] || { echo "check-site: --root needs a directory" >&2; exit 1; }
            root=$2
            shift 2
            ;;
        *)
            [ -z "$catalog" ] || { echo "check-site: unexpected argument: $1" >&2; exit 1; }
            catalog=$1
            shift
            ;;
    esac
done
[ -n "$catalog" ] || { echo "usage: check-site.sh [--root DIR] CATALOG | --selftest" >&2; exit 1; }
[ -f "$catalog" ] || { echo "check-site: no such catalog: $catalog" >&2; exit 1; }
jq -e . "$catalog" > /dev/null 2>&1 || { echo "check-site: not valid JSON: $catalog" >&2; exit 1; }

REGISTRY="$root/agents.json"
export REGISTRY
set --
# shellcheck disable=SC1091  # sourced relative to the repository root after cd
source scripts/lib/registry.sh

# The jq on a Windows PATH emits CRLF; a stray carriage return would corrupt
# every value captured below.
jqr() {
    jq -r "$@" | tr -d '\r'
}
jqc() {
    jq -c "$@" | tr -d '\r'
}

failures=0
# fail ID PLUGIN DETAIL
fail() {
    printf 'FAIL %s: %s: %s\n' "$1" "$2" "$3"
    failures=$((failures + 1))
}

footprints="$root/docs/footprints"
oc_raw=https://raw.githubusercontent.com/ckir/aiplugins/refs/heads/main

# --- catalog-wide: plugin-set, family-set, stray nulls and empty strings -----
expected_keys=$(
    for agent in $(jqr '.agents[].id' "$REGISTRY"); do
        registry_plugins "$agent"
    done | LC_ALL=C sort
)
actual_keys=$(jqr '[.families[]?.plugins[]?.dirKey // ""] | .[]' "$catalog" | LC_ALL=C sort)
while IFS= read -r key; do
    [ -n "$key" ] || continue
    n=$(grep -cxF -- "$key" <<< "$actual_keys" || true)
    [ "$n" -eq 1 ] || fail plugin-set - "$key appears $n times in the catalog (expected once, per agents.json)"
done <<< "$expected_keys"
while IFS= read -r key; do
    grep -qxF -- "$key" <<< "$expected_keys" \
        || fail plugin-set - "catalog plugin '$key' is not in agents.json"
done <<< "$actual_keys"

families_file="$root/docs/site/families.txt"
if [ -f "$families_file" ]; then
    want_families=$(tr -d '\r' < "$families_file" | grep -v '^$' | LC_ALL=C sort || true)
else
    want_families=
    fail family-set - "$families_file is missing"
fi
have_families=$(jqr '[.families[]?.key // ""] | .[]' "$catalog" | LC_ALL=C sort)
if [ "$want_families" != "$have_families" ]; then
    fail family-set - "catalog family keys [$(tr '\n' ' ' <<< "$have_families")] differ from docs/site/families.txt [$(tr '\n' ' ' <<< "$want_families")]"
fi

rest_nulls=$(jqr '[del(.families[]?.plugins) | paths(. == null)] | length' "$catalog")
[ "$rest_nulls" -eq 0 ] || fail null-fields - "null outside the plugin entries"
rest_bad=$(jqr '[del(.families[]?.plugins) | .. | strings | select(length == 0 or test("undefined|NaN"))] | length' "$catalog")
[ "$rest_bad" -eq 0 ] || fail null-fields - "empty string, undefined or NaN outside the plugin entries"

# --- per plugin --------------------------------------------------------------
# agent_of KEY: the agent id whose plugin list holds KEY (empty if none).
agent_of() {
    local a
    for a in $(jqr '.agents[].id' "$REGISTRY"); do
        if registry_plugins "$a" | grep -qxF -- "$1"; then
            echo "$a"
            return
        fi
    done
}

# label_is KEY P INDEX WANT: the install entry's label must equal WANT.
label_is() {
    local got
    got=$(jqr --argjson i "$3" '.install[$i].label // ""' <<< "$2")
    [ "$got" = "$4" ] \
        || fail install-shape "$1" "install entry $3 label is '$got', expected '$4'; install table is in scripts/gen-site.sh"
}

# check_install KEY AGENT NAME URL SKILLS_ONLY P
check_install() {
    local key=$1 agent=$2 name=$3 url=$4 p=$6
    local n i label cmd saw_linux=0 saw_win=0
    n=$(jqr '(.install // []) | length' <<< "$p")
    case "$agent" in
        claude-code)
            if ! jq -e --arg a "claude plugin marketplace add ckir/aiplugins" --arg b "claude plugin install ${name}@aiplugins" \
                '[(.install // [])[] | .command] == [$a, $b]' <<< "$p" > /dev/null; then
                fail install-shape "$key" "Claude Code entries are not the two literal commands (claude plugin marketplace add ckir/aiplugins; claude plugin install ${name}@aiplugins); install table is in scripts/gen-site.sh"
            fi
            label_is "$key" "$p" 0 "Add the marketplace"
            label_is "$key" "$p" 1 "Install"
            ;;
        qwen)
            for ((i = 0; i < n; i++)); do
                label=$(jqr --argjson i "$i" '.install[$i].label // ""' <<< "$p")
                cmd=$(jqr --argjson i "$i" '.install[$i].command // ""' <<< "$p")
                [[ $cmd == *"$url"* ]] \
                    || fail install-shape "$key" "Qwen entry '$label' lacks the marketplace source.url; install table is in scripts/gen-site.sh"
                case "$label" in
                    "Linux and macOS")
                        saw_linux=1
                        [[ $cmd == *unzip* ]] \
                            || fail install-shape "$key" "Qwen 'Linux and macOS' entry lacks unzip; install table is in scripts/gen-site.sh"
                        ;;
                    "Windows (PowerShell)")
                        saw_win=1
                        [[ $cmd == *Expand-Archive* ]] \
                            || fail install-shape "$key" "Qwen 'Windows (PowerShell)' entry lacks Expand-Archive; install table is in scripts/gen-site.sh"
                        ;;
                esac
            done
            { [ "$saw_linux" = 1 ] && [ "$saw_win" = 1 ]; } \
                || fail install-shape "$key" "Qwen needs a 'Linux and macOS' and a 'Windows (PowerShell)' entry; install table is in scripts/gen-site.sh"
            ;;
        opencode)
            for ((i = 0; i < n; i++)); do
                cmd=$(jqr --argjson i "$i" '.install[$i].command // ""' <<< "$p")
                [[ $cmd == *"$oc_raw/opencode/plugins/$key/INSTALL.md"* ]] \
                    || fail install-shape "$key" "OpenCode command lacks $oc_raw/opencode/plugins/$key/INSTALL.md; install table is in scripts/gen-site.sh"
                label_is "$key" "$p" "$i" "Ask your agent"
            done
            ;;
        antigravity)
            for ((i = 0; i < n; i++)); do
                cmd=$(jqr --argjson i "$i" '.install[$i].command // ""' <<< "$p")
                [[ $cmd == *"$url"* ]] \
                    || fail install-shape "$key" "Antigravity command lacks the marketplace source.url; install table is in scripts/gen-site.sh"
                if [ "$key" = repo-user-guide-agy ]; then
                    label_is "$key" "$p" "$i" "Copy the skill"
                    { [[ $cmd == *"cp -r"* ]] && [[ $cmd != *"agy plugin install"* ]]; } \
                        || fail install-shape "$key" "the repo-user-guide-agy override needs cp -r and no agy plugin install; install table is in scripts/gen-site.sh"
                else
                    label_is "$key" "$p" "$i" "Install"
                    [[ $cmd == *"agy plugin install"* ]] \
                        || fail install-shape "$key" "Antigravity command lacks agy plugin install; install table is in scripts/gen-site.sh"
                fi
            done
            ;;
        *) fail install-shape "$key" "no install shape for agent ${agent}; add a row to the install table in scripts/gen-site.sh" ;;
    esac
}

total=0
while IFS= read -r row; do
    [ -n "$row" ] || continue
    total=$((total + 1))
    agent=$(jqr '.agent' <<< "$row")
    family=$(jqr '.family' <<< "$row")
    p=$(jqc '.p' <<< "$row")
    key=$(jqr '.dirKey // ""' <<< "$p")
    [ -n "$key" ] || key="?"

    # Unknown plugin, or filed under the wrong agent: nothing else can be looked up.
    if [ "$(agent_of "$key")" != "$agent" ]; then
        fail plugin-set "$key" "plugin is not an entry of agent '$agent' in agents.json"
        continue
    fi
    expname=$(registry_published_name "$agent" "$key")
    marketplace="$root/$(registry_field "$agent" marketplace)"
    item=$(jqc --arg n "$expname" '[.plugins[] | select(.name == $n)][0] // empty' "$marketplace")
    fpfile="$footprints/$key.json"

    # family-set: dirKey minus its agent suffix is the enclosing family key.
    stripped=$(sed -E 's/-(cc|qwen|opencode|agy)$//' <<< "$key")
    [ "$stripped" = "$family" ] \
        || fail family-set "$key" "dirKey minus its agent suffix is '$stripped' but it sits in family '$family'; families are listed in docs/site/families.txt"

    # name
    name=$(jqr '.name // ""' <<< "$p")
    [ "$name" = "$expname" ] || fail name "$key" "name '$name' should be '$expname' (publishedNames or the dirKey)"
    if [ -z "$item" ]; then
        fail name "$key" "no marketplace entry named '$expname' in $(registry_field "$agent" marketplace)"
    fi

    # footprint-null, tier-totals, version (pluginVersion), budget
    fp=$(jqc '.footprint' <<< "$p")
    fp_type=$(jqr 'type' <<< "$fp")
    nofp=true
    nosetup=false
    if [ -f "$fpfile" ]; then
        nofp=false
        [ "$fp_type" = object ] || fail footprint-null "$key" "footprint is $fp_type but docs/footprints/$key.json exists"
        if ! jq -e '.tiers.setup' "$fpfile" > /dev/null 2>&1; then nosetup=true; fi
        if [ "$fp_type" = object ]; then
                if ! jq -e -n --slurpfile f "$fpfile" --argjson c "$fp" '
                $f[0].tiers as $t
                | {resident: $t.resident.bytes, invocation: $t.invocation.bytes, setup: ($t.setup.bytes // null)}
                  == {resident: $c.resident, invocation: $c.invocation, setup: $c.setup}' > /dev/null; then
                fail tier-totals "$key" "footprint resident/invocation/setup differ from the tiers in docs/footprints/$key.json"
            fi
                if ! jq -e -n --slurpfile b "$footprints/budgets.json" --argjson c "$fp" --arg k "$key" \
                '$c.budget == $b[0][$k]' > /dev/null; then
                fail budget "$key" "footprint.budget differs from docs/footprints/budgets.json[$key]"
            fi
        fi
    else
        [ "$fp_type" = null ] || fail footprint-null "$key" "footprint is $fp_type but docs/footprints/$key.json does not exist"
    fi

    # install-count
    n=$(jqr '(.install // []) | length' <<< "$p")
    case "$agent" in
        claude-code | qwen)
            [ "$n" -eq 2 ] || fail install-count "$key" "$agent needs exactly 2 install entries, found $n"
            ;;
        *)
            [ "$n" -ge 1 ] || fail install-count "$key" "needs at least 1 install entry, found $n"
            ;;
    esac
    empty=$(jqr '[(.install // [])[] | (.command // "") | select(length == 0)] | length' <<< "$p")
    [ "$empty" -eq 0 ] || fail install-count "$key" "$empty install command(s) are empty"

    # null-fields
    stray=$(jqr --argjson nofp "$nofp" --argjson nosetup "$nosetup" '
        [paths(. == null) | map(tostring) | join(".")]
        - [if $nofp then "footprint" else empty end, if $nosetup then "footprint.setup" else empty end]
        | .[]' <<< "$p" | tr '\n' ' ')
    [ -z "$stray" ] || fail null-fields "$key" "unexpected null at: $stray"
    badstr=$(jqr '[.. | strings | select(length == 0 or test("undefined|NaN"))] | length' <<< "$p")
    [ "$badstr" -eq 0 ] || fail null-fields "$key" "$badstr string(s) are empty or contain undefined/NaN"

    # url-prefix
    while IFS= read -r url; do
        [ -n "$url" ] || continue
        case "$url" in
            https://github.com/ckir/aiplugins/* | https://raw.githubusercontent.com/ckir/aiplugins/*) ;;
            *) fail url-prefix "$key" "URL outside https://github.com/ckir/aiplugins/ and https://raw.githubusercontent.com/ckir/aiplugins/: $url" ;;
        esac
    done < <(jqr '[.readme?, ((.install // [])[]? | .command?)] | .[] | strings' <<< "$p" | grep -oE 'https?://[^][:space:]"<>]+' || true)

    # version
    version=$(jqr '.version // ""' <<< "$p")
    [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail version "$key" "version '$version' is not bare semver"
    url=
    if [ -n "$item" ]; then
        url=$(jqr '.source.url // ""' <<< "$item")
        tag=
        if [[ $url =~ -v([0-9]+\.[0-9]+\.[0-9]+)/ ]]; then tag=${BASH_REMATCH[1]}; fi
        [ "$version" = "$tag" ] || fail version "$key" "version '$version' differs from the -v tag '$tag' in the marketplace source.url"
        readme=$(jqr '.readme // ""' <<< "$p")
        home=$(jqr '.homepage // ""' <<< "$item")
        [ "$readme" = "$home" ] || fail readme "$key" "readme '$readme' differs from the marketplace homepage '$home'"
    else
        fail readme "$key" "no marketplace entry '$expname' to compare the readme against"
    fi
    if [ -f "$fpfile" ]; then
        pv=$(jqr '.pluginVersion // ""' "$fpfile")
        [ "$version" = "$pv" ] || fail version "$key" "version '$version' differs from pluginVersion '$pv' in docs/footprints/$key.json"
    fi

    # skills-only
    so=false
    if registry_is_skills_only "$agent" "$key"; then so=true; fi
    [ "$(jqr '.skillsOnly' <<< "$p")" = "$so" ] \
        || fail skills-only "$key" "skillsOnly should be $so (membership in the skillsOnly list of $agent in agents.json)"

    # install-shape (against the inputs, not the generator)
    if [ -n "$item" ]; then
        check_install "$key" "$agent" "$expname" "$url" "$so" "$p"
    fi
done < <(jqc '.families[]? | .key as $f | (.plugins // {}) | to_entries[] | {family: $f, agent: .key, p: .value}' "$catalog" | tr -d '\r')

if [ "$failures" -gt 0 ]; then
    exit 1
fi
echo "check-site: ok ($total plugins)"
