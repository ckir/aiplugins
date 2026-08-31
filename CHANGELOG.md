## [0.5.0] - 2026-09-01

### 🔧 Fixes

- *(qwen)* Switch extension manifests to use pre-built binaries from release artifacts (#14)
  - `re-ghidra-mcp-qwen`: version 0.2.1 → 0.5.0, uses `${extensionPath}/bin/re-ghidra-qwen-mcp`
  - `rtk-mcp-qwen`: uses `${extensionPath}/bin/rtk-mcp-qwen`
  - Extensions now installable via `qwen extensions install` without requiring `cargo`

## [0.4.0] - 2026-08-31

### 🚀 Features

- *(ghidra-worker-ctl)* Implement Linux/macOS JVM lifecycle via process groups (#12)
  - Unix process group kill-guard replaces Windows-only Job Object
  - `setsid()` + `SIGKILL` to process group ensures clean JVM teardown on Linux/macOS
  - Cross-platform compilation verified for Windows, Linux, and macOS

## [0.3.0] - 2026-08-31

### 🚀 Features

- *(ghidra)* Port ghidrust as shared/ghidra-* crates and a Claude Code plugin (#5)
- *(qwen)* Port re-ghidra-mcp as a Qwen Code extension (#9)
- *(antigravity)* Implement re-ghidra-mcp-agy plugin (#10)

### 🧪 Testing

- *(ghidra)* Derive fixture addresses at runtime instead of hardcoding them (#7)
- *(ghidra)* Stop the cold-call warming test asserting machine speed (#8)

### ⚙️ Miscellaneous Tasks

- *(ghidra)* Pin LLVM for the live E2E fixture and update all actions (#6)

## [0.2.1] - 2026-08-29

### 🚜 Refactor

- *(qwen)* Rename the qwen-bridge package to rtk-mcp-qwen (#3)

### ⚙️ Miscellaneous Tasks

- Release 0.2.1 (#4)

## [0.2.0] - 2026-08-29

### 🚀 Features

- *(claude-code)* Add rtk-mcp-cc plugin

### 🚜 Refactor

- *(agy)* Extract pure logic into a lib, add tests, fix portability

### 📚 Documentation

- *(qwen)* Add installation and update instructions to README
- List scripts/ in the repository structure (#1)

### 🧪 Testing

- *(qwen)* Cover exit code 3 and the updated_input omission

### ⚙️ Miscellaneous Tasks

- Add cross-platform test matrix, plugin wiring check, clean recipes
- Track local agent tooling and skill lockfile
- Stop tracking Qwen per-session scratch
- Ignore per-developer Claude Code state
- Release 0.2.0 (#2)

## [0.1.5] - 2026-08-29

### 🚀 Features

- Add --version and --help to all hook/MCP binaries

### ⚙️ Miscellaneous Tasks

- Bump workspace version to 0.1.5

## [0.1.4] - 2026-08-29

### 🐛 Bug Fixes

- Exclude example packages from dist releases with dist = false

### ⚙️ Miscellaneous Tasks

- Bump workspace version to 0.1.4

## [0.1.3] - 2026-08-29

### 🚀 Features

- *(claude-code)* Add full example plugin in Rust

### 🐛 Bug Fixes

- *(qwen)* Exclude mock-rtk test fixture from release artifacts

### 🚜 Refactor

- Rename opaque folders to self-document content, exclude examples from releases

### ⚙️ Miscellaneous Tasks

- Expand ignore rules and add .antigravityignore
- Stop tracking agent-local config files
- Bump workspace version to 0.1.3

## [0.1.2] - 2026-08-28

### 🐛 Bug Fixes

- Allow dirty ci file for cargo-dist

### ⚙️ Miscellaneous Tasks

- Release

## [0.1.1] - 2026-08-28

### 🚀 Features

- *(qwen)* Add bridge extension with PreToolUse rtk command rewriter

### ⚙️ Miscellaneous Tasks

- Setup workspace dev tools, CI, dist, and e2e tests
- Release
