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
# Needs: git, cargo, rustup, just, lefthook, gitleaks, cargo-nextest, cargo-deny, typos.
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

# A clean environment for every git, cargo and just command. These are the families the
# scaffold's own test helpers drop (toolchain-tests/tests/common/mod.rs, spec 4.4): a value
# left in the caller's shell must not change what is measured. An exported JUST_DRY_RUN, for
# one, makes `just check` print its recipes and exit 0 without running a single gate.
for name in $(compgen -v); do
  case "$name" in
    GIT_* | LEFTHOOK* | GITLEAKS_* | JUST_* | HOOK_ARG_* | NEXTEST_* | TYPOS_* | CARGO_BUILD_* | CARGO_PROFILE_*) unset "$name" ;;
  esac
done
unset CDPATH BASH_ENV ENV RUSTFLAGS CARGO_ENCODED_RUSTFLAGS CARGO_TARGET_DIR RUSTC_WRAPPER RUSTUP_TOOLCHAIN
export GIT_CONFIG_GLOBAL="$work/empty" GIT_CONFIG_NOSYSTEM=1
: > "$work/empty"
export GIT_AUTHOR_NAME=scaffold-test GIT_AUTHOR_EMAIL=scaffold-test@example.invalid
export GIT_COMMITTER_NAME=scaffold-test GIT_COMMITTER_EMAIL=scaffold-test@example.invalid

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

# in_dir <dir> <command...>: run a command in a directory without leaving this one.
in_dir() { ( cd "$1" && shift && "$@" ); }

# gates <dir> <label>: commit the scaffold, install the hooks, then run the scaffolded
# repository's own `just check`, which includes the toolchain suite.
gates() {
  local dir=$1 label=$2
  # deny.toml was edited above on purpose in the first repository; start from the shipped one.
  cp "$skill/assets/template/deny.toml.tmpl" "$dir/deny.toml"
  git -C "$dir" add -A
  git -C "$dir" commit -q --no-verify -m "scaffold"
  expect "$label: the pinned toolchain is installed" in_dir "$dir" rustup toolchain install
  expect "$label: just setup installs the hooks" in_dir "$dir" just setup
  if in_dir "$dir" just check >"$work/check.out" 2>&1; then
    ok "$label: just check passes on the scaffolded repository"
  else
    tail -40 "$work/check.out" >&2
    bad "$label: just check failed on the scaffolded repository"
  fi
}

echo "== the scaffolded single-crate repository runs its own gates"
gates "$r1" "single crate"
echo "== the scaffolded workspace repository runs its own gates"
gates "$r3" "workspace"

cd "$r1"

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
