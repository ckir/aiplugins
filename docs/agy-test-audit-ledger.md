# agy-test-audit ledger

One row per completed AGY-TEST-AUDIT run: the audited range, how many rounds it took, the verdict, and
evidence a reader can check independently (the commits that closed gaps, or the review transcript). This is a
RECORD, not a proof.

| date | range | rounds | verdict | evidence |
|---|---|---|---|---|
| 2026-10-21 | 4aeeb54..be3fcd3 | 1 | GAPS FOUND, all 5 verified gaps closed (owner scoped all) | opencode-ctl-mcp, audited 4aeeb54..597d255. SC-1 was a source defect, fixed in d6f991a (capstone re-run, round 4 GREEN). CD-1 (max_turn_s boundary), PP-1 (session_create agent/model), MG-1 (opencode hook pushed text), CA-2 (Unix proc-guard release) pinned in be3fcd3; each new test went red under a logic mutant. CA-1 REJECTED: CreatedSet::contains never returns Err, so the guard_created IoError arm is unreachable. Transcript: agy cascade 5ce44626-6fbf-4c80-9e16-d7703dd4bc21 |
| 2026-10-22 | 3756da1..2a04784 | 1 | GAPS FOUND, all 5 verified gaps closed (owner scoped all) | spec-panel-review gate. Gaps measured green on a mutated tree, then FOLDED in 2a04784: no committed negative controls (now check-devtools-skills.sh --mutation-test, 17 seeded defects, wired into just and CI), skill frontmatter checked as a whole-file grep, a command could name another skill and pass, the OpenCode name rewrite also hit a body line (and the gate repeated the same transform), the emit recipe never ran in CI (CI now runs it and requires no change). Each new case is non-vacuous: four logic mutants of the gate turned their named case red. Source changed, so the capstone was re-run (round 4 GREEN, see docs/agy-capstone-ledger.md). Brief: .clavity/seams/spec-panel-test-audit.md; transcript: agy cascade f22c53a7-9bbe-4a13-96be-e1a3a2da12fd |
