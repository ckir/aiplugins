# rust-toolchain-scaffold: a tested quality toolchain for Rust repositories, as an aiplugins-devtools skill

Status: design, approved section by section by the owner on 2026-11-24. The line-level plan waits for the owner's
review of this spec, and it is written only against code that exists then (see section 8).

## 1. Problem and intent

The owner keeps a personal skill, `bun-toolchain-scaffold` (under `~/.claude/skills/`), that installs a quality
toolchain into a Bun and TypeScript repository: just recipes, lefthook git hooks, type check, Biome, gitleaks, a
SHA-pinned GitHub Actions workflow, CODEOWNERS, and above all a test suite proving that every gate blocks what it
claims to. An untested hook or CI step is routinely weakened without anyone noticing; the tests are the point, the
config files are short.

This repository is Rust. Its own toolchain (`Justfile`, `lefthook.yml`, `deny.toml`, `typos.toml`) is good but has
no gitleaks, no CODEOWNERS and no tests that prove its gates block. The owner wants the Bun skill ported to Rust and
shipped from this repository so that any Rust repository can get the same tested toolchain, on every agent this
repository supports (Claude Code, OpenCode, Qwen Code, Antigravity).

Intent: a new skill `rust-toolchain-scaffold` in the `aiplugins-devtools` plugin.

## 2. Decisions already ruled by the owner

1. **Target:** a new skill in the `aiplugins-devtools` plugin, for scaffolding any Rust repository. Not a retrofit
   of this repository only, not a language-neutral skill.
2. **Tool set:** a faithful translation plus `cargo-deny` and `typos` on by default. just, lefthook, `cargo fmt`,
   clippy with `-D warnings`, nextest, cargo-deny, typos, gitleaks, a SHA-pinned CI workflow, CODEOWNERS, and the
   gate and toolchain tests. Other tools (mlc, license headers, a gate-wiring script) go in `references/adapting.md`
   as additions, not defaults.
3. **Hook runner:** lefthook from `PATH`, hardened (section 4.3). Agreed by both peers after one negotiation round.
4. **Test host:** an excluded standalone crate inside the target repository (section 4.4).
5. **License of generated files:** no header. The files the scaffold writes belong to the repository they are written
   into. The skill's own text stays under the plugin license (PolyForm-Noncommercial-1.0.0).

## 3. Goals, non-goals, anti-goals

Goals:

- One skill that, given a Rust repository and a code owner handle, installs the toolchain without overwriting
  anything, and tells the agent how to merge what it skipped.
- Every installed gate has a test that proves it blocks, and a rule that a gate change comes with a test change
  shown failing when the gate is weakened.
- The skill is proven to work by this repository's own CI, on Linux, Windows (Git Bash) and macOS.

Non-goals:

- Agent-behaviour evals like the Bun skill's `evals/` directory. A separate deliverable.
- A Node or Bun variant, or a language-neutral core. The existing Bun skill is not touched.
- Retrofitting this repository's own toolchain (its `just setup` defect is tracked separately, section 9).
- Replacing `cargo deny` advisories with anything; the advisory database needs the network and stays out of the
  default offline test path (section 7).

Anti-goals (a failure even if it technically works):

- A gate that passes while doing nothing: a missing tool, an unset override, a skipped hook that exits 0.
- Template files that this repository's own gates, cargo or lefthook treat as live files.
- The four agent copies drifting apart, or the skill pointing at a file that is not in the installed plugin.

## 4. Design

### 4.1 Placement and layout

The canonical source is `shared/devtools/skills/rust-toolchain-scaffold/`. The existing recipe
`emit-devtools-skills` copies `shared/devtools/skills/` into the four plugin folders (OpenCode gets the prefixed
folder name `aiplugins-devtools-rust-toolchain-scaffold` and one rewritten `name:` line). The drift check
`scripts/check-devtools-skills.sh` discovers a new skill folder by itself. OpenCode needs one command file,
`shared/devtools/commands/rust-toolchain-scaffold.md`, whose body names the prefixed skill and contains
`$ARGUMENTS`, which is the user's free-text request (the target path and, optionally, the code owner). The command
only loads the skill; the agent following `SKILL.md` runs `apply.sh`, and finds the code owner itself (from
`gh repo view`, or by asking) when the request does not give one. No change to `agents.json`, the marketplaces or
the bundlers is expected; the plan confirms that.

```
rust-toolchain-scaffold/
  SKILL.md                   short frontmatter; the scaffold workflow, host-neutral
  scripts/apply.sh           never overwrites; strips the .tmpl suffix; takes the code owner handle
  assets/template/           every file stored as <name>.tmpl:
    justfile, lefthook.yml, .lefthook/rc.sh, .lefthook/pre-push/check.sh,
    deny.toml, typos.toml, .gitattributes, .gitignore,
    .github/workflows/ci.yml, .github/CODEOWNERS,
    toolchain-tests/        the excluded standalone crate: Cargo.toml, gate/ and toolchain/ tests
                            (its Cargo.lock is generated at scaffold time, not shipped as a template;
                            its own .gitignore ignores /target/)
  assets/snippets/           agent-instructions section, README "Development" section, Cargo lints table
  references/                pitfalls.md, adapting.md, github-settings.md, review-workflow.md
```

Why every template is a `.tmpl`: this repository's license-header gate (`scripts/check-license-headers.sh`) requires
this repository's PolyForm header on every tracked `.rs`, `.sh`, `.ts`, `.js`, `.py` and `.ps1` file outside a short
exclusion list, and generated files must not carry that header (decision 5). A `.tmpl` suffix keeps the templates out
of that gate, out of cargo (a `Cargo.toml.tmpl` is not a manifest) and out of this repository's own lefthook and CI.
`scripts/apply.sh` and any other script under `scripts/` that is not a template are real `.sh` files and carry the
header, like `license-headers.sh` does today.

### 4.2 What the scaffold installs

| Piece | Files |
|---|---|
| Task runner | `justfile`: `setup`, `fmt`, `clippy`, `lint`, `test`, `test-toolchain`, `deny`, `spellcheck`, `secrets`, `secrets-staged`, `secrets-message`, `check-fast`, `check` |
| Git hooks | `lefthook.yml`, `.lefthook/rc.sh`, `.lefthook/pre-push/check.sh` |
| Settings | `deny.toml`, `typos.toml`, `.gitattributes`, `.gitignore` additions, a Cargo `[lints]` snippet (clippy `-D warnings` is the recipe's job, the table pins the rest) |
| CI | `.github/workflows/ci.yml` (runs `just check`), `.github/CODEOWNERS`; `apply.sh` also prints every other workflow file it finds, because the gate tests apply to all of them (section 4.4) |
| Tests | `toolchain-tests/` (section 4.4) |
| Docs | agent-instructions section, README "Development" section, `docs/accepted-boundaries.md` |

Recipe composition, ported from the Bun skill's justfile: `lint` is `cargo fmt --all -- --check` plus clippy with
`-D warnings` over the workspace and over `toolchain-tests/`; `check-fast` is `lint`, `test`, `deny`, `spellcheck`
and `secrets`; `check` is `check-fast` plus `test-toolchain`. CI runs `just check`.

Hook jobs, ported from the Bun skill's `lefthook.yml`: pre-commit runs the format check, the same `just clippy`
recipe that `lint` uses (the workspace and `toolchain-tests/`) when a staged `.rs` file exists (clippy works on
crates, not files, and like every hook here it sees the working tree, not the staged snapshot), typos and
`just secrets-staged`; commit-msg runs `just secrets-message`; pre-push runs the always-on fast
gate as a `script:` job and the toolchain suite as a `run:` job glob-conditioned on every file the suite copies.
Whether clippy belongs in pre-commit (this repository's own hook runs it) or only in pre-push is left to the plan,
because it trades commit latency against earlier feedback.

Toolchain and tool pins: a `rust-toolchain.toml` pins a recent stable channel with the `rustfmt` and `clippy`
components, and CI installs `just`, `gitleaks`, `lefthook` and the cargo tools at pinned versions. Clippy with
`-D warnings` on a floating `stable` turns CI red whenever a new lint ships. As in the Bun skill, `apply.sh` leaves
the pinned values as written and `SKILL.md` tells the agent to resolve them fresh at scaffold time.

Machine prerequisites: `just`, `cargo`, `lefthook`, `gitleaks`, `git`. `cargo-nextest`, `cargo-deny` and
`typos-cli` come from `cargo binstall`. `lefthook` and `gitleaks` have no cargo route (measured, section 6), so
`just setup` and the README snippet give a per-OS install command for each. For CI, `taiki-e/install-action` (which
this repository's CI already uses) lists `just`, `cargo-nextest`, `cargo-deny` and `typos` in its `manifests/` but
neither `gitleaks` nor `lefthook` (measured), so those two need a pinned release download with a checksum check or an
OS package manager. A pinned download is the default, because the pins above require a version to be nameable; the
plan confirms it on all three runners. The Bun skill's
`.claude/recommended-tools.json` is dropped: it is one user's convention, not something four agents share.

### 4.3 Hook runner (agreed by the owner, agy and the driver)

lefthook from `PATH`, hardened:

- `lefthook.yml` sets `min_version` (a floor), `assert_lefthook_installed: true` and `rc: .lefthook/rc.sh`. It does
  not set the `lefthook:` pin, because a Rust repository has no `node_modules` to pin into.
- `.lefthook/rc.sh` exports `HOOK_ARG_1` from `$1` and runs `unset LEFTHOOK_BIN LEFTHOOK_CONFIG`. `lefthook.yml`
  never uses `{0}`, `{1}` and so on: the generated hook pastes them into `sh -c` unescaped (Bun skill
  `references/pitfalls.md`).
- `just setup` checks that `lefthook` is on `PATH` and at or above the floor before it runs `lefthook install`,
  because `lefthook install` does not enforce `min_version` (measured).
- The version floor is written once, in `lefthook.yml`. `just setup` reads it from there instead of repeating the
  number, and a gate test fails if the two ever differ.
- `.gitattributes` pins `lefthook.yml` and `.lefthook/**` to `eol=lf`.
- Pre-push keeps the Bun skill's split: the always-on fast gate is a `script:` job, because a `run:` job is skipped
  when no pushed file matches its glob; only the slow toolchain suite is glob-conditioned.

Failure matrix. Every row blocks or is a documented accepted boundary; none silently passes.

| Case | Result | How it is proven |
|---|---|---|
| Runner missing | Blocks (exit 1) | toolchain test: install from a copy, delete the copy, strip `PATH` |
| Runner missing, `assert_lefthook_installed` removed | Silent pass | gate test pins the setting |
| Runner too old | Blocks, with "required lefthook version (X) is higher than current (Y)" | toolchain test with an impossible `min_version`; `just setup` stub test |
| `LEFTHOOK_BIN` or `LEFTHOOK_CONFIG` set in the environment | Blocks with `rc.sh`; silent pass without it | toolchain test both ways; gate test pins `rc.sh` and the `unset` line |
| Setup never run, `--no-verify`, `LEFTHOOK=0`, `PATH` spoofing, local deletion of `rc.sh` | Accepted boundary | `docs/accepted-boundaries.md`; CI is the authoritative gate |
| Windows Git Bash, macOS | Unverified until the end-to-end CI job (section 4.5) runs | that job |

A naive missing-runner test passes for the wrong reason: lefthook falls back to the path it was installed from. The
test must install from a copy and delete it, which is why that is the specified method.

Considered and not chosen: plain `.githooks/` scripts with `core.hooksPath` (agy's first choice; removes the binary
and the rc pitfalls, but drops lefthook's parallel jobs and staged globs and reverses the faithful-translation
ruling), and a repo-local pinned lefthook binary downloaded by `just setup` (closest to the Bun design, but needs
per-platform archive handling).

### 4.4 Tests in the target repository

The tests live in `toolchain-tests/`, an excluded standalone crate: its `Cargo.toml` carries its own empty
`[workspace]` table, and it is run with `cargo nextest run --manifest-path toolchain-tests/Cargo.toml`. This keeps
the target's root `Cargo.toml` and `Cargo.lock` untouched (measured, section 6) and lets the tests use a YAML crate
(`yaml-rust2`), which a std-only Rust test cannot.

- `gate` tests (fast, under a second) read the workflow, CODEOWNERS, `deny.toml`, `typos.toml`, `lefthook.yml`,
  `.gitattributes` and the justfile. They check, in every file under `.github/workflows/` and not only `ci.yml`:
  actions pinned to a 40-hex SHA, read-only permissions, no `env:` key at workflow, job or step level that `just`
  itself reads (`JUST_DRY_RUN` makes `just check` exit 0 without running anything, measured; the plan takes the
  full list from `just --help`), no `continue-on-error`, `if`, `container`, `services`, `||` or `set +` in a workflow script; CI runs `just check`,
meaning a workflow step whose `run:` line is that command, not a line that merely mentions it (the same rule
`scripts/check-gate-wiring.sh` applies in this repository, so an `echo "just check"` does not pass);
  CODEOWNERS is evaluated last-match-wins like GitHub and covers the protected files; the lefthook settings of
  section 4.3; no tracked `lefthook-local.yml`; the recipes the tests name exist. They run inside `just test`, so a
  pull request that downgrades CI still runs them.
- `toolchain` tests (slow) build throwaway git repositories and prove each gate blocks: pre-commit rejects
  unformatted code, a clippy failure and a typo; the commit-msg secret scan; the pre-push check; `just setup` steps
  and their order; `secrets` finds a token buried in history, in a merge and in a message; the hook-runner rows of
  section 4.3.
- Carried over from the Bun skill: a gate change comes with a test change shown failing when the gate is weakened;
  token-shaped literals are built at run time and never committed; the test environment strips `GIT_*`,
  `LEFTHOOK*`, `GITLEAKS_*`, `JUST_*`, `HOOK_ARG_*`, `CDPATH`, `BASH_ENV` and `ENV`, and in Rust also an explicit list
  of the variables that change a gate's outcome (`RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_*`,
  `CARGO_PROFILE_*`, `CARGO_TARGET_DIR`, `RUSTC_WRAPPER`, `RUSTUP_TOOLCHAIN`, `NEXTEST_*`, `TYPOS_*`; the plan fixes
  the list). It is deliberately not a `CARGO_*` glob: `CARGO_HOME`, `RUSTUP_HOME` and the registry and network
  settings stay, because a mirror or credentials configured there are part of a working environment, and
  `CARGO_TARGET_DIR` goes so that each throwaway repository builds into its own `target/`. Stripping variables does not isolate git's config
  files, so the throwaway repositories also set `GIT_CONFIG_GLOBAL` to an empty file and `GIT_CONFIG_NOSYSTEM=1`:
  a developer's global `core.hooksPath` makes `lefthook install` fail (measured, section 6).
- The suite is not split into a smaller target-side set. A fresh no-dependency crate costs about 0.14 s for
  `cargo fmt --check`, 0.4 to 0.6 s for a cold clippy and 1.3 to 1.4 s for a cold nextest run (section 6), so
  throwaway repositories are affordable. The total suite time is not known until it is built. The Bun skill's
  `lefthook.yml` says its own suite takes about 12 seconds in pre-push; the plan measures this one, and if the
  pre-push job is slower than the owner accepts it becomes CI-only.
- `toolchain-tests/` is a private crate (`publish = false`). `apply` runs `cargo generate-lockfile` for it and the
  lock is committed, so CI resolves the crates the scaffold resolved. A nested crate builds into its own `target/`
  (measured), and the root `.gitignore` does not cover it: `apply.sh` skips a root `.gitignore` that exists, and the
  usual `/target` entry is anchored to the root (measured with `git check-ignore`). So the crate carries its own
  `.gitignore` containing `/target/`, which `apply.sh` always copies because it is a new file. `cargo deny` covers
  the crate with the repository's `deny.toml`. The crate sets `publish = false` and the template `deny.toml` sets
  `[licenses] private = { ignore = true }`; both are needed, because `publish = false` alone still fails `licenses`
  with exit 4 (measured). For an existing `deny.toml` that `apply.sh` skips, `references/adapting.md` names the line
  to merge.
- Consequence the skill must handle: `cargo fmt --all`, `cargo clippy --workspace`, `cargo nextest --workspace` and
  `cargo deny` do not cover an excluded crate (measured for `fmt`, section 6), so the justfile has explicit recipe
  lines for `toolchain-tests/` in each of them, a gate test fails if one is missing, and `references/adapting.md`
  says to exclude `toolchain-tests` from any root `members` glob (a glob that captures it fails with "multiple
  workspace roots").
- A developer whose global git config sets `core.hooksPath` cannot run `lefthook install` at all, which is
  lefthook's own refusal. `just setup` reports that and stops; the case is listed in `docs/accepted-boundaries.md`.

### 4.5 Proving the skill works, in this repository

1. `scripts/check-rust-scaffold.sh`: a cheap static gate in the style of `scripts/check-devtools-skills.sh`, with a
   `--mutation-test` that applies one defect at a time to a throwaway copy and requires the gate to fail with the
   message naming it. It checks that every file under `assets/template/` ends in `.tmpl`, that `apply.sh` covers
   every template and refuses to overwrite (including a `justfile` when `Justfile` or `.justfile` already exists:
   `just` fails with "multiple candidate justfiles" when two spellings coexist, measured, and this repository uses
   `Justfile`), that every file the `SKILL.md` and references name exists, that the
   template workflow's action pins are 40-hex SHAs, and that the `SKILL.md` frontmatter name matches its folder.
   It is wired into `just check` and CI, as `scripts/check-gate-wiring.sh` requires of every `check-*.sh`.
2. `scripts/test-rust-scaffold.sh`: the end-to-end test, run by a new CI job on ubuntu, windows and macos. It applies
   the scaffold to a throwaway single-crate repository and to a workspace repository, then runs the scaffolded
   `just check` and the toolchain suite in each. A passing scaffold proves little on its own, because the templates
   would be testing themselves, so the script also applies a few seeded weakenings to the scaffolded repository and
   requires the matching test to go red for each: remove `assert_lefthook_installed`, remove the `unset` line from
   `rc.sh`, replace an action SHA with a tag, drop a CODEOWNERS entry. Its name does not start with `check-`, so it is not a gate that
   must be in `just check`; it has its own `just` recipe and its own CI job, like `test-live-ghidra`.

### 4.6 Changes from the Bun original

- "CLAUDE.md" becomes "the agent instructions file" (`CLAUDE.md`, `AGENTS.md`, `GEMINI.md` or `QWEN.md`), because
  the skill ships to four agents.
- `.claude/recommended-tools.json` is dropped (section 4.2).
- `references/review-workflow.md` stays generic: it says to run whatever review gates the user's own instructions
  require, and names no peer tool.
- The `SKILL.md` description is shortened to fit the footprint cap (section 5).

## 5. Constraints from this repository

- **License-header gate:** applies to tracked `.rs`, `.sh`, `.ts`, `.js`, `.py`, `.ps1` files; handled by the `.tmpl`
  rule (section 4.1).
- **Footprint cap:** the resident cost of every new `SKILL.md` frontmatter and every OpenCode command file's
  frontmatter counts against a per-change delta (`deltaBytes` 500 for each `aiplugins-devtools` package in
  `docs/footprints/budgets.json`), enforced by `scripts/check-footprint-budgets.sh`, and growth that trips it is
  never overridden (`shared/devtools/README.md`). The four existing skills cost 2369 resident bytes in the Claude
  Code and Antigravity packages and 2388 in the OpenCode one, about 590 bytes a skill, and the Bun skill's
  description alone is about 800 bytes. So the new frontmatter, plus the command file's frontmatter on OpenCode,
  must fit in 500 bytes; that is a requirement of this design, measured by `just footprint-regen` in the plan, and
  the budget file itself is not edited to make room. The short description still has to
  name Rust and Cargo and the verbs that should trigger it (set up, bootstrap, harden hooks, CI, secret scanning),
  because it competes with the owner's Bun skill and with generic tooling skills for the same request.
- **Cargo workspace:** the root `Cargo.toml` already excludes `shared/devtools`, so nothing under the skill folder
  joins the workspace.
- **Markdown checks:** this repository runs typos, mlc and a doc-path check (`lefthook.yml`, `Justfile`). Which of
  them reach `shared/devtools/skills/` and the emitted copies is not established here; the plan reads each one and
  the skill's markdown is written to pass whichever do (links as code spans, not relative links).
- **Versioning:** a `feat:` commit makes release-please bump the four devtools packages by a minor version. No tags
  are created by hand: the "first release order" steps in `shared/devtools/README.md` apply only to a plugin's first
  release.

## 6. Verified facts

Each was measured or read on 2026-11-24 and is reproducible.

- `cargo binstall --dry-run` succeeds for `just`, `cargo-nextest`, `cargo-deny`, `typos-cli` and `mlc`, and fails
  with "is not found" for `lefthook` and `gitleaks`. `cargo search lefthook` returns no such crate. This
  repository's `Justfile` `setup` recipe runs `cargo binstall ... lefthook ...`: the dry run of that line exits 76,
  and the same line without `lefthook` exits 0.
- lefthook v2.1.17 (source fetched at the tag, read as data): the generated hook skips a missing `rc` file silently
  (`internal/templates/hook.tmpl:13`), lets `$LEFTHOOK_BIN` win over everything (`hook.tmpl:18`), falls back to the
  path it was installed from (`hook.tmpl:36`), and with `assert_lefthook_installed` exits 1 when no lefthook is found
  (`hook.tmpl:100-104`). `min_version` is enforced by `lefthook run` (`internal/command/run.go:364-381`).
- Measured against the real lefthook 2.1.17 in a throwaway repository with a job that always fails:
  `LEFTHOOK_CONFIG=/nonexistent` and `LEFTHOOK_BIN=/bin/true` let the commit through without `rc.sh` and block it
  with `rc.sh` unsetting both; `lefthook install` exits 0 under `min_version: 99.0.0` while the commit then fails
  with "required lefthook version (99.0.0) is higher than current (2.1.17)"; with the installed copy still present
  and `PATH` stripped the hook still runs, after the copy is deleted it exits 1, and without
  `assert_lefthook_installed` it exits 0.
- Layout: a nested crate with its own `[workspace]` table inside a repository whose root workspace uses
  `members = ["crates/*"]` is not a member, runs under `--manifest-path`, writes its own `Cargo.lock` and `target/`,
  and a root `members` entry that names it fails with "multiple workspace roots".
- Timing, two runs each in a fresh no-dependency crate on an 8-CPU machine with a load average near 1.8 before the
  run (background load not controlled): `cargo fmt --check` 136 ms and 136 ms; cold `cargo clippy --all-targets -D
  warnings` 615 ms and 385 ms; cold `cargo nextest run` 1266 ms and 1393 ms; warm 625 ms and 268 ms.
- `cargo fmt --all --check` run at the root of that same layout reported an unformatted file in the member crate
  and none in the nested crate: it does not reach an excluded crate.
- A directory holding both `justfile` and `Justfile` makes `just a` fail with "multiple candidate justfiles found".
- With a global `core.hooksPath` set in a fake `HOME`, `lefthook install` exits 1 ("Custom hooks paths are not
  supported by default"); with `GIT_CONFIG_GLOBAL=/dev/null` it exits 0.
- `JUST_DRY_RUN=true just check` exits 0 and prints the recipe without running it; `JUST_JUSTFILE=/dev/null` fails
  closed ("justfile does not contain recipe"); a `just` call from an empty subdirectory still finds the parent's
  justfile, so `working-directory` does not change the result.
- `cargo deny --offline check bans licenses sources` on the nested crate with its `Cargo.lock` deleted regenerated the
  lock and ran; it exited 4 because the fixture crate was `unlicensed`, not because of the missing lock.
- `taiki-e/install-action`'s `manifests/` (read with `gh api`) contains `just`, `cargo-nextest`, `cargo-deny` and
  `typos`, and neither `gitleaks` nor `lefthook`.
- `docs/footprints/budgets.json`: `deltaBytes` 500 and `residentBytes` 2369, 2369 and 2388 for the claude-code,
  antigravity and opencode devtools packages.
- The Bun skill's hook jobs and recipe composition (section 4.2) were read in its `assets/template/lefthook.yml` and
  `justfile`.
- The repository's gates and conventions named in section 5 were read in `scripts/check-license-headers.sh`,
  `shared/devtools/README.md`, the `emit-devtools-skills` recipe and the root `Cargo.toml`.

## 7. Not established, left for the plan to read and measure

- Windows (Git Bash) and macOS behaviour of every part: `core.hooksPath` is not used, but `lefthook.exe` and
  `lefthook.bat` resolution, CRLF from `lefthook version` inside `just setup`, `git-common-dir` spelled `C:\` or
  `/c/` in `secrets-message`, `just` shell settings, symlink privileges on the Windows runner. Verified only by the
  end-to-end CI job, and the spec and the pull request will say so until it has run.
- How `cargo deny` behaves offline on a no-dependency fixture. The advisory database needs the network, so that
  gate test may have to cover `bans`, `licenses` and `sources` only.
- Whether lefthook and gitleaks install on all three CI runners with each OS's package manager, and what that costs.
- How this repository pins tool versions in CI (the install-pins work, PRs 163 to 166), so the template follows the
  same convention.
- The total run time of the toolchain suite, and whether the `SKILL.md` description fits the footprint cap.
- Whether the four bundlers and `tools/plugin-footprint` accept a skill with a `scripts/` directory and `.tmpl`
  assets (the `license-headers` skill is the precedent for scripts and assets).

## 8. Plan discipline

This is a spec, not a plan. The line-level plan (file contents, recipe bodies, test code) is written only after the
owner approves this spec and only against code that exists then, with every file, line and symbol cited by reading
it. Template contents are not fixed here.

## 9. Related defect, tracked separately

`Justfile:13` in this repository fails for the reason in section 6. It is recorded in `ROADMAP.md`
("just setup fails at its binstall line because lefthook is not a crates.io crate") and should be fixed the same way
the scaffold's `just setup` is built, once this design is agreed.

## 10. Rollout

One feature branch, one `feat:` commit group. Order: the source skill and templates, the emitted copies, the static
gate, the end-to-end CI job, footprint regeneration. `just skills-bundles` confirms the zips still build. Nothing is
claimed verified on a host or an OS that has not run it.

## 11. Stand-downs (adversarial panel review)

Round 1, solo seats (Axiom Breaker, Cascade Analyst, Mechanism Gamer, Literal Implementer, Dependency Cynic, State
Corruptor, Activation Auditor, Blindspot Auditor). Nothing was discarded below the floor and nothing is
unverified-accepted. Folded: the lefthook floor is written once (4.3); the excluded crate needs explicit `fmt`,
clippy, nextest and deny lines, `fmt` measured (4.4); `justfile` against `Justfile` collision, measured (4.5);
global git config isolation in the test repositories, measured (4.4); "CI runs `just check`" defined as a run
line, not a mention (4.4); seeded weakenings in the end-to-end script (4.5); hook jobs, recipe composition and
toolchain pins specified (4.2); the description must name Rust and Cargo (5). Blindspot Auditor: no new findings.

Round 1, agy escalation (nine findings, each measured or read before folding). FOLDED: clippy runs over the whole
workspace when a `.rs` file is staged (4.2); a step-level `env:` that `just` reads, `JUST_DRY_RUN`, turns the CI gate
into a green no-op, measured, and the gate tests now cover every workflow file (4.4); `toolchain-tests/` gets a
generated and committed `Cargo.lock` and an ignored `target/` (4.4); the `CARGO_*` glob in the test environment is
an explicit list (4.4); a CI install route for `gitleaks` and `lefthook`, which `taiki-e/install-action` does not
list, measured (4.2); the footprint cap is a stated requirement with the measured numbers (5). REJECTED, each with
its measurement: `working-directory` changing which justfile runs (an empty subdirectory still finds the parent's);
`cargo deny` crashing without a lockfile (it regenerated the lock and failed `licenses` on the fixture, exit 4);
"multi-minute" pre-push latency (a cold throwaway build is 1.3 to 1.4 s; folded as a plan-time measurement with a
CI-only fallback); OpenCode passing too few arguments to `apply.sh` (the command only loads the skill, the agent
finds the owner; folded as a wording fix). Nothing is discarded below the floor; nothing is unverified-accepted.
