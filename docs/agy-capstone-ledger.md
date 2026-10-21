# agy-capstone ledger

One row per completed AGY-CAPSTONE run: the reviewed range, how many rounds it took, the verdict, and
evidence a reader can check independently (fold commits or the review transcript).

| date | range | rounds | verdict | evidence | set aside (anti-sweep) |
|---|---|---|---|---|---|
| 2026-10-21 | 4aeeb54..597d255 | 3 | GREEN (owner), peer ALIGNED | opencode-ctl-mcp. R1 found 2: SC-1 FOLDED in 597d255 (side_liveness: a dead older generation hid a fresh attach wrapper; pinned by liveness_reopened_tab_is_starting_over_a_dead_older_generation), SC-2 REJECTED (attach keeping generation n's files is spec design.md:345-347). R2 and R3 zero findings. Transcripts: agy cascade 5ce44626-6fbf-4c80-9e16-d7703dd4bc21 (R3 reply 5ce44626-...-877.md). Spot-checks found peer slips (R2 starting window 20 s vs 30 s; R3 STDERR_CAP 64 KiB vs 256 KiB) | proc-guard release() might fail on restricted Windows, and an exit code of 259 (STILL_ACTIVE) might fool is_alive: speculative, exited sentinels are checked first. R3: a restart onto a new port strands the MCP server: refuted, every call spawns `opencode api` (transport/cli.rs:622) |
