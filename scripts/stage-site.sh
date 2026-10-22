#!/usr/bin/env bash
#
# Assemble the GitHub Pages tree (rustdoc output plus the storefront).
#
#   bash scripts/stage-site.sh [--root DIR] [--doc-dir DIR] [--out DIR] [--gate-only]
#
# --root DIR     repository root handed to gen-site.sh and check-site.sh ONLY
#                (default "."). The static site files (docs/site/index.html,
#                styles.css, app.js) and docs/USER_GUIDE.pdf are always read
#                from the real repository root.
# --doc-dir DIR  rustdoc output to copy (default target/doc).
# --out DIR      staged tree to write (default target/site).
# --gate-only    run only the staged-tree gate on an existing --out directory.
#
# Relative paths resolve against the repository root. Every failure is
# `stage-site: <reason>` on stderr with exit 1.
set -euo pipefail
cd "$(dirname "$0")/.."

# Crates that stay on the site but are not listed in api/index.html: one
# constant, used by both the crate list (crate_set) and the gate.
DENY_PATTERNS=('*example*' 'mock_*' 'probe_target' 'footprint_gate' 'plugin_footprint')

# The jq on a Windows PATH emits CRLF; strip it from every captured value.
jqr() {
    jq -r "$@" | tr -d '\r'
}

die() {
    echo "stage-site: $*" >&2
    exit 1
}

# denied NAME: succeeds when NAME matches a deny pattern.
denied() {
    local pat
    for pat in "${DENY_PATTERNS[@]}"; do
        # shellcheck disable=SC2254  # the unquoted pattern is the point: shell globbing
        case "$1" in
            $pat) return 0 ;;
        esac
    done
    return 1
}

# crate_set DIR: sorted names of DIR's top-level directories, other than api,
# that contain an index.html, minus the deny patterns.
crate_set() {
    local d name
    for d in "$1"/*/; do
        [ -d "$d" ] || continue
        name=$(basename "$d")
        [ "$name" != api ] || continue
        [ -f "$d/index.html" ] || continue
        denied "$name" && continue
        echo "$name"
    done | LC_ALL=C sort
}

# gate_crate_scan OUT: the gate's own scan (deliberately a different
# implementation from crate_set; it must never call it).
gate_crate_scan() {
    local p name
    while IFS= read -r p; do
        name=${p##*/}
        if [ "$name" = api ] || [ ! -e "$p/index.html" ]; then
            continue
        fi
        if denied "$name"; then
            continue
        fi
        printf '%s\n' "$name"
    done < <(find "$1" -mindepth 1 -maxdepth 1 -type d) | LC_ALL=C sort
}

# gate_staged_tree OUT: run every check, one `stage-site: gate: <detail>` line
# per failure, exit 1 if any failed.
gate_staged_tree() {
    local out=$1 bad=0 f scanned listed name pat n
    gfail() {
        echo "stage-site: gate: $*" >&2
        bad=$((bad + 1))
    }

    # a. required files, non-empty
    for f in index.html styles.css app.js catalog.json USER_GUIDE.pdf api/index.html; do
        [ -s "$out/$f" ] || gfail "missing or empty: $f"
    done

    # b, c. index.html: placeholder replaced, noscript present, footer API link
    if [ -f "$out/index.html" ]; then
        if grep -qxF '<!-- NOSCRIPT -->' "$out/index.html"; then
            gfail "index.html still contains the <!-- NOSCRIPT --> placeholder line"
        fi
        grep -qF '<noscript>' "$out/index.html" || gfail "index.html has no <noscript> block"
        grep -qF 'href="api/index.html"' "$out/index.html" \
            || gfail 'index.html lacks the footer link href="api/index.html"'
    fi

    # d. scanned crate set non-empty
    scanned=$(gate_crate_scan "$out")
    [ -n "$scanned" ] || gfail "no crate directory with an index.html under $out"

    # e. api/index.html links exactly the scanned set
    if [ -f "$out/api/index.html" ]; then
        listed=$(
            { grep -o 'href="\.\./[^"/]*/index\.html"' "$out/api/index.html" || true; } \
                | sed -e 's|^href="\.\./||' -e 's|/index\.html"$||' | LC_ALL=C sort
        )
        if [ "$listed" != "$scanned" ]; then
            gfail "api/index.html lists [$(tr '\n' ' ' <<< "$listed")] but the scan found [$(tr '\n' ' ' <<< "$scanned")]"
        fi
        while IFS= read -r name; do
            [ -n "$name" ] || continue
            [ -f "$out/api/../$name/index.html" ] || gfail "api/index.html links ../$name/index.html which does not exist"
            if denied "$name"; then gfail "api/index.html lists the denied crate $name"; fi
        done <<< "$listed"
    fi

    # f. no root-absolute references in the files this script writes or copies
    for f in index.html api/index.html styles.css app.js; do
        [ -f "$out/$f" ] || continue
        for pat in 'href="/' 'src="/' "href='/" "src='/" 'url(/'; do
            if grep -qF -- "$pat" "$out/$f"; then gfail "$f contains the root-absolute reference $pat"; fi
        done
    done

    # g. app.js never builds markup from strings
    if [ -f "$out/app.js" ]; then
        for pat in innerHTML outerHTML insertAdjacentHTML document.write; do
            if grep -qF -- "$pat" "$out/app.js"; then gfail "app.js contains $pat"; fi
        done
    fi

    if [ "$bad" -gt 0 ]; then
        exit 1
    fi
    n=$(grep -c . <<< "$scanned" || true)
    echo "stage-site: gate ok ($n crates listed)"
}

root=.
doc=target/doc
out=target/site
gate_only=0
while [ $# -gt 0 ]; do
    case "$1" in
        --root | --doc-dir | --out)
            [ $# -ge 2 ] || die "$1 needs a directory"
            case "$1" in
                --root) root=$2 ;;
                --doc-dir) doc=$2 ;;
                --out) out=$2 ;;
            esac
            shift 2
            ;;
        --gate-only)
            gate_only=1
            shift
            ;;
        *) die "unexpected argument: $1" ;;
    esac
done

if [ "$gate_only" -eq 1 ]; then
    [ -d "$out" ] || die "no such output directory: $out"
    gate_staged_tree "$out"
    exit 0
fi

# --- safety: only ever remove a directory strictly inside the repository -----
repo=$(pwd -P)
out_abs=$(realpath -m -- "$out")
doc_abs=$(realpath -m -- "$doc")
[ -d "$doc_abs" ] || die "no such doc directory: $doc (run cargo doc first)"
case "$out_abs" in
    "$repo"/*) ;;
    *) die "refusing to remove $out_abs" ;;
esac
case "$doc_abs/" in
    "$out_abs"/*) die "refusing to remove $out_abs" ;;
esac
case "$out_abs/" in
    "$doc_abs"/*) die "refusing to remove $out_abs" ;;
esac

# --- 1. clean output, copy the doc directory ---------------------------------
rm -rf -- "$out_abs"
mkdir -p -- "$out_abs"
cp -R "$doc_abs"/. "$out_abs"/ || die "could not copy $doc_abs"

# --- 2. collision guard ------------------------------------------------------
for name in index.html styles.css app.js catalog.json USER_GUIDE.pdf api; do
    if [ -e "$out_abs/$name" ]; then
        die "collision: $name already exists in the doc directory; run 'cargo clean --doc' (a stale target/doc/index.html from an older run is the likely cause)"
    fi
done

# --- 3. catalog, site files, noscript list ----------------------------------
bash scripts/gen-site.sh --root "$root" "$out_abs/catalog.json" || die "gen-site.sh failed"
bash scripts/check-site.sh --root "$root" "$out_abs/catalog.json" || die "check-site.sh rejected the catalog"
for f in index.html styles.css app.js; do
    cp "docs/site/$f" "$out_abs/$f" || die "cannot copy docs/site/$f"
done

blk=$(mktemp)
trap 'rm -f "$blk"' EXIT
# shellcheck disable=SC2016  # the single-quoted string is a jq program
jqr '
    def esc: gsub("&"; "&amp;") | gsub("<"; "&lt;") | gsub(">"; "&gt;") | gsub("\""; "&quot;");
    . as $c
    | ($c.agents | map(.id)) as $order
    | "<noscript>", "<ul>",
      ($c.families[] | .plugins as $p | $order[] as $id | select($p[$id] != null)
        | $p[$id] as $e
        | ($c.agents[] | select(.id == $id) | .label) as $label
        | "<li><a href=\"\($e.readme | esc)\">\($e.name | esc)</a> (\($label | esc)): \($e.description | esc)</li>"),
      "</ul>", "</noscript>"
' "$out_abs/catalog.json" > "$blk" || die "could not build the noscript list"
[ "$(grep -cxF '<!-- NOSCRIPT -->' "$out_abs/index.html" || true)" -eq 1 ] \
    || die "docs/site/index.html must contain the line <!-- NOSCRIPT --> exactly once"
BLK=$blk awk '
    $0 == "<!-- NOSCRIPT -->" {
        while ((getline line < ENVIRON["BLK"]) > 0) print line
        close(ENVIRON["BLK"])
        next
    }
    { print }
' "$out_abs/index.html" > "$out_abs/index.html.new" || die "could not splice the noscript list"
mv "$out_abs/index.html.new" "$out_abs/index.html"

# --- 4. api/index.html (crate list computed before api/ exists) --------------
crates=$(crate_set "$out_abs")
mkdir -p "$out_abs/api"
{
    cat << 'EOF'
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>API documentation</title>
</head>
<body>
<h1>API documentation</h1>
<p><a href="../index.html">Back to the marketplace</a></p>
<ul>
EOF
    while IFS= read -r crate; do
        [ -n "$crate" ] || continue
        printf '<li><a href="../%s/index.html">%s</a></li>\n' "$crate" "$crate"
    done <<< "$crates"
    cat << 'EOF'
</ul>
</body>
</html>
EOF
} > "$out_abs/api/index.html"

# --- 5. user guide -----------------------------------------------------------
cp docs/USER_GUIDE.pdf "$out_abs/USER_GUIDE.pdf" || die "cannot copy docs/USER_GUIDE.pdf"

# --- 6. gate -----------------------------------------------------------------
gate_staged_tree "$out_abs"
