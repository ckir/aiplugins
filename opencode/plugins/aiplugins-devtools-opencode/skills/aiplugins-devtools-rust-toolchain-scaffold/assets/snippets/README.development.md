## Development

Prerequisites, installed on your machine and available on `PATH`:

- [Rust](https://rustup.rs) — the toolchain in `rust-toolchain.toml` is installed by `rustup toolchain install`
- [just](https://just.systems) — task runner
- [lefthook](https://lefthook.dev) — git hooks (not on crates.io: `brew install lefthook`, `scoop install lefthook`, `winget install evilmartians.lefthook`, or `go install github.com/evilmartians/lefthook/v2@latest`)
- [gitleaks](https://github.com/gitleaks/gitleaks) — secret scanning (not on crates.io: `brew install gitleaks`, `scoop install gitleaks`, or a release binary)
- `cargo-nextest`, `cargo-deny`, `typos-cli` — `cargo install --locked cargo-nextest cargo-deny typos-cli`

Then:

```sh
just setup        # check the tools and install the git hooks
just check        # everything CI runs: lint, tests, dependency check, spell check, secret scan
just check-fast   # the same without the slow toolchain tests
```

Run `just` to list every recipe.

The git hooks give fast local feedback but check your working tree, not the exact commit, and can be
skipped. CI runs `just check` on every pull request and is the gate that counts; protect `main` on
GitHub so the `check` job must pass before merging.
