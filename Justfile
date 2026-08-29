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

# Run all pre-flight checks (what CI and lefthook would run)
check: fmt lint test deny spellcheck wiring

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
    rm -rf claude-code/example/bin claude-code/rtk-mcp-cc/bin
    @echo "Removed target/ and staged plugin binaries."
