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

# Run all pre-flight checks (what CI and lefthook would run)
check: fmt lint test deny spellcheck
