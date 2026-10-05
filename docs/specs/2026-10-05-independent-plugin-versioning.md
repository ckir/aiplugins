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
Each shippable agent plugin becomes a distinct "package" in the Release Please configuration.
- Packages: `claude-code/rtk-mcp-cc`, `claude-code/re-ghidra-mcp-cc`, `qwen/rtk-mcp-qwen`, `qwen/re-ghidra-mcp-qwen`, `opencode/rtk-mcp-opencode`, `opencode/re-ghidra-mcp-opencode`, `antigravity/rtk-mcp-agy`, `antigravity/re-ghidra-mcp-agy`. Example plugins (`claude-code/example`, `qwen/example`, `antigravity/example`) are excluded: reference code, never released.
- `shared/*` crates and `tools/*` are NOT packages. They are libraries and maintainer tooling versioned with the workspace; their changes release through the plugins that embed them, never on their own sequence.
- Release tags follow the format `<package-name>-v<version>`, where `<package-name>` is the plugin directory basename (e.g., `rtk-mcp-cc-v1.2.0`, never the `host/plugin` path). The legacy `v0.7.x` scheme is retired; old tags stay untouched.

### Version Sources (one owner per number)
Today every crate inherits the single `[workspace.package] version` (`version.workspace = true` in all 14 `Cargo.toml` files), so no per-package Cargo version exists for release-please to own — two packages' release PRs would contend on the same line, and any binary built after a sibling's release would report the sibling's version. Therefore, before the first independent release:
- Each of the six Rust-backed plugins gets an explicit `[package] version` in its own crate(s), dropping `version.workspace`: `claude-code/rtk-mcp-cc`, `claude-code/re-ghidra-mcp-cc`, `qwen/rtk-mcp-qwen`, `qwen/re-ghidra-mcp-qwen`, `antigravity/rtk-mcp-agy`, `antigravity/re-ghidra-mcp-agy`.
- `shared/*`, `tools/*`, and examples keep `version.workspace = true`. The workspace version field becomes internal build identity only and MUST NOT be read as any plugin's release version.
- The two opencode plugins have no crates: their version lives solely in `opencode/<plugin>/package.json`, and they ship binaries built from the Claude crates at the same commit (as the `build-*-opencode` recipes already do). Binary identity for opencode bundles is the building crate's version, recorded in the bundle; the plugin release version is the `package.json` version.
- A per-package version check replaces `check-manifest-versions.sh`: each shippable crate MUST carry an explicit version (workspace inheritance there fails the check), and each host manifest must equal its package's version. The release-please `rust` strategy owns the six crate versions (+ `Cargo.lock`); the two opencode packages use the `simple` strategy with `extra-files` on `package.json`.

### The Release-PR Flow (how tags happen)
Nobody hand-cuts tags. A single `release-please` workflow (`.github/workflows/release-please.yml`, trigger: push to `main` plus `workflow_dispatch`, one invocation managing all eight packages from `release-please-config.json`, token permissions `contents: write` and `pull-requests: write`, concurrency group `release-please`) opens a version-bump PR per package from merged conventional commits; a maintainer merges it, and the tag plus GitHub release follow automatically. Tag timing becomes derived, never chosen, which is what retires the manual-tag failure mode rather than merely discouraging it. Only the release-please identity may create `*-v*` tags (see Tag Protection below); manual tag pushes are forbidden and documented as such in onboarding docs.

### Bootstrap
Every package's first independent version is `0.7.2`, continuing the last lockstep release; sequences diverge from there. Seed by script-tagging each package `<name>-v0.7.2` once, so release-please has a base to compute from. The script defaults to dry-run (prints tag → commit pairs); with `--execute` it creates annotated tags and pushes them strictly sequentially, verifying each with `git ls-remote` before the next. The base commit for all eight tags is the single commit at which all eight packages' version files already read `0.7.2` (verified by the per-package version check before tagging); tagging HEAD with unreleased version-relevant changes is an error the script refuses.

### Heterogeneous Manifests Sync
A single plugin may have its version declared across multiple files (e.g., `plugin.json`, `qwen-extension.json`, `package.json`, plus its crate `Cargo.toml` where one exists).
- `release-please-config.json` utilizes `extra-files` for the host manifests (and the generic-strategy opencode packages) so every version declaration within a package boundary is bumped atomically in the release PR. Crate versions are owned by the `rust` strategy, not by `extra-files`; the two mechanisms must never target the same file.

## Agent Registry (add/remove with best effort)
`agents.json` at the repo root is the single list of supported agent fronts. Each entry names the plugin directory, its manifest paths, its bundle script, and its wiring check. CI matrices, wiring checks, and footprint measurement iterate the registry — no per-agent special-casing anywhere. Adding an agent means copying the template front plus one registry entry; removing one means deleting the directory plus deregistering. "Best effort" is exactly this contract: unregistered paths are ignored by every gate, registered ones must pass all of them. Resolution is fail-closed: every path a registry entry references must exist, or the consuming check exits non-zero — a typo'd entry fails loudly, never silently skips a plugin.

### Registry Is the Only List
The release-please package list is derived from `agents.json`, not maintained separately: a `scripts/gen-release-please-packages.sh` step rewrites only the top-level `packages` object of `release-please-config.json` (formatting of the rest preserved byte-for-byte), and CI fails when the committed config drifts from the registry. Two lists of "what exists" would recreate the copy-drift this design eliminates.

## Single-Source Manifests & Check Parity
Marketplace and registry files are GENERATED from each plugin's own manifest via a shared core library (`scripts/lib/registry.sh`, extended from the agent-registry work — one core, not per-host codegen), invoked as `just gen-marketplaces`, and CI verifies the committed copies match — a stale copy fails the same way a stale footprint does. Because every host check calls the same core, a rule added for one host cannot be missing from another, the way the `example` exclusion was present for Claude Code but absent for Qwen.

## Closing the Opencode Gap

Opencode plugins will be elevated to first-class citizens alongside Claude and Qwen plugins.
- **Artifacts:** `opencode` plugins will be packaged into versioned `-opencode.zip` bundles containing their TS code, `package.json`, and skills, assembled by a new `scripts/bundle-opencode-plugin.sh` (there is no opencode bundler today; like the Claude bundler it must preserve Unix exec bits and therefore runs on Linux).
- **Marketplace:** A `.opencode-plugin/marketplace.json` file (dot-dir, matching `.claude-plugin/` and `.qwen-plugin/` convention) will track the available opencode plugins.
- **Installation:** `INSTALL.md` instructions will be updated to extract the versioned `.zip` bundle from the corresponding GitHub Release, rather than cloning files from the `main` branch.

## Antigravity Distribution (same gap, same fix)
Antigravity has the same hole opencode had: distributable archives exist per target but there is no install-zip pipeline and no registry. Antigravity plugins ship `-agy-plugin.zip` bundles assembled by a new `scripts/bundle-agy-plugin.sh` from the plugin directory plus its per-target binaries, tracked in `.agy-plugin/marketplace.json`. Until both new bundlers land, their packages stay out of the release-please package table: no releasable artifact, no release.

## Tooling Adaptations

### The Footprint Tool
The `tools/plugin-footprint` utility currently measures token costs and stamps them with the global `pluginVersion`.
- **Refactoring:** The footprint tool reads the `pluginVersion` from the specific package's version source (its crate `Cargo.toml` where one exists, else its host manifest) rather than assuming a global workspace version. All four footprint documents carry the stamp, including the two opencode ones that lack the field today.
- **Baselines stay merge-base-derived per package document.** Per-package releases shrink the blast radius (a stale base blocks only that package), but a stale base can still deadlock a correction behind the delta cap, as measured in October 2026. Two guards: (1) a scheduled main-branch freshness job (weekly schedule plus `workflow_dispatch`, single-flight concurrency group, early exit when no measured input path changed since the last green run, updating one `chore/footprint-freshness` branch instead of opening duplicates) that opens a correction PR the moment `main` drifts, so the base never goes stale silently; (2) a recorded override procedure for the correction itself — an admin merge whose merge message contains `footprint-override: <reason>`, used once per incident, never as routine, with each use visible in the log for later audit.

## CI/CD Workflows

cargo-dist keeps building the per-target binaries, scoped by package tag: the release workflow triggers on the exact glob `*-v[0-9]*`. Cutover ordering is load-bearing: the legacy `**[0-9]+.[0-9]+.[0-9]+*` glob — which the substring `1.2.0` inside `rtk-mcp-cc-v1.2.0` satisfies, so it would fire on every new-style tag — is retired in the same change *before* the first `<name>-v0.7.2` bootstrap tag is pushed. Ordering per package release: release-please creates the tag and GitHub release; dist builds that package's binaries and uploads them to it; the host bundle workflows assemble install zips from those artifacts. The `cargo-dist-version` pin and target matrix stay shared across packages. Release workflow runs carry the triggering tag in their run name so a failed per-package release is findable among siblings.

### Tag Protection
GitHub tag protection rules restrict `*-v*` creation to the release-please automation identity. Humans never push tags (`git push --tags` after local experimentation would otherwise mint real releases); the rule makes that attempt fail loudly instead.

## Migration (clean break, documented)

- `v0.7.x` tags freeze as the final lockstep line. Their `releases/latest/download/*` URLs do NOT keep resolving once per-package releases exist (`latest` is repo-wide; the first per-package release displaces them), so as part of migration every old INSTALL.md URL is rewritten to the tag-pinned form `releases/download/v0.7.2/<file>`.
- New installs use tag-pinned per-package URLs only: `releases/download/<package>-v<version>/<file>`. The `latest` shorthand MUST NOT appear in any install docs going forward — a floating pointer cannot serve versioned installs.
- Per-package changelogs live as `CHANGELOG.md` inside each package directory (release-please default); this monolithic `CHANGELOG.md` stays for archaeology and gains no new entries.
- `check-manifest-versions.sh` is retired in its global form: version agreement is enforced per package boundary (release-please `extra-files` plus the per-package check from Version Sources), not against one workspace number.

## Checkout Health

`.gitattributes` `eol=lf` pins stay as the mechanism. Add a `just doctor` recipe in the `Justfile` that reports working-copy files checked out as CRLF against an `eol=lf` pin and offers the `rm` + `checkout` renormalize, and link it from onboarding docs — so stale checkouts self-heal instead of failing byte-identity gates.
