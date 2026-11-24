# Why the scaffold looks the way it does

Each item is a hole that was found by review, by measurement or by a surviving mutant, and what
closes it. Read this before changing `justfile`, `lefthook.yml`, `.lefthook/` or the tests, so a
tidy-up does not reopen one.

## lefthook

- **Hook arguments are pasted into `sh -c` unescaped.** `{1}` (the commit message path) in a `run:`
  command lets a path containing shell syntax execute; quoting it does not help. So `lefthook.yml`
  sets `rc: .lefthook/rc.sh`, the generated hook script sources that file where `$1` is still intact,
  and it exports `HOOK_ARG_1`. Never use `{0}`, `{1}` and so on in `lefthook.yml`. In a linked
  worktree git passes an absolute path that contains the worktree's name, which is why a test names
  a worktree `wt$(touch pwned)`.
- **lefthook is not pinned by path.** A Rust repository has no `node_modules` to pin into and
  lefthook is not on crates.io, so the generated hook looks for `lefthook` on `PATH`. The settings
  that keep that from failing open are `assert_lefthook_installed: true` (a missing lefthook exits 1;
  without it the hook exits 0 and skips silently) and `min_version` (enforced when a hook runs).
- **`lefthook install` does not enforce `min_version`.** Only running a hook does. So `just setup`
  checks the installed version against the floor in `lefthook.yml` before it installs anything. The
  floor is written once, in `lefthook.yml`; the recipe reads it and a gate test fails if the recipe
  writes a number of its own.
- **`LEFTHOOK_BIN`, `LEFTHOOK_CONFIG` and `LEFTHOOK_EXCLUDE` override everything.** The first picks the
  binary ahead of `PATH`, the second swaps the configuration for any file, including one that does not
  exist, and the third skips the named hook jobs; any of them set in the caller's environment turns
  the hooks into a no-op that exits 0. `rc.sh` runs `unset LEFTHOOK_BIN LEFTHOOK_CONFIG
  LEFTHOOK_EXCLUDE`, and the generated hook sources it before it reads any of them.
  Deleting `rc.sh` locally removes that protection (an accepted boundary: CI is the gate).
- **A missing-runner test passes for the wrong reason unless it installs from a copy.** The generated
  hook remembers the path lefthook was installed from and tries it after `PATH`. The test installs
  from a copy, deletes the copy, and runs with a `PATH` holding only git, uname, tr and sed. Two
  controls sit beside it: with the copy present the hook does find lefthook, and without
  `assert_lefthook_installed` the missing runner passes silently.
- **`rc` is sourced only by the installed hook scripts**, so `lefthook run commit-msg` by hand fails.
  `just secrets-message` refuses to run when `rc.sh` is missing or `HOOK_ARG_1` is unset, rather than
  scanning nothing and reporting success.
- **Pre-push `run:` jobs are skipped when no pushed file matches their glob**; `script:` jobs are not.
  The always-on fast gate is therefore a script job (`.lefthook/pre-push/check.sh`), and only the
  slow toolchain suite is a glob-conditioned `run:` job.
- **A tracked `lefthook-local.yml` overrides `lefthook.yml` for everyone**; a gate test forbids
  tracking one.
- **A global `core.hooksPath` makes `lefthook install` refuse.** That is lefthook's own rule. The
  tests set `GIT_CONFIG_GLOBAL` to an empty file and `GIT_CONFIG_NOSYSTEM=1`, because stripping
  `GIT_*` variables does not isolate git's config files.

## The commit message check (`just secrets-message`)

The recipe scans only a regular file, not a symbolic link, whose resolved directory is inside the git
common directory. Each clause has a test or a comment:

- `unset CDPATH`: with `CDPATH` exported, `cd` prints the directory it chose, which corrupts the
  captured path for a relative `.git`.
- `pwd -P`: without it a linked directory inside `.git` passes as "inside".
- the `/` in `"$common"/*`: without it a sibling such as `.git-other/` passes.
- Linked worktrees work because the comparison is against `--git-common-dir`.

## Secret scan (`just secrets`)

- The history scan uses `--log-opts="--full-history -m --diff-filter=uxdb"`: merge commits and type
  changes are included. Tests bury tokens under several commits, in a merge commit and in a commit
  message body, so that a depth or first-parent limit is caught.
- Commit message bodies are scanned separately (`git log --format=%B | gitleaks stdin`); the patch
  scan does not read them.
- Staged changes and unstaged changes to tracked files are scanned too.
- Known and accepted limits live in `docs/accepted-boundaries.md`.

## The excluded test crate

- **`toolchain-tests/` has its own `[workspace]` table**, so adding it leaves the root `Cargo.toml`
  and `Cargo.lock` alone. The price: `cargo fmt --all`, `cargo clippy --workspace`,
  `cargo nextest --workspace` and `cargo deny` do not reach it. The justfile names its manifest in
  each, and a gate test fails if one is missing.
- **A root `members` glob that matches it fails with "multiple workspace roots".** Add it to
  `exclude`.
- **A nested crate builds into its own `target/`**, and a root `/target` entry is anchored to the
  root, so the crate has its own `.gitignore` containing `/target/`.
- **Its `Cargo.lock` is generated at scaffold time and committed**, so CI resolves the crates the
  scaffold resolved. `apply.sh` runs `cargo generate-lockfile` for it.
- **`publish = false` alone does not pass `cargo deny check licenses`**; `deny.toml` also needs
  `[licenses.private]` with `ignore = true`.

## Tests

- `toolchain-tests/tests/gate.rs` is separate from `toolchain.rs` on purpose: it is fast, so it runs
  in `just test` and therefore in `check-fast`. A pull request that downgrades CI to the fast gate
  still runs the tests that notice.
- The toolchain suite copies the files in `TOOLCHAIN_FILES` into throwaway repositories and drops
  `GIT_*`, `LEFTHOOK*`, `GITLEAKS_*`, `JUST_*`, `HOOK_ARG_*`, `CDPATH`, `BASH_ENV`, `ENV`,
  `NEXTEST_*`, `TYPOS_*`, `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_*`, `CARGO_PROFILE_*`,
  `CARGO_TARGET_DIR`, `RUSTC_WRAPPER` and `RUSTUP_TOOLCHAIN` from the environment, so a developer's
  shell cannot change the result. It keeps `CARGO_HOME`, `RUSTUP_HOME` and the registry and network
  settings, because a mirror or credentials configured there are part of a working environment.
- `rust-toolchain.toml` is not copied into the throwaway repositories, so they build with the
  toolchain the developer has installed instead of triggering a download.
- A deliberate misspelling or a token-shaped string written as a literal fails the repository's own
  spell check or secret scan, so the tests assemble both at run time.
- CODEOWNERS is evaluated last-match-wins, like GitHub: a trailing `*` line would take every
  protected file away from its entry. The matcher handles `**` matching zero directories.
- **The workflow rules are tiered.** In every workflow file: no `env:` key, at workflow, job or step
  level, from the tool-configuration families `JUST_`, `GITLEAKS_`, `LEFTHOOK`, `HOOK_ARG_`, `NEXTEST_` and `TYPOS_` (the variables the gate's tools read: `JUST_DRY_RUN=true` makes `just check`
  print the recipe and exit 0 without running it, and `GITLEAKS_CONFIG_TOML` with an empty rule set
  makes the secret scan pass on a committed token; `RUSTFLAGS`, `CARGO_*` and `RUSTUP_*` are not on the
  list because CI legitimately sets them), and no job named `check`, so a second workflow cannot report green under the
  gate's name. In `ci.yml` only: read-only permissions, actions pinned to a 40-hex SHA, the default
  shell, no `continue-on-error`, `if`, `container`, `services`, `||` or `set +`, and a step whose
  `run:` line is exactly `just check` (the SHA, `if`, `continue-on-error`, `||` and `set +` rules read
  the steps of every job in `ci.yml`; the `just check` rule reads the `check` job). Release and docs
  workflows with write permissions pass unchanged.
- **Tool configuration files that switch a gate off are protected by CODEOWNERS, not by a test.** An
  empty `.gitleaks.toml` makes `gitleaks git` exit 0 on a committed token, `rustfmt.toml` with
  `disable_all_formatting = true` makes the format check pass on unformatted code, and a
  `.config/nextest.toml` with `default-filter = "none()"` makes nextest pass with a failing test. The
  scaffold's CODEOWNERS lists these paths (and `clippy.toml`, `_typos.toml`, `.typos.toml`, `.cargo/`,
  `.gitleaksignore`), with a test that each has an owner; whether a given setting is legitimate is a
  review decision.
- A `justfile` and a `Justfile` side by side make `just` fail with "multiple candidate justfiles", and
  `just` accepts the name in any case. `apply.sh` treats any spelling as present.

## Still open, by design

- Hooks check the working tree, not the exact commit, and `--no-verify` skips them.
- `JUST_DRY_RUN=true` in a developer's shell defeats every local hook, because each calls `just`; a
  shell function named `cd` defeats the local commit-msg check. CI is unaffected.
- Whether GitHub lets a same-named job in another workflow satisfy a required check is not tested.
- The tests that build a `PATH` without lefthook use symbolic links and run on Unix only.
- Windows and macOS are covered by the end-to-end job in the repository that ships this skill, not
  by anything in the scaffold itself.
