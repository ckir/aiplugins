#!/usr/bin/env bash
# Usage: html_to_pdf.sh input.html output.pdf
# Renders a self-contained HTML file to PDF with headless Chrome/Chromium.
set -euo pipefail
in="${1:?input.html}"; out="${2:?output.pdf}"
for c in google-chrome chromium chromium-browser chrome; do
  if command -v "$c" >/dev/null 2>&1; then bin="$c"; break; fi
done
[ -n "${bin:-}" ] || { echo "No Chrome/Chromium found; install one or print the HTML manually." >&2; exit 1; }
in_abs="$(cd "$(dirname "$in")" && pwd)/$(basename "$in")"
out_abs="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
"$bin" --headless=new --no-sandbox --disable-gpu --no-pdf-header-footer \
  --print-to-pdf="$out_abs" "file://$in_abs" >/dev/null 2>&1
[ -s "$out_abs" ] && echo "Wrote $out_abs" || { echo "PDF generation failed" >&2; exit 1; }
