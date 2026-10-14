# Todo: Per-Agent Category Restructure

## Task 1: Create category dirs + placeholder docs

**Description:** Create `plugins/ agents/ skills/ commands/ models/ themes/` under each of `claude-code/ antigravity/ qwen/ opencode/`, each non-plugins dir with `.gitkeep` + short `README.md` noting standalone components live there while plugin-embedded skills/agents stay inside `plugins/*/`.

**Acceptance criteria:**
- [x] All 4×6 category dirs exist
- [x] All non-plugins categories contain `.gitkeep` + `README.md`
- [x] No existing plugin files touched yet

**Verification:**
- [x] `ls` shows new dirs; `git status --porcelain` shows only new untracked dirs
- [x] Manual check: placeholder README wording

**Dependencies:** None

**Files likely touched:**
- `claude-code/{agents,skills,commands,models,themes}/README.md`
- `antigravity/{agents,skills,commands,models,themes}/README.md`
- `qwen/{agents,skills,commands,models,themes}/README.md`
- `opencode/{agents,skills,commands,models,themes}/README.md`

**Estimated scope:** Medium: ~20 small files

## Task 2: `git mv` plugins into `<agent>/plugins/`

**Description:** Move every current plugin dir whole into `<agent>/plugins/` via `git mv`: 3× claude-code, 3× antigravity, 3× qwen, 2× opencode (11 moves).

**Acceptance criteria:**
- [x] `claude-code/plugins/{example,re-ghidra-mcp-cc,rtk-mcp-cc}` exist with full contents (incl. embedded skills/agents/hooks)
- [x] `antigravity/plugins/{example,re-ghidra-mcp-agy,rtk-mcp-agy}` exist
- [x] `qwen/plugins/{example,re-ghidra-mcp-qwen,rtk-mcp-qwen}` exist
- [x] `opencode/plugins/{re-ghidra-mcp-opencode,rtk-mcp-opencode}` exist
- [x] No leftover plugin dirs at old paths

**Verification:**
- [x] `git status` shows renames (`R` entries)
- [x] Manual check: one embedded path still intact, e.g. `claude-code/plugins/re-ghidra-mcp-cc/skills/ghidra-re-driver/SKILL.md`

**Dependencies:** Task 1

**Files likely touched:**
- (moves only, no edits)

**Estimated scope:** Medium: 11 dirs

## Task 3: Update `Cargo.toml` workspace members

**Description:** Change members `claude-code/* antigravity/* qwen/*` → `claude-code/plugins/* antigravity/plugins/* qwen/plugins/*`; keep `shared/* tools/*`.

**Acceptance criteria:**
- [x] `cargo metadata --no-deps` succeeds and lists `re-ghidra-mcp-cc`, `rtk-mcp-cc`, `claude-example`, qwen/antigravity equivalents
- [x] No duplicate/excluded-package warnings

**Verification:**
- [x] `cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name' | sort`
- [x] `cargo nextest run --workspace --no-run` or `cargo metadata` passes

**Dependencies:** Task 2

**Files likely touched:**
- `Cargo.toml`

**Estimated scope:** Small: 1 file

## Checkpoint: Foundation
- [x] `cargo metadata --no-deps` green
- [x] `git status` shows renames, no content loss
- [x] Review with human before proceeding

## Task 4: Update `scripts/*.sh` globs + copy-pair paths

**Description:** Update all path globs: `check-plugin-wiring.sh` (`claude-code/*/hooks`, `antigravity/*/hooks.json`), `check-marketplace.sh` (`claude-code/$name`, `claude-code/*/`), `check-qwen-marketplace.sh` (`qwen/$name`, `qwen/*/`), `check-opencode-wiring.sh` (`opencode/*/`, 4 copy pairs, pin loop), `check-manifest-versions.sh` (3 loops), `bundle-plugin.sh` + `smoke-bundle.sh` (`src="$repo/claude-code/$plugin"`), `bundle-qwen-extension.sh` (`src="$repo/qwen/$ext"`), `smoke-opencode.sh` (`opencode/*/`).

**Acceptance criteria:**
- [x] No `claude-code/*/`-style bare globs remain where a plugin dir is meant (all point under `plugins/`)
- [x] Copy pairs in `check-opencode-wiring.sh` use `claude-code/plugins/...:opencode/plugins/...`
- [x] Each script still fails loudly on zero-match (existing guard preserved)

**Verification:**
- [x] `bash scripts/check-plugin-wiring.sh`
- [x] `bash scripts/check-marketplace.sh`
- [x] `bash scripts/check-qwen-marketplace.sh`
- [x] `bash scripts/check-opencode-wiring.sh`
- [x] `bash scripts/check-manifest-versions.sh`

**Dependencies:** Task 3

**Files likely touched:**
- `scripts/check-plugin-wiring.sh`
- `scripts/check-marketplace.sh`
- `scripts/check-qwen-marketplace.sh`
- `scripts/check-opencode-wiring.sh`
- `scripts/check-manifest-versions.sh`
- `scripts/bundle-plugin.sh`
- `scripts/bundle-qwen-extension.sh`
- `scripts/smoke-bundle.sh`
- `scripts/smoke-opencode.sh`

**Estimated scope:** Medium: ~9 files

## Task 5: Update `Justfile` recipes

**Description:** Update `build-*` staging paths, `clean` + `clean-stale` globs, `links` ignore-path, `test-opencode` + CI loops (`opencode/*/` → `opencode/plugins/*/`), `footprint-regen`/`footprint` measure paths + `readmes` glob, `emit-ghidra-skill` outputs.

**Acceptance criteria:**
- [x] `mkdir -p`/`cp` staging targets point at `<agent>/plugins/<name>/bin`
- [x] `footprint-regen` measures `claude-code/plugins/$plugin` + `opencode/plugins/*`
- [x] `emit-ghidra-skill` writes to the three new `skills/ghidra-re-driver/SKILL.md` paths

**Verification:**
- [x] `just --evaluate` or `just --list` parses
- [x] `grep -rn 'claude-code/\*\|opencode/\*/' Justfile` shows only intended remaining hits (none bare)

**Dependencies:** Task 4

**Files likely touched:**
- `Justfile`

**Estimated scope:** Small: 1 file

## Task 6: Update marketplace manifests + dependabot + workflows + footprint tool

**Description:** Update `.claude-plugin/marketplace.json` homepages → `claude-code/plugins/...`; `.qwen-plugin/marketplace.json` → `qwen/plugins/...`; `.github/dependabot.yml` dirs → `/opencode/plugins/...`; `.github/workflows/ci.yml` loops + readmes glob; `tools/plugin-footprint/src/main.rs` + `footprint-gate.rs` (`read_dir("opencode")` → `read_dir("opencode/plugins")`, dir branch `"opencode"`/`"claude-code"`); tests `real_plugin.rs`, `manifest.rs` fixture joins.

**Acceptance criteria:**
- [x] Marketplace homepage URLs resolve to new paths
- [x] `opencode_plugins()` enumerates `opencode/plugins/*`
- [x] CI `readmes=` glob covers new README locations
- [x] Footprint `measure` accepts new paths

**Verification:**
- [x] `bash scripts/check-marketplace.sh && bash scripts/check-qwen-marketplace.sh`
- [x] `cargo test -p plugin-footprint` passes
- [x] `grep -rn 'claude-code/re-ghidra\|opencode/rtk' tools/ .github/ | head` shows only updated paths

**Dependencies:** Task 5

**Files likely touched:**
- `.claude-plugin/marketplace.json`
- `.qwen-plugin/marketplace.json`
- `.github/dependabot.yml`
- `.github/workflows/ci.yml`
- `tools/plugin-footprint/src/main.rs`
- `tools/plugin-footprint/src/bin/footprint-gate.rs`
- `tools/plugin-footprint/src/manifest.rs`
- `tools/plugin-footprint/tests/real_plugin.rs`

**Estimated scope:** Medium: ~8 files

## Checkpoint: Wiring
- [x] All five wiring scripts green
- [x] `cargo test -p plugin-footprint` green
- [x] Review with human before proceeding

## Task 7: Update docs, README, gitignores

**Description:** Update root `README.md` structure section + doc-index links (`claude-code/rtk-mcp-cc` → `claude-code/plugins/rtk-mcp-cc`, etc.), `QWEN.md` if it carries paths, `.gitignore` (`opencode/*/bin/` → `opencode/plugins/*/bin/`, add `claude-code/plugins/*/bin/` if staged bins are ignored), `lefthook.yml`/`bacon.toml`/`typos.toml` if they glob agent paths, per-category `README.md` already created in Task 1, plus any `docs/` references.

**Acceptance criteria:**
- [x] No stale `](claude-code/<plugin>` / `](qwen/<ext>` / `](opencode/<plugin>` links remain
- [x] `just links` (mlc offline) passes
- [x] Per-category READMEs explain embedded-vs-standalone rule

**Verification:**
- [x] `grep -rn 'claude-code/re-ghidra\|claude-code/rtk\|qwen/re-ghidra\|qwen/rtk\|opencode/re-ghidra\|opencode/rtk' README.md QWEN.md docs/ --include='*.md'`
- [x] `mlc --offline` equivalent (`just links`) passes

**Dependencies:** Task 6

**Files likely touched:**
- `README.md`
- `QWEN.md`
- `.gitignore`
- `docs/**/*.md`

**Estimated scope:** Medium: 3-5 files

## Task 8: Full verification

**Description:** Run the repo's gate suite as far as possible locally: fmt check, wiring scripts, marketplace scripts, opencode wiring, footprint unit tests, workspace metadata build.

**Acceptance criteria:**
- [x] `cargo fmt --all -- --check` clean
- [x] All wiring/marketplace scripts green
- [x] `cargo nextest run --workspace` green (or documented subset if Ghidra-gated tests need env)
- [x] `git status` shows intended moves only, no orphans

**Verification:**
- [x] Tests pass: `cargo nextest run --workspace`
- [x] Build succeeds: `cargo metadata --no-deps`
- [x] Manual check: `git status --porcelain | head -50`

**Dependencies:** Task 7

**Files likely touched:**
- (verification only)

**Estimated scope:** Small: 0 files

## Checkpoint: Complete
- [x] All acceptance criteria met
- [x] Ready for review
