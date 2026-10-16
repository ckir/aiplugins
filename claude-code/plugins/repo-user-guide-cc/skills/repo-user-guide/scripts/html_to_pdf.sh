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
# Remove a stale PDF first so an old file cannot mask a failed render.
rm -f "$out_abs"
log="$(mktemp)"; trap 'rm -f "$log"' EXIT
"$bin" --headless=new --no-sandbox --disable-gpu --no-pdf-header-footer \
  --print-to-pdf="$out_abs" "file://$in_abs" >"$log" 2>&1 || true
if [ -s "$out_abs" ]; then
  echo "Wrote $out_abs"
  # A non-empty file is not proof of a good render: Chrome still writes a PDF when parts of the page fail.
  grep -E "ERR_|Page load failed" "$log" >&2 && echo "WARNING: Chrome reported load errors above; check the pages." >&2
  if command -v pdfinfo >/dev/null 2>&1; then
    echo "Pages: $(pdfinfo "$out_abs" 2>/dev/null | awk '/^Pages:/ {print $2}')"
  fi
  if command -v pdftotext >/dev/null 2>&1 && [ "$(pdftotext "$out_abs" - 2>/dev/null | wc -w)" -lt 20 ]; then
    echo "WARNING: the PDF has almost no text; the page content probably failed to render." >&2
  fi
else
  echo "PDF generation failed. Last lines of $bin output:" >&2; tail -n 15 "$log" >&2
  grep -q "Socket path too long" "$log" && echo "HINT: Chrome puts a socket under \$TMPDIR (limit about 100 characters). Run with a short TMPDIR, e.g. TMPDIR=/tmp." >&2
  exit 1
fi
