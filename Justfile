# The default task
default: help

# Show available tasks
help:
    @just --list

# Install required dev tools (using cargo-binstall for speed if available, otherwise fallback)
setup:
    @echo "Installing cargo-binstall..."
    cargo install cargo-binstall
    @echo "Installing dev tools..."
    cargo binstall cargo-nextest cargo-deny bacon typos-cli lefthook -y
    @echo "Installing git hooks..."
    lefthook install

# Format all Rust code
fmt:
    cargo fmt --all

# Lint all Rust code with Clippy
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Run all tests using nextest (faster than cargo test)
test:
    cargo nextest run --workspace

# Run tests in the background (using bacon)
watch-test:
    bacon nextest

# Check dependencies for vulnerabilities, licenses, and bans
deny:
    cargo deny check

# Check for spelling mistakes
spellcheck:
    typos

# Verify plugin configs point at binaries this workspace actually builds.
# Nothing else checks those strings: rename a [[bin]] and every test still
# passes while the installed plugin silently does nothing.
wiring:
    bash scripts/check-plugin-wiring.sh

# Verify the root marketplace manifest still matches the plugins it publishes.
# Every field in it is a copy of something owned elsewhere, and a stale copy
# installs happily while advertising — or fetching — the wrong thing.
marketplace:
    bash scripts/check-marketplace.sh

# Verify the Qwen marketplace manifest still matches the extensions it publishes.
qwen-marketplace:
    bash scripts/check-qwen-marketplace.sh

# Regenerate the committed footprint document for every published plugin, and
# maintain the thresholds alongside them.
# Requires the plugin binaries: `just build-rtk-mcp-cc build-re-ghidra-mcp-cc`.
footprint-regen:
    #!/usr/bin/env bash
    set -euo pipefail
    for plugin in $(jq -r '.plugins[].name' .claude-plugin/marketplace.json | tr -d '\r'); do
        cargo run -q -p plugin-footprint --bin plugin-footprint -- measure "claude-code/$plugin" \
            --out "docs/footprints/$plugin.json"
        cargo run -q -p plugin-footprint --bin plugin-footprint -- ratchet \
            --measured "docs/footprints/$plugin.json" \
            --budgets docs/footprints/budgets.json
        echo "regenerated docs/footprints/$plugin.json"
    done

# Verify each published plugin's committed footprint against a fresh measurement,
# then against the thresholds. Requires the plugin binaries; see `footprint-regen`.
footprint:
    #!/usr/bin/env bash
    set -euo pipefail
    # Freshness (spec §6). Regenerating and requiring no diff is what makes the
    # committed document a claim about the world rather than a file that agrees
    # with itself. `check-qwen-marketplace.sh` keeps its generated manifest
    # honest exactly this way, on the stated reasoning that nothing fails when a
    # copy goes stale — it just advertises the wrong thing.
    # `git status --porcelain`, NOT `git diff`. MEASURED: `git diff --quiet --
    # docs/footprints/` exits 0 for an UNTRACKED file, because git diff only ever
    # compares things git is tracking. A new plugin's document, regenerated but
    # never `git add`ed, was therefore invisible here AND in CI — where
    # footprint-regen recreates it identically, the diff stays quiet, and the
    # gate reads it off disk and passes. The pull request merges with no
    # committed document at all, so the NEXT change has no baseline to compare
    # against, which is the one thing this file exists to provide.
    just footprint-regen
    if [ -n "$(git status --porcelain -- docs/footprints/)" ]; then
        echo "ERROR: the committed footprint documents are stale or incomplete." >&2
        echo "A fresh measurement disagrees with what is committed:" >&2
        git status --porcelain -- docs/footprints/ >&2
        git --no-pager diff --stat -- docs/footprints/ >&2
        echo "Run 'just footprint-regen' and 'git add docs/footprints/', then commit." >&2
        exit 1
    fi
    # NOT `HEAD`. Against HEAD the baseline is the developer's own last commit,
    # so once they have run `footprint-regen` and committed it to satisfy the
    # freshness check above, the measured delta is zero BY CONSTRUCTION and the
    # delta cap can never fire locally. `just check` would report green on
    # exactly the change the cap exists to catch, and CI would be the first to
    # say otherwise.
    #
    # `origin/main` FIRST, then a local `main`. Two different developers break on
    # the two different orders: one who cloned and ran `git switch -c feat` has
    # no local `main` at all, and one who cloned months ago and has worked on
    # branches since has a local `main` that resolves perfectly and points
    # somewhere long superseded. Only `origin/main` moves on every fetch, and it
    # is what CI's `origin/<base_ref>` actually means.
    #
    # HEAD remains the last resort, for a checkout with no remote. The cap is
    # vacuous there, which is why CI runs the same check against the base branch
    # with `fetch-depth: 0` — this recipe is a convenience, never the authority.
    base=origin/main
    git rev-parse --verify --quiet "$base" >/dev/null 2>&1 || base=main
    git rev-parse --verify --quiet "$base" >/dev/null 2>&1 || base=HEAD
    echo "footprint: comparing against $base"
    cargo run -q -p plugin-footprint --bin footprint-gate -- "$base"

# Drive every branch of the dispatcher shipped as bin/<name> in a plugin bundle.
# Any one machine exercises exactly one branch of it, so the platform is faked;
# the Git Bash branch shipped broken in 0.6.0 for want of this.
dispatch:
    bash scripts/check-bundle-dispatch.sh

# Assemble a host-only bundle for each Claude Code plugin and start its entry
# points both ways Claude Code does — spawned directly, and through a shell.
# Those two routes resolve bin/<name> differently on Windows, and only running
# the assembled thing covers both.
smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    for plugin in $(jq -r '.plugins[].name' .claude-plugin/marketplace.json | tr -d '\r'); do
        bash scripts/smoke-bundle.sh "$plugin"
    done

# Run all pre-flight checks (what CI and lefthook would run)
check: fmt lint test deny spellcheck wiring marketplace dispatch smoke footprint

# Build the example Claude Code plugin's binaries into its bin/ directory.
# Windows developers run this locally; CI produces the other platforms.
build-claude-example:
    cargo build -p claude-example --release --bin claude-example-mcp --bin claude-example-hook
    mkdir -p claude-code/example/bin
    for b in claude-example-mcp claude-example-hook; do \
        if [ -f "target/release/$b.exe" ]; then \
            cp "target/release/$b.exe" claude-code/example/bin/; \
        else \
            cp "target/release/$b" claude-code/example/bin/; \
        fi; \
    done
    @echo "Plugin binaries staged in claude-code/example/bin/"

# Build the rtk-mcp-cc Claude Code plugin's binaries into its bin/ directory.
# Windows developers run this locally; CI produces the other platforms.
build-rtk-mcp-cc:
    cargo build -p rtk-mcp-cc --release --bin rtk-cc-hook --bin rtk-cc-mcp
    mkdir -p claude-code/rtk-mcp-cc/bin
    for b in rtk-cc-hook rtk-cc-mcp; do \
        if [ -f "target/release/$b.exe" ]; then \
            cp "target/release/$b.exe" claude-code/rtk-mcp-cc/bin/; \
        else \
            cp "target/release/$b" claude-code/rtk-mcp-cc/bin/; \
        fi; \
    done
    @echo "Plugin binaries staged in claude-code/rtk-mcp-cc/bin/"

# Build the re-ghidra-mcp-cc Claude Code plugin's binaries into its bin/ directory.
# Windows developers run this locally; CI produces the other platforms.
build-re-ghidra-mcp-cc:
    cargo build -p re-ghidra-mcp-cc --release --bin re-ghidra-cc-mcp --bin re-ghidra-cc-hook
    mkdir -p claude-code/re-ghidra-mcp-cc/bin
    for b in re-ghidra-cc-mcp re-ghidra-cc-hook; do \
        if [ -f "target/release/$b.exe" ]; then \
            cp "target/release/$b.exe" claude-code/re-ghidra-mcp-cc/bin/; \
        else \
            cp "target/release/$b" claude-code/re-ghidra-mcp-cc/bin/; \
        fi; \
    done
    @echo "Plugin binaries staged in claude-code/re-ghidra-mcp-cc/bin/"

# Assemble the installable plugin zips for a published release, the same way
# .github/workflows/plugin-bundles.yml does — for testing a change to the
# bundling before tagging, or for rebuilding an asset by hand.
#
# Unix only: the bundles carry exec bits that a Windows zip cannot record, and
# MSYS resolves `bin/foo` to `bin/foo.exe` and would overwrite the binary with
# the dispatcher. On a Windows machine run this from WSL.
bundle-plugins tag:
    #!/usr/bin/env bash
    set -euo pipefail
    names=$(jq -r '.plugins[].name' .claude-plugin/marketplace.json | tr -d '\r')
    rm -rf target/plugin-assets target/plugin-bundles
    mkdir -p target/plugin-assets
    for name in $names; do
        gh release download "{{ tag }}" -D target/plugin-assets \
            -p "$name-*.tar.xz" -p "$name-*.zip" --clobber
    done
    for name in $names; do
        bash scripts/bundle-plugin.sh "$name" target/plugin-assets target/plugin-bundles
    done

# Assemble the installable Qwen extension zips for a published release, the same
# way .github/workflows/qwen-extension-bundles.yml does.
#
# Unix only: the bundles carry exec bits that a Windows zip cannot record, and
# MSYS resolves `bin/foo` to `bin/foo.exe` and would overwrite the binary with
# the dispatcher. On a Windows machine run this from WSL.
bundle-qwen tag:
    #!/usr/bin/env bash
    set -euo pipefail
    names=$(jq -r '.plugins[].name' .qwen-plugin/marketplace.json | tr -d '\r')
    rm -rf target/qwen-assets target/qwen-bundles
    mkdir -p target/qwen-assets
    for name in $names; do
        gh release download "{{ tag }}" -D target/qwen-assets \
            -p "$name-*.tar.xz" -p "$name-*.zip" --clobber
    done
    for name in $names; do
        bash scripts/bundle-qwen-extension.sh "$name" target/qwen-assets target/qwen-bundles
    done

# Regenerate EVERY plugin's committed copy of the ghidra-re-driver skill from
# the binary that embeds it.
#
# The canonical skill lives at shared/ghidra-mcp/skill/SKILL.md and is compiled
# into every agent plugin's binary. Editing a plugin's copy directly is a
# mistake the emit test catches; this is how you propagate a canonical edit.
#
# Every copy, not just the Claude Code one. This recipe used to regenerate that
# single front, so a canonical edit left the qwen copy stale — green locally
# under `-p re-ghidra-mcp-cc`, and caught only by the workspace run in CI, as
# re-ghidra-mcp-qwen::skill_emit.
emit-ghidra-skill: build-re-ghidra-mcp-cc
    cargo build -p re-ghidra-mcp-qwen --release --bin re-ghidra-qwen-mcp
    ./claude-code/re-ghidra-mcp-cc/bin/re-ghidra-cc-mcp emit-skill \
        > claude-code/re-ghidra-mcp-cc/skills/ghidra-re-driver/SKILL.md
    ./target/release/re-ghidra-qwen-mcp emit-skill \
        > qwen/re-ghidra-mcp-qwen/skills/ghidra-re-driver/SKILL.md
    @echo "Regenerated the committed ghidra-re-driver skill copies (claude-code, qwen)"

# Run the live Ghidra suite. Needs a real Ghidra 12.1.2 + JDK 21 and an analyzed
# fixture project; see shared/ghidra-mcp/tests/fixtures/README.md to build one.
#
# These ~60 tests are gated at RUNTIME on GHIDRA_MCP_E2E, not with #[ignore]:
# without the variable they early-return and pass, which is what keeps the
# 3-OS CI matrix green on runners that have no Ghidra.
#
# -j1 is not a performance choice. One Ghidra project supports ONE live worker;
# parallel test binaries collide on Ghidra's project.lock.
test-live-ghidra:
    GHIDRA_MCP_E2E=1 cargo nextest run -p ghidra-mcp -j1

# Remove ORPHANED executables that cargo leaves behind, without discarding the
# whole build cache.
#
# Cargo never deletes an executable whose [[bin]] target was renamed or removed:
# it only ever writes. The orphan keeps sitting in target/{debug,release}, and
# anything that resolves a binary BY NAME — CARGO_BIN_EXE_*, a staged plugin
# bin/, a PATH lookup — will happily keep finding the old one. That failure is
# silent and looks like a logic bug, not a build-system one.
#
# Two real instances of this, both from renaming a bin target:
#   * two workspace packages each declared a fixture named `mock-rtk`, so they
#     shared one output path and the last build won; the losing package's tests
#     then drove the WRONG binary.
#   * after renaming that fixture, the abandoned `mock-rtk` executable stayed on
#     disk, and the package that still legitimately owns the name saw its own
#     target as fresh and never re-linked over it.
#
# `cargo clean -p <pkg>` cannot fix the second case in general: an orphan from a
# renamed target is no longer associated with any package. So enumerate the bin
# targets that currently EXIST and delete every top-level executable that is not
# one of them.
clean-stale:
    #!/usr/bin/env bash
    set -euo pipefail
    known=$(cargo metadata --no-deps --format-version 1 \
        | jq -r '.packages[].targets[] | select(.kind[] == "bin") | .name' | sort -u)
    removed=0
    for profile in debug release; do
        dir="target/$profile"
        [ -d "$dir" ] || continue
        for path in "$dir"/*; do
            [ -f "$path" ] || continue
            name=$(basename "$path")
            # Only consider built executables; skip .d/.pdb/.rlib and friends.
            case "$name" in
                *.exe) name="${name%.exe}" ;;
                *.*)   continue ;;
            esac
            if ! printf '%s\n' "$known" | grep -qxF "$name"; then
                echo "  stale: $path"
                rm -f "$path"
                removed=$((removed + 1))
            fi
        done
    done
    # Staged plugin binaries are copies, so they go stale the same way.
    for staged in claude-code/*/bin/*; do
        [ -f "$staged" ] || continue
        name=$(basename "$staged"); name="${name%.exe}"
        if ! printf '%s\n' "$known" | grep -qxF "$name"; then
            echo "  stale: $staged"
            rm -f "$staged"
            removed=$((removed + 1))
        fi
    done
    echo "Removed $removed stale artifact(s)."

# Remove every build artifact: the cargo cache and the staged plugin binaries.
# Reach for `clean-stale` first — it fixes the usual problem without costing a
# full rebuild.
clean:
    cargo clean
    rm -rf claude-code/example/bin claude-code/rtk-mcp-cc/bin claude-code/re-ghidra-mcp-cc/bin
    @echo "Removed target/ and staged plugin binaries."
