# Implementation Plan: Review fixes

## Overview
Fix the findings from the repo-wide review (Rust safety/correctness, OpenCode TS correctness/perf/hardening, docs/wiring/CI drift). Thin vertical slices, each verified before the next.

Tasks tracked here (existing `tasks/plan.md`/`todo.md` belong to the completed restructure; preserved untouched).

## Task List

### Phase 1: Rust safety/correctness
- [x] Task 1: `logging.rs` unaligned TOKEN_USER cast (read_unaligned)
- [x] Task 2: Windows token-file ACL hardening (owner-only boot dir)
- [x] Task 3: `execute.rs` poison-mutex `unwrap()` → `into_inner`
- [x] Task 4: `worker_asset_extract` expect→Result; `qwen-example` version `env!`; `cli.rs` panic→error
- [x] Task 5: `Justfile` clean/clean-stale globs for qwen+agy

### Checkpoint: Phase 1
- [x] `cargo fmt --check`, `cargo clippy`, targeted `cargo test` green

### Phase 2: OpenCode TS
- [x] Task 6: Wrong-host skill docs (rtk-policy, gain, doctor) + wiring cmp scope fix
- [x] Task 7: `opencode.jsonc` relative-command note + package.json alignment (version text, test scripts, todov2 dup dep)
- [x] Task 8: rtk hook: `maxBuffer`, config-load cache, cwd note
- [x] Task 9: todov2 bounds + guards (maxTodos/caps, sessionID sanitize, hook guards, bool coerce, tui scan cap, trim unify, theme guards)

### Checkpoint: Phase 2
- [x] `bun test` per plugin green, `tsc --noEmit` where configured

### Phase 3: Docs/wiring/CI + tests
- [x] Task 10: `just check` includes `qwen-marketplace`; fix `check` comment; delete spent `bootstrap-package-tags.sh` (all 8 tags on origin); CRLF-harden `repo_url` oracle in 3 scripts
- [x] Task 11: Docs refresh: `QWEN.md` version+OpenCode+workflows+mlc+CI claim; `gen-marketplaces.sh` header; `HACK.md` scope claim
- [x] Task 12: agy Ghidra front minimal tests (preflight/settings parity with cc/qwen)
- [x] Task 13: Env-mutation serialisation for `paths`/`boot` tests

### Checkpoint: Complete
- [x] `cargo fmt --check`, wiring scripts (`check-plugin-wiring`, `check-marketplace`, `check-qwen-marketplace`, `check-opencode-wiring`), `bun test` green
- [x] Ready for review

## Risks
| Risk | Impact | Mitigation |
|------|--------|------------|
| Windows-only ACL code untestable on Linux CI | Med | Gate with `#[cfg(windows)]`, keep Unix path unchanged, verify compile |
| Over-broad `just check` change breaks local flow | Low | Additive only (add missing recipe to `check` line) |
| todov2 cap breaks existing tests | Low | Choose generous caps, update tests first |

## Open Questions
- None — caps chosen generously (maxTodos 100, field caps 10k/1k) to avoid breaking legit use.
