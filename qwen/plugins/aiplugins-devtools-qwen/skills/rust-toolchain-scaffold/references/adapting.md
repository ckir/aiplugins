# When the repository does not match the template

The templates assume: Rust and Cargo, GitHub, one default branch called `main`, a root crate or a
workspace, and no tooling of their own yet.

| Difference | What to change |
|---|---|
| The repository already has a `justfile`, `Justfile` or `.justfile` | `apply.sh` counts any spelling as present and skips it. Merge recipe by recipe; the tests name the recipes they need (`setup`, `fmt`, `fmt-check`, `clippy`, `lint`, `test`, `test-toolchain`, `deny`, `spellcheck`, `secrets`, `secrets-staged`, `secrets-message`, `check-fast`, `check`). Keep one spelling: `just` fails when two coexist. The CODEOWNERS template lists `justfile`, `Justfile` and `.justfile`. |
| It already has `deny.toml`, `typos.toml`, `.gitattributes` or `rust-toolchain.toml` | `apply.sh` skips them. Merge in what the tests read: `deny.toml` needs `[licenses.private]` with `ignore = true`; `.gitattributes` needs the LF pins; `rust-toolchain.toml` needs a channel with the `rustfmt` and `clippy` components. |
| The root `Cargo.toml` is a workspace | Add `toolchain-tests` to its `exclude` list if a `members` glob could match it. The workspace's own crates keep being checked with `--workspace`. |
| Existing husky, pre-commit or `.git/hooks` scripts | Port each hook into `lefthook.yml` as a job that calls a `just` recipe, then remove the old runner. Hooks installed twice run twice or fight. |
| An existing CI workflow, or several | `apply.sh` skips an existing `ci.yml` and prints the other workflow files. The gate tests check every workflow for an `env:` key from the `JUST_`, `GITLEAKS_`, `LEFTHOOK`, `HOOK_ARG_`, `NEXTEST_` and `TYPOS_` families and for a job named `check`, and check `ci.yml` strictly. Rename a clashing job in another workflow. Pinning actions to a SHA in the other workflows is advice, not a tested rule. |
| The CI workflow needs write permissions, a container or services | The strict rules apply to `ci.yml`. Put that work in another workflow, or change the specific assertion in `gate.rs`, say why in a comment, and prove the changed test still fails on the cases it should. |
| Code that already fails `cargo fmt --check`, clippy `-D warnings` or `cargo deny` | Run `just fmt`, report the rest, and let the user choose between fixing the code and relaxing a setting. Never loosen a gate quietly; if one is loosened, update the test that pins it. |
| Not on GitHub | Drop `.github/` and the workflow and CODEOWNERS tests in `gate.rs`; keep the lefthook half. Recreate "CI runs `just check`" on the other platform. |
| Another hook runner, or none | Out of scope for the templates as written: the recipes, the tests and `rc.sh` assume lefthook. Say so rather than half-porting. |
| A new tool config file appears | Add it to `TOOLCHAIN_FILES` in `toolchain-tests/tests/common/mod.rs`, to the `toolchain-tests` glob in `lefthook.yml`, and consider CODEOWNERS. |
| A `justfile` spelled in another case (`JUSTFILE`) | `just` accepts it, but CODEOWNERS and the tests know three spellings. Rename it. |
| Markdown links should not rot | Add `mlc`, the markup link checker (`cargo install mlc`), as a recipe and a CI step once the repository has documentation worth protecting. Offline mode checks relative links only: `mlc --offline .`; it exits 1 and lists each broken link. `--ignore-path` skips directories such as `target`. Add the recipe to `check-fast` and to the CI workflow's tool installs. |
| Every source file should carry a license header | Add it when the repository's license asks for one. The scaffold ships none. A check that needs no extra tool lists the files without the marker, for example `grep -rL 'SPDX-License-Identifier' --include='*.rs' src` (the files it prints lack it); wrap it in a recipe that fails when the output is not empty, and add it to `check-fast`. `cargo deny` checks dependency licenses, not your source files. |
| The repository keeps check scripts that the gate must run (named `check-*.sh`, for example) | Add a wiring check once the repository has several: a script that fails when a check script is named in neither the justfile nor the CI workflow. A minimal form: `for f in scripts/check-*.sh; do grep -q -- "$f" justfile || echo "not wired: $f"; done`, failing when it prints anything; run the same loop against `.github/workflows/ci.yml`. A check that exists but is never run gives no protection. |

After any adaptation: `just check`, then weaken the adapted gate once to see its test fail.
