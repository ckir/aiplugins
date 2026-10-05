## [0.7.1] - 2026-10-05

The v0.7.0 tag predates #39, #41, #42 and #44, so this is the first
release whose artifacts contain the OpenCode V2 support and footprint
gates the 0.7.0 entry describes. On top of that code:

### 🐛 Bug Fixes

- *(rtk-mcp-agy)* Fix hooks.json schema and wire Antigravity plugin binaries (#39)
- *(qwen)* Align marketplace copies with extension manifests and exclude the example (#44)
- Correct stale footprint documents and published README regions (#44)
- *(opencode)* Condense re-ghidra plugin.ts comments to fit the setup delta cap (#44)

### 📚 Documentation

- *(rtk-mcp-opencode)* Complete truncated INSTALL.md merge rules and verify step (#44)

### ⚙️ Miscellaneous Tasks

- Add dependabot configuration (#42)

## [0.7.0] - 2026-10-04

### 🚀 Features

- *(ghidra-mcp)* Serve without a resolved Ghidra configuration
- *(plugin-footprint)* Scaffold the crate with canonical bytes and manifest reading
- *(plugin-footprint)* Probe a live MCP server for what it advertises
- *(plugin-footprint)* Emit the footprint document and a CLI to produce it
- *(plugin-footprint)* Split frontmatter from body
- *(plugin-footprint)* Read skill and agent sources from disk
- *(plugin-footprint)* Count skill and agent frontmatter as resident
- *(plugin-footprint)* Commit footprint documents and ratcheted budgets
- *(plugin-footprint)* The gate's five layers
- *(plugin-footprint)* The gate binary, baselined on the merge base
- *(plugin-footprint)* Publish the byte figures, honestly labelled
- *(ghidra-mcp)* Serve without a resolved Ghidra configuration
- *(plugin-footprint)* Scaffold the crate with canonical bytes and manifest reading
- *(plugin-footprint)* Probe a live MCP server for what it advertises
- *(plugin-footprint)* Emit the footprint document and a CLI to produce it
- *(plugin-footprint)* Split frontmatter from body
- *(plugin-footprint)* Read skill and agent sources from disk
- *(plugin-footprint)* Count skill and agent frontmatter as resident
- *(plugin-footprint)* Commit footprint documents and ratcheted budgets
- *(plugin-footprint)* The gate's five layers
- *(plugin-footprint)* The gate binary, baselined on the merge base
- *(opencode)* Rtk-mcp-opencode hook with fail-open delegate and settings
- *(opencode)* Re-ghidra-mcp-opencode port with session hooks and agent
- *(footprint)* Read opencode.jsonc MCP servers for measurement
- *(footprint)* Measure opencode plugins with setup tier and budgets

### 🐛 Bug Fixes

- Add install scripts to bypass broken qwen extensions install (#35)
- *(ghidra-mcp)* Report which configuration is wrong, not always Ghidra
- *(ghidra-mcp)* Stop config errors colliding with PROGRAM_NOT_FOUND
- *(re-ghidra-mcp-cc)* Retune the doctor skill for the new failure shape
- *(plugin-footprint)* Close three routes to a confidently wrong number
- *(plugin-footprint)* Normalising a path must not change what running it means
- *(plugin-footprint)* Keep the recorded binary honest about what was probed
- *(plugin-footprint)* Make "declared wins" true rather than merely documented
- *(plugin-footprint)* Complete two guards that only covered ASCII
- *(plugin-footprint)* Close four gate holes found by the capstone review
- *(plugin-footprint)* Close three more gate holes, capstone round 2
- *(plugin-footprint)* The ratchet's untested edges, capstone round 3
- *(plugin-footprint)* Enforce Fork 2's D < H, refuse lossy source names
- *(plugin-footprint)* Normalise line endings before measuring
- *(plugin-footprint)* Two tests that assumed the author's platform
- *(plugin-footprint)* The non-UTF-8 test cannot create its own fixture on macOS
- The freshness check needs BOTH git diff and ls-files, not either
- *(ghidra-mcp)* Report which configuration is wrong, not always Ghidra
- *(ghidra-mcp)* Stop config errors colliding with PROGRAM_NOT_FOUND
- *(re-ghidra-mcp-cc)* Retune the doctor skill for the new failure shape
- *(plugin-footprint)* Close three routes to a confidently wrong number
- *(plugin-footprint)* Normalising a path must not change what running it means
- *(plugin-footprint)* Keep the recorded binary honest about what was probed
- *(plugin-footprint)* Make "declared wins" true rather than merely documented
- *(plugin-footprint)* Complete two guards that only covered ASCII
- *(plugin-footprint)* Close four gate holes found by the capstone review
- *(plugin-footprint)* Close three more gate holes, capstone round 2
- *(plugin-footprint)* The ratchet's untested edges, capstone round 3
- *(plugin-footprint)* Enforce Fork 2's D < H, refuse lossy source names
- *(plugin-footprint)* Normalise line endings before measuring
- *(plugin-footprint)* Two tests that assumed the author's platform
- *(plugin-footprint)* The non-UTF-8 test cannot create its own fixture on macOS
- The freshness check needs BOTH git diff and ls-files, not either
- *(opencode)* Final-review wave — handler guards, naming, tests, pins
- Pin LF for ghidra skill source and emit copies so emit byte-identity holds on Windows
- Pin LF for all skill/agent sources so byte-compare gates hold on Windows
- *(rtk)* Add -- separator to rtk command invocations to prevent flag parsing collisions

### 💼 Other

- *(opencode)* Pin V2 shell execute.before shape; mutation works (opencode v2.0.20, @opencode/plugin 2.0.21)

### 📚 Documentation

- *(qwen)* Fix install URLs to use extension bundles, add manual workaround (#34)
- *(doctor)* Rule config out of the "died on launch" branch explicitly
- Add the plugin-footprint spec the crate was built from
- Add the implementation plan for the file-backed sources and the gate
- Record the qwen extensions install bug this repo works around
- *(plugin-footprint)* The resident tier is no longer a lower bound
- *(plugin-footprint)* Name the recovery path for an invalid base policy
- *(plugin-footprint)* Record why D < H is refused rather than clamped
- *(rtk-mcp-agy)* Update installation instructions to use agy plugin install via github
- *(re-ghidra-mcp-agy)* Update installation instructions to use agy plugin install via github
- *(doctor)* Rule config out of the "died on launch" branch explicitly
- Add the plugin-footprint spec the crate was built from
- Add the implementation plan for the file-backed sources and the gate
- Record the qwen extensions install bug this repo works around
- *(plugin-footprint)* The resident tier is no longer a lower bound
- *(plugin-footprint)* Name the recovery path for an invalid base policy
- *(plugin-footprint)* Record why D < H is refused rather than clamped
- *(re-ghidra-mcp-agy)* Align repo-scoped install instructions with rtk-mcp-agy
- Align DESIGN.md across plugins and update config instructions
- *(spec)* OpenCode V2 support — opencode/ dir plus port of both plugins
- *(plan)* Drop frozen-lockfile variant from test-opencode recipe
- *(opencode)* Rtk-mcp-opencode config, skills, settings template, README
- *(opencode)* Agent-fetchable INSTALL.md per plugin plus root pointer
- *(opencode)* README Installing sections cover the @opencode/plugin dependency

### 🧪 Testing

- *(e2e)* Stop an unconfigured-serve panic orphaning the server
- *(plugin-footprint)* Snapshot each plugin's source breakdown
- *(e2e)* Stop an unconfigured-serve panic orphaning the server
- *(plugin-footprint)* Snapshot each plugin's source breakdown
- *(rtk)* Harden mock_rtk to strictly verify -- separator and add regression tests for flag parsing

### ⚙️ Miscellaneous Tasks

- Gate every pull request on plugin footprint
- Pin the generated footprint documents to LF
- Gate every pull request on plugin footprint
- Pin the generated footprint documents to LF
- Track opencode v2 plan; ignore superpowers scratch
- Ignore node_modules so typos hook skips third-party code
- Pin LF for opencode sources read by line-oriented checks
- *(opencode)* Wiring gate, bun tests, just recipes, CI jobs, README
- *(opencode)* Setup-path smoke harness across the 3-OS matrix
## [0.6.4] - 2026-09-01

### 🚀 Features

- *(qwen)* Add extension bundle workflow and marketplace manifest (#31)

### 🐛 Bug Fixes

- *(qwen)* Extract binary name from \${/} path separator, not /bin/ (#33)

### 📚 Documentation

- Add .qwenignore and QWEN.md project context (#30)
## [0.6.3] - 2026-09-01

### 📚 Documentation

- *(ghidra)* Why an empty get_xrefs is a fact, and why Rust strings cause it (#28)
## [0.6.2] - 2026-09-01

### 🐛 Bug Fixes

- *(ci)* Create the parent directory the verify job extracts into (#23)

### 📚 Documentation

- *(rtk)* Explain the false "No hook installed" notice, and how to silence it (#25)
- *(re-ghidra)* Teach the doctor three things a real run got wrong (#26)

### ⚙️ Miscellaneous Tasks

- Declare the aiplugins marketplace for this repository (#24)
## [0.6.1] - 2026-09-01

### 🐛 Bug Fixes

- *(ci)* Trigger the plugin bundles from the Release run, not the release (#20)
- *(claude-code)* Dispatch to the .exe under Git Bash, and start the assembled plugins in CI (#21)
## [0.6.0] - 2026-09-01

### 🚀 Features

- *(claude-code)* Publish the plugins through a repo marketplace (#18)

### 🐛 Bug Fixes

- *(qwen)* Sync extension manifest versions to 0.5.0 (#16)

### 📚 Documentation

- *(qwen)* Add CLI installation instructions to extension READMEs (#17)
## [0.5.0] - 2026-08-31

### 🐛 Bug Fixes

- *(qwen)* Switch extension manifests to pre-built binaries (#14)
## [0.4.0] - 2026-08-31

### 🚀 Features

- *(ghidra)* Port ghidrust as shared/ghidra-* crates and a Claude Code plugin (#5)
- *(qwen)* Port re-ghidra-mcp as a Qwen Code extension (#9)
- *(antigravity)* Implement re-ghidra-mcp-agy plugin (#10)
- *(ghidra-worker-ctl)* Implement Linux/macOS JVM lifecycle via process groups (#12)

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
