# agy-test-audit ledger

One row per completed AGY-TEST-AUDIT run: the audited range, how many rounds it took, the verdict, and
evidence a reader can check independently (the commits that closed gaps, or the review transcript). This is a
RECORD, not a proof.

| date | range | rounds | verdict | evidence |
|---|---|---|---|---|
| 2026-10-21 | 4aeeb54..be3fcd3 | 1 | GAPS FOUND, all 5 verified gaps closed (owner scoped all) | opencode-ctl-mcp, audited 4aeeb54..597d255. SC-1 was a source defect, fixed in d6f991a (capstone re-run, round 4 GREEN). CD-1 (max_turn_s boundary), PP-1 (session_create agent/model), MG-1 (opencode hook pushed text), CA-2 (Unix proc-guard release) pinned in be3fcd3; each new test went red under a logic mutant. CA-1 REJECTED: CreatedSet::contains never returns Err, so the guard_created IoError arm is unreachable. Transcript: agy cascade 5ce44626-6fbf-4c80-9e16-d7703dd4bc21 |
