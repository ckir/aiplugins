# Implementation Plan: Per-Agent Category Restructure

## Overview
Restructure the four agent dirs (`claude-code/`, `antigravity/`, `qwen/`, `opencode/`) so each exposes the six categories `plugins/`, `agents/`, `skills/`, `commands/`, `models/`, `themes/`. Existing whole plugins move under `plugins/` with `git mv` (history preserved); embedded `skills/`/`agents/` inside plugins stay intact per user decision. Empty categories are created with `.gitkeep` + `README.md`. `shared/` and `tools/` stay untouched. All path-dependent configs, scripts, workflows, manifests, and docs are updated to the new paths.

## Architecture Decisions
- Per-agent categories (`claude-code/plugins/...`), not top-level categories — per user answer.
- Keep embedded: no extraction of `skills/`/`agents/` out of plugins. Sibling `skills/`, `agents/`, `commands/` start as documented placeholders for future standalone components.
- `git mv` for every move, one agent per commit-group, so history follows.
- Cargo workspace members change from `claude-code/*` style to `claude-code/plugins/*` style; `shared/*`, `tools/*` unchanged.
- Scripts/CI updated by glob replacement (`claude-code/*/` → `claude-code/plugins/*/`, etc.), not by hardcoding plugin names, so future plugins need no script edit.
- Marketplace `homepage` URLs updated to `/tree/main/<agent>/plugins/<name>`.

## Target Tree (after)
```text
claude-code/{plugins/{example,re-ghidra-mcp-cc,rtk-mcp-cc},agents,skills,commands,models,themes}
antigravity/{plugins/{example,re-ghidra-mcp-agy,rtk-mcp-agy},agents,skills,commands,models,themes}
qwen/{plugins/{example,re-ghidra-mcp-qwen,rtk-mcp-qwen},agents,skills,commands,models,themes}
opencode/{plugins/{re-ghidra-mcp-opencode,rtk-mcp-opencode},agents,skills,commands,models,themes}
shared/... (untouched)
tools/... (untouched)
```

## Task List

### Phase 1: Foundation — moves
- [ ] Task 1: Create category dirs + placeholder docs
- [ ] Task 2: `git mv` plugins into `<agent>/plugins/`
- [ ] Task 3: Update `Cargo.toml` workspace members + verify `cargo metadata`

### Checkpoint: Foundation
- [ ] `cargo metadata --no-deps` lists all moved crates, no missing members
- [ ] `git status` shows renames (not deletes+adds without history)

### Phase 2: Wiring — scripts, manifests, CI
- [ ] Task 4: Update `scripts/*.sh` globs + copy-pair paths
- [ ] Task 5: Update `Justfile` recipes (build, clean, footprint, test-opencode, emit-skill, links)
- [ ] Task 6: Update marketplace manifests + dependabot + workflows + footprint tool paths

### Checkpoint: Wiring
- [ ] `bash scripts/check-plugin-wiring.sh`, `check-marketplace.sh`, `check-qwen-marketplace.sh`, `check-opencode-wiring.sh`, `check-manifest-versions.sh` all green
- [ ] `cargo nextest run --workspace` green (or at least metadata + targeted `skill_emit` tests)

### Phase 3: Docs + final verification
- [ ] Task 7: Update root `README.md`, `QWEN.md`, `.gitignore`, per-category READMEs, leftover `docs/` references
- [ ] Task 8: Full `just check`-adjacent verification (fmt, wiring, marketplace, opencode-wiring, links)

### Checkpoint: Complete
- [ ] All acceptance criteria met
- [ ] Ready for review

## Risks and Mitigations
| Risk | Impact | Mitigation |
|------|--------|------------|
| Missed hardcoded path in scripts/CI/workflows | High — silent green check that checks nothing, or red CI | Grep for `claude-code/`, `qwen/`, `opencode/`, `antigravity/` after moves; wiring scripts fail on zero-match by design — trust that |
| Cargo workspace glob over-matches `plugins/` dir itself | Med — `cargo metadata` error | Use explicit `plugins/*` members, verify with `cargo metadata` |
| `bin/` staged binaries + `.gitignore` (`opencode/*/bin/`) go stale | Med — binaries ignored at wrong path | Update to `opencode/plugins/*/bin/` + `claude-code/plugins/*/bin/` if present; verify `git check-ignore` |
| Marketplace homepage URLs drift | Med — installs work but links 404 | Update both marketplace.json files + README doc-index links |
| Footprint tool `measure <path>` + `opencode_plugins()` read_dir | High — footprint gate fails | Update `tools/plugin-footprint/src/main.rs`, `footprint-gate.rs`, tests (`real_plugin.rs`, `manifest.rs`) |

## Open Questions
- None — user confirmed layout, embedded handling, empty-category docs, shared/tools untouched.
