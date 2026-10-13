#!/usr/bin/env bash
#
# Build every skills-only plugin bundle and assert the zips are what an
# installer expects. The oracle for scripts/bundle-skills-plugin.sh.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"
# shellcheck source=lib/registry.sh disable=SC1091
source scripts/lib/registry.sh

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail=0
bad() {
    echo "  FAIL  $1"
    fail=$((fail + 1))
}

pairs="claude-code:aiplugins-devtools-cc
qwen:aiplugins-devtools-qwen
antigravity:aiplugins-devtools-agy
opencode:aiplugins-devtools-opencode"

out="$tmp/out"
for pair in $pairs; do
    agent=${pair%%:*}
    key=${pair#*:}
    dir=$(registry_field "$agent" dir)
    suffix=$(registry_field "$agent" artifactSuffix)
    zip="$out/$key$suffix"
    manifest=$(registry_manifest_path "$agent" "$key")
    rel=${manifest#"$dir/$key/"}

    if ! bash scripts/bundle-skills-plugin.sh "$agent" "$key" "$out" >/dev/null 2>&1; then
        bad "$agent/$key: bundler failed"
        continue
    fi
    [ -f "$zip" ] || { bad "$agent/$key: $zip missing"; continue; }

    disk_version=$(jq -r .version "$manifest")
    zname=$(unzip -p "$zip" "$rel" | jq -r .name)
    zversion=$(unzip -p "$zip" "$rel" | jq -r .version)
    [ "$zname" = "aiplugins-devtools" ] ||
        bad "$agent/$key: manifest name is '$zname'"
    { [ "$zversion" = "$disk_version" ] && [ "$disk_version" != null ]; } ||
        bad "$agent/$key: zip version '$zversion' != disk '$disk_version'"

    n=0
    while IFS= read -r f; do
        n=$((n + 1))
        rel_skill=${f#"$dir/$key/"}
        unzip -p "$zip" "$rel_skill" | cmp -s - "$f" ||
            bad "$agent/$key: $rel_skill differs from disk"
    done < <(find "$dir/$key/skills" -name SKILL.md | sort)
    [ "$n" -gt 0 ] || bad "$agent/$key: no SKILL.md on disk"
done

# Negative controls: throwaway copies passed as the 4th argument.
neg="$tmp/neg"
mkdir -p "$neg"

cp -r claude-code/aiplugins-devtools-cc "$neg/noskills"
rm -rf "$neg/noskills/skills"
rc=0
bash scripts/bundle-skills-plugin.sh claude-code aiplugins-devtools-cc "$neg/o1" "$neg/noskills" >/dev/null 2>&1 || rc=$?
[ "$rc" -eq 1 ] || bad "control (a): copy without skills/ exited $rc, expected 1"
[ ! -e "$neg/o1/aiplugins-devtools-cc-plugin.zip" ] || bad "control (a): zip left behind"

cp -r claude-code/aiplugins-devtools-cc "$neg/withbin"
mkdir -p "$neg/withbin/bin"
echo x > "$neg/withbin/bin/x"
rc=0
bash scripts/bundle-skills-plugin.sh claude-code aiplugins-devtools-cc "$neg/o2" "$neg/withbin" >/dev/null 2>&1 || rc=$?
[ "$rc" -eq 1 ] || bad "control (b): copy with bin/x exited $rc, expected 1"
[ ! -e "$neg/o2/aiplugins-devtools-cc-plugin.zip" ] || bad "control (b): zip left behind"

rc=0
bash scripts/bundle-skills-plugin.sh claude-code rtk-mcp-cc "$neg/o3" >/dev/null 2>&1 || rc=$?
[ "$rc" -eq 1 ] || bad "control (c): non-skills-only exited $rc, expected 1"
[ ! -e "$neg/o3/rtk-mcp-cc-plugin.zip" ] || bad "control (c): zip left behind"

# Controls (d)/(e): the staging dir must never coincide with, sit inside, or
# contain the source tree (the bundler rm -rf's its staging dir).
for ctl in d e; do
    ct="$tmp/ctl-$ctl"
    mkdir -p "$ct/p"
    cp -r claude-code/aiplugins-devtools-cc "$ct/p/aiplugins-devtools-cc"
    srcdir="$ct/p/aiplugins-devtools-cc"
    before=$(find "$srcdir" -type f | wc -l)
    if [ "$ctl" = d ]; then od="$ct/p"; else od="$srcdir/out"; fi
    rc=0
    msg=$(bash scripts/bundle-skills-plugin.sh claude-code aiplugins-devtools-cc "$od" "$srcdir" 2>&1) || rc=$?
    after=$(find "$srcdir" -type f -not -path "$srcdir/out/*" | wc -l)
    [ "$rc" -eq 1 ] || bad "control ($ctl): overlapping out-dir exited $rc, expected 1"
    [ "$before" = "$after" ] || bad "control ($ctl): source files $before -> $after"
    printf '%s' "$msg" | grep -q 'overlaps the source' ||
        bad "control ($ctl): error does not name the containment problem"
done

if [ "$fail" -gt 0 ]; then
    echo "Skills bundles inconsistent: $fail problem(s)." >&2
    exit 1
fi
echo "Skills bundles consistent."
