# agy-capstone ledger

One row per completed AGY-CAPSTONE run: the reviewed range, how many rounds it took, the verdict, and
evidence a reader can check independently (fold commits or the review transcript).

| date | range | rounds | verdict | evidence | set aside (anti-sweep) |
|---|---|---|---|---|---|
| 2026-10-21 | 4aeeb54..597d255 | 3 | GREEN (owner), peer ALIGNED | opencode-ctl-mcp. R1 found 2: SC-1 FOLDED in 597d255 (side_liveness: a dead older generation hid a fresh attach wrapper; pinned by liveness_reopened_tab_is_starting_over_a_dead_older_generation), SC-2 REJECTED (attach keeping generation n's files is spec design.md:345-347). R2 and R3 zero findings. Transcripts: agy cascade 5ce44626-6fbf-4c80-9e16-d7703dd4bc21 (R3 reply 5ce44626-...-877.md). Spot-checks found peer slips (R2 starting window 20 s vs 30 s; R3 STDERR_CAP 64 KiB vs 256 KiB) | proc-guard release() might fail on restricted Windows, and an exit code of 259 (STILL_ACTIVE) might fool is_alive: speculative, exited sentinels are checked first. R3: a restart onto a new port strands the MCP server: refuted, every call spawns `opencode api` (transport/cli.rs:622) |
| 2026-10-21 | 597d255..be3fcd3 | 1 | GREEN (owner), peer ALIGNED | opencode-ctl-mcp, round 4, re-opened because the agy-test-audit fold d6f991a changed source (side_liveness: a re-opened tab that exits before writing its tui-pid reads dead, pinned by liveness_reopened_tab_that_exited_before_its_pid_is_dead) and be3fcd3 added tests. Zero findings; spot-checks held (STARTING_MS 30_000 liveness.rs:359, attach candidates n+1..n+5 launcher/mod.rs:1209). Transcript: agy cascade 5ce44626-6fbf-4c80-9e16-d7703dd4bc21 | none set aside |
