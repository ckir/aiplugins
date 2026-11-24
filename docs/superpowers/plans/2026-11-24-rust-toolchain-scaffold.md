# Rust Toolchain Scaffold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `rust-toolchain-scaffold`, a skill in the `aiplugins-devtools` plugin that installs a tested quality toolchain (just, lefthook, rustfmt, clippy, nextest, cargo-deny, typos, gitleaks, SHA-pinned CI, CODEOWNERS and tests proving each gate blocks) into any Rust repository, on all four supported agents.

**Architecture:** One canonical skill folder under `shared/devtools/skills/`, copied into the four plugins by the existing `emit-devtools-skills` recipe. Every template is stored as `<name>.tmpl` so this repository's own gates and cargo never treat it as a live file; `apply.sh` strips the suffix and never overwrites. The scaffold's tests live in an excluded standalone crate (`toolchain-tests/`) in the target repository. This repository proves the skill with a cheap static gate (`scripts/check-rust-scaffold.sh`, in `just check`) and an end-to-end job on Linux, Windows and macOS (`scripts/test-rust-scaffold.sh`).

**Tech Stack:** bash, just, lefthook 2.1.17, gitleaks 8.30.1, Rust 1.99.1 with cargo-nextest, cargo-deny, typos, yaml-rust2 0.13 (test crate only), GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-24-rust-toolchain-scaffold-design.md` (read it first; the plan implements it and cites it by section).

## Verification status

Read this before trusting a block. While writing the plan, the files below were created in a scratch directory outside the repository and run on Linux with the real tools (just 1.58.0, lefthook 2.1.17, gitleaks 8.30.1, cargo 1.99.1, cargo-nextest 0.9.146, cargo-deny 0.20.2, typos 1.51.1, actionlint 1.7.12, shellcheck, addlicense):

- **Executed and passing:** every template; the whole `toolchain-tests` crate (26 gate tests in 0.107 s, 27 toolchain tests in 14.7 s, both clippy-clean under `-D warnings`); `apply.sh`; `scripts/test-rust-scaffold.sh` in both modes (apply checks, then `just setup` and `just check` on a scaffolded repository, then nine seeded weakenings each turning the named gate test red); `scripts/check-rust-scaffold.sh` and its `--mutation-test` (11 of 11 seeded defects caught); `shellcheck` on all three scripts, `actionlint` on the CI template, `typos` on every file, `addlicense -check` on the three scripts; the footprint tool on scratch copies (the skill adds 341 resident bytes on Claude Code and Antigravity and 418 on OpenCode, cap 500).
- **Not executed:** anything that touches the real repository (the emit recipe, the Justfile and `ci.yml` edits, the README edits, footprint regeneration, bundles, `just check` of this repository); Windows and macOS; the new CI job; the `ci.yml` of the scaffold on GitHub itself. Those steps say "Expected:" and the executor must report what actually happened, not the expectation.

## Global Constraints

Every task's requirements implicitly include these, copied from the spec.

- The skill is `rust-toolchain-scaffold` in the plugin `aiplugins-devtools`; its source is `shared/devtools/skills/rust-toolchain-scaffold/`; its OpenCode command is `shared/devtools/commands/rust-toolchain-scaffold.md`.
- Every file under `assets/template/` is stored as `<name>.tmpl` (spec 4.1).
- This repository's license-header gate covers tracked `.rs`, `.sh`, `.ts`, `.js`, `.py` and `.ps1` files: the three scripts carry the PolyForm header; templates carry no license header (spec 2, decision 5).
- The footprint cap: the new skill's frontmatter plus, on OpenCode, its command file's frontmatter must fit in `deltaBytes` 500 per package. The budget file is not edited to make room (spec 5).
- Never commit a token-shaped literal or a deliberate misspelling, even a fake one; the tests build both at run time (spec 4.4).
- Hook runner: lefthook from `PATH` with `min_version`, `assert_lefthook_installed: true`, and an `rc.sh` that unsets `LEFTHOOK_BIN` and `LEFTHOOK_CONFIG`; the floor is written once, in `lefthook.yml` (spec 4.3).
- Workflow scope is tiered: every workflow is checked for a `JUST_*` env key and for a job named `check`; `ci.yml` alone gets the strict rules (spec 2, decision 6). The CI tool pins are named `TOOL_*`.
- The suite's environment drops the explicit variable list in spec 4.4, not a `CARGO_*` glob, and points git's config files at an empty file.
- Commit with conventional-commit subjects (`feat:`, `docs:`, `ci:`, `fix:`) and end each message with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. Add files by explicit path, never `git add -A`. Do not push or open a pull request unless the user asks.

## Execution notes

Each task is "write exactly these bytes, run the stated verification". Tasks 1 and 3 are file creation with a verification command and can go to the lowest-capability tier; Tasks 4 to 6 edit existing repository files and need judgment about what the repository's own gates say, so use a mid tier. Per the repository's rule, verify lower-tier output harder: after any task, run its stated verification yourself and read the diff against the block in this plan.

The commits along the way are not releasable states: after Task 1 the plugins list a skill whose `apply.sh` and templates arrive in Task 3. Nothing releases from a feature branch (release-please acts on `main`), so keep the whole branch in one pull request and merge only after Task 6.

## File structure

| Path | Responsibility | Task |
|---|---|---|
| `shared/devtools/skills/rust-toolchain-scaffold/SKILL.md` | the agent's workflow | 1 |
| `.../references/{pitfalls,adapting,github-settings,review-workflow}.md` | why, and how to adapt | 1 |
| `.../assets/snippets/{agent-instructions,README.development}.md` | text merged into the user's docs | 1 |
| `shared/devtools/commands/rust-toolchain-scaffold.md` | OpenCode slash command | 1 |
| `scripts/test-rust-scaffold.sh` | end-to-end test | 2, 3 |
| `.../scripts/apply.sh` | copies templates, never overwrites | 3 |
| `.../assets/template/**/*.tmpl` | the scaffold itself | 3 |
| `scripts/check-rust-scaffold.sh` | static gate with mutation test | 4 |
| `Justfile`, `.github/workflows/ci.yml` | wiring | 4, 5 |
| four plugin `README.md` files, `docs/footprints/*` | listing and footprint | 1 |
| four emitted `skills/` copies, one OpenCode `commands/` copy | generated by `just emit-devtools-skills` | 1, 3 |

---

### Task 0: Preflight

**Files:** none.

- [ ] **Step 1: Confirm the tools**

```bash
for t in git cargo rustup just lefthook gitleaks cargo-nextest cargo-deny typos actionlint shellcheck addlicense mlc gh jq tar zip unzip zipinfo; do printf '%-14s' "$t"; command -v "$t" >/dev/null && echo ok || echo MISSING; done
rustup toolchain list | grep -c '^1\.99\.1' 
```

Expected: no `MISSING`. `mlc` is the link checker that `just links` runs; `tar`, `zip`, `unzip`, `zipinfo` and `jq` are what `just skills-bundles` needs. (Install any that is missing, per `shared/devtools/skills/rust-toolchain-scaffold/SKILL.md` once Task 1 has run, or the lines in the spec's section 4.2.) The second command must print `1`, the toolchain. If `1.99.1` is missing, `rustup toolchain install 1.99.1` before Task 3.

- [ ] **Step 2: Start the branch**

The spec and this plan live on `docs/rust-toolchain-scaffold-spec`. Branch from it (or from `main` once that pull request has merged):

```bash
git switch docs/rust-toolchain-scaffold-spec
git status --short            # Expected: nothing
git switch -c feat/rust-toolchain-scaffold
```

---

### Task 1: The skill's text, its command, the emitted copies and the footprint

**Files:**
- Create: `shared/devtools/skills/rust-toolchain-scaffold/SKILL.md`
- Create: `shared/devtools/skills/rust-toolchain-scaffold/references/pitfalls.md`, `adapting.md`, `github-settings.md`, `review-workflow.md`
- Create: `shared/devtools/skills/rust-toolchain-scaffold/assets/snippets/agent-instructions.md`, `README.development.md`
- Create: `shared/devtools/commands/rust-toolchain-scaffold.md`
- Modify: `claude-code/plugins/aiplugins-devtools-cc/README.md`, `antigravity/plugins/aiplugins-devtools-agy/README.md`, `qwen/plugins/aiplugins-devtools-qwen/README.md`, `opencode/plugins/aiplugins-devtools-opencode/README.md`
- Generated by recipes: the four plugins' `skills/rust-toolchain-scaffold/` copies, `opencode/plugins/aiplugins-devtools-opencode/skills/aiplugins-devtools-rust-toolchain-scaffold/`, `opencode/plugins/aiplugins-devtools-opencode/commands/rust-toolchain-scaffold.md`, `docs/footprints/aiplugins-devtools-{cc,agy,opencode}.json`, `docs/footprints/budgets.json`, the footprint regions of the three READMEs that carry one and of the root `README.md`.

**Interfaces:**
- Consumes: the existing `emit-devtools-skills` recipe and `scripts/check-devtools-skills.sh` (read in full: per skill it requires `name: <folder>` and `description:` in the frontmatter and every backticked `references/<file>` to exist; per command file it requires `$ARGUMENTS` and `aiplugins-devtools-<skill>` in the body).
- Produces: the skill folder that Tasks 3 and 4 add to. `SKILL.md` names `scripts/apply.sh`, `assets/template/`, `assets/snippets/*` and four `references/*` files; the references exist after this task, the others after Task 3.

- [ ] **Step 1: Create `SKILL.md`**

Create `shared/devtools/skills/rust-toolchain-scaffold/SKILL.md` with exactly this content:

````md
---
name: rust-toolchain-scaffold
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
- Other workflows may stay as they are. The tests check every workflow only for a `JUST_*`
  env key and for a job named `check`; the strict rules apply to `ci.yml`.

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
  this user and keep only what they accept; the dates and "(owner, ...)" notes in the template
  are not theirs.

### 5. Prove it works here

```sh
just setup     # checks the tools, then installs the git hooks
just check     # lint, tests, deny, spell check, secret scan, then the toolchain suite
```

Expect 26 gate tests and 27 toolchain tests (24 on Windows, where the three tests that build a
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
````

- [ ] **Step 2: Create the four references**

Create `shared/devtools/skills/rust-toolchain-scaffold/references/pitfalls.md`:

```md
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
- **`LEFTHOOK_BIN` and `LEFTHOOK_CONFIG` override everything.** The first picks the binary ahead of
  `PATH`, the second swaps the configuration for any file, including one that does not exist, and
  either one set in the caller's environment turns the hooks into a no-op that exits 0. `rc.sh` runs
  `unset LEFTHOOK_BIN LEFTHOOK_CONFIG`, and the generated hook sources it before it reads either.
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
  tests set `GIT_CONFIG_GLOBAL` to an empty file, because stripping `GIT_*` variables does not isolate
  git's config files.

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
- **The workflow rules are tiered.** In every workflow file: no `env:` key starting with `JUST_` (the
  variables `just` itself reads: `JUST_DRY_RUN=true` makes `just check` print the recipe and exit 0
  without running it), and no job named `check`, so a second workflow cannot report green under the
  gate's name. In `ci.yml` only: read-only permissions, actions pinned to a 40-hex SHA, the default
  shell, no `continue-on-error`, `if`, `container`, `services`, `||` or `set +`, and a step whose
  `run:` line is exactly `just check`. Release and docs workflows with write permissions pass
  unchanged.
- A `justfile` and a `Justfile` side by side make `just` fail with "multiple candidate justfiles", and
  `just` accepts the name in any case. `apply.sh` treats any spelling as present.

## Still open, by design

- Hooks check the working tree, not the exact commit, and `--no-verify` skips them.
- `JUST_DRY_RUN=true`, or a shell function named `cd`, in a developer's shell defeats the local
  commit-msg check. CI is unaffected.
- Whether GitHub lets a same-named job in another workflow satisfy a required check is not tested.
- The tests that build a `PATH` without lefthook use symbolic links and run on Unix only.
- Windows and macOS are covered by the end-to-end job in the repository that ships this skill, not
  by anything in the scaffold itself.
```

Create `.../references/adapting.md`:

```md
# When the repository does not match the template

The templates assume: Rust and Cargo, GitHub, one default branch called `main`, a root crate or a
workspace, and no tooling of their own yet.

| Difference | What to change |
|---|---|
| The repository already has a `justfile`, `Justfile` or `.justfile` | `apply.sh` counts any spelling as present and skips it. Merge recipe by recipe; the tests name the recipes they need (`setup`, `fmt`, `fmt-check`, `clippy`, `lint`, `test`, `test-toolchain`, `deny`, `spellcheck`, `secrets`, `secrets-staged`, `secrets-message`, `check-fast`, `check`). Keep one spelling: `just` fails when two coexist. The CODEOWNERS template lists `justfile`, `Justfile` and `.justfile`. |
| It already has `deny.toml`, `typos.toml`, `.gitattributes` or `rust-toolchain.toml` | `apply.sh` skips them. Merge in what the tests read: `deny.toml` needs `[licenses.private]` with `ignore = true`; `.gitattributes` needs the LF pins; `rust-toolchain.toml` needs a channel with the `rustfmt` and `clippy` components. |
| The root `Cargo.toml` is a workspace | Add `toolchain-tests` to its `exclude` list if a `members` glob could match it. The workspace's own crates keep being checked with `--workspace`. |
| Existing husky, pre-commit or `.git/hooks` scripts | Port each hook into `lefthook.yml` as a job that calls a `just` recipe, then remove the old runner. Hooks installed twice run twice or fight. |
| An existing CI workflow, or several | `apply.sh` skips an existing `ci.yml` and prints the other workflow files. The gate tests check every workflow for a `JUST_*` env key and for a job named `check`, and check `ci.yml` strictly. Rename a clashing job in another workflow. Pinning actions to a SHA in the other workflows is advice, not a tested rule. |
| The CI workflow needs write permissions, a container or services | The strict rules apply to `ci.yml`. Put that work in another workflow, or change the specific assertion in `gate.rs`, say why in a comment, and prove the changed test still fails on the cases it should. |
| Code that already fails `cargo fmt --check`, clippy `-D warnings` or `cargo deny` | Run `just fmt`, report the rest, and let the user choose between fixing the code and relaxing a setting. Never loosen a gate quietly; if one is loosened, update the test that pins it. |
| Not on GitHub | Drop `.github/` and the workflow and CODEOWNERS tests in `gate.rs`; keep the lefthook half. Recreate "CI runs `just check`" on the other platform. |
| Another hook runner, or none | Out of scope for the templates as written: the recipes, the tests and `rc.sh` assume lefthook. Say so rather than half-porting. |
| A new tool config file appears | Add it to `TOOLCHAIN_FILES` in `toolchain-tests/tests/common/mod.rs`, to the `toolchain-tests` glob in `lefthook.yml`, and consider CODEOWNERS. |
| A `justfile` spelled in another case (`JUSTFILE`) | `just` accepts it, but CODEOWNERS and the tests know three spellings. Rename it. |

After any adaptation: `just check`, then weaken the adapted gate once to see its test fail.
```

Create `.../references/github-settings.md`:

````md
# GitHub: action pins and repository settings

## Resolving an action's commit SHA

The workflow pins each action as `owner/repo@<40-hex sha> # vX.Y.Z`, because a tag can be moved to
different code. Resolve from the release tag; do not copy a SHA from memory or from the template:

```sh
gh api repos/actions/checkout/git/ref/tags/v7.0.1 -q '.object | "\(.type) \(.sha)"'
# if the type is "tag" (an annotated tag), dereference it to the commit:
gh api repos/actions/checkout/git/tags/<sha-from-above> -q .object.sha
```

Do the same for `taiki-e/install-action`. Use the latest release of each
(`gh api repos/<owner>/<repo>/releases/latest -q .tag_name`) and keep the version comment in step
with the SHA.

`taiki-e/install-action` installs `just`, `cargo-nextest`, `cargo-deny` and `typos` at the versions
the workflow names. It does not list `gitleaks` or `lefthook`, so the workflow downloads each one by
version from its GitHub release and verifies it against the release's checksums file
(`gitleaks_<version>_checksums.txt`, `lefthook_checksums.txt`). The toolchain comes from
`rust-toolchain.toml` through `rustup toolchain install`.

## Settings to offer (the user switches them on)

These are outward-facing account settings: describe them, get a yes, then apply.

1. **Ruleset on the default branch** requiring a pull request and the `check` status check. Without
   it CI is advisory.
2. **Require review from Code Owners.** CODEOWNERS does nothing until this is on. Say this first: an
   author cannot approve their own pull request, so a repository with a single owner will need the
   admin bypass for the owner's own pull requests that touch owned paths. The rule only bites for
   other contributors. Alternatives: a second owner, or leaving it off.
3. **CodeQL default setup** (`gh api -X PATCH repos/<o>/<r>/code-scanning/default-setup -f state=configured`).
   Expect the first scan to raise alerts in existing code; check
   `gh api 'repos/<o>/<r>/code-scanning/alerts?state=open'` afterwards.

Before changing a ruleset through the API, save the current one (`gh api repos/<o>/<r>/rulesets/<id>`),
change only the field asked for, and `PUT` the whole object back, bypass actors included.

## When a merge is refused

`gh pr view <n> --json mergeStateStatus` saying `BLOCKED` with green checks usually means a rule that
cannot be met, not a failing one. Read the rules (`gh api repos/<o>/<r>/rules/branches/<branch>`) and
look for:

- a **code scanning** rule while code scanning is not configured: it waits forever for results that
  never come;
- **"branch must be up to date"** while the base has commits the branch lacks;
- **Code Owner review** with nobody eligible to give it.

Report which rule blocks and let the user choose between fixing the cause and `gh pr merge --admin`.
Bypassing branch protection is their decision, each time.

## Smaller traps

- A pull request that was merged while work continued: check its state before pushing to the same
  branch, and ask before opening a follow-up pull request.
- Before overwriting a pull request description, fetch the current body with
  `gh api repos/<o>/<r>/pulls/<n> -q .body` into a file and confirm the file is not empty.
- CodeQL flags a token generator that takes a random byte modulo the alphabet length as biased
  randomness, rated high, even in tests. The template's `fake_token()` keeps six bits of each byte and
  discards the two values past the end of the alphabet.
````

Create `.../references/review-workflow.md`:

```md
# Proving the tests, reviewing the scaffold, recording limits

## Mutants: the only proof a test works

For every gate, apply one small weakening, run the tests, and require a specific test to go red; then
restore the file. A suite that stays green under the weakening has a gap.

- Weaken one thing at a time: delete a recipe line, drop a flag, add `|| true`, negate a condition,
  loosen a version pin, remove a CODEOWNERS entry, add `continue-on-error`.
- Check that the test that failed is the one written for that gate, not a bystander.
- Restore from a saved copy of the file, not `git checkout`, when the file has uncommitted changes you
  want to keep: `git checkout` silently discards them along with the mutant.
- The gate tests read the working tree, so mutating `justfile` or `lefthook.yml` in place is enough; no
  commit is needed.

## What adversarial review found in this scaffold before

Useful as a checklist of where to aim a reviewer, or yourself, after adapting anything:

- shell syntax in a hook argument being executed;
- a hook falling back to a different binary than the one intended: the path lefthook was installed
  from, `LEFTHOOK_BIN`, `LEFTHOOK_CONFIG`;
- a check that "passes" by scanning nothing (missing rc file, a symlink, a file outside `.git`);
- ambient shell settings changing a recipe's result (`CDPATH`, `JUST_DRY_RUN`);
- history scans that could be narrowed (depth, first-parent, no merges, subject only);
- tests that replace a step with a stub and so miss the step being deleted;
- tests that read a config file by exact string and miss an override elsewhere (a later CODEOWNERS
  rule, a job-level `permissions`, a second workflow, a local lefthook config, a step-level `env:`);
- CI steps made non-fatal (`continue-on-error`, `|| true`, `set +e`, a custom shell);
- a recipe that runs on the workspace but never reaches the excluded test crate.

Two questions find most of the rest: "what single edit turns this gate off with every test still
green?" and "which test here would pass even if the thing it names broke?"

## Being straight about what was verified

Reviewers repeatedly caught overstated claims in commit messages and summaries, not just defects in
code: "every test mutated" when some were, "would block every commit" when only one path was affected,
"no custom shell" when only the steps were checked. Count before writing a number, and say what a clean
review round did not cover (narrowed scope, platforms not tested, CI not yet seen).

## Recording accepted limits

`docs/accepted-boundaries.md` lists behaviours the tests deliberately do not cover, each with what
compensates for it and where. Add an entry only when the user accepts the limit; remove it when it is
closed. A later test audit re-checks that each compensation still exists instead of re-raising the item.

If the user's environment has formal review gates (for example a capstone review of the committed code
and a separate audit of the tests), run them after `just check` and CI are green, verify every finding
and every suggested fix by measurement before acting on it, let the user decide which gaps to close, and
record the outcome where those gates say to. Waiving a gate is the user's call, never the agent's.
```

- [ ] **Step 3: Create the two snippets**

Create `shared/devtools/skills/rust-toolchain-scaffold/assets/snippets/agent-instructions.md`:

```md
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
```

Create `.../assets/snippets/README.development.md`:

````md
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
````

- [ ] **Step 4: Create the OpenCode command**

Create `shared/devtools/commands/rust-toolchain-scaffold.md`:

```md
---
description: Scaffold a tested Rust toolchain into a repo
---
Load the skill `aiplugins-devtools-rust-toolchain-scaffold` with the skill tool, then run it on this request: $ARGUMENTS
```

- [ ] **Step 5: Emit the copies and run the existing drift gate**

```bash
just emit-devtools-skills
bash scripts/check-devtools-skills.sh
bash scripts/check-devtools-skills.sh --mutation-test
```

Expected: `Devtools skills consistent.` then `Devtools skills gate fails on all 21 seeded defects.` (its cases are fixed, so adding a skill does not change the 21). A failure names the missing file or reference.

- [ ] **Step 6: Measure the footprint with the repository's own tool**

```bash
fp() { cargo run -q -p plugin-footprint --bin plugin-footprint -- "$@"; }
fp measure claude-code/plugins/aiplugins-devtools-cc --out docs/footprints/aiplugins-devtools-cc.json
fp ratchet --measured docs/footprints/aiplugins-devtools-cc.json --budgets docs/footprints/budgets.json
fp measure antigravity/plugins/aiplugins-devtools-agy --out docs/footprints/aiplugins-devtools-agy.json
fp ratchet --measured docs/footprints/aiplugins-devtools-agy.json --budgets docs/footprints/budgets.json
fp measure opencode/plugins/aiplugins-devtools-opencode --out docs/footprints/aiplugins-devtools-opencode.json
fp ratchet --measured docs/footprints/aiplugins-devtools-opencode.json --budgets docs/footprints/budgets.json
fp publish
git status --short
git diff --stat -- docs/footprints README.md
bash scripts/check-footprint-budgets.sh
```

Expected: no error from `ratchet` (it fails when a delta exceeds `deltaBytes`). The resident bytes in each regenerated document rise by 341 (Claude Code, Antigravity) and 418 (OpenCode) over the committed ones; the `git status` lists the three `docs/footprints/aiplugins-devtools-*.json`, `docs/footprints/budgets.json`, the footprint regions of the plugin READMEs and of the root `README.md`, and nothing else besides the files from Steps 1 to 5 and the emitted copies. If a `ratchet` fails, shorten the `description:` line in `SKILL.md` (the measured cost is the frontmatter text), run `just emit-devtools-skills` again and repeat this step. Do not edit `budgets.json` by hand. `ratchet` rewrites `budgets.json` in place, so before repeating this step run `git checkout -- docs/footprints README.md` to start from the committed baseline; otherwise the retry measures against a half-updated one.

(`just footprint-regen` does the same for every plugin but needs the other plugins' binaries built; the loop above is its devtools part.)

- [ ] **Step 7: List the skill in the four plugin READMEs**

In each README, make the three edits shown. Use the exact old text; each occurs once in its file.

First the opening line of each file, line 3, which says `four` and must say `five`:

```
claude-code/plugins/aiplugins-devtools-cc/README.md
  old: A Claude Code plugin that ships four dev-tool skills:
  new: A Claude Code plugin that ships five dev-tool skills:
qwen/plugins/aiplugins-devtools-qwen/README.md
  old: A Qwen Code extension that ships four dev-tool skills:
  new: A Qwen Code extension that ships five dev-tool skills:
antigravity/plugins/aiplugins-devtools-agy/README.md
  old: An Antigravity plugin that ships four dev-tool skills:
  new: An Antigravity plugin that ships five dev-tool skills:
opencode/plugins/aiplugins-devtools-opencode/README.md
  old: An OpenCode plugin that ships four dev-tool skills:
  new: An OpenCode plugin that ships five dev-tool skills:
```

Then, in `claude-code/plugins/aiplugins-devtools-cc/README.md` and `qwen/plugins/aiplugins-devtools-qwen/README.md` (the same text in both):

Old:

```
  `LICENSE-ADDENDUM` template for extra terms. Bundles a wrapper script; needs `addlicense`.

Slash commands: `/aiplugins-devtools:spec-panel-review <path>` and `/aiplugins-devtools:human-friendly <path>`.
```

New:

```
  `LICENSE-ADDENDUM` template for extra terms. Bundles a wrapper script; needs `addlicense`.
- `rust-toolchain-scaffold`, scaffolds a tested quality toolchain into a Rust repository: just
  recipes, lefthook hooks, rustfmt, clippy, nextest, cargo-deny, typos, gitleaks, SHA-pinned CI and
  CODEOWNERS, plus tests that prove each gate blocks. Its checks need `just`, `lefthook` and `gitleaks`.

Slash commands: `/aiplugins-devtools:spec-panel-review <path>`, `/aiplugins-devtools:human-friendly <path>` and `/aiplugins-devtools:rust-toolchain-scaffold <request>`.
```

`antigravity/plugins/aiplugins-devtools-agy/README.md`:

Old:

```
  `LICENSE-ADDENDUM` template for extra terms. Bundles a wrapper script; needs `addlicense`.

Slash commands: `/spec-panel-review <path>` and `/human-friendly <path>`.
```

New:

```
  `LICENSE-ADDENDUM` template for extra terms. Bundles a wrapper script; needs `addlicense`.
- `rust-toolchain-scaffold`, scaffolds a tested quality toolchain into a Rust repository: just
  recipes, lefthook hooks, rustfmt, clippy, nextest, cargo-deny, typos, gitleaks, SHA-pinned CI and
  CODEOWNERS, plus tests that prove each gate blocks. Its checks need `just`, `lefthook` and `gitleaks`.

Slash commands: `/spec-panel-review <path>`, `/human-friendly <path>` and `/rust-toolchain-scaffold <request>`.
```

`opencode/plugins/aiplugins-devtools-opencode/README.md`. Edit one:

Old:

```
  `LICENSE-ADDENDUM` template for extra terms. Bundles a wrapper script; needs `addlicense`.

Slash commands: `/spec-panel-review <path>`, `/human-friendly <path>` and `/license-headers <request>` (command files; see Invocation).
```

New:

```
  `LICENSE-ADDENDUM` template for extra terms. Bundles a wrapper script; needs `addlicense`.
- `rust-toolchain-scaffold`, scaffolds a tested quality toolchain into a Rust repository: just
  recipes, lefthook hooks, rustfmt, clippy, nextest, cargo-deny, typos, gitleaks, SHA-pinned CI and
  CODEOWNERS, plus tests that prove each gate blocks. Its checks need `just`, `lefthook` and `gitleaks`.

Slash commands: `/spec-panel-review <path>`, `/human-friendly <path>`, `/license-headers <request>` and `/rust-toolchain-scaffold <request>` (command files; see Invocation).
```

In the same file, the invocation paragraph:

Old:

```
`aiplugins-devtools-spec-panel-review`, `aiplugins-devtools-human-friendly` and `aiplugins-devtools-license-headers`. OpenCode also does not list skills in
its `/` menu, so the plugin ships command files, `commands/spec-panel-review.md`, `commands/human-friendly.md` and `commands/license-headers.md`;
`/spec-panel-review <path>`, `/human-friendly <path>` and `/license-headers <request>` load the skill and run it on the argument. A colon is not valid in a folder
```

New:

```
`aiplugins-devtools-spec-panel-review`, `aiplugins-devtools-human-friendly`, `aiplugins-devtools-license-headers` and `aiplugins-devtools-rust-toolchain-scaffold`. OpenCode also does not list skills in
its `/` menu, so the plugin ships command files, `commands/spec-panel-review.md`, `commands/human-friendly.md`, `commands/license-headers.md` and `commands/rust-toolchain-scaffold.md`;
`/spec-panel-review <path>`, `/human-friendly <path>`, `/license-headers <request>` and `/rust-toolchain-scaffold <request>` load the skill and run it on the argument. A colon is not valid in a folder
```

Then check the edits applied:

```bash
grep -c 'rust-toolchain-scaffold' claude-code/plugins/aiplugins-devtools-cc/README.md antigravity/plugins/aiplugins-devtools-agy/README.md qwen/plugins/aiplugins-devtools-qwen/README.md opencode/plugins/aiplugins-devtools-opencode/README.md
grep -n 'four dev-tool' claude-code/plugins/aiplugins-devtools-cc/README.md antigravity/plugins/aiplugins-devtools-agy/README.md qwen/plugins/aiplugins-devtools-qwen/README.md opencode/plugins/aiplugins-devtools-opencode/README.md
```

Expected: a count of at least 2 in each file (more in the OpenCode one) and no line from the second command.

- [ ] **Step 8: Spell check and link check the new markdown**

```bash
typos shared/devtools claude-code/plugins/aiplugins-devtools-cc antigravity/plugins/aiplugins-devtools-agy qwen/plugins/aiplugins-devtools-qwen opencode/plugins/aiplugins-devtools-opencode
just links
```

Expected: both silent and exit 0.

- [ ] **Step 9: Commit**

```bash
git add shared/devtools/skills/rust-toolchain-scaffold shared/devtools/commands/rust-toolchain-scaffold.md \
  claude-code/plugins/aiplugins-devtools-cc antigravity/plugins/aiplugins-devtools-agy qwen/plugins/aiplugins-devtools-qwen opencode/plugins/aiplugins-devtools-opencode \
  docs/footprints README.md
git status --short            # Expected: only the paths above, all staged
git commit -m "$(cat <<'EOF'
feat: add the rust-toolchain-scaffold skill text and its footprint

SKILL.md, four references, two snippets and the OpenCode command, emitted into the
four devtools plugins, with the footprint documents regenerated (+341 resident bytes
on Claude Code and Antigravity, +418 on OpenCode, against a cap of 500). The templates
and apply.sh follow.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Write the end-to-end test first

**Files:**
- Create: `scripts/test-rust-scaffold.sh`

**Interfaces:**
- Consumes: `shared/devtools/skills/rust-toolchain-scaffold/scripts/apply.sh <repo> @<owner>` (Task 3), which prints `COPIED (n):`, `SKIPPED, identical to the template (n):`, `SKIPPED, already present and DIFFERENT - merge by hand, keeping the user's content (n):` each followed by indented paths, then optional `NOTE:` lines.
- Produces: `bash scripts/test-rust-scaffold.sh [--quick]`, exit 0 only if every check passes; `RUST_SCAFFOLD_SKILL_DIR` overrides the skill folder for experiments.

- [ ] **Step 1: Create the script**

Create `scripts/test-rust-scaffold.sh` with exactly this content, then `chmod +x scripts/test-rust-scaffold.sh`:

```bash
#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# End-to-end test of the rust-toolchain-scaffold skill: apply it to throwaway
# repositories and prove that what it installs works and that its gate tests
# notice a weakened gate.
#
#   bash scripts/test-rust-scaffold.sh [--quick]
#
# --quick runs only the apply.sh checks (no tools, no network beyond cargo's lockfile).
# Without it the scaffolded repository runs its own `just check`, and then a set
# of seeded weakenings is applied to it one at a time: the gate tests must fail
# on each, or a passing scaffold would prove nothing (the templates would be
# testing themselves).
# Needs: git, cargo, just, lefthook, gitleaks, cargo-nextest, cargo-deny, typos.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd -P)
skill=${RUST_SCAFFOLD_SKILL_DIR:-$repo/shared/devtools/skills/rust-toolchain-scaffold}
apply=$skill/scripts/apply.sh
mode=${1:-}
[ -z "$mode" ] || [ "$mode" = "--quick" ] || { echo "usage: $0 [--quick]" >&2; exit 2; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail=0
bad() { echo "  FAIL  $1" >&2; fail=1; }
ok() { echo "  ok    $1"; }
# expect <description> <command...>: ok when the command succeeds, FAIL when it does not.
expect() {
  local description=$1
  shift
  if "$@"; then ok "$description"; else bad "$description"; fi
}
# refuse <description> <command...>: ok when the command FAILS.
refuse() {
  local description=$1
  shift
  if "$@" >/dev/null 2>&1; then bad "$description"; else ok "$description"; fi
}
has_line() { grep -qx -- "$1"; }
# The lines of apply.sh's report under one heading ("identical" or "DIFFERENT").
section() { awk -v h="$1" 'index($0, h) { found = 1; next } /^(SKIPPED|NOT TOUCHED|NOTE|ERROR)/ { found = 0 } found'; }
has_text() { grep -q -- "$1"; }

# A clean environment for every git and cargo command, as the suite's own helpers build.
export GIT_CONFIG_GLOBAL="$work/empty" GIT_CONFIG_NOSYSTEM=1
: > "$work/empty"
export GIT_AUTHOR_NAME=scaffold-test GIT_AUTHOR_EMAIL=scaffold-test@example.invalid
export GIT_COMMITTER_NAME=scaffold-test GIT_COMMITTER_EMAIL=scaffold-test@example.invalid
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE LEFTHOOK LEFTHOOK_BIN LEFTHOOK_CONFIG HOOK_ARG_1 CDPATH BASH_ENV ENV

# new_repo <dir>: a git repository holding one small, clean, private crate.
new_repo() {
  mkdir -p "$1/src"
  git -C "$1" init -q -b main
  printf '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\npublish = false\n' > "$1/Cargo.toml"
  printf '/// Adds two numbers.\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn adds() {\n        assert_eq!(add(1, 2), 3);\n    }\n}\n' > "$1/src/lib.rs"
  printf '/target\n*.log\n' > "$1/.gitignore"
  git -C "$1" add -A
  git -C "$1" commit -q --no-verify -m initial
}

echo "== apply.sh"
r1="$work/single"
new_repo "$r1"
out=$(bash "$apply" "$r1" @someone 2>&1) || { echo "$out" >&2; bad "apply.sh failed on a fresh repository"; }
for f in justfile lefthook.yml .lefthook/rc.sh .github/workflows/ci.yml .github/CODEOWNERS deny.toml typos.toml rust-toolchain.toml .gitattributes toolchain-tests/Cargo.toml toolchain-tests/Cargo.lock toolchain-tests/.gitignore toolchain-tests/tests/gate.rs toolchain-tests/tests/toolchain.rs toolchain-tests/tests/common/mod.rs docs/accepted-boundaries.md; do
  expect "copied $f" test -f "$r1/$f"
done
expect "no .tmpl file was copied as it is" test -z "$(find "$r1" -name '*.tmpl' -not -path '*/target/*' -print -quit)"
expect "the code owner was filled in" has_text '@someone' < "$r1/.github/CODEOWNERS"
refuse "no @OWNER placeholder is left" has_text '@OWNER' < "$r1/.github/CODEOWNERS"
expect "an existing .gitignore was left alone" test "$(cat "$r1/.gitignore")" = "$(printf '/target\n*.log')"
expect "the differing .gitignore is reported as different" has_line '  .gitignore' < <(section "DIFFERENT" <<<"$out")
refuse "the new files are not reported as different" has_line '  justfile' < <(section "DIFFERENT" <<<"$out")

out2=$(bash "$apply" "$r1" @someone 2>&1) || bad "the second apply.sh run failed"
expect "a second run copies nothing" has_text 'COPIED (0)' <<<"$out2"
expect "a second run reports only the .gitignore as different" test "$(section "DIFFERENT" <<<"$out2")" = "  .gitignore"
expect "a second run reports the copied files as identical" has_line '  lefthook.yml' < <(section "identical" <<<"$out2")
printf '# edited\n' >> "$r1/deny.toml"
out3=$(bash "$apply" "$r1" @someone 2>&1) || bad "the third apply.sh run failed"
expect "an edited file shows up as different" has_line '  deny.toml' < <(section "DIFFERENT" <<<"$out3")
expect "the edit was not overwritten" has_text '# edited' < "$r1/deny.toml"

r2="$work/capital"
new_repo "$r2"
printf 'default:\n\t@echo mine\n' > "$r2/Justfile"
bash "$apply" "$r2" @someone >/dev/null 2>&1 || bad "apply.sh failed beside a Justfile"
expect "no second justfile spelling was created" test "$(find "$r2" -maxdepth 1 -iname justfile | wc -l | tr -d ' ')" = 1

r3="$work/workspace"
new_repo "$r3"
printf '[workspace]\nmembers = ["crates/*"]\nresolver = "2"\n' > "$r3/Cargo.toml"
mkdir -p "$r3/crates/a/src"
printf '[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\npublish = false\n' > "$r3/crates/a/Cargo.toml"
printf '//! a\n' > "$r3/crates/a/src/lib.rs"
rm -rf "$r3/src"
out4=$(bash "$apply" "$r3" @someone 2>&1) || bad "apply.sh failed on a workspace"
expect "a workspace root gets the exclude note" has_text 'root Cargo.toml is a workspace' <<<"$out4"

refuse "a missing directory is refused" bash "$apply" "$work/missing-dir" @someone
refuse "an owner without @ is refused" bash "$apply" "$r1" someone
refuse "an owner with sed syntax is refused" bash "$apply" "$r1" '@a|b'

if [ "$mode" = "--quick" ]; then
  [ "$fail" -eq 0 ] || { echo "Rust scaffold test failed." >&2; exit 1; }
  echo "Rust scaffold apply.sh checks passed (--quick: the scaffolded gates were not run)."
  exit 0
fi

echo "== the scaffolded repository runs its own gates"
cd "$r1"
# deny.toml was edited above on purpose; start the gates from the shipped one.
cp "$skill/assets/template/deny.toml.tmpl" deny.toml
git add -A
git commit -q --no-verify -m "scaffold"
expect "the pinned toolchain is installed" rustup toolchain install
expect "just setup installs the hooks" just setup
if just check >"$work/check.out" 2>&1; then ok "just check passes on the scaffolded repository"; else tail -40 "$work/check.out" >&2; bad "just check failed on the scaffolded repository"; fi

echo "== seeded weakenings: the gate tests must fail on each"
gate() { cargo nextest run --manifest-path toolchain-tests/Cargo.toml --test gate --no-fail-fast 2>&1 || true; }
# weaken <description> <expected failing test> <function that weakens the scaffolded repository>
weaken() {
  local name=$1 want=$2 change=$3 out
  git checkout -q -- .
  git clean -fdq -e toolchain-tests/target -e target
  "$change"
  if git diff --quiet && [ -z "$(git ls-files --others --exclude-standard)" ]; then bad "weakening '$name' changed nothing"; return; fi
  out=$(gate)
  if printf '%s\n' "$out" | grep -E "^\s+FAIL .*$want" >/dev/null; then ok "$name -> $want fails"; else bad "$name: $want did not fail"; fi
}
# dropline <pattern> <file>: delete every line matching the pattern.
dropline() { grep -v -- "$1" "$2" > "$2.tmp" && mv "$2.tmp" "$2"; }
# edit <sed -E script> <file>
edit() { sed -E "$1" "$2" > "$2.tmp" && mv "$2.tmp" "$2"; }
w_remove_assert() { dropline '^assert_lefthook_installed' lefthook.yml; }
w_remove_unset() { dropline '^unset ' .lefthook/rc.sh; }
w_pin_to_tag() { edit 's/(actions\/checkout)@[0-9a-f]{40}/\1@v7/' .github/workflows/ci.yml; }
w_drop_owner() { dropline '^/lefthook.yml ' .github/CODEOWNERS; }
w_just_env() { printf '        env:\n          JUST_DRY_RUN: "true"\n' >> .github/workflows/ci.yml; }
w_skippable_step() { printf '        if: always()\n' >> .github/workflows/ci.yml; }
w_clippy_gap() { dropline 'cargo clippy --manifest-path toolchain-tests' justfile; }
w_fast_gate() { edit 's/run: just check$/run: just check-fast/' .github/workflows/ci.yml; }
w_shadow_job() { printf 'name: Other\non: push\njobs:\n  check:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo hi\n' > .github/workflows/other.yml; }
weaken "remove assert_lefthook_installed" lefthook_fails_closed_and_loads_the_rc_file w_remove_assert
weaken "remove the unset line from rc.sh" the_rc_file_exports_the_hook_argument_and_drops_the_override_variables w_remove_unset
weaken "pin an action to a tag" ci_pins_every_action_to_a_full_commit_sha w_pin_to_tag
weaken "drop a CODEOWNERS entry" each_have_an_owner w_drop_owner
weaken "set JUST_DRY_RUN on the check step" no_workflow_sets_an_env_key_that_just_reads w_just_env
weaken "let a step be skipped" nothing_in_the_ci_job_lets_a_failing_step_pass_or_be_skipped w_skippable_step
weaken "stop clippy covering the test crate" every_cargo_recipe_also_covers_the_excluded_test_crate w_clippy_gap
weaken "make CI run the fast gate only" ci_runs_the_full_gate_not_the_fast_one w_fast_gate
weaken "add a workflow with a job named check" no_other_workflow_has_a_job_that_shadows_the_gate_job w_shadow_job
git checkout -q -- .
git clean -fdq -e toolchain-tests/target -e target

[ "$fail" -eq 0 ] || { echo "Rust scaffold test failed." >&2; exit 1; }
echo "Rust scaffold test passed."
```

- [ ] **Step 2: Lint it and the license header**

```bash
shellcheck -s bash scripts/test-rust-scaffold.sh
bash scripts/check-license-headers.sh scripts/test-rust-scaffold.sh
```

Expected: both silent, exit 0.

- [ ] **Step 3: Run it and see it fail for the right reason**

```bash
bash scripts/test-rust-scaffold.sh --quick
```

Expected: exit status 1. The output starts with `bash: .../scripts/apply.sh: No such file or directory` and `FAIL  apply.sh failed on a fresh repository`, then one `FAIL  copied <file>` for each of the sixteen files it expects, then it stops at `No such file or directory` for `.github/CODEOWNERS`. (Measured on a copy of the skill with `apply.sh` removed: 17 FAIL lines.) 
- [ ] **Step 4: Commit the failing test**

Nothing wires this script into `just check`, `lefthook.yml` or CI yet (`grep -n test-rust-scaffold Justfile .github/workflows/ci.yml lefthook.yml` prints nothing), so committing it now cannot turn a gate red, and it will not be lost to a working-tree reset.

```bash
git add scripts/test-rust-scaffold.sh
git commit -m "$(cat <<'EOF'
test: add the end-to-end test for the rust-toolchain-scaffold skill

Applies the scaffold to throwaway repositories, runs its own just check, and weakens
nine gates one at a time. It fails until apply.sh and the templates land in the next
commit, and nothing runs it yet.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `apply.sh` and the templates

**Files:**
- Create: `shared/devtools/skills/rust-toolchain-scaffold/scripts/apply.sh`
- Create under `shared/devtools/skills/rust-toolchain-scaffold/assets/template/` (every file ends in `.tmpl`): `justfile.tmpl`, `lefthook.yml.tmpl`, `.lefthook/rc.sh.tmpl`, `.lefthook/pre-push/check.sh.tmpl`, `deny.toml.tmpl`, `typos.toml.tmpl`, `.gitattributes.tmpl`, `.gitignore.tmpl`, `rust-toolchain.toml.tmpl`, `.github/CODEOWNERS.tmpl`, `.github/workflows/ci.yml.tmpl`, `docs/accepted-boundaries.md.tmpl`, `toolchain-tests/.gitignore.tmpl`, `toolchain-tests/Cargo.toml.tmpl`, `toolchain-tests/src/lib.rs.tmpl`, `toolchain-tests/tests/common/mod.rs.tmpl`, `toolchain-tests/tests/gate.rs.tmpl`, `toolchain-tests/tests/toolchain.rs.tmpl`
- Regenerated: the emitted copies of the skill.
- `scripts/test-rust-scaffold.sh` (Task 2) is already committed and is the test for this task.

**Interfaces:**
- Consumes: `scripts/test-rust-scaffold.sh` (Task 2) as the test.
- Produces: the scaffold. The names later tasks rely on: justfile recipes `setup fmt fmt-check clippy lint test test-toolchain deny spellcheck secrets secrets-staged secrets-message check-fast check`; gate test names in `gate.rs` that Task 2's weakenings expect (`lefthook_fails_closed_and_loads_the_rc_file`, `the_rc_file_exports_the_hook_argument_and_drops_the_override_variables`, `ci_pins_every_action_to_a_full_commit_sha`, `each_have_an_owner` as a substring, `no_workflow_sets_an_env_key_that_just_reads`, `nothing_in_the_ci_job_lets_a_failing_step_pass_or_be_skipped`, `every_cargo_recipe_also_covers_the_excluded_test_crate`, `ci_runs_the_full_gate_not_the_fast_one`, `no_other_workflow_has_a_job_that_shadows_the_gate_job`).

- [ ] **Step 1: Create `apply.sh`**

Create `shared/devtools/skills/rust-toolchain-scaffold/scripts/apply.sh`, then `chmod +x` it:

```bash
#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Copy the Rust toolchain scaffold into a repository without overwriting anything.
#
#   apply.sh <target-repo> <code-owner>     e.g. apply.sh . @octocat
#
# Every file under ../assets/template/ is stored as <name>.tmpl and is copied to
# <name>. A file that already exists in the target is never changed: it is listed
# as SKIPPED, marked identical to the template or different from it, so that
# running this again on a scaffolded repository doubles as a drift report. A
# justfile counts as existing under any spelling `just` accepts (justfile,
# Justfile, .justfile), because `just` refuses to run when two coexist.
# Nothing is committed. Run `git status` afterwards.
set -euo pipefail

target="${1:?usage: apply.sh <target-repo> <code-owner>}"
owner="${2:?usage: apply.sh <target-repo> <code-owner> (for example @octocat or @org/team)}"
case "$owner" in
  @*) ;;
  *) echo "apply.sh: the code owner must start with @ (got: $owner)" >&2; exit 2 ;;
esac
# The owner becomes sed replacement text, so allow only what a GitHub handle holds.
case "${owner#@}" in
  "" | *[!A-Za-z0-9_/-]* | /* | */ | */*/*)
    echo "apply.sh: '$owner' is not an @user or @org/team handle" >&2; exit 2 ;;
esac

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
template="$here/../assets/template"
[ -d "$template" ] || { echo "apply.sh: no template directory at $template" >&2; exit 2; }
target="$(cd -- "$target" && pwd -P)"
git -C "$target" rev-parse --git-dir >/dev/null 2>&1 || {
  echo "apply.sh: $target is not a git repository (run git init first)" >&2; exit 2; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# `find -iname` is in both GNU and BSD find; `just` accepts the name in any case.
existing_justfile="$(find "$target" -maxdepth 1 \( -iname justfile -o -iname .justfile \) -print | head -n 1)"

copied="" identical="" differs=""
while IFS= read -r -d '' file; do
  rel="${file#"$template"/}"
  case "$rel" in
    *.tmpl) ;;
    *) echo "apply.sh: template file without the .tmpl suffix: $rel" >&2; exit 1 ;;
  esac
  dest="${rel%.tmpl}"
  rendered="$tmp/rendered"
  if [ "$dest" = ".github/CODEOWNERS" ]; then
    sed "s|@OWNER|$owner|g" "$file" > "$rendered"
  else
    cp -p -- "$file" "$rendered"
  fi

  existing="$target/$dest"
  if [ "$dest" = "justfile" ] && [ -n "$existing_justfile" ]; then
    existing="$existing_justfile"
    dest="${existing#"$target"/}"
  fi
  if [ -e "$existing" ]; then
    if cmp -s -- "$rendered" "$existing"; then
      identical="$identical$dest"$'\n'
    else
      differs="$differs$dest"$'\n'
    fi
    continue
  fi
  mkdir -p -- "$(dirname -- "$target/$dest")"
  cp -p -- "$rendered" "$target/$dest"
  copied="$copied$dest"$'\n'
done < <(find "$template" -type f -print0 | sort -z)

lock_failed=0
if printf '%s' "$copied" | grep -qx 'toolchain-tests/Cargo.toml'; then
  if command -v cargo >/dev/null 2>&1; then
    # The lock is generated here, not shipped: a shipped one would be stale on arrival.
    (cd "$target" && cargo generate-lockfile --manifest-path toolchain-tests/Cargo.toml) || lock_failed=1
  else
    lock_failed=1
  fi
fi

count() { if [ -z "$1" ]; then echo 0; else printf '%s' "$1" | grep -c .; fi; }
list() { if [ -n "$1" ]; then printf '%s' "$1" | sed 's/^/  /'; fi; }

echo "COPIED ($(count "$copied")):"
list "$copied"
echo "SKIPPED, identical to the template ($(count "$identical")):"
list "$identical"
echo "SKIPPED, already present and DIFFERENT - merge by hand, keeping the user's content ($(count "$differs")):"
list "$differs"
echo "NOT TOUCHED: the agent instructions file, README.md - see assets/snippets/ in the skill."

if [ "$lock_failed" -ne 0 ]; then
  echo "ERROR: toolchain-tests/Cargo.lock was not generated. Run this, with network access, then commit it:" >&2
  echo "  cargo generate-lockfile --manifest-path toolchain-tests/Cargo.toml" >&2
fi
if printf '%s' "$differs" | grep -qx '.gitignore'; then
  echo "NOTE: .gitignore already existed. It needs /target, and toolchain-tests/.gitignore covers the test crate."
fi
if [ -f "$target/Cargo.toml" ] && grep -q '^\[workspace\]' "$target/Cargo.toml"; then
  echo "NOTE: the root Cargo.toml is a workspace. toolchain-tests/ has its own [workspace] table; if a members glob"
  echo "      could match it, add it to the root \`exclude\` list or cargo fails with 'multiple workspace roots'."
fi
if [ -n "$existing_justfile" ]; then
  case "$(basename -- "$existing_justfile")" in
    justfile | Justfile | .justfile) ;;
    *) echo "NOTE: the justfile is spelled $(basename -- "$existing_justfile"). just accepts that, but CODEOWNERS and the"
       echo "      tests know only justfile, Justfile and .justfile; rename it or extend both." ;;
  esac
fi
others="$(find "$target/.github/workflows" -maxdepth 1 -type f \( -name '*.yml' -o -name '*.yaml' \) ! -name ci.yml 2>/dev/null | sort || true)"
if [ -n "$others" ]; then
  echo "NOTE: other workflow files exist. The gate tests check every one of them for a JUST_* env key and for a job"
  echo "      named like the gate job (check):"
  printf '%s\n' "$others" | sed "s|^$target/|  |"
fi
[ "$lock_failed" -eq 0 ] || exit 1
```

- [ ] **Step 2: Create the task runner and the hooks**

Create `.../assets/template/justfile.tmpl`:

```just
# Project tasks. Run `just` to list them.

# Run recipe lines with bash in strict mode. `pipefail` matters for `secrets`:
# without it a failing `git log` piped into gitleaks would be reported as clean.
set shell := ["bash", "-euo", "pipefail", "-c"]

# List available recipes
default:
    @just --list

# Check the tools, then install the git hooks. The tools are not installed for you:
# see README.md (Development) for one install command per OS. lefthook is checked
# against the `min_version` in lefthook.yml, because `lefthook install` itself does
# not enforce it.
setup:
    @for tool in cargo gitleaks lefthook; do command -v "$tool" >/dev/null || { echo "$tool is not installed; see README.md (Development)" >&2; exit 1; }; done
    @floor="$(sed -n 's/^min_version:[[:space:]]*"\{0,1\}\([0-9][0-9.]*\)"\{0,1\}[[:space:]]*$/\1/p' lefthook.yml | tr -d '\r')"; test -n "$floor" || { echo "setup: lefthook.yml has no min_version" >&2; exit 1; }; have="$(lefthook version | tr -d '\r' | grep -oE '[0-9]+(\.[0-9]+)+' | head -n1)"; test -n "$have" || { echo "setup: cannot read the lefthook version" >&2; exit 1; }; [ "$(printf '%s\n%s\n' "$floor" "$have" | sort -V | head -n1)" = "$floor" ] || { echo "setup: lefthook $have is older than the $floor that lefthook.yml requires" >&2; exit 1; }
    lefthook install

# Apply rustfmt (the workspace and the test crate, which the workspace excludes)
fmt:
    cargo fmt --all
    cargo fmt --manifest-path toolchain-tests/Cargo.toml

# Check formatting without changing anything
fmt-check:
    cargo fmt --all -- --check
    cargo fmt --manifest-path toolchain-tests/Cargo.toml -- --check

# Clippy with warnings as errors, over the workspace and the excluded test crate
clippy:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo clippy --manifest-path toolchain-tests/Cargo.toml --all-targets -- -D warnings

# Formatting and clippy
lint: fmt-check clippy

# Run every test except the toolchain suite, which is slow and only matters when
# a tooling file changes. `--no-tests=warn` keeps a repository that has no tests
# yet from failing its first run.
test:
    cargo nextest run --workspace --no-tests=warn
    cargo nextest run --manifest-path toolchain-tests/Cargo.toml --test gate

# Run the toolchain suite: proves each gate in this file, the hooks and the
# config blocks what it should
test-toolchain:
    cargo nextest run --manifest-path toolchain-tests/Cargo.toml --test toolchain

# Dependency licences, bans and sources (the workspace and the test crate)
deny:
    cargo deny check
    cargo deny --manifest-path toolchain-tests/Cargo.toml check

# Spelling
spellcheck:
    typos

# Scan for secrets: staged changes, the history of the current commit (patches,
# then commit messages), and unstaged changes to tracked files. Untracked files
# are scanned once they are staged. Only the current commit's history is
# scanned, not other branches or stashes. `-m` makes git show merge commits as
# patches; without it, content added in a merge commit is never scanned. The diff
# filter skips deletions and unmerged, unknown and broken entries.
secrets: secrets-staged
    gitleaks git --log-opts="--full-history -m --diff-filter=uxdb" --no-banner --redact .
    git log --full-history --format=%B | gitleaks stdin --no-banner --redact
    gitleaks git --pre-commit --no-banner --redact .

# Scan only staged changes for secrets (used by the pre-commit hook)
secrets-staged:
    gitleaks git --staged --no-banner --redact .

# Scan the commit message file for secrets (used by the commit-msg hook).
# The path arrives in HOOK_ARG_1, exported by .lefthook/rc.sh, never as text
# pasted into a command: a path containing shell syntax is then not run.
# Without it there is nothing to scan, so the recipe fails rather than passes.
# It also fails when rc.sh is missing: lefthook skips a missing rc file
# silently, and HOOK_ARG_1 could then be a stale value from the caller's
# environment rather than the file git is about to commit. For the same
# reason the path must be a regular file inside the git directory, which is
# where git keeps every commit message file, linked worktrees included.
# CDPATH is unset for the comparison because with it set `cd` prints the
# directory it chose, which would end up in the captured path.
secrets-message:
    @test -f .lefthook/rc.sh || { echo "secrets-message: .lefthook/rc.sh is missing, so the message file cannot be identified" >&2; exit 1; }
    @test -n "${HOOK_ARG_1:-}" || { echo "secrets-message: HOOK_ARG_1 is not set; this recipe runs from the commit-msg hook" >&2; exit 1; }
    @test -f "$HOOK_ARG_1" && ! test -L "$HOOK_ARG_1" || { echo "secrets-message: HOOK_ARG_1 does not name a regular file" >&2; exit 1; }
    @unset CDPATH; dir="$(cd -- "$(dirname -- "$HOOK_ARG_1")" && pwd -P)"; common="$(cd -- "$(git rev-parse --git-common-dir)" && pwd -P)"; case "$dir/" in "$common"/*) ;; *) echo "secrets-message: HOOK_ARG_1 is outside the git directory, so it is not a commit message file" >&2; exit 1 ;; esac
    gitleaks stdin --no-banner --redact < "$HOOK_ARG_1"

# The fast gate: what the pre-push hook always runs
check-fast: lint test deny spellcheck secrets

# The full gate: everything CI runs
check: check-fast test-toolchain
```

Create `.../assets/template/lefthook.yml.tmpl`:

```yaml
# Git hooks. Install with `just setup`.
#
# These hooks are fast local feedback, not the gate: they check the working
# tree rather than the exact commit, so uncommitted or unstaged files can mask
# a problem. CI (.github/workflows/ci.yml) is authoritative.

# The one place the lefthook version floor is written. `just setup` reads it
# from here and a gate test fails if the two ever disagree. lefthook enforces it
# when it runs a hook; `lefthook install` does not.
min_version: 2.1.17

# Sourced by each generated hook before lefthook starts. It exports git's hook
# argument as HOOK_ARG_1 and drops the lefthook override variables; see the file.
rc: .lefthook/rc.sh

# Fail the git operation, rather than silently skipping the hooks, when the
# lefthook binary cannot be found.
assert_lefthook_installed: true

pre-commit:
  jobs:
    # Cargo works on crates, not files: these run the whole recipe whenever a
    # staged .rs file exists, and see the working tree, not the staged snapshot.
    - name: fmt
      run: just fmt-check
      glob: "*.rs"

    - name: clippy
      run: just clippy
      glob: "*.rs"

    - name: spellcheck
      run: just spellcheck

    - name: secrets
      run: just secrets-staged

commit-msg:
  jobs:
    # The message is scanned before the commit exists, so a secret typed into
    # it never enters the history. The message file's path is not passed here:
    # lefthook would paste `{1}` into a shell command unescaped. The recipe
    # reads it from HOOK_ARG_1, which .lefthook/rc.sh exports.
    - name: secrets
      run: just secrets-message

pre-push:
  jobs:
    # The fast gate, on every push. A script, not `run`: lefthook skips a
    # pre-push `run` job when the push changes no existing files (deletion-only
    # pushes, tags, empty commits). Script jobs are not skipped. The script is
    # .lefthook/pre-push/check.sh and runs `just check-fast`.
    - name: check
      script: check.sh
      runner: sh
      # Explicit args replace the remote name and URL that git passes to the
      # hook, which lefthook would otherwise append to the shell command unquoted.
      args: "--"

    # The toolchain suite is slower, so it runs only when the push touches a
    # file it tests. CI runs it on every change. The list must cover every file
    # the suite copies (TOOLCHAIN_FILES in toolchain-tests/tests/common/mod.rs);
    # a gate test fails if it does not.
    - name: toolchain-tests
      run: just test-toolchain
      glob:
        - ".gitattributes"
        - ".gitignore"
        - ".lefthook/**"
        - "deny.toml"
        - "justfile"
        - "lefthook.yml"
        - "rust-toolchain.toml"
        - "toolchain-tests/**"
        - "typos.toml"
```

Create `.../assets/template/.lefthook/rc.sh.tmpl`:

```sh
# Sourced by every git hook that lefthook generates, before lefthook starts
# (the `rc` setting in lefthook.yml). At that point `$1` is still the argument
# git gave the hook, untouched by any shell parsing.
#
# lefthook pastes hook arguments into the commands it runs without escaping
# them, so a path with shell syntax in it would be executed. Exporting the
# argument here lets a recipe read it from the environment instead.
#
# For commit-msg, `$1` is the path of the commit message file.
HOOK_ARG_1="${1:-}"
export HOOK_ARG_1

# lefthook lets these two override which binary and which configuration it
# uses, ahead of anything in this repository. A value left in the caller's
# environment would swap the hooks for a no-op, so drop both here.
unset LEFTHOOK_BIN LEFTHOOK_CONFIG
```

Create `.../assets/template/.lefthook/pre-push/check.sh.tmpl`:

```sh
#!/bin/sh
# Run by the lefthook pre-push hook (see lefthook.yml).
exec just check-fast
```

- [ ] **Step 3: Create the settings**

Create `.../assets/template/deny.toml.tmpl`:

```toml
[advisories]
version = 2

[licenses]
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Unlicense",
    "Zlib",
]
confidence-threshold = 0.8

# The toolchain-tests crate is private (publish = false) and carries no licence
# field; without this, cargo deny reports it as unlicensed.
[licenses.private]
ignore = true

[bans]
multiple-versions = "warn"
wildcards = "allow"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

Create `.../assets/template/typos.toml.tmpl`:

```toml
[files]
extend-exclude = [
    # release-please and similar tools generate changelogs with commit SHAs that
    # typos would read as prose.
    "**/CHANGELOG.md",
]
```

Create `.../assets/template/.gitattributes.tmpl`:

```text
# The hooks, the recipes and the tests that read them run on every platform, and a
# CRLF in a script or in a config they parse breaks them: `sh` reads `set -e\r`
# as an invalid option, and the gate tests compare lines exactly.
*.sh text eol=lf
justfile text eol=lf
Justfile text eol=lf
lefthook.yml text eol=lf
.lefthook/** text eol=lf
rust-toolchain.toml text eol=lf
deny.toml text eol=lf
typos.toml text eol=lf
toolchain-tests/** text eol=lf
.github/** text eol=lf
```

Create `.../assets/template/.gitignore.tmpl`:

```text
/target
```

Create `.../assets/template/rust-toolchain.toml.tmpl`:

```toml
[toolchain]
channel = "1.99.1"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 4: Create CODEOWNERS, CI and the boundaries document**

Create `.../assets/template/.github/CODEOWNERS.tmpl`:

```text
# Changes to the CI workflow, to the recipes and hooks it and the developers
# run, to the tool settings and pinned dependencies they rely on, to the tests
# that guard them, or to this file need the owner's review. This only takes
# effect once branch protection on `main` has "Require review from Code
# Owners" switched on. The three justfile spellings are listed because paths
# here are case-sensitive and a repository has only one of them.

/.github/workflows/ @OWNER
/.github/CODEOWNERS @OWNER
/toolchain-tests/ @OWNER
/justfile @OWNER
/Justfile @OWNER
/.justfile @OWNER
/lefthook.yml @OWNER
/.lefthook/ @OWNER
/Cargo.toml @OWNER
/Cargo.lock @OWNER
/rust-toolchain.toml @OWNER
/deny.toml @OWNER
/typos.toml @OWNER
/.gitattributes @OWNER
```

Create `.../assets/template/.github/workflows/ci.yml.tmpl`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    # `edited` reruns the check when a pull request's base branch is changed.
    types: [opened, synchronize, reopened, edited]

permissions:
  contents: read

# Tool versions are pinned so a new release cannot break an unchanged commit.
# Actions are pinned to a commit, with the release it corresponds to in a
# comment, because a tag can be moved. The Rust toolchain comes from
# rust-toolchain.toml. The names start with TOOL_ on purpose: the gate tests
# reject any env key starting with JUST_, which `just` itself reads.
env:
  TOOL_LEFTHOOK_VERSION: 2.1.17
  TOOL_GITLEAKS_VERSION: 8.30.1
  TOOL_JUST_VERSION: 1.58.0
  TOOL_NEXTEST_VERSION: 0.9.146
  TOOL_DENY_VERSION: 0.20.2
  TOOL_TYPOS_VERSION: 1.51.1

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          # Full history so gitleaks can scan every commit reachable from the checkout.
          fetch-depth: 0

      - name: Install the Rust toolchain pinned in rust-toolchain.toml
        run: rustup toolchain install

      - uses: taiki-e/install-action@fe3acb53a98b62c772db35de75ce0e4595e7ca8c # v2.90.8
        with:
          tool: just@${{ env.TOOL_JUST_VERSION }},cargo-nextest@${{ env.TOOL_NEXTEST_VERSION }},cargo-deny@${{ env.TOOL_DENY_VERSION }},typos@${{ env.TOOL_TYPOS_VERSION }}

      - name: Install lefthook
        run: |
          base="https://github.com/evilmartians/lefthook/releases/download/v${TOOL_LEFTHOOK_VERSION}"
          bin="lefthook_${TOOL_LEFTHOOK_VERSION}_Linux_x86_64"
          curl -fsSLO "${base}/${bin}"
          curl -fsSLO "${base}/lefthook_checksums.txt"
          grep " ${bin}\$" lefthook_checksums.txt | sha256sum -c -
          mkdir -p "${RUNNER_TEMP}/lefthook"
          install -m 0755 "${bin}" "${RUNNER_TEMP}/lefthook/lefthook"
          echo "${RUNNER_TEMP}/lefthook" >> "${GITHUB_PATH}"
          rm "${bin}" lefthook_checksums.txt

      - name: Install gitleaks
        run: |
          base="https://github.com/gitleaks/gitleaks/releases/download/v${TOOL_GITLEAKS_VERSION}"
          tarball="gitleaks_${TOOL_GITLEAKS_VERSION}_linux_x64.tar.gz"
          curl -fsSLO "${base}/${tarball}"
          curl -fsSLO "${base}/gitleaks_${TOOL_GITLEAKS_VERSION}_checksums.txt"
          grep " ${tarball}\$" "gitleaks_${TOOL_GITLEAKS_VERSION}_checksums.txt" | sha256sum -c -
          mkdir -p "${RUNNER_TEMP}/gitleaks"
          tar -xzf "${tarball}" -C "${RUNNER_TEMP}/gitleaks" gitleaks
          echo "${RUNNER_TEMP}/gitleaks" >> "${GITHUB_PATH}"
          rm "${tarball}" "gitleaks_${TOOL_GITLEAKS_VERSION}_checksums.txt"

      - name: Check
        run: just check
```

Create `.../assets/template/docs/accepted-boundaries.md.tmpl`:

```md
# Accepted boundaries

Behaviours the tests deliberately do not cover, each accepted by the owner, with what compensates for
it. A test audit does not re-raise these while the compensation still exists. Remove an entry when it
is closed. The dates and names below are the scaffold's examples: keep only what you accept, and put
your own name and date on it.

## Secret scan

- **Binary files, annotated tag messages, submodule contents, and a secret split across several
  commits are not scanned.** Compensation: none in this repository; accepted as limits of the scan.
  Anchor: `justfile`, recipe `secrets`.
- **`gitleaks:allow` comments are honoured.** Compensation: each use is visible in review. Anchor:
  `justfile`, recipe `secrets`.
- **Further `git log` options that would narrow the history scan are not each tested.** The suite pins
  depth, merges, commit-message bodies and unstaged changes; the set of possible narrowing options is
  open-ended. Compensation: a change to the `secrets` recipe is a one-line diff in review. Anchor:
  `toolchain-tests/tests/toolchain.rs`.

## Hooks

- **The local hooks check the working tree, not the exact commit, and can be skipped** (`--no-verify`,
  `LEFTHOOK=0`). Compensation: CI runs `just check` on every pull request and push to `main`. Anchor:
  `.github/workflows/ci.yml`, step `Check`.
- **A lefthook on `PATH` is trusted.** The hooks find `lefthook` on `PATH`, with `min_version` as a floor
  and `assert_lefthook_installed` as the fail-closed setting; a spoofed `PATH`, an older lefthook that
  does not know `min_version`, and a local deletion of `.lefthook/rc.sh` (which removes the unset of
  `LEFTHOOK_BIN` and `LEFTHOOK_CONFIG`) are not blocked. Compensation: the same CI run. Anchor:
  `lefthook.yml`.
- **Hooks are not installed until `just setup` runs, and a global `core.hooksPath` stops
  `lefthook install`.** Compensation: the same CI run. Anchor: `justfile`, recipe `setup`.
- **The conditional `toolchain-tests` pre-push job is skipped for a deletion-only tooling commit, for
  uncommitted tooling edits, and when pushing a ref other than the checked-out one.** Compensation: the
  same CI run. Anchor: `lefthook.yml`, job `toolchain-tests`.

## Toolchain suite

- **A config file that a tool reads automatically is not covered until it is added to
  `TOOLCHAIN_FILES` and the hook's glob.** Compensation: documented in the agent instructions file,
  section Tests. Anchor: `toolchain-tests/tests/common/mod.rs`, `TOOLCHAIN_FILES`.
- **The CI workflow is checked by reading it, not by running it.** Compensation: it runs on every pull
  request. Anchor: `toolchain-tests/tests/gate.rs`.
- **Workflows other than `ci.yml` are checked only for a `JUST_*` env key and for a job named `check`.**
  Compensation: review of workflow changes through CODEOWNERS. Anchor: `toolchain-tests/tests/gate.rs`.
- **Whether GitHub lets a same-named job in another workflow satisfy a required check is not tested.**
  Compensation: the shadowing rule forbids the collision. Anchor: `toolchain-tests/tests/gate.rs`.
- **The tests that build a `PATH` without lefthook run on Unix only.** Compensation: the Windows run of
  the end-to-end job in the repository that ships this scaffold. Anchor:
  `toolchain-tests/tests/toolchain.rs`, module `missing_runner`.
```

- [ ] **Step 5: Create the test crate**

Create `.../assets/template/toolchain-tests/.gitignore.tmpl`:

```text
/target/
```

Create `.../assets/template/toolchain-tests/Cargo.toml.tmpl`:

```toml
# The scaffold's tests. This crate is deliberately outside the workspace of the
# repository it guards: the empty [workspace] table below keeps it out of the
# root Cargo.toml and Cargo.lock, so adding it changes neither. The justfile runs
# it with --manifest-path.
[package]
name = "toolchain-tests"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dev-dependencies]
yaml-rust2 = "0.13"
```

Create `.../assets/template/toolchain-tests/src/lib.rs.tmpl`:

```rust
//! Tests for the repository's own tooling: the gate tests in `tests/gate.rs` and
//! the toolchain tests in `tests/toolchain.rs`. There is no library code here.
```

Create `.../assets/template/toolchain-tests/tests/common/mod.rs.tmpl`:

```rust
//! Helpers shared by the gate and toolchain tests.
//!
//! Each toolchain test builds a throwaway git repository holding a copy of the
//! tooling files, then runs the real recipes and hooks in it to prove that a
//! gate blocks what it should.

// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::collections::hash_map::RandomState;
use std::ffi::OsStr;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

/// The line gitleaks prints when it finds something ("no leaks found" has no colon).
pub const LEAK: &str = "leaks found:";

/// The tooling under test, copied into every throwaway repository, besides the
/// justfile (whose spelling varies, see `justfile_name`). Neither `src/` nor
/// `tests/` of the repository is copied: the result must not depend on whatever
/// product code or scratch files are in the working tree. `rust-toolchain.toml`
/// is not copied either, so that a throwaway repository builds with the
/// toolchain the developer has installed instead of triggering a download.
pub const TOOLCHAIN_FILES: &[&str] = &[
    ".gitattributes",
    ".gitignore",
    ".lefthook",
    "deny.toml",
    "lefthook.yml",
    "typos.toml",
];

/// Environment variables dropped by prefix, upper-cased. They change what the
/// tools under test do, so a developer's shell settings must not alter a result.
const DROPPED_PREFIXES: &[&str] = &[
    "GIT_",
    "LEFTHOOK",
    "GITLEAKS_",
    "JUST_",
    // Exported by .lefthook/rc.sh in the hook this suite may be running under.
    "HOOK_ARG_",
    // Changes what `cd` prints; a test sets it on purpose where that matters.
    "CDPATH",
    "NEXTEST_",
    "TYPOS_",
    "CARGO_BUILD_",
    "CARGO_PROFILE_",
];

/// Environment variables dropped by exact name, upper-cased. `CARGO_HOME`,
/// `RUSTUP_HOME` and the registry and network settings stay: a mirror or
/// credentials configured there are part of a working environment.
const DROPPED_NAMES: &[&str] = &[
    "BASH_ENV",
    "ENV",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_TARGET_DIR",
    "RUSTC_WRAPPER",
    "RUSTUP_TOOLCHAIN",
];

const FIXTURE_MANIFEST: &str =
    "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\npublish = false\n";
const FIXTURE_LIB: &str =
    "/// Adds two numbers.\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
const FIXTURE_TEST: &str =
    "use fixture::add;\n\n#[test]\nfn adds() {\n    assert_eq!(add(1, 2), 3);\n}\n";
const STUB_MANIFEST: &str = "[package]\nname = \"toolchain-tests\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n";
const STUB_TEST: &str = "#[test]\nfn placeholder() {}\n";

/// The repository the tests guard: the parent of this crate.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("toolchain-tests has a parent directory")
        .to_path_buf()
}

/// The spelling of the justfile this repository uses. `just` refuses to run
/// when two spellings coexist, so a repository has exactly one.
pub fn justfile_name() -> &'static str {
    ["justfile", "Justfile", ".justfile"]
        .into_iter()
        .find(|name| repo_root().join(name).is_file())
        .expect("the repository has no justfile")
}

/// A file of the repository, as text. Panics with the path if it cannot be read.
pub fn read(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

/// A directory removed when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "toolchain-suite-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create a temporary directory");
        TempDir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// An empty file, used wherever git or a tool needs a config file that says nothing.
/// (A file rather than /dev/null, which a native Windows git does not understand.)
fn empty_file() -> &'static Path {
    static EMPTY: OnceLock<PathBuf> = OnceLock::new();
    EMPTY.get_or_init(|| {
        let path =
            std::env::temp_dir().join(format!("toolchain-suite-empty-{}", std::process::id()));
        fs::write(&path, "").expect("create an empty file");
        path
    })
}

/// A command with the clean environment every spawned process gets. Git exports
/// `GIT_DIR` and friends to hooks, and this suite can run inside the pre-push
/// hook: left in place, those variables would point the throwaway repositories'
/// git commands at the real repository. Settings for lefthook, gitleaks, just,
/// cargo and bash are dropped too, so that for example `LEFTHOOK=0` or
/// `JUST_DRY_RUN` in the caller's shell cannot switch off what is being tested.
/// Git's config files are replaced by an empty one: stripping `GIT_*` variables
/// does not isolate the files, and a global `core.hooksPath` makes
/// `lefthook install` fail.
pub fn command<S: AsRef<OsStr>>(dir: &Path, program: S) -> Command {
    let mut cmd = Command::new(program);
    cmd.current_dir(dir).stdin(Stdio::null()).env_clear();
    for (key, value) in std::env::vars() {
        let upper = key.to_ascii_uppercase();
        let dropped = DROPPED_NAMES.contains(&upper.as_str())
            || DROPPED_PREFIXES
                .iter()
                .any(|prefix| upper.starts_with(prefix));
        if !dropped {
            cmd.env(key, value);
        }
    }
    let empty = empty_file();
    cmd.env("GIT_CONFIG_GLOBAL", empty)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_COUNT", "2")
        .env("GIT_CONFIG_KEY_0", "core.excludesFile")
        .env("GIT_CONFIG_VALUE_0", empty)
        .env("GIT_CONFIG_KEY_1", "core.attributesFile")
        .env("GIT_CONFIG_VALUE_1", empty)
        .env("GIT_AUTHOR_NAME", "Toolchain Test")
        .env("GIT_AUTHOR_EMAIL", "toolchain-test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Toolchain Test")
        .env("GIT_COMMITTER_EMAIL", "toolchain-test@example.invalid");
    cmd
}

/// What a command printed and how it ended.
pub struct Ran {
    pub code: i32,
    pub output: String,
}

fn finish(mut cmd: Command) -> Ran {
    let out = cmd.output().expect("spawn the command");
    Ran {
        code: out.status.code().unwrap_or(-1),
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    }
}

/// Run a command in `dir`; never panics on a failing exit.
pub fn run<S: AsRef<OsStr>>(dir: &Path, program: S, args: &[&str]) -> Ran {
    let mut cmd = command(dir, program);
    cmd.args(args);
    finish(cmd)
}

/// Like `run`, with extra environment variables set after the clean environment.
pub fn run_env<S: AsRef<OsStr>>(
    dir: &Path,
    program: S,
    args: &[&str],
    env: &[(&str, &OsStr)],
) -> Ran {
    let mut cmd = command(dir, program);
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    finish(cmd)
}

/// Run a setup command that has to succeed; panic with its output if it does not.
pub fn must<S: AsRef<OsStr>>(dir: &Path, program: S, args: &[&str]) -> Ran {
    let ran = run(dir, &program, args);
    assert!(
        ran.code == 0,
        "setup command failed ({}): {} {}\n{}",
        ran.code,
        program.as_ref().to_string_lossy(),
        args.join(" "),
        ran.output
    );
    ran
}

/// Write a file under `dir`, creating its parent directories.
pub fn write(dir: &Path, relative: &str, content: &str) {
    let path = dir.join(relative);
    fs::create_dir_all(path.parent().expect("a file has a parent")).expect("create directories");
    fs::write(&path, content).unwrap_or_else(|error| panic!("write {relative}: {error}"));
}

/// Append to a file under `dir`.
pub fn append(dir: &Path, relative: &str, content: &str) {
    let path = dir.join(relative);
    let mut text = fs::read_to_string(&path).unwrap_or_default();
    text.push_str(content);
    fs::write(&path, text).unwrap_or_else(|error| panic!("append {relative}: {error}"));
}

/// Write an executable shell script (the mode bit only matters on Unix).
pub fn write_script(path: &Path, body: &str) {
    fs::write(path, body).expect("write a script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod a script");
    }
}

/// Copy a file or a whole directory.
pub fn copy(from: &Path, to: &Path) {
    if from.is_dir() {
        fs::create_dir_all(to).expect("create a directory");
        for entry in fs::read_dir(from).expect("read a directory") {
            let entry = entry.expect("a directory entry");
            copy(&entry.path(), &to.join(entry.file_name()));
        }
    } else {
        fs::create_dir_all(to.parent().expect("a file has a parent")).expect("create directories");
        fs::copy(from, to).unwrap_or_else(|error| panic!("copy {}: {error}", from.display()));
    }
}

/// Find a program on `PATH`.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|dir| [dir.join(name), dir.join(format!("{name}.exe"))])
        .find(|candidate| candidate.is_file())
}

/// A new git repository on branch `main` holding a copy of the tooling files, a
/// small fixture crate and a stub of this test crate (the justfile's recipes
/// name it), with one commit and a clean working tree.
pub fn make_repo() -> TempDir {
    let dir = TempDir::new();
    build_repo(dir.path());
    dir
}

/// Build that repository in an existing, empty directory.
pub fn build_repo(dir: &Path) {
    let root = repo_root();
    let name = justfile_name();
    copy(&root.join(name), &dir.join(name));
    for file in TOOLCHAIN_FILES {
        let from = root.join(file);
        if from.exists() {
            copy(&from, &dir.join(file));
        }
    }
    write(dir, "Cargo.toml", FIXTURE_MANIFEST);
    write(dir, "src/lib.rs", FIXTURE_LIB);
    write(dir, "tests/add.rs", FIXTURE_TEST);
    write(dir, "toolchain-tests/Cargo.toml", STUB_MANIFEST);
    write(
        dir,
        "toolchain-tests/src/lib.rs",
        "//! Stub of the test crate.\n",
    );
    write(dir, "toolchain-tests/tests/gate.rs", STUB_TEST);
    write(dir, "toolchain-tests/tests/toolchain.rs", STUB_TEST);
    write(dir, "toolchain-tests/.gitignore", "/target/\n");
    // The copied .gitignore may not ignore build output.
    append(dir, ".gitignore", "\n/target\n");
    must(dir, "git", &["init", "--quiet", "--initial-branch=main"]);
    commit_all(dir, "initial");
}

/// How many commits `HEAD` has.
pub fn commit_count(dir: &Path) -> u32 {
    must(dir, "git", &["rev-list", "--count", "HEAD"])
        .output
        .trim()
        .parse()
        .expect("a commit count")
}

/// Stage everything and commit it, bypassing hooks.
pub fn commit_all(dir: &Path, message: &str) {
    must(dir, "git", &["add", "--all"]);
    must(
        dir,
        "git",
        &["commit", "--quiet", "--no-verify", "-m", message],
    );
}

/// Give `dir` a bare remote named `origin` and push `main` to it, bypassing hooks.
pub fn add_remote(dir: &Path) -> TempDir {
    let remote = TempDir::new();
    must(remote.path(), "git", &["init", "--quiet", "--bare"]);
    let url = remote.path().to_string_lossy().into_owned();
    must(dir, "git", &["remote", "add", "origin", &url]);
    must(
        dir,
        "git",
        &[
            "push",
            "--quiet",
            "--no-verify",
            "--set-upstream",
            "origin",
            "main",
        ],
    );
    remote
}

/// Install the git hooks in `dir` with the real lefthook.
pub fn install_hooks(dir: &Path) {
    must(dir, "lefthook", &["install"]);
}

/// A string shaped like a GitHub personal access token, generated at run time so
/// that no token-shaped literal is ever committed to the repository.
pub fn fake_token() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let state = RandomState::new();
    let mut body = String::new();
    let mut counter: u64 = 0;
    while body.len() < 36 {
        let mut hasher = state.build_hasher();
        hasher.write_u64(counter);
        counter += 1;
        for byte in hasher.finish().to_le_bytes() {
            // Six bits of each byte give a number from 0 to 63, an even draw from 64 characters.
            if let Some(character) = ALPHABET.get(usize::from(byte & 63)) {
                if body.len() < 36 {
                    body.push(char::from(*character));
                }
            }
        }
    }
    format!("ghp_{body}")
}
```

Create `.../assets/template/toolchain-tests/tests/gate.rs.tmpl`:

```rust
//! Gate tests: fast tests that read the CI workflow, CODEOWNERS and the tool
//! settings and pin the lines that make them gates. The workflow cannot be run
//! locally, so these read the file: they catch an edit that quietly weakens it,
//! not a fault in GitHub Actions itself.
//!
//! They live apart from the toolchain tests on purpose. They are fast, so `just
//! test` runs them and therefore so does `check-fast`: a pull request that
//! downgrades the workflow to the fast gate still runs them, and fails.

mod common;

use common::{justfile_name, read, repo_root, run, TOOLCHAIN_FILES};
use std::collections::HashMap;
use std::fs;
use yaml_rust2::{Yaml, YamlLoader};

// ---- reading files -------------------------------------------------------

fn load(text: &str) -> Yaml {
    YamlLoader::load_from_str(text)
        .expect("valid YAML")
        .into_iter()
        .next()
        .expect("one YAML document")
}

fn ci() -> Yaml {
    load(&read(".github/workflows/ci.yml"))
}

fn steps(workflow: &Yaml) -> Vec<&Yaml> {
    workflow["jobs"]["check"]["steps"]
        .as_vec()
        .expect("ci.yml has a `check` job with steps")
        .iter()
        .collect()
}

fn step_using<'a>(all: &[&'a Yaml], action: &str) -> &'a Yaml {
    let prefix = format!("{action}@");
    all.iter()
        .copied()
        .find(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with(&prefix))
        })
        .unwrap_or_else(|| panic!("ci.yml has no {action} step"))
}

fn step_named<'a>(all: &[&'a Yaml], name: &str) -> &'a Yaml {
    all.iter()
        .copied()
        .find(|step| step["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("ci.yml has no step named {name}"))
}

fn script(step: &Yaml) -> &str {
    step["run"].as_str().unwrap_or("")
}

/// Every workflow file, as (file name, parsed document), in name order.
fn workflows() -> Vec<(String, Yaml)> {
    let directory = repo_root().join(".github/workflows");
    let mut found: Vec<(String, Yaml)> = fs::read_dir(&directory)
        .expect("the repository has .github/workflows")
        .map(|entry| {
            entry
                .expect("a directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".yml") || name.ends_with(".yaml"))
        .map(|name| {
            let text = read(&format!(".github/workflows/{name}"));
            (name, load(&text))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// The names of every `env:` entry anywhere in a document: workflow, job and step level.
fn env_keys(node: &Yaml, found: &mut Vec<String>) {
    match node {
        Yaml::Hash(map) => {
            for (key, value) in map {
                if key.as_str() == Some("env") {
                    if let Yaml::Hash(env) = value {
                        found.extend(env.keys().filter_map(Yaml::as_str).map(str::to_owned));
                    }
                }
                env_keys(value, found);
            }
        }
        Yaml::Array(items) => items.iter().for_each(|item| env_keys(item, found)),
        _ => {}
    }
}

// ---- the CI workflow (ci.yml only: the strict rules) ----------------------

#[test]
fn ci_runs_on_pushes_to_main_and_on_pull_requests_including_a_changed_base() {
    let workflow = ci();
    let branches = workflow["on"]["push"]["branches"]
        .as_vec()
        .expect("push branches");
    assert!(branches
        .iter()
        .any(|branch| branch.as_str() == Some("main")));
    let types: Vec<&str> = workflow["on"]["pull_request"]["types"]
        .as_vec()
        .expect("pull_request types")
        .iter()
        .filter_map(Yaml::as_str)
        .collect();
    for wanted in ["opened", "synchronize", "reopened", "edited"] {
        assert!(
            types.contains(&wanted),
            "pull_request does not run on {wanted}"
        );
    }
}

#[test]
fn ci_has_read_only_access_to_the_repository() {
    let workflow = ci();
    let permissions = workflow["permissions"]
        .as_hash()
        .expect("workflow permissions");
    assert_eq!(permissions.len(), 1);
    assert_eq!(workflow["permissions"]["contents"].as_str(), Some("read"));
}

#[test]
fn ci_checks_out_the_full_history_which_the_secret_scan_needs() {
    let workflow = ci();
    let checkout = step_using(&steps(&workflow), "actions/checkout");
    assert_eq!(checkout["with"]["fetch-depth"].as_i64(), Some(0));
}

#[test]
fn ci_runs_the_full_gate_not_the_fast_one() {
    let workflow = ci();
    let runs: Vec<&str> = steps(&workflow)
        .iter()
        .map(|step| script(step).trim())
        .collect();
    assert!(
        runs.contains(&"just check"),
        "no step runs exactly `just check`"
    );
    assert!(!runs.contains(&"just check-fast"));
}

#[test]
fn ci_pins_every_action_to_a_full_commit_sha() {
    // A version tag can be moved to different code; a commit SHA cannot.
    let workflow = ci();
    let used: Vec<&str> = steps(&workflow)
        .iter()
        .filter_map(|step| step["uses"].as_str())
        .collect();
    assert!(!used.is_empty());
    for uses in used {
        let (_, sha) = uses
            .rsplit_once('@')
            .unwrap_or_else(|| panic!("{uses} has no @ref"));
        let pinned = sha.len() == 40 && sha.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'));
        assert!(pinned, "{uses} is not pinned to a 40-hex commit SHA");
    }
}

#[test]
fn ci_pins_the_version_of_every_tool_it_installs() {
    let workflow = ci();
    let all = steps(&workflow);
    let names = [
        "TOOL_LEFTHOOK_VERSION",
        "TOOL_GITLEAKS_VERSION",
        "TOOL_JUST_VERSION",
        "TOOL_NEXTEST_VERSION",
        "TOOL_DENY_VERSION",
        "TOOL_TYPOS_VERSION",
    ];
    for name in names {
        let value = workflow["env"][name]
            .as_str()
            .unwrap_or_else(|| panic!("env {name} is not set"));
        let parts: Vec<&str> = value.split('.').collect();
        let numeric = parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
        assert!(numeric, "{name} is {value}, not an exact x.y.z version");
    }
    let tool = step_using(&all, "taiki-e/install-action")["with"]["tool"]
        .as_str()
        .expect("install-action tool list");
    for (name, tool_name) in [
        ("TOOL_JUST_VERSION", "just"),
        ("TOOL_NEXTEST_VERSION", "cargo-nextest"),
        ("TOOL_DENY_VERSION", "cargo-deny"),
        ("TOOL_TYPOS_VERSION", "typos"),
    ] {
        let expected = format!("{tool_name}@${{{{ env.{name} }}}}");
        assert!(
            tool.contains(&expected),
            "install-action does not install {expected}"
        );
    }
}

#[test]
fn ci_downloads_each_binary_at_its_pinned_version_and_verifies_its_checksum() {
    let workflow = ci();
    let all = steps(&workflow);
    for (name, variable) in [
        ("Install lefthook", "TOOL_LEFTHOOK_VERSION"),
        ("Install gitleaks", "TOOL_GITLEAKS_VERSION"),
    ] {
        let text = script(step_named(&all, name));
        assert!(
            text.contains(&format!("${{{variable}}}")),
            "{name} does not use the pinned {variable}"
        );
        assert!(
            text.contains("sha256sum -c"),
            "{name} does not verify a checksum"
        );
        assert!(!text.contains("latest"), "{name} downloads `latest`");
    }
}

#[test]
fn nothing_in_the_ci_job_lets_a_failing_step_pass_or_be_skipped() {
    let workflow = ci();
    for key in ["defaults", "container", "services"] {
        assert!(workflow[key].is_badvalue(), "the workflow sets `{key}`");
        assert!(
            workflow["jobs"]["check"][key].is_badvalue(),
            "the check job sets `{key}`"
        );
    }
    assert!(workflow["jobs"]["check"]["continue-on-error"].is_badvalue());
    assert!(workflow["jobs"]["check"]["if"].is_badvalue());
    for step in steps(&workflow) {
        for key in ["continue-on-error", "if", "shell"] {
            assert!(step[key].is_badvalue(), "a step sets `{key}`: {step:?}");
        }
    }
}

#[test]
fn the_ci_job_does_not_widen_the_workflows_read_only_access() {
    let workflow = ci();
    for (name, job) in workflow["jobs"].as_hash().expect("jobs") {
        assert!(
            job["permissions"].is_badvalue(),
            "job {name:?} sets its own permissions"
        );
    }
}

#[test]
fn no_ci_script_swallows_a_failing_command() {
    // `|| true` after the checksum check, or `set +e`, would let a tampered
    // download through. No script here needs `||` or `set +...` at all; a future
    // one that does should change this test.
    let workflow = ci();
    let all = steps(&workflow);
    for step in &all {
        let text = script(step);
        assert!(!text.contains("||"), "a script contains ||: {text}");
        assert!(!text.contains("set +"), "a script contains `set +`: {text}");
        assert!(
            !text.contains("; true"),
            "a script contains `; true`: {text}"
        );
    }
    for name in ["Install lefthook", "Install gitleaks"] {
        let checks: Vec<&str> = script(step_named(&all, name))
            .lines()
            .map(str::trim)
            .filter(|line| line.contains("sha256sum -c"))
            .collect();
        assert_eq!(
            checks.len(),
            1,
            "{name} must have exactly one checksum line"
        );
        assert!(
            checks[0].ends_with("| sha256sum -c -"),
            "{name}: {}",
            checks[0]
        );
        assert!(!checks[0].starts_with('#'));
    }
}

// ---- every workflow file: the two shadowing rules --------------------------

#[test]
fn no_workflow_sets_an_env_key_that_just_reads() {
    // `JUST_DRY_RUN=true` makes `just check` print the recipe and exit 0 without
    // running it, which would turn the gate into a green no-op. The rule is every
    // key starting with JUST_ (just reads only those), so a variable added to a
    // future just release is covered too. The pins in ci.yml are TOOL_*.
    for (file, workflow) in workflows() {
        let mut keys = Vec::new();
        env_keys(&workflow, &mut keys);
        for key in keys {
            assert!(
                !key.to_ascii_uppercase().starts_with("JUST_"),
                "{file} sets env {key}"
            );
        }
    }
}

#[test]
fn no_other_workflow_has_a_job_that_shadows_the_gate_job() {
    // A required status check is matched by the check's name. A second workflow
    // with a job called `check` could report green under the gate's name.
    let gate = ci();
    let gate_job = gate["jobs"]["check"]["name"]
        .as_str()
        .unwrap_or("check")
        .to_owned();
    for (file, workflow) in workflows() {
        if file == "ci.yml" {
            continue;
        }
        for (id, job) in workflow["jobs"].as_hash().expect("jobs") {
            let id = id.as_str().unwrap_or_default();
            let name = job["name"].as_str().unwrap_or(id);
            assert!(
                id != "check" && name != gate_job,
                "{file} has a job {id:?} named {name:?}, shadowing the gate"
            );
        }
    }
}

// ---- CODEOWNERS ------------------------------------------------------------

/// The owned paths in CODEOWNERS in file order, each with its owners.
fn code_owners() -> Vec<(String, Vec<String>)> {
    read(".github/CODEOWNERS")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split_whitespace();
            let path = parts.next().expect("a path").to_owned();
            (path, parts.map(str::to_owned).collect())
        })
        .collect()
}

/// `*` and `?` inside one path segment.
fn glob(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|skip| glob(rest, &text[skip..])),
        Some(('?', rest)) => !text.is_empty() && glob(rest, &text[1..]),
        Some((c, rest)) => text.first() == Some(c) && glob(rest, &text[1..]),
    }
}

fn segments_match(pattern: &[&str], file: &[&str], directory: bool) -> bool {
    match pattern.split_first() {
        // The pattern is used up: what is left of the path is below it. A
        // directory pattern owns only what is inside it, a file pattern also itself.
        None => !directory || !file.is_empty(),
        Some((&"**", rest)) => {
            (0..=file.len()).any(|skip| segments_match(rest, &file[skip..], directory))
        }
        Some((segment, rest)) => match file.split_first() {
            Some((name, below)) => {
                let segment: Vec<char> = segment.chars().collect();
                let name: Vec<char> = name.chars().collect();
                glob(&segment, &name) && segments_match(rest, below, directory)
            }
            None => false,
        },
    }
}

/// Whether a CODEOWNERS pattern applies to a repository-relative file path.
fn owns(pattern: &str, file: &str) -> bool {
    let directory = pattern.ends_with('/');
    let body = pattern.trim_start_matches('/').trim_end_matches('/');
    let anchored = pattern.starts_with('/') || body.contains('/');
    let pattern_segments: Vec<&str> = body.split('/').collect();
    let file_segments: Vec<&str> = file.split('/').collect();
    let starts: Vec<usize> = if anchored {
        vec![0]
    } else {
        (0..file_segments.len()).collect()
    };
    starts
        .into_iter()
        .any(|start| segments_match(&pattern_segments, &file_segments[start..], directory))
}

#[test]
fn the_ci_workflow_the_recipes_the_hooks_the_tool_settings_their_tests_and_codeowners_each_have_an_owner(
) {
    let owned: HashMap<String, Vec<String>> = code_owners().into_iter().collect();
    for path in [
        "/.github/workflows/",
        "/.github/CODEOWNERS",
        "/toolchain-tests/",
        "/justfile",
        "/Justfile",
        "/.justfile",
        "/lefthook.yml",
        "/.lefthook/",
        "/Cargo.toml",
        "/Cargo.lock",
        "/rust-toolchain.toml",
        "/deny.toml",
        "/typos.toml",
        "/.gitattributes",
    ] {
        let owners = owned
            .get(path)
            .unwrap_or_else(|| panic!("CODEOWNERS has no entry for {path}"));
        assert!(!owners.is_empty(), "{path} has no owner");
        for owner in owners {
            let handle = owner.strip_prefix('@').unwrap_or("");
            let valid = !handle.is_empty()
                && handle.matches('/').count() <= 1
                && handle
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '/'));
            assert!(valid, "{path}: {owner} is not an @user or @org/team handle");
        }
    }
}

#[test]
fn no_later_codeowners_rule_takes_a_protected_file_away_from_its_entry() {
    // GitHub uses the LAST matching rule, so a `*` line added at the end would
    // replace or clear the owners of everything above it.
    let rules: Vec<String> = code_owners().into_iter().map(|(path, _)| path).collect();
    for (file, entry) in [
        (".github/workflows/ci.yml", "/.github/workflows/"),
        (".github/CODEOWNERS", "/.github/CODEOWNERS"),
        ("toolchain-tests/tests/gate.rs", "/toolchain-tests/"),
        ("justfile", "/justfile"),
        ("Justfile", "/Justfile"),
        (".justfile", "/.justfile"),
        ("lefthook.yml", "/lefthook.yml"),
        (".lefthook/rc.sh", "/.lefthook/"),
        (".lefthook/pre-push/check.sh", "/.lefthook/"),
        ("Cargo.toml", "/Cargo.toml"),
        ("Cargo.lock", "/Cargo.lock"),
        ("rust-toolchain.toml", "/rust-toolchain.toml"),
        ("deny.toml", "/deny.toml"),
        ("typos.toml", "/typos.toml"),
        (".gitattributes", "/.gitattributes"),
    ] {
        let decided_by = rules.iter().rfind(|rule| owns(rule, file));
        assert_eq!(
            decided_by.map(String::as_str),
            Some(entry),
            "{file} is decided by the wrong rule"
        );
    }
}

#[test]
fn the_codeowners_matcher_follows_github_for_the_cases_these_tests_rely_on() {
    assert!(owns("/justfile", "justfile"));
    assert!(!owns("/justfile", "sub/justfile"));
    assert!(owns("justfile", "sub/justfile"));
    assert!(owns("/.lefthook/", ".lefthook/rc.sh"));
    assert!(!owns("/.lefthook/", ".lefthook"));
    assert!(owns("**/justfile", "justfile"));
    assert!(owns("/docs/*.md", "docs/a.md"));
    assert!(!owns("/docs/*.md", "docs/sub/a.md"));
    assert!(owns("*", "anything/at/all"));
}

// ---- the recipes -----------------------------------------------------------

/// The recipes of the justfile: name to (header line, indented body lines joined).
fn recipes() -> HashMap<String, (String, String)> {
    let text = read(justfile_name());
    let mut found: HashMap<String, (String, String)> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let header = !indented
            && !line.starts_with('#')
            && !line.starts_with("set ")
            && !line.contains(":=")
            && line.contains(':');
        if header {
            let name = line
                .split(':')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_owned();
            found.insert(name.clone(), (line.trim().to_owned(), String::new()));
            current = Some(name);
        } else if indented {
            if let Some((_, body)) = current.as_ref().and_then(|name| found.get_mut(name)) {
                body.push_str(line);
                body.push('\n');
            }
        } else if line.trim().is_empty() {
            current = None;
        }
    }
    found
}

#[test]
fn the_recipes_the_tests_and_the_hooks_name_exist() {
    let found = recipes();
    for name in [
        "setup",
        "fmt",
        "fmt-check",
        "clippy",
        "lint",
        "test",
        "test-toolchain",
        "deny",
        "spellcheck",
        "secrets",
        "secrets-staged",
        "secrets-message",
        "check-fast",
        "check",
    ] {
        assert!(
            found.contains_key(name),
            "the justfile has no recipe `{name}`"
        );
    }
    assert_eq!(found["check"].0, "check: check-fast test-toolchain");
    assert_eq!(
        found["check-fast"].0,
        "check-fast: lint test deny spellcheck secrets"
    );
    assert!(
        read(justfile_name()).contains("set shell := [\"bash\", \"-euo\", \"pipefail\", \"-c\"]")
    );
}

#[test]
fn every_cargo_recipe_also_covers_the_excluded_test_crate() {
    // The test crate is outside the workspace, so `--workspace` and `--all` skip it.
    let found = recipes();
    for name in [
        "fmt",
        "fmt-check",
        "clippy",
        "test",
        "test-toolchain",
        "deny",
    ] {
        assert!(
            found[name].1.contains("toolchain-tests/Cargo.toml"),
            "`{name}` does not cover toolchain-tests"
        );
    }
}

// ---- the hooks -------------------------------------------------------------

fn lefthook() -> Yaml {
    load(&read("lefthook.yml"))
}

#[test]
fn lefthook_fails_closed_and_loads_the_rc_file() {
    let config = lefthook();
    assert_eq!(config["assert_lefthook_installed"].as_bool(), Some(true));
    assert_eq!(config["rc"].as_str(), Some(".lefthook/rc.sh"));
    let floor = config["min_version"]
        .as_str()
        .expect("min_version is a string");
    assert!(
        floor.split('.').count() == 3 && floor.split('.').all(|p| p.parse::<u32>().is_ok()),
        "min_version {floor}"
    );
    assert!(
        config["lefthook"].is_badvalue(),
        "lefthook.yml pins a `lefthook:` path, which has nothing to pin into"
    );
}

#[test]
fn the_rc_file_exports_the_hook_argument_and_drops_the_override_variables() {
    let rc = read(".lefthook/rc.sh");
    let lines: Vec<&str> = rc.lines().map(str::trim).collect();
    assert!(lines.contains(&"HOOK_ARG_1=\"${1:-}\""));
    assert!(lines.contains(&"export HOOK_ARG_1"));
    assert!(lines.contains(&"unset LEFTHOOK_BIN LEFTHOOK_CONFIG"));
}

#[test]
fn lefthook_yml_never_pastes_a_hook_argument_into_a_command() {
    // lefthook pastes {0}, {1} and so on into `sh -c` unescaped. Comment lines
    // may mention the placeholders (this file's own comments do); commands may not.
    for line in read("lefthook.yml")
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
    {
        let mut rest = line;
        while let Some(open) = rest.find('{') {
            let after = &rest[open + 1..];
            let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
            assert!(
                digits.is_empty() || !after[digits.len()..].starts_with('}'),
                "lefthook.yml uses the numbered placeholder {{{digits}}}: {line}"
            );
            rest = after;
        }
    }
}

#[test]
fn the_lefthook_floor_is_written_once_and_setup_reads_it_from_there() {
    let found = recipes();
    let body = &found["setup"].1;
    assert!(
        body.contains("min_version"),
        "`setup` does not read min_version"
    );
    assert!(body.contains("lefthook.yml"));
    let digits_dotted = body
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .any(|word| {
            let parts: Vec<&str> = word.split('.').collect();
            parts.len() == 3 && parts.iter().all(|p| !p.is_empty())
        });
    assert!(!digits_dotted, "`setup` writes a version number of its own");
}

#[test]
fn lefthook_yml_is_the_only_tracked_lefthook_configuration() {
    // lefthook merges `lefthook-local.yml` over `lefthook.yml`. Committed, a line
    // such as `commit-msg: { skip: true }` there switches a hook off for everyone.
    let listing = run(&repo_root(), "git", &["ls-files"]);
    assert_eq!(listing.code, 0, "{}", listing.output);
    let configs: Vec<&str> = listing
        .output
        .lines()
        .filter(|path| {
            let name = path
                .rsplit('/')
                .next()
                .unwrap_or(path)
                .trim_start_matches('.');
            let stem = name.split('.').next().unwrap_or("");
            let extension = name.rsplit('.').next().unwrap_or("");
            (stem == "lefthook" || stem == "lefthook-local")
                && ["yml", "yaml", "toml", "json", "jsonc"].contains(&extension)
        })
        .collect();
    assert_eq!(configs, ["lefthook.yml"]);
}

#[test]
fn the_pre_push_toolchain_job_covers_every_file_the_suite_copies() {
    let config = lefthook();
    let job = config["pre-push"]["jobs"]
        .as_vec()
        .expect("pre-push jobs")
        .iter()
        .find(|job| job["name"].as_str() == Some("toolchain-tests"))
        .expect("pre-push has a toolchain-tests job");
    let globs: Vec<&str> = job["glob"]
        .as_vec()
        .expect("a glob list")
        .iter()
        .filter_map(Yaml::as_str)
        .collect();
    let mut wanted: Vec<&str> = TOOLCHAIN_FILES.to_vec();
    wanted.extend([justfile_name(), "rust-toolchain.toml", "toolchain-tests"]);
    for file in wanted {
        let covered = globs.iter().any(|glob| {
            *glob == file
                || glob.strip_suffix("/**").is_some_and(|directory| {
                    directory == file || file.starts_with(&format!("{directory}/"))
                })
        });
        assert!(covered, "no glob of the toolchain-tests job covers {file}");
    }
}

// ---- the settings ----------------------------------------------------------

#[test]
fn the_toolchain_is_pinned_with_rustfmt_and_clippy() {
    let text = read("rust-toolchain.toml");
    assert!(
        text.lines()
            .any(|line| line.trim_start().starts_with("channel = \"")),
        "no pinned channel"
    );
    assert!(text.contains("\"rustfmt\"") && text.contains("\"clippy\""));
}

#[test]
fn the_private_test_crate_passes_the_licence_check_by_configuration() {
    let deny = read("deny.toml");
    assert!(deny.contains("[licenses.private]"));
    let after = deny.split("[licenses.private]").nth(1).expect("a section");
    let section = after.split("\n[").next().unwrap_or("");
    assert!(section.lines().any(|line| line.trim() == "ignore = true"));
    assert!(read("toolchain-tests/Cargo.toml").contains("publish = false"));
    assert!(read("toolchain-tests/.gitignore")
        .lines()
        .any(|line| line.trim() == "/target/"));
}

#[test]
fn the_hooks_and_the_recipes_keep_lf_line_endings() {
    let text = read(".gitattributes");
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    for wanted in [
        "*.sh text eol=lf",
        "lefthook.yml text eol=lf",
        ".lefthook/** text eol=lf",
        "toolchain-tests/** text eol=lf",
    ] {
        assert!(
            lines.contains(&wanted),
            ".gitattributes does not pin `{wanted}`"
        );
    }
    assert!(lines
        .iter()
        .any(|line| line.ends_with("text eol=lf") && line.starts_with(justfile_name())));
}
```

Create `.../assets/template/toolchain-tests/tests/toolchain.rs.tmpl`:

```rust
//! Toolchain tests: slow tests that build throwaway git repositories holding a
//! copy of the tooling, then run the real hooks and recipes in them to prove each
//! gate blocks what it should. They need `just`, `lefthook`, `gitleaks`, `cargo`
//! and `typos` on `PATH`; a missing tool fails the test instead of skipping it.
//!
//! `just test` leaves these out and `just test-toolchain` runs them. CI runs them
//! on every change; the pre-push hook runs them when a tooling file changed.

mod common;

use common::{
    add_remote, append, build_repo, commit_count, fake_token, install_hooks, make_repo, must, run,
    run_env, write, write_script, Ran, TempDir, LEAK,
};
use std::fs;
use std::path::{Path, PathBuf};

const UNFORMATTED: &str = "pub fn  twice(n:i32)->i32{n*2}\n";
const CLIPPY_FAILURE: &str =
    "\n/// Counts.\npub fn count(items: &Vec<i32>) -> usize {\n    items.len()\n}\n";
const CLEAN_ADDITION: &str =
    "\n/// Doubles a number.\npub fn double(n: i32) -> i32 {\n    n * 2\n}\n";

/// A repository with the hooks installed.
fn hooked_repo() -> TempDir {
    let dir = make_repo();
    install_hooks(dir.path());
    dir
}

/// Stage everything and try to commit it through the hooks.
fn commit(dir: &Path, message: &str) -> Ran {
    must(dir, "git", &["add", "--all"]);
    run(dir, "git", &["commit", "--quiet", "-m", message])
}

fn blocked(ran: &Ran) -> bool {
    ran.code != 0
}

// ---- pre-commit -------------------------------------------------------------

#[test]
fn pre_commit_blocks_unformatted_code() {
    let dir = hooked_repo();
    append(dir.path(), "src/lib.rs", UNFORMATTED);
    let ran = commit(dir.path(), "unformatted");
    assert!(blocked(&ran), "{}", ran.output);
    assert!(
        ran.output.contains("Diff in"),
        "rustfmt did not report a diff: {}",
        ran.output
    );
    assert_eq!(commit_count(dir.path()), 1);
}

#[test]
fn pre_commit_blocks_a_clippy_failure() {
    let dir = hooked_repo();
    append(dir.path(), "src/lib.rs", CLIPPY_FAILURE);
    let ran = commit(dir.path(), "clippy");
    assert!(blocked(&ran), "{}", ran.output);
    assert!(
        ran.output.contains("ptr_arg"),
        "clippy did not report the lint: {}",
        ran.output
    );
    assert_eq!(commit_count(dir.path()), 1);
}

#[test]
fn pre_commit_blocks_a_typo() {
    // The misspelling is assembled at run time: written out, it would fail this
    // repository's own spell check, which is the gate being tested.
    let typo = ["rec", "ieve"].concat();
    let dir = hooked_repo();
    append(
        dir.path(),
        "src/lib.rs",
        &format!("\n// we {typo} the value here\n"),
    );
    let ran = commit(dir.path(), "typo");
    assert!(blocked(&ran), "{}", ran.output);
    assert!(
        ran.output.contains(&typo),
        "typos did not name the word: {}",
        ran.output
    );
    assert_eq!(commit_count(dir.path()), 1);
}

#[test]
fn pre_commit_blocks_a_token_in_a_staged_file() {
    let dir = hooked_repo();
    append(
        dir.path(),
        "src/lib.rs",
        &format!("\npub const KEY: &str = \"{}\";\n", fake_token()),
    );
    let ran = commit(dir.path(), "token");
    assert!(blocked(&ran), "{}", ran.output);
    assert!(
        ran.output.contains(LEAK),
        "gitleaks did not report a leak: {}",
        ran.output
    );
    assert_eq!(commit_count(dir.path()), 1);
}

#[test]
fn pre_commit_lets_a_clean_change_through() {
    let dir = hooked_repo();
    append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
    let ran = commit(dir.path(), "add double");
    assert!(!blocked(&ran), "{}", ran.output);
    assert_eq!(commit_count(dir.path()), 2);
}

// ---- commit-msg -------------------------------------------------------------

#[test]
fn commit_msg_blocks_a_token_in_the_message() {
    let dir = hooked_repo();
    append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
    let ran = commit(dir.path(), &format!("leak {}", fake_token()));
    assert!(blocked(&ran), "{}", ran.output);
    assert!(ran.output.contains(LEAK), "{}", ran.output);
    assert_eq!(commit_count(dir.path()), 1);
}

#[test]
fn commit_msg_lets_a_clean_message_through() {
    let dir = hooked_repo();
    append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
    assert!(!blocked(&commit(dir.path(), "add double")));
}

#[test]
fn a_linked_worktree_named_with_shell_syntax_is_not_executed() {
    // lefthook pastes `{1}` into the command it runs, unescaped. In a linked
    // worktree git gives the commit-msg hook an absolute path that contains the
    // worktree's name, so a name with shell syntax in it would be run. The recipe
    // reads the path from HOOK_ARG_1 instead, and the message is still scanned.
    let dir = hooked_repo();
    let parent = TempDir::new();
    let worktree = parent.path().join("wt$(touch pwned)");
    let worktree_arg = worktree.to_string_lossy().into_owned();
    must(
        dir.path(),
        "git",
        &["worktree", "add", "--quiet", "-b", "side", &worktree_arg],
    );
    append(&worktree, "src/lib.rs", CLEAN_ADDITION);
    let ok = commit(&worktree, "add double");
    assert!(!blocked(&ok), "{}", ok.output);
    assert!(
        !worktree.join("pwned").exists() && !parent.path().join("pwned").exists(),
        "the worktree name was executed"
    );
    append(&worktree, "src/lib.rs", "\n// more\n");
    let leak = commit(&worktree, &format!("leak {}", fake_token()));
    assert!(blocked(&leak), "{}", leak.output);
    assert!(leak.output.contains(LEAK), "{}", leak.output);
}

// ---- the secret scan (`just secrets`) ------------------------------------------

fn token_file(dir: &Path, name: &str) {
    write(
        dir,
        name,
        &format!("pub const KEY: &str = \"{}\";\n", fake_token()),
    );
}

fn commit_unchecked(dir: &Path, message: &str) {
    must(dir, "git", &["add", "--all"]);
    must(
        dir,
        "git",
        &["commit", "--quiet", "--no-verify", "-m", message],
    );
}

fn secrets(dir: &Path) -> Ran {
    run(dir, "just", &["secrets"])
}

#[test]
fn secrets_pass_on_a_clean_repository() {
    let dir = make_repo();
    let ran = secrets(dir.path());
    assert_eq!(ran.code, 0, "{}", ran.output);
}

#[test]
fn secrets_find_a_token_buried_in_history_and_since_removed() {
    let dir = make_repo();
    token_file(dir.path(), "old.rs");
    commit_unchecked(dir.path(), "add");
    must(dir.path(), "git", &["rm", "--quiet", "old.rs"]);
    commit_unchecked(dir.path(), "remove");
    append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
    commit_unchecked(dir.path(), "later");
    let ran = secrets(dir.path());
    assert!(blocked(&ran) && ran.output.contains(LEAK), "{}", ran.output);
}

#[test]
fn secrets_find_a_token_added_by_a_merge_commit() {
    // Without `-m`, git shows a merge commit with no patch and its content is never scanned.
    let dir = make_repo();
    must(dir.path(), "git", &["checkout", "--quiet", "-b", "side"]);
    write(dir.path(), "side.txt", "side\n");
    commit_unchecked(dir.path(), "side");
    must(dir.path(), "git", &["checkout", "--quiet", "main"]);
    write(dir.path(), "main.txt", "main\n");
    commit_unchecked(dir.path(), "main");
    must(
        dir.path(),
        "git",
        &["merge", "--quiet", "--no-ff", "--no-commit", "side"],
    );
    token_file(dir.path(), "evil.rs");
    commit_unchecked(dir.path(), "merge");
    let ran = secrets(dir.path());
    assert!(blocked(&ran) && ran.output.contains(LEAK), "{}", ran.output);
}

#[test]
fn secrets_find_a_token_in_a_commit_message_body() {
    let dir = make_repo();
    append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
    commit_unchecked(
        dir.path(),
        &format!("subject\n\nbody with {}", fake_token()),
    );
    let ran = secrets(dir.path());
    assert!(blocked(&ran) && ran.output.contains(LEAK), "{}", ran.output);
}

#[test]
fn secrets_find_a_token_in_an_unstaged_change_to_a_tracked_file() {
    let dir = make_repo();
    append(
        dir.path(),
        "src/lib.rs",
        &format!("\npub const KEY: &str = \"{}\";\n", fake_token()),
    );
    let ran = secrets(dir.path());
    assert!(blocked(&ran) && ran.output.contains(LEAK), "{}", ran.output);
}

#[test]
fn secrets_staged_scans_only_what_is_staged() {
    let dir = make_repo();
    append(
        dir.path(),
        "src/lib.rs",
        &format!("\npub const KEY: &str = \"{}\";\n", fake_token()),
    );
    let unstaged = run(dir.path(), "just", &["secrets-staged"]);
    assert_eq!(unstaged.code, 0, "{}", unstaged.output);
    must(dir.path(), "git", &["add", "--all"]);
    let staged = run(dir.path(), "just", &["secrets-staged"]);
    assert!(
        blocked(&staged) && staged.output.contains(LEAK),
        "{}",
        staged.output
    );
}

#[test]
fn secrets_message_refuses_to_run_outside_the_commit_msg_hook() {
    let dir = make_repo();
    let ran = run(dir.path(), "just", &["secrets-message"]);
    assert!(blocked(&ran), "{}", ran.output);
    assert!(
        ran.output.contains("HOOK_ARG_1 is not set"),
        "{}",
        ran.output
    );
}

// ---- `just setup` -------------------------------------------------------------

#[test]
fn setup_installs_the_three_hooks() {
    let dir = make_repo();
    let ran = run(dir.path(), "just", &["setup"]);
    assert_eq!(ran.code, 0, "{}", ran.output);
    for hook in ["pre-commit", "commit-msg", "pre-push"] {
        assert!(
            dir.path().join(".git/hooks").join(hook).is_file(),
            "no {hook} hook was installed"
        );
    }
}

fn require_floor(dir: &Path, floor: &str) {
    let path = dir.join("lefthook.yml");
    let text = fs::read_to_string(&path).expect("read lefthook.yml");
    let edited: Vec<String> = text
        .lines()
        .map(|line| {
            if line.starts_with("min_version:") {
                format!("min_version: {floor}")
            } else {
                line.to_owned()
            }
        })
        .collect();
    fs::write(&path, edited.join("\n") + "\n").expect("write lefthook.yml");
}

#[test]
fn setup_refuses_a_lefthook_older_than_the_floor_and_installs_nothing() {
    // `lefthook install` itself does not enforce min_version, so the recipe must.
    let dir = make_repo();
    require_floor(dir.path(), "99.0.0");
    let ran = run(dir.path(), "just", &["setup"]);
    assert!(blocked(&ran), "{}", ran.output);
    assert!(
        ran.output.contains("older than the 99.0.0"),
        "{}",
        ran.output
    );
    assert!(
        !dir.path().join(".git/hooks/pre-commit").exists(),
        "hooks were installed anyway"
    );
}

#[test]
fn setup_stops_when_git_config_sets_a_hooks_path() {
    // lefthook refuses to install over a core.hooksPath. That is its own rule,
    // listed in docs/accepted-boundaries.md; this pins that the recipe stops.
    let dir = make_repo();
    must(
        dir.path(),
        "git",
        &["config", "core.hooksPath", "elsewhere"],
    );
    let ran = run(dir.path(), "just", &["setup"]);
    assert!(blocked(&ran), "{}", ran.output);
    assert!(ran.output.contains("core.hooksPath"), "{}", ran.output);
}

// ---- the hook runner --------------------------------------------------------

/// The tests that need a `PATH` without lefthook in it. They build that `PATH` from
/// symbolic links, so they run on Unix only; the end-to-end job covers Windows.
#[cfg(unix)]
mod missing_runner {
    use super::common::{copy, find_on_path};
    use super::*;
    use std::ffi::OsString;

    /// A directory holding only the tools a generated hook needs once it has failed
    /// to find lefthook, so `PATH` can be set to something with no lefthook in it
    /// even when lefthook lives beside git (as it does in /usr/bin).
    fn path_without_lefthook(parent: &Path) -> OsString {
        let farm = parent.join("farm");
        fs::create_dir_all(&farm).expect("create the tool directory");
        for tool in ["git", "uname", "tr", "sed"] {
            let real = find_on_path(tool).unwrap_or_else(|| panic!("{tool} is not on PATH"));
            std::os::unix::fs::symlink(real, farm.join(tool)).expect("link a tool");
        }
        farm.into_os_string()
    }

    /// A copy of the real lefthook at `<parent>/bin/lefthook`.
    fn lefthook_copy(parent: &Path) -> PathBuf {
        let real = find_on_path("lefthook").expect("lefthook is not on PATH");
        let target = parent.join("bin").join(if cfg!(windows) {
            "lefthook.exe"
        } else {
            "lefthook"
        });
        copy(&real, &target);
        target
    }

    fn git_path() -> PathBuf {
        find_on_path("git").expect("git is not on PATH")
    }

    #[test]
    fn a_missing_runner_blocks_the_commit() {
        // Install from a copy and delete the copy. The generated hook remembers the
        // path it was installed from and tries it after PATH, so a test that only
        // empties PATH passes for the wrong reason (see the next test).
        let dir = make_repo();
        let tools = TempDir::new();
        let copy = lefthook_copy(tools.path());
        must(dir.path(), &copy, &["install"]);
        fs::remove_file(&copy).expect("delete the copy");
        append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
        must(dir.path(), git_path(), &["add", "--all"]);
        let path = path_without_lefthook(tools.path());
        let ran = run_env(
            dir.path(),
            git_path(),
            &["commit", "--quiet", "-m", "x"],
            &[("PATH", path.as_os_str())],
        );
        assert!(blocked(&ran), "{}", ran.output);
        assert!(ran.output.contains("Can't find lefthook"), "{}", ran.output);
        assert_eq!(commit_count(dir.path()), 1);
    }

    #[test]
    fn the_install_path_is_a_fallback_so_emptying_path_alone_proves_nothing() {
        let dir = make_repo();
        let tools = TempDir::new();
        let copy = lefthook_copy(tools.path());
        must(dir.path(), &copy, &["install"]);
        append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
        must(dir.path(), git_path(), &["add", "--all"]);
        let path = path_without_lefthook(tools.path());
        let ran = run_env(
            dir.path(),
            git_path(),
            &["commit", "--quiet", "-m", "x"],
            &[("PATH", path.as_os_str())],
        );
        assert!(
            !ran.output.contains("Can't find lefthook"),
            "the hook did not fall back to its install path: {}",
            ran.output
        );
    }

    #[test]
    fn without_assert_lefthook_installed_a_missing_runner_passes_silently() {
        // The negative control for the test above: it is the setting that blocks.
        let dir = make_repo();
        let path = dir.path().join("lefthook.yml");
        let text = fs::read_to_string(&path).expect("read lefthook.yml");
        let without: Vec<&str> = text
            .lines()
            .filter(|line| !line.starts_with("assert_lefthook_installed"))
            .collect();
        fs::write(&path, without.join("\n") + "\n").expect("write lefthook.yml");
        let tools = TempDir::new();
        let copy = lefthook_copy(tools.path());
        must(dir.path(), &copy, &["install"]);
        fs::remove_file(&copy).expect("delete the copy");
        append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
        must(dir.path(), git_path(), &["add", "--all"]);
        let farm = path_without_lefthook(tools.path());
        let ran = run_env(
            dir.path(),
            git_path(),
            &["commit", "--quiet", "-m", "x"],
            &[("PATH", farm.as_os_str())],
        );
        assert_eq!(ran.code, 0, "{}", ran.output);
        assert_eq!(commit_count(dir.path()), 2);
    }
}

fn stub_runner(parent: &Path) -> PathBuf {
    let stub = parent.join("stub-lefthook");
    write_script(&stub, "#!/bin/sh\nexit 0\n");
    stub
}

#[test]
fn a_lefthook_bin_override_does_not_bypass_the_hooks() {
    let dir = hooked_repo();
    let tools = TempDir::new();
    let stub = stub_runner(tools.path());
    append(dir.path(), "src/lib.rs", UNFORMATTED);
    must(dir.path(), "git", &["add", "--all"]);
    let ran = run_env(
        dir.path(),
        "git",
        &["commit", "--quiet", "-m", "x"],
        &[("LEFTHOOK_BIN", stub.as_os_str())],
    );
    assert!(
        blocked(&ran),
        "LEFTHOOK_BIN swapped the hooks for a no-op: {}",
        ran.output
    );
}

#[test]
fn a_lefthook_config_override_does_not_bypass_the_hooks() {
    let dir = hooked_repo();
    let nowhere = dir.path().join("does-not-exist.yml");
    append(dir.path(), "src/lib.rs", UNFORMATTED);
    must(dir.path(), "git", &["add", "--all"]);
    let ran = run_env(
        dir.path(),
        "git",
        &["commit", "--quiet", "-m", "x"],
        &[("LEFTHOOK_CONFIG", nowhere.as_os_str())],
    );
    assert!(
        blocked(&ran),
        "LEFTHOOK_CONFIG swapped the configuration for nothing: {}",
        ran.output
    );
}

#[test]
fn without_the_unset_line_the_overrides_bypass_the_hooks() {
    // The negative control for the two tests above: it is rc.sh that blocks them.
    let dir = hooked_repo();
    let rc = dir.path().join(".lefthook/rc.sh");
    let text = fs::read_to_string(&rc).expect("read rc.sh");
    let without: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("unset "))
        .collect();
    fs::write(&rc, without.join("\n") + "\n").expect("write rc.sh");
    let tools = TempDir::new();
    let stub = stub_runner(tools.path());
    append(dir.path(), "src/lib.rs", UNFORMATTED);
    must(dir.path(), "git", &["add", "--all"]);
    let ran = run_env(
        dir.path(),
        "git",
        &["commit", "--quiet", "-m", "x"],
        &[("LEFTHOOK_BIN", stub.as_os_str())],
    );
    assert_eq!(ran.code, 0, "{}", ran.output);
}

#[test]
fn a_lefthook_older_than_the_floor_blocks_the_commit() {
    let dir = hooked_repo();
    require_floor(dir.path(), "99.0.0");
    append(dir.path(), "src/lib.rs", CLEAN_ADDITION);
    let ran = commit(dir.path(), "x");
    assert!(blocked(&ran), "{}", ran.output);
    assert!(
        ran.output.contains("required lefthook version (99.0.0)"),
        "{}",
        ran.output
    );
}

// ---- pre-push -----------------------------------------------------------------

#[test]
fn pre_push_blocks_a_push_that_fails_the_fast_gate() {
    // The fast gate stops at its first failing recipe, formatting here, before
    // `deny` would reach for the network.
    let dir = hooked_repo();
    let _remote = add_remote(dir.path());
    append(dir.path(), "src/lib.rs", UNFORMATTED);
    must(dir.path(), "git", &["add", "--all"]);
    must(
        dir.path(),
        "git",
        &["commit", "--quiet", "--no-verify", "-m", "unformatted"],
    );
    let ran = run(dir.path(), "git", &["push", "origin", "main"]);
    assert!(blocked(&ran), "{}", ran.output);
    assert!(
        ran.output.contains("fmt-check"),
        "the fast gate did not run: {}",
        ran.output
    );
    let local = must(dir.path(), "git", &["rev-parse", "main"]).output;
    let remote = must(dir.path(), "git", &["ls-remote", "origin", "main"]).output;
    assert!(
        !remote.contains(local.trim()),
        "the push reached the remote"
    );
}

#[test]
fn a_repository_can_be_built_in_a_named_directory() {
    let parent = TempDir::new();
    let dir = parent.path().join("named");
    fs::create_dir_all(&dir).expect("create the directory");
    build_repo(&dir);
    assert_eq!(commit_count(&dir), 1);
}
```

- [ ] **Step 6: Lint what was written**

```bash
S=shared/devtools/skills/rust-toolchain-scaffold
shellcheck -s bash $S/scripts/apply.sh
shellcheck -s sh $S/assets/template/.lefthook/rc.sh.tmpl $S/assets/template/.lefthook/pre-push/check.sh.tmpl
actionlint $S/assets/template/.github/workflows/ci.yml.tmpl
bash scripts/check-license-headers.sh $S/scripts/apply.sh
typos $S
find $S/assets/template -type f ! -name '*.tmpl'          # Expected: no output
```

Expected: every command silent. (`actionlint` reads the `.tmpl` file by path; if it refuses the extension, copy it to a temporary `ci.yml` and lint that.)

- [ ] **Step 7: Run the quick end-to-end test (it must now pass)**

```bash
bash scripts/test-rust-scaffold.sh --quick
```

Expected: every line `ok`, ending `Rust scaffold apply.sh checks passed (--quick: the scaffolded gates were not run).` It runs `cargo generate-lockfile` for the test crate, so it needs network access to crates.io.

- [ ] **Step 8: Run the full end-to-end test**

```bash
bash scripts/test-rust-scaffold.sh
```

Expected: after the apply checks, `ok  just setup installs the hooks`, `ok  just check passes on the scaffolded repository`, nine `ok  <weakening> -> <test> fails` lines, then `Rust scaffold test passed.` This takes a few minutes: it runs `cargo deny` against the network, 26 gate tests and 27 toolchain tests. If `just check` fails on the scaffolded repository, the script prints the last 40 lines of its output; the failing gate names itself.

- [ ] **Step 9: Emit the copies and run the drift gates**

```bash
just emit-devtools-skills
bash scripts/check-devtools-skills.sh
just skills-bundles
git status --short
```

Expected: `Devtools skills consistent.`, `Skills bundles consistent.`, and the status shows the new files under the skill and the four emitted copies, nothing under `target/`.

- [ ] **Step 10: Commit**

```bash
git add shared/devtools/skills/rust-toolchain-scaffold \
  claude-code/plugins/aiplugins-devtools-cc antigravity/plugins/aiplugins-devtools-agy qwen/plugins/aiplugins-devtools-qwen opencode/plugins/aiplugins-devtools-opencode
git status --short
git commit -m "$(cat <<'EOF'
feat: add the rust-toolchain-scaffold templates and apply.sh

The scaffold: a justfile, lefthook hooks with a fail-closed install, deny, typos and
toolchain settings, a pinned CI workflow, CODEOWNERS, and an excluded test crate with
26 gate tests and 27 toolchain tests. apply.sh never overwrites and reports drift. The
end-to-end test from the previous commit now passes: it applies the scaffold to
throwaway repositories, runs its own just check, and weakens nine gates one at a time.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: The static gate and its wiring

**Files:**
- Create: `scripts/check-rust-scaffold.sh`
- Modify: `Justfile` (add recipes after `devtools-skills`; add `rust-scaffold` to `check`)
- Modify: `.github/workflows/ci.yml` (two steps in the `wiring` job)

**Interfaces:**
- Consumes: the skill tree from Tasks 1 and 3; `scripts/check-gate-wiring.sh`, which requires every `scripts/check-*.sh` to be run by a recipe `just check` reaches and by a CI line (its rule: the plain run on its own line; a `--mutation-test` line does not count as the plain run).
- Produces: `bash scripts/check-rust-scaffold.sh [--mutation-test]`; recipes `rust-scaffold` and `test-rust-scaffold`.

- [ ] **Step 1: Create the gate**

Create `scripts/check-rust-scaffold.sh`, then `chmod +x` it:

```bash
#!/usr/bin/env bash
# Copyright 2026 Costas Kirgoussios
# SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
#
# Source-available software. It is NOT open source.
# Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
# with the additional terms in LICENSE-ADDENDUM (evaluation grant,
# anti-circumvention). See both files at the repository root.

# Checks the rust-toolchain-scaffold skill source under shared/devtools/.
# The generic devtools gate (check-devtools-skills.sh) checks every skill's frontmatter,
# its references/ links, its OpenCode command and the emitted copies. This one checks what
# is particular to the Rust scaffold: every template is a .tmpl file (so this repository's
# own gates, cargo and lefthook never treat a template as a live file), every file the
# skill names exists, the CI template pins its actions, the test crate stays outside the
# workspace of the repository it guards, and this repository keeps pinning LF for the
# skill's files (the `.gitattributes` line that covers every devtools skill). The end-to-end test (scripts/test-rust-scaffold.sh) proves the scaffold works.
# `--mutation-test` seeds a throwaway copy of the tree, applies one defect at a time and
# requires this gate to fail with the message that names that defect.
set -euo pipefail
mode=${1:-}
# Clear the arguments so nothing sourced later reads $1.
set --
cd "$(dirname "$0")/.."

fail=0
bad() {
    echo "  FAIL  $1" >&2
    fail=1
}

skill=shared/devtools/skills/rust-toolchain-scaffold
cmd=shared/devtools/commands/rust-toolchain-scaffold.md
tpl=$skill/assets/template
pin='shared/devtools/skills/** text eol=lf'

# The templates the skill cannot work without, relative to $tpl.
REQUIRED=(
    .gitattributes.tmpl
    .github/CODEOWNERS.tmpl
    .github/workflows/ci.yml.tmpl
    .gitignore.tmpl
    .lefthook/pre-push/check.sh.tmpl
    .lefthook/rc.sh.tmpl
    deny.toml.tmpl
    docs/accepted-boundaries.md.tmpl
    justfile.tmpl
    lefthook.yml.tmpl
    rust-toolchain.toml.tmpl
    toolchain-tests/.gitignore.tmpl
    toolchain-tests/Cargo.toml.tmpl
    toolchain-tests/src/lib.rs.tmpl
    toolchain-tests/tests/common/mod.rs.tmpl
    toolchain-tests/tests/gate.rs.tmpl
    toolchain-tests/tests/toolchain.rs.tmpl
    typos.toml.tmpl
)

# The frontmatter is the lines between the opening `---` on line 1 and the next `---`.
frontmatter() { awk 'NR == 1 { next } /^---$/ { exit } { print }' "$1" 2>/dev/null; }

# One defect at a time on a throwaway copy of the tree; the gate must fail with the
# message that names it. Without this nothing re-runs the failure branches, so an edit
# could turn one into a no-op and the gate would stay green.
mutation_test() {
    local pristine work total=0 failed=0
    local sk=$skill/SKILL.md
    # (global: the EXIT trap runs after this function's locals are gone)
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    pristine=$tmp/pristine
    work=$tmp/work
    mkdir -p "$pristine"
    tar -cf - scripts shared/devtools .gitattributes | tar -xf - -C "$pristine"

    # case_run <name> <want-rc> <want-message-substring> <mutation-shell>
    case_run() {
        local name=$1 want_rc=$2 want_msg=$3 mutation=$4 rc=0 out
        total=$((total + 1))
        rm -rf "$work"
        mkdir -p "$work"
        tar -cf - -C "$pristine" . | tar -xf - -C "$work"
        if ! (cd "$work" && eval "$mutation") >/dev/null 2>&1; then
            echo "  FAIL  mutation '$name': the mutation itself did not apply" >&2
            failed=$((failed + 1))
            return
        fi
        out=$(bash "$work/scripts/check-rust-scaffold.sh" 2>&1) || rc=$?
        if [ "$rc" -ne "$want_rc" ] || ! grep -qF -- "$want_msg" <<<"$out"; then
            echo "  FAIL  mutation '$name': exit $rc (want $want_rc), wanted: $want_msg" >&2
            failed=$((failed + 1))
        fi
    }

    case_run "baseline is green" 0 "Rust toolchain scaffold consistent." "true"
    case_run "a template without the .tmpl suffix" 1 "template file without the .tmpl suffix" \
        "cp $tpl/justfile.tmpl $tpl/justfile"
    case_run "a dangling assets reference" 1 "dangling reference assets/nope.md" \
        "printf '%s\n' 'See \`assets/nope.md\`.' >> $sk"
    case_run "a dangling scripts reference" 1 "dangling reference scripts/nope.sh" \
        "printf '%s\n' 'See \`scripts/nope.sh\`.' >> $sk"
    case_run "an action pinned to a tag" 1 "is not pinned to a 40-hex SHA" \
        "sed -E 's/@[0-9a-f]{40}/@v7/' $tpl/.github/workflows/ci.yml.tmpl > t.tmp && mv t.tmp $tpl/.github/workflows/ci.yml.tmpl"
    case_run "a required template missing" 1 "missing template: lefthook.yml.tmpl" \
        "rm $tpl/lefthook.yml.tmpl"
    case_run "the LF pin removed" 1 "the LF pin for the devtools skill trees is gone" \
        "grep -vxF '$pin' .gitattributes > t.tmp; mv t.tmp .gitattributes"
    case_run "apply.sh missing" 1 "missing file: $skill/scripts/apply.sh" \
        "rm $skill/scripts/apply.sh"
    case_run "the test crate loses its workspace table" 1 "lacks its own [workspace] table" \
        "grep -vx '\\[workspace\\]' $tpl/toolchain-tests/Cargo.toml.tmpl > t.tmp; mv t.tmp $tpl/toolchain-tests/Cargo.toml.tmpl"
    case_run "the test crate becomes publishable" 1 "lacks publish = false" \
        "grep -vx 'publish = false' $tpl/toolchain-tests/Cargo.toml.tmpl > t.tmp; mv t.tmp $tpl/toolchain-tests/Cargo.toml.tmpl"
    case_run "the description stops naming Rust" 1 "does not name Rust and Cargo" \
        "sed 's/^description: .*/description: Scaffold a tested toolchain into a repository./' $sk > t.tmp && mv t.tmp $sk"
    case_run "the OpenCode command missing" 1 "missing file: $cmd" "rm $cmd"

    if [ "$failed" -ne 0 ]; then
        echo "  $failed of $total mutation cases failed" >&2
        return 1
    fi
    echo "Rust scaffold gate fails on all $((total - 1)) seeded defects."
}

if [ "$mode" = "--mutation-test" ]; then
    mutation_test
    exit $?
fi
[ -z "$mode" ] || { echo "usage: $0 [--mutation-test]" >&2; exit 2; }

# --- the skill ---
for f in "$skill/SKILL.md" "$skill/scripts/apply.sh" "$cmd" \
    "$skill/references/pitfalls.md" "$skill/references/adapting.md" \
    "$skill/references/github-settings.md" "$skill/references/review-workflow.md" \
    "$skill/assets/snippets/agent-instructions.md" "$skill/assets/snippets/README.development.md"; do
    [ -f "$f" ] || bad "missing file: $f"
done

if [ -f "$skill/SKILL.md" ]; then
    fm=$(frontmatter "$skill/SKILL.md")
    grep -qx 'name: rust-toolchain-scaffold' <<<"$fm" || bad "$skill/SKILL.md: frontmatter lacks 'name: rust-toolchain-scaffold'"
    desc=$(grep '^description: ' <<<"$fm" || true)
    { grep -q 'Rust' <<<"$desc" && grep -q 'Cargo' <<<"$desc"; } \
        || bad "$skill/SKILL.md: the description does not name Rust and Cargo"
    # Every backticked assets/, references/ or scripts/ path must resolve under the skill.
    # shellcheck disable=SC2016  # the backticks are literal markdown, not a command
    while IFS= read -r ref; do
        [ -e "$skill/$ref" ] || bad "$skill/SKILL.md: dangling reference $ref"
    done < <(grep -oE '`(assets|references|scripts)/[^`]*`' "$skill/SKILL.md" | tr -d '`' | sort -u)
fi

# --- the templates ---
while IFS= read -r f; do
    case "$f" in
        *.tmpl) ;;
        *) bad "template file without the .tmpl suffix: $f" ;;
    esac
done < <(find "$tpl" -type f | sort)

for t in "${REQUIRED[@]}"; do
    [ -f "$tpl/$t" ] || bad "missing template: $t"
done

ci=$tpl/.github/workflows/ci.yml.tmpl
if [ -f "$ci" ]; then
    uses=0
    while IFS= read -r line; do
        uses=$((uses + 1))
        ref=${line#*uses:}
        ref=${ref# }
        ref=${ref%% *}
        sha=${ref##*@}
        grep -qE '^[0-9a-f]{40}$' <<<"$sha" || bad "$ci: action $ref is not pinned to a 40-hex SHA"
    done < <(grep -E '^[[:space:]-]*uses:' "$ci" || true)
    [ "$uses" -gt 0 ] || bad "$ci: no action is used, so the pin check checked nothing"
fi

crate=$tpl/toolchain-tests/Cargo.toml.tmpl
if [ -f "$crate" ]; then
    grep -qx '\[workspace\]' "$crate" || bad "$crate: lacks its own [workspace] table"
    grep -qx 'publish = false' "$crate" || bad "$crate: lacks publish = false"
fi

# --- this repository ---
grep -qxF "$pin" .gitattributes || bad ".gitattributes: the LF pin for the devtools skill trees is gone ($pin)"

if [ "$fail" -ne 0 ]; then
    exit 1
fi
echo "Rust toolchain scaffold consistent."
```

- [ ] **Step 2: Run it and its mutation test**

```bash
shellcheck -s bash scripts/check-rust-scaffold.sh
bash scripts/check-license-headers.sh scripts/check-rust-scaffold.sh
bash scripts/check-rust-scaffold.sh
bash scripts/check-rust-scaffold.sh --mutation-test
```

Expected: `Rust toolchain scaffold consistent.` and `Rust scaffold gate fails on all 11 seeded defects.`

- [ ] **Step 3: Prove it fails on a real defect, then restore**

```bash
S=shared/devtools/skills/rust-toolchain-scaffold
cp $S/assets/template/justfile.tmpl $S/assets/template/justfile
bash scripts/check-rust-scaffold.sh; echo "rc=$?"
rm $S/assets/template/justfile
bash scripts/check-rust-scaffold.sh; echo "rc=$?"
```

Expected: the first run prints `FAIL  template file without the .tmpl suffix: ...assets/template/justfile` and `rc=1`; the second prints the consistent line and `rc=0`.

- [ ] **Step 4: Wire it into the Justfile**

In `Justfile`, find:

```
devtools-skills:
    bash scripts/check-devtools-skills.sh
    bash scripts/check-devtools-skills.sh --mutation-test
```

and add after it, separated by blank lines:

```
# Check the rust-toolchain-scaffold skill source (structure, pins), then prove the gate
# fails on each seeded defect
rust-scaffold:
    bash scripts/check-rust-scaffold.sh
    bash scripts/check-rust-scaffold.sh --mutation-test

# End to end: apply the Rust scaffold to throwaway repositories, run its own gates, then
# weaken each gate once and require its test to fail. Slow. Needs just, lefthook, gitleaks,
# cargo-nextest, cargo-deny and typos; pass --quick for the apply.sh checks alone.
test-rust-scaffold *args:
    bash scripts/test-rust-scaffold.sh {{args}}
```

Then, in the `check:` line, replace `devtools-skills skills-bundles` with `devtools-skills rust-scaffold skills-bundles` (that text occurs once, in the dependency list).

- [ ] **Step 5: Wire it into the CI `wiring` job**

In `.github/workflows/ci.yml`, find:

```
      - name: Check devtools skills gate against seeded defects
        run: bash scripts/check-devtools-skills.sh --mutation-test
```

and add after it:

```
      - name: Check the Rust scaffold skill
        run: bash scripts/check-rust-scaffold.sh

      # The gate must fail on each seeded defect, or an edit to it could turn a
      # failure branch into a no-op and leave it green.
      - name: Check the Rust scaffold gate against seeded defects
        run: bash scripts/check-rust-scaffold.sh --mutation-test
```

- [ ] **Step 6: Verify the wiring with the repository's own gate**

```bash
just --list | grep -E 'rust-scaffold'
actionlint .github/workflows/ci.yml
bash scripts/check-gate-wiring.sh --selftest
bash scripts/check-gate-wiring.sh
just rust-scaffold
```

Expected: both recipes listed; `actionlint` prints only the pre-existing SC2086 notes at the footprint-freshness step (see ROADMAP), nothing new; the gate-wiring self-test and check pass and the check does not name `check-rust-scaffold.sh`; `just rust-scaffold` passes.

- [ ] **Step 7: Commit**

```bash
git add scripts/check-rust-scaffold.sh Justfile .github/workflows/ci.yml
git commit -m "$(cat <<'EOF'
feat: gate the rust-toolchain-scaffold skill structure in just check and CI

scripts/check-rust-scaffold.sh checks that every template is a .tmpl, every file the
skill names exists, the CI template pins its actions, the test crate stays outside
the workspace, and the devtools LF pin is still there. Its mutation test seeds eleven
defects. Wired into the check aggregate and the wiring job; test-rust-scaffold is the
slow end-to-end recipe.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: The three-system end-to-end job

**Files:**
- Modify: `.github/workflows/ci.yml` (one new job)

**Interfaces:**
- Consumes: `scripts/test-rust-scaffold.sh` (Task 3).
- Produces: the CI job `Rust scaffold (<os>)` on ubuntu, windows and macos: the only run that measures Windows Git Bash and macOS for this scaffold.

- [ ] **Step 1: Add the job**

In `.github/workflows/ci.yml`, find:

```
  # Stages the Pages tree on pull requests so a staging failure is seen before
  # the merge. On push to main docs.yml already stages, so this skips there.
  site-stage:
```

and insert before it (keep the two-space job indentation):

```
  # The Rust scaffold ships to other repositories and works through bash scripts,
  # git hooks, lefthook and gitleaks, all of which behave differently on Git Bash
  # and on macOS. This applies it to throwaway repositories on all three systems,
  # runs its own `just check` there, and weakens each gate once to see its test
  # fail; a Linux-only run would leave Windows and macOS unmeasured.
  rust-scaffold:
    name: Rust scaffold (${{ matrix.os }})
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    defaults:
      run:
        shell: bash
    steps:
      - uses: actions/checkout@v7

      - name: Install just, cargo-nextest, cargo-deny and typos
        uses: taiki-e/install-action@v2
        with:
          tool: just,cargo-nextest,cargo-deny,typos

      # lefthook and gitleaks have no cargo or install-action route. A version-pinned
      # `go install` works the same on all three runners (Go is preinstalled) and is
      # verified by Go's module checksum database. gitleaks declares its module path
      # under the old organization name.
      - name: Install lefthook and gitleaks
        run: |
          go install github.com/evilmartians/lefthook/v2@v2.1.17
          go install github.com/zricethezav/gitleaks/v8@v8.30.1
          echo "$(go env GOPATH)/bin" >> "$GITHUB_PATH"

      - name: Test the Rust scaffold
        run: bash scripts/test-rust-scaffold.sh

```

- [ ] **Step 2: Lint the workflow and confirm the gate-wiring rule still holds**

```bash
actionlint .github/workflows/ci.yml
bash scripts/check-gate-wiring.sh
```

Expected: no new `actionlint` finding (the pre-existing SC2086 notes remain); the gate-wiring check passes. `scripts/test-rust-scaffold.sh` is not named `check-*.sh`, so it is not a gate and needs no `just check` reach.

- [ ] **Step 3: Commit, then watch the first run**

```bash
git add .github/workflows/ci.yml
git commit -m "$(cat <<'EOF'
ci: run the Rust scaffold end to end on Linux, Windows and macOS

Applies the scaffold to throwaway repositories, runs its own just check, and weakens
nine gates one at a time. This is the only measurement of Git Bash and macOS for the
scaffold. lefthook and gitleaks come from a version-pinned go install.

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
EOF
)"
```

Do not push unless the user asks. When the first run happens, read each system's log: Windows and macOS have never executed this scaffold. Expected failures to look at first, in order of likelihood: the three `missing_runner` tests are compiled out on Windows (24 toolchain tests there, not 27), `rustup toolchain install` on a runner without the pinned channel, `$(go env GOPATH)` spelling a Windows path, and `LEFTHOOK_BIN` pointing at a script under Git Bash. Record what fails in `docs/superpowers/specs/2026-11-24-rust-toolchain-scaffold-design.md` section 7 and fix the scaffold; never weaken a gate test to get a system green.

---

### Task 6: Whole-repository verification

**Files:** none (read-only), unless a check finds something.

- [ ] **Step 1: Run the repository's own gates that this change can affect**

```bash
just devtools-skills rust-scaffold skills-bundles skills-release-wiring footprint-budgets wiring opencode-wiring agy-wiring marketplace qwen-marketplace agy-marketplace site-check gate-wiring-test gate-wiring doc-paths-test doc-paths spellcheck
just links
bash scripts/check-license-headers.sh
bash scripts/check-package-versions.sh
```

Expected: all pass. `opencode-wiring` and `agy-wiring` fail if a skill's frontmatter `name` differs from its folder name, which the emit recipe guarantees, including OpenCode's `aiplugins-devtools-` prefix. `just check` also runs `footprint` (rebuilds other plugins' binaries) and `agy-validate` (needs the `agy` CLI); run them where the binaries and the CLI exist, and otherwise let CI be the first run.

- [ ] **Step 2: Re-run the end-to-end test from a clean state**

```bash
git status --short            # Expected: nothing
just emit-devtools-skills && git status --short   # Expected: nothing (the emit recipe reproduces the committed copies)
just test-rust-scaffold
```

Expected: `Rust scaffold test passed.`

- [ ] **Step 3: Review the diff against the spec**

```bash
git diff docs/rust-toolchain-scaffold-spec..HEAD --stat
```

Check each spec section 4.x requirement against a task in this plan; any gap is a missing task, not a deferral. Then report to the user: what ran, what did not (Windows, macOS, the new CI job until it has run), and ask before pushing or opening a pull request.

---

## Self-review

**Spec coverage.** 4.1 layout and `.tmpl`: Tasks 1 and 3, enforced by Task 4's gate. 4.1 re-run drift report: `apply.sh` and its checks in Task 2. 4.2 recipes, hook jobs, toolchain pin, CI pins, install route: the templates in Task 3, with the install route split as the spec says (pinned download in the scaffold's `ci.yml`, `go install` in Task 5). 4.2 first run on existing code: SKILL.md step 5. 4.3 hook runner and its matrix: `lefthook.yml.tmpl`, `rc.sh.tmpl`, `justfile.tmpl` `setup`, and the `missing_runner`, override and floor tests. 4.4 gate and toolchain tests, tiered workflow scope, explicit environment list, git-config isolation, `.gitignore`, `deny.toml` private setting: `gate.rs`, `toolchain.rs`, `common/mod.rs`, the templates. 4.5 static gate with mutation test and the end-to-end job with seeded weakenings: Tasks 4, 2, 5. 4.6 host-neutral wording: SKILL.md. 5 footprint: Task 1 Step 6; license headers: Tasks 2 to 4; versioning: no task needed (a `feat:` commit). 7 unknowns: Windows and macOS are Task 5's first run; the same-name-job question stays untested and is listed in `accepted-boundaries.md.tmpl`.

**Deliberate refinements of the spec, found by measurement.** The `JUST_` rule is by prefix (39 variables today). No new `.gitattributes` rule: the repository already pins `shared/devtools/skills/**` and each emitted copy to LF, and the static gate guards that line. The scaffold's CI tool pins are `TOOL_*`. The gate test count is 26 and the toolchain count 27 (24 off Unix), recorded in SKILL.md.

**Gaps closed here, and one left open.** Left open, on purpose: Windows and macOS behaviour, because nothing on this machine can measure it; Task 5 is the measurement and Task 5 Step 3 says what to read first. No step defers a decision; the two places that say "if X, then Y" (a failing `ratchet`, a missing toolchain) name the action.

## Stand-downs (adversarial panel review)

Round 1 (agy, with a Cold Executor seat that walked Tasks 0 to 6), three findings, each checked against the repository before folding. FOLDED: the preflight now lists `mlc`, `tar`, `zip`, `unzip` and `zipinfo`, which `just links` and `just skills-bundles` need (Task 0); the README opening-line edit has exact old and new text (Task 1 Step 7); `ratchet` rewrites `budgets.json` in place, so a retry starts from `git checkout -- docs/footprints README.md` (Task 1 Step 6); the end-to-end script is committed straight after it is written, because nothing wires it yet and an untracked file can be lost to a reset (Task 2 Step 4). RECORDED, not changed: the commits along the way are not releasable states (a skill advertised before its templates land), which the Execution notes now say, because nothing releases from a feature branch and moving the README edits to the end would only add a commit. Not re-raised: Windows and macOS behaviour, left to Task 5's first run. Nothing is discarded below the floor; nothing is unverified-accepted.
