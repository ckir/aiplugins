---
name: aiplugins-devtools-rust-toolchain-scaffold
description: Scaffold a tested quality toolchain into a Rust or Cargo repository - just recipes, lefthook hooks, rustfmt, clippy, nextest, cargo-deny, typos, gitleaks, SHA-pinned CI, CODEOWNERS, and tests proving each gate blocks. Use to set up, bootstrap or harden hooks, CI or secret scanning in a Rust repo.
---

# rust-toolchain-scaffold

Installs a quality toolchain that has been through several adversarial review rounds,
together with the tests that keep it honest. The point of the skill is not the config
files, which are short. It is that every gate comes with a test proving it blocks what
it claims to, because an untested hook or CI step is routinely weakened without anyone
noticing.

What the target repository ends up with:

| Piece | Files |
|---|---|
| Task runner | `justfile`: `setup`, `fmt`, `fmt-check`, `clippy`, `lint`, `test`, `test-toolchain`, `deny`, `spellcheck`, `secrets`, `secrets-staged`, `secrets-message`, `check-fast`, `check` |
| Git hooks | `lefthook.yml`, `.lefthook/rc.sh`, `.lefthook/pre-push/check.sh` |
| Settings | `rust-toolchain.toml`, `deny.toml`, `typos.toml`, `.gitattributes`, `.gitignore` |
| CI | `.github/workflows/ci.yml` (runs `just check`), `.github/CODEOWNERS` |
| Fast tests | `toolchain-tests/tests/gate.rs`: read the workflow, CODEOWNERS and the config files |
| Slow tests | `toolchain-tests/tests/toolchain.rs`: build throwaway git repositories and prove each gate blocks |
| Docs | an agent-instructions section, a README "Development" section, `docs/accepted-boundaries.md` |

`toolchain-tests/` is a standalone crate with its own `[workspace]` table, so adding it
changes neither the root `Cargo.toml` nor the root `Cargo.lock`. Because the workspace
excludes it, `cargo fmt --all`, `clippy --workspace`, `nextest --workspace` and `cargo deny`
do not reach it; the justfile names it in each of them.

## Machine prerequisites

`git`, `cargo` and `just` are needed everywhere. The others have one install command per
system; give the user the line for theirs rather than working around a missing tool.

| Tool | Install |
|---|---|
| `cargo-nextest`, `cargo-deny`, `typos-cli` | `cargo install --locked cargo-nextest cargo-deny typos-cli`, or `cargo binstall` for prebuilt binaries |
| `just` | `cargo install --locked just`, `brew install just`, `winget install Casey.Just`, or a distribution package |
| `lefthook` | `brew install lefthook`, `scoop install lefthook`, `winget install evilmartians.lefthook`, `go install github.com/evilmartians/lefthook/v2@latest`, or `npm install -g lefthook`. It is not on crates.io. |
| `gitleaks` | `brew install gitleaks`, `scoop install gitleaks`, `winget install gitleaks.gitleaks`, or a release binary from the gitleaks GitHub releases page. It is not on crates.io. |

## Workflow

### 1. Look before copying

- Confirm the repository is Rust and uses Cargo. If it has husky, pre-commit, a Makefile,
  an existing CI workflow, a `Justfile` or its own `deny.toml`, read `references/adapting.md`
  first and tell the user what will and will not carry over.
- Find the code owner handle (`gh repo view --json owner`, or ask). It goes into CODEOWNERS.
- Work on a branch, never directly on the default branch. Check whether a pull request for
  the branch already exists or was merged before pushing more commits to it.
- If the user's git config sets a global `core.hooksPath`, `lefthook install` refuses to run.
  Tell them; it is lefthook's own rule and `docs/accepted-boundaries.md` lists it.

### 2. Copy the templates

```sh
bash <skill-dir>/scripts/apply.sh <target-repo> @<owner>
```

It never overwrites. It lists what it copied, what it skipped because the file is identical
to the template, and what it skipped because the file differs. Merge each differing file by
hand against `assets/template/`, keeping the user's content, and keep what the tests need:

- `.gitignore` needs `/target`.
- `deny.toml` needs `[licenses.private]` with `ignore = true`: the test crate is private and
  carries no `license` field, and `publish = false` alone still fails the `licenses` check.
- `.gitattributes` needs the LF pins the template lists, because a CRLF in a script or a
  config the tests parse breaks them.
- `justfile` needs the recipe names listed above, and each of `fmt`, `fmt-check`, `clippy`,
  `test`, `test-toolchain` and `deny` must name `toolchain-tests/Cargo.toml`.
- If the root `Cargo.toml` is a workspace whose `members` glob could match `toolchain-tests`,
  add it to `exclude`, or cargo fails with "multiple workspace roots".
- Other workflows may stay as they are. The tests check every workflow only for an `env:` key
  from the tool-configuration families `JUST_`, `GITLEAKS_`, `LEFTHOOK`, `HOOK_ARG_`, `NEXTEST_` and `TYPOS_`, and for a job named `check`; the strict rules apply to `ci.yml`.

### 3. Dependencies and versions

The templates carry the versions from when the skill was written. Bring them up to date:

- `rust-toolchain.toml`: the channel, from the current stable release.
- `.github/workflows/ci.yml`: every `TOOL_*_VERSION`, and each action's commit SHA. Resolve
  every SHA fresh from the release tag and keep the version in the trailing comment;
  `references/github-settings.md` has the commands. Do not trust the SHAs in the template.
- `lefthook.yml`: `min_version`, the floor, written only here. Set it to the lefthook the
  user has installed. `just setup` reads it from this file.

### 4. Docs

- The agent instructions file (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md` or `QWEN.md`): merge
  `assets/snippets/agent-instructions.md` under the project's own heading.
- `README.md`: add `assets/snippets/README.development.md`.
- `docs/accepted-boundaries.md`: these are limits a previous owner accepted. Show the list to
  this user and keep only what they accept; the entries are the scaffold's examples, so edit
  them to match this repository.

### 5. Prove it works here

```sh
just setup     # checks the tools, then installs the git hooks
just check     # lint, tests, deny, spell check, secret scan, then the toolchain suite
```

Expect 28 gate tests and 27 toolchain tests (24 on Windows, where the three tests that build a
`PATH` without lefthook are skipped). The secret scan needs at least one commit to scan.

Existing project code will often fail `cargo fmt --check` or clippy with `-D warnings` the first
time. Run `just fmt` (it fixes formatting), report the clippy, deny and test failures, and let the
user choose between fixing the code and relaxing a setting. Do not quietly loosen a gate to get to
green, and if a gate is loosened, update the test that pins it and show it failing against the
old setting.

### 6. Commit, push, CI

Commit on the branch, ask before opening a pull request, and watch the first CI run: the workflow
is only read by the tests locally, so GitHub is the first place it really runs. CI is the
authoritative gate; the hooks are fast local feedback that can be skipped.

### 7. Review and GitHub settings

- If the user's own instructions require review gates before work is called complete, run them as
  those instructions say. This scaffold is a good subject for them: see `references/review-workflow.md`.
- Offer the GitHub settings in `references/github-settings.md` (branch ruleset, CodeQL, Code Owner
  review). They are the user's to switch on, and two of them have consequences worth saying out
  loud before enabling.

Running `apply.sh` again on a scaffolded repository changes nothing and prints which files differ
from the templates, which is how to see drift. Adopting a newer gate is a manual merge.

## Rules that keep the scaffold honest

- **A gate change comes with a test change.** When adding, loosening or rewording a recipe, hook,
  setting or workflow step, add or update a test and show it fails when the gate is weakened (apply
  the weakening, run the test, see it go red, restore). A test that was never seen failing proves
  nothing.
- **Never commit a token-shaped literal or a deliberate misspelling**, even a fake one. The tests
  build them at run time (`fake_token()`, and the typo in the pre-commit test); the secret scan and
  the spell check would otherwise reject the repository's own tests.
- **Do not "simplify" the odd-looking parts** before reading `references/pitfalls.md`. The `rc.sh`
  file, the missing lefthook pin, the copy-install-delete test, the script job in pre-push and the
  `toolchain-tests` exclusion each close a real hole.
- **A new config file a tool reads automatically** (for example `.gitleaks.toml`) is invisible to
  the toolchain suite until it is added to `TOOLCHAIN_FILES` in `toolchain-tests/tests/common/mod.rs`
  and to the `toolchain-tests` glob in `lefthook.yml`.

## Reference files

- `references/pitfalls.md` - why each non-obvious design choice exists. Read before changing
  `justfile`, `lefthook.yml`, `.lefthook/` or the tests.
- `references/adapting.md` - repositories that differ from the template: other layouts, existing
  tools, no GitHub, more than one workflow.
- `references/github-settings.md` - resolving action SHAs, ruleset, CodeQL, Code Owner review, and
  what blocks a merge.
- `references/review-workflow.md` - proving tests non-vacuous, what reviewers found in this
  scaffold before, how to record accepted limits.
