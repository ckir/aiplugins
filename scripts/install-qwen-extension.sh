#!/usr/bin/env bash
#
# Install a Qwen Code extension from a GitHub release zip.
#
# Works around the silent-failure bug in `qwen extensions install` on Qwen Code
# v0.22.x (https://github.com/QwenLM/qwen-code/issues/10741) by downloading
# and extracting the bundle directly into ~/.qwen/extensions/<name>/.
#
# Usage (one-liner):
#
#   curl -fsSL https://raw.githubusercontent.com/ckir/aiplugins/main/scripts/install-qwen-extension.sh | bash -s -- re-ghidra-mcp-qwen
#   curl -fsSL https://raw.githubusercontent.com/ckir/aiplugins/main/scripts/install-qwen-extension.sh | bash -s -- rtk-mcp-qwen
#
# Or run directly:
#
#   bash scripts/install-qwen-extension.sh re-ghidra-mcp-qwen
#   bash scripts/install-qwen-extension.sh rtk-mcp-qwen
#
set -euo pipefail

REPO="ckir/aiplugins"

if [ $# -lt 1 ]; then
    echo "Usage: $0 <extension-name>" >&2
    echo "" >&2
    echo "Available extensions:" >&2
    echo "  re-ghidra-mcp-qwen   Ghidra MCP for Qwen Code (19 RE tools)" >&2
    echo "  rtk-mcp-qwen         RTK command rewriter hook" >&2
    exit 2
fi

ext="$1"
zip_name="${ext}-extension.zip"
url="https://github.com/${REPO}/releases/latest/download/${zip_name}"
ext_dir="${HOME}/.qwen/extensions/${ext}"

# Require either curl or wget
if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fSL --progress-bar -o "$1" "$2"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -q --show-progress -O "$1" "$2"; }
else
    echo "ERROR: curl or wget required." >&2
    exit 1
fi

# Require unzip
command -v unzip >/dev/null 2>&1 || {
    echo "ERROR: unzip is required and not on PATH." >&2
    exit 1
}

echo "Installing Qwen Code extension: ${ext}"
echo "  from: ${url}"
echo "  to:   ${ext_dir}"
echo ""

# Download to a temp file
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT
tmpzip="${tmpdir}/${zip_name}"

echo "Downloading..."
fetch "$tmpzip" "$url"

# Validate it looks like a Qwen extension zip
if ! unzip -l "$tmpzip" 2>/dev/null | grep -q 'qwen-extension.json'; then
    echo "ERROR: ${zip_name} does not contain qwen-extension.json — not a valid Qwen extension." >&2
    exit 1
fi

# Back up existing installation
if [ -d "$ext_dir" ]; then
    backup="${ext_dir}.bak.$(date +%s)"
    echo "Backing up existing installation to $(basename "$backup")"
    mv "$ext_dir" "$backup"
fi

# Extract
mkdir -p "$ext_dir"
unzip -q -o "$tmpzip" -d "$ext_dir"

# Make dispatchers and binaries executable (in case the zip lost exec bits)
if [ -d "$ext_dir/bin" ]; then
    find "$ext_dir/bin" -type f ! -name '*.exe' -exec chmod 755 {} +
fi

echo ""
echo "✓ Extension '${ext}' installed to ${ext_dir}"
echo ""

# Verify
if command -v qwen >/dev/null 2>&1; then
    echo "Verifying with 'qwen extensions list'..."
    echo ""
    qwen extensions list 2>/dev/null | grep -A5 "$ext" || echo "(extension should appear on next 'qwen extensions list')"
fi

echo ""
echo "Done. Restart any open Qwen Code session to pick up the extension."
