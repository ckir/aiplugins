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
