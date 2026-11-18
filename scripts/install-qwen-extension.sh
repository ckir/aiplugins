#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

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
#   curl -fsSL https://raw.githubusercontent.com/ckir/aiplugins/main/scripts/install-qwen-extension.sh | bash -s -- opencode-ctl-mcp-qwen
#   curl -fsSL https://raw.githubusercontent.com/ckir/aiplugins/main/scripts/install-qwen-extension.sh | bash -s -- aiplugins-devtools-qwen
#
# Or run directly:
#
#   bash scripts/install-qwen-extension.sh re-ghidra-mcp-qwen
#   bash scripts/install-qwen-extension.sh rtk-mcp-qwen
#   bash scripts/install-qwen-extension.sh opencode-ctl-mcp-qwen
#   bash scripts/install-qwen-extension.sh aiplugins-devtools-qwen
#
# Every extension is released under its own tag (<extension>-v<version>), so the
# repository's "latest" release is whichever plugin shipped last and almost never
# carries this extension's zip. The script therefore looks up the newest release
# tagged <extension>-v* and downloads the zip from that tag. Set GITHUB_TOKEN to
# raise the API rate limit (60 requests an hour unauthenticated).
#
set -euo pipefail

REPO="ckir/aiplugins"

if [ $# -lt 1 ]; then
    echo "Usage: $0 <extension-name>" >&2
    echo "" >&2
    echo "Available extensions:" >&2
    echo "  re-ghidra-mcp-qwen   Ghidra MCP for Qwen Code (19 RE tools)" >&2
    echo "  rtk-mcp-qwen         RTK command rewriter hook" >&2
    echo "  opencode-ctl-mcp-qwen  OpenCode worker sessions (20 MCP tools)" >&2
    echo "  aiplugins-devtools-qwen  Devtools skills (code review, spec panel review, ...)" >&2
    echo "  repo-tasks-mcp-qwen  Per-repository task list (4 MCP tools)" >&2
    exit 2
fi

ext="$1"
# The name becomes part of a URL and a directory path, so refuse anything else.
case "$ext" in
    "" | *[!a-z0-9-]*)
        echo "ERROR: '${ext}' is not a valid extension name (lowercase letters, digits and '-')." >&2
        exit 2
        ;;
esac
zip_name="${ext}-extension.zip"
ext_dir="${HOME}/.qwen/extensions/${ext}"

# Require either curl or wget. fetch <out-file> <url>; fetch_text <url> prints the body.
if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fSL --progress-bar -o "$1" "$2"; }
    fetch_text() {
        if [ -n "${GITHUB_TOKEN:-}" ]; then
            curl -fsSL -H "Authorization: Bearer ${GITHUB_TOKEN}" "$1"
        else
            curl -fsSL "$1"
        fi
    }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -q --show-progress -O "$1" "$2"; }
    fetch_text() {
        if [ -n "${GITHUB_TOKEN:-}" ]; then
            wget -qO- --header="Authorization: Bearer ${GITHUB_TOKEN}" "$1"
        else
            wget -qO- "$1"
        fi
    }
else
    echo "ERROR: curl or wget required." >&2
    exit 1
fi

# Newest release tagged <ext>-v*. The API lists releases newest first, so the first
# match on the first page that has one is the latest. The trailing "-v" keeps
# 'rtk-mcp-qwen' from matching a longer name that merely starts with it.
resolve_tag() {
    local page=1 body tag
    while [ "$page" -le 10 ]; do
        body=$(fetch_text "https://api.github.com/repos/${REPO}/releases?per_page=100&page=${page}") || return 1
        tag=$(printf '%s\n' "$body" | grep -o '"tag_name": *"[^"]*"' | sed 's/.*: *"\(.*\)"/\1/' \
            | grep -m1 "^${ext}-v[0-9]" || true)
        if [ -n "$tag" ]; then
            printf '%s\n' "$tag"
            return 0
        fi
        # An empty page (just "[]") means there are no older releases to look at.
        printf '%s' "$body" | grep -q '"tag_name"' || return 1
        page=$((page + 1))
    done
    return 1
}

echo "Looking up the latest release of ${ext}..."
tag=$(resolve_tag) || {
    echo "ERROR: no release tagged '${ext}-v*' found in ${REPO} (or the GitHub API could not be reached)." >&2
    exit 1
}
url="https://github.com/${REPO}/releases/download/${tag}/${zip_name}"

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
# (List first, grep after: under `pipefail`, `grep -q` quitting early can hand `unzip`
# a SIGPIPE and fail a zip that does hold the manifest. Seen once on a valid zip.)
zip_listing=$(unzip -l "$tmpzip" 2>/dev/null || true)
if ! grep -q 'qwen-extension.json' <<<"$zip_listing"; then
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
