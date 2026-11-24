## Toolchain

Use [just](https://just.systems) for project tasks, not shell scripts or Makefiles.

- Define recipes in the root `justfile` and run them with `just <recipe>`. Run `just` to list them.
- Recipes call Cargo (`cargo fmt`, `cargo clippy`, `cargo nextest`, `cargo deny`), not wrappers around it.

## Git hooks

Use [lefthook](https://lefthook.dev) for git hooks, not husky, pre-commit or hand-written `.git/hooks` scripts.

- lefthook is a machine-level tool, not a Cargo dependency, and it is not on crates.io: install it with your system's package manager (see the README). `lefthook.yml` sets `min_version` and `assert_lefthook_installed: true`, so a missing or too-old lefthook makes a commit fail instead of skipping the hooks. `just setup` checks the version, then installs the hooks.
- Configure hooks in the root `lefthook.yml`. Hooks call `just` recipes, so checks run the same way locally and in hooks. Three hooks are installed: pre-commit, commit-msg and pre-push.
- The commit-msg hook gets the message file's path through the environment variable `HOOK_ARG_1`, exported by `.lefthook/rc.sh`, not as a command argument. lefthook pastes hook arguments into shell commands unescaped, so a path with shell syntax in it would be run: do not use `{0}`, `{1}` and so on in `lefthook.yml`. `.lefthook/rc.sh` also unsets `LEFTHOOK_BIN` and `LEFTHOOK_CONFIG`, which would otherwise let the caller's environment swap the hooks for a no-op.
- A global `core.hooksPath` in your git config makes `lefthook install` refuse. Unset it for this repository.

## Format, lint, dependencies, spelling

- `just fmt` applies rustfmt, `just lint` checks formatting and runs clippy with warnings as errors, `just deny` checks dependency licences, bans and sources, `just spellcheck` runs typos. Settings are in `rust-toolchain.toml`, `deny.toml` and `typos.toml`.
- `toolchain-tests/` is a separate crate outside the workspace, so `cargo ... --workspace` does not reach it. The recipes name its manifest explicitly; a new recipe that runs cargo over the workspace should do the same.

## Secret scanning

Use [gitleaks](https://github.com/gitleaks/gitleaks) to keep secrets out of this repo.

- `just secrets` scans the history of the current commit (patches, merge commits and commit messages) plus staged and unstaged changes to tracked files; the pre-commit hook scans staged changes. Other branches and stashes are not scanned.
- gitleaks is a standalone binary, not on crates.io. Install it before committing: the hook fails if it is missing.

## Tests

- `just test` runs the workspace tests and `toolchain-tests/tests/gate.rs`: fast tests that read the CI workflow, CODEOWNERS and the config files. They run here, and so in `check-fast`, so that a change to the workflow cannot skip them.
- `just test-toolchain` runs `toolchain-tests/tests/toolchain.rs`, which tests the tooling itself: each test builds a throwaway git repository with a copy of the justfile, hook and config files plus a small fixture crate, and proves a gate blocks what it should.
- When you change `justfile`, `lefthook.yml`, `.lefthook/`, `deny.toml`, `typos.toml` or `rust-toolchain.toml`, run `just test-toolchain`. If you add or loosen a gate, add or update a test, and check the test fails when the gate is removed.
- The suite copies the files listed in `TOOLCHAIN_FILES` (`toolchain-tests/tests/common/mod.rs`). A new config file that a tool reads automatically (for example `.gitleaks.toml`) is not covered until it is added there and to the `toolchain-tests` glob in `lefthook.yml`.
- Test secrets and the deliberate misspelling are generated at run time. Never commit a token-shaped literal or a deliberate misspelling, even a fake one: the secret scan and the spell check will reject it.

## Checks and CI

- `just setup` checks the tools and installs the git hooks on a fresh clone.
- `just check` is the full gate: `check-fast` (lint, tests, `deny`, spell check, secret scan) plus `just test-toolchain`. Run it before declaring work done. `just check-fast` is the quick version for the inner loop.
- pre-commit runs the format check and clippy when a `.rs` file is staged, the spell check, and the staged secret scan. commit-msg scans the commit message for secrets before the commit is created. pre-push always runs `just check-fast`, and also runs `just test-toolchain` when the push touches a file that suite tests.
- GitHub Actions (`.github/workflows/ci.yml`) runs `just check` on pushes to `main` and on pull requests. Add a new check as a `just` recipe under `check-fast` so the hook and CI both pick it up.
- The workflow pins each action to a full commit SHA, with the version in a trailing comment, and each tool to an exact version in a `TOOL_*` env entry. Do not name an env entry `JUST_...`: the gate tests reject it, because `just` reads those. `.github/CODEOWNERS` routes changes to the workflow, `justfile`, `lefthook.yml`, `.lefthook/`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `deny.toml`, `typos.toml`, `.gitattributes` and `toolchain-tests/` to the owner; it only has effect with branch protection requiring Code Owner review.
- CI is the authoritative gate. The hooks are fast local feedback: they check the working tree, not the exact commit, so uncommitted or unstaged files can hide a problem in what is committed or pushed, and `--no-verify` skips them. Do not treat a passing hook as proof a commit is clean; commit or stash first and run `just check` when it matters.
- `gitleaks:allow` comments are honoured, as the way to mark a false positive. Use them only for values that are not secrets.
