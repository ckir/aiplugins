# Independent Plugin Versioning & Release Please

## Overview

The `aiplugins` repository is transitioning from a monolithic release model (where every agent's plugin shares a global workspace version, e.g., `0.7.2`) to an independent, per-plugin versioning model. 

This change addresses three core issues:
1. **Manual Tag Timing:** Tags are currently cut manually.
2. **Coupled Cadence:** An update to a Qwen extension shouldn't bump the version of the Claude Code plugin.
3. **The Opencode Gap:** `opencode` artifacts are missing from releases. They are currently distributed via fragile `curl` scripts pointing to `live-main` files.

By adopting Google's `release-please` action in a monorepo configuration, tags will be generated purely from merged conventional commits, and each plugin will maintain its own version sequence, changelog, and distribution bundles.

## Architecture & Configuration

### Release-Please Monorepo Setup
Each agent plugin becomes a distinct "package" in the Release Please configuration.
- Packages will include: `claude-code/re-ghidra-mcp-cc`, `opencode/re-ghidra-mcp-opencode`, `qwen/re-ghidra-mcp-qwen`, `antigravity/re-ghidra-mcp-agy`, and the shared Rust core (`shared/ghidra-mcp`).
- Release tags will follow the format `<package-name>-v<version>` (e.g., `rtk-mcp-cc-v1.2.0`).

### Heterogeneous Manifests Sync
A single plugin may have its version declared across multiple files (e.g., `Cargo.toml`, `plugin.json`, `qwen-extension.json`, `package.json`).
- `release-please-config.json` will utilize `extra-files` to ensure all manifests within a package boundary are atomically version-bumped in the release PR. This completely eliminates manual version sync drift.

## Closing the Opencode Gap

Opencode plugins will be elevated to first-class citizens alongside Claude and Qwen plugins.
- **Artifacts:** `opencode` plugins will be packaged into versioned `-opencode.zip` bundles containing their TS code, `package.json`, and skills.
- **Marketplace:** An `opencode-plugin/marketplace.json` (or equivalent registry file) will be established to track the available opencode plugins.
- **Installation:** `INSTALL.md` instructions will be updated to extract the versioned `.zip` bundle from the corresponding GitHub Release, rather than cloning files from the `main` branch.

## Tooling Adaptations

### The Footprint Tool
The `tools/plugin-footprint` utility currently measures token costs and stamps them with the global `pluginVersion`.
- **Refactoring:** The footprint tool must be updated to read the `pluginVersion` from the specific package's manifest (e.g., `plugin.json` or `package.json`) rather than assuming a global workspace version.

## CI/CD Workflows

The current monolithic CI workflow relies heavily on `cargo-dist` triggering off a single tag.
- Workflows must be adapted to respond to the `release-please` tags (e.g., `*v[0-9]*`).
- When a package-specific tag is pushed, CI will isolate the build, bundle the specific plugin (whether it's `plugin-bundles.sh` for Claude or a new bundler for Opencode), and attach the assets to the corresponding GitHub Release.
