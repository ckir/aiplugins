## [0.6.1] - 2026-09-01

### 🔧 Fixes

- *(claude-code)* Dispatch to the `.exe` when a bundled hook runs under Git Bash (#21)
  - A hook declared without `args` runs through a shell; where Git Bash is installed that shell executes the extensionless dispatcher rather than the sibling `.exe`, and 0.6.0's dispatcher rejected `MINGW64_NT` outright
  - The `SessionStart` hook failed with `unsupported operating system MINGW64_NT-10.0-26200`; MCP servers were unaffected, because a direct spawn resolves the `.exe`
  - Fixed for `MINGW*`, `MSYS*` and `CYGWIN*`; the bundles in 0.6.0 carry the broken dispatcher and are superseded by this release
- *(ci)* Remove `--force-local` from the bundling scripts, which macOS's bsdtar does not accept (#21)

### 🧪 Testing

- *(ci)* Start the assembled plugins, on every PR and after every release (#21)
  - `Bundle Smoke` stages a host-only bundle on ubuntu, windows and macOS and starts each entry point by BOTH routes Claude Code uses — through a shell, and spawned directly
  - `plugin-bundles.yml` gains a `verify` job that downloads the published zips and starts them on all three platforms
  - `scripts/check-bundle-dispatch.sh` drives all ten branches of the bin dispatcher with `uname` stubbed, since any one machine exercises only one of them

## [0.6.0] - 2026-09-01

### 🚀 Features

- *(claude-code)* Publish the plugins through a repo marketplace (#18)
  - The repository root is now the `aiplugins` marketplace: `claude plugin marketplace add ckir/aiplugins`, then `claude plugin install <plugin>@aiplugins`
  - Each release carries `<plugin>-plugin.zip`, a bundle holding the binaries for Windows x64 plus Linux and macOS on x86_64 and aarch64, so nothing is cloned or compiled at install time
  - `scripts/bundle-plugin.sh` + `.github/workflows/plugin-bundles.yml` build and attach those bundles; `scripts/check-marketplace.sh` guards the manifest against drift, in `just check` and CI

### 🔧 Fixes

- *(claude-code)* Sync the plugin manifests, which sat three releases behind at 0.2.1, and point `homepage`/`repository` at `ckir/aiplugins` rather than a repository that does not exist (#18)
- *(claude-code)* Repair the agent frontmatter in `re-analyst` and `todo-triager` (#18)
  - The `<example>` blocks sat between frontmatter keys as bare text, which is not valid YAML
  - `tools`, `model` and `color` were silently dropped at load; the agents ran with only their filename
- *(antigravity)* Sync the plugin manifest versions to the workspace version

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
