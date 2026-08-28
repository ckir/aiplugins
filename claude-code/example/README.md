# claude-example

A complete, working Claude Code plugin implemented in Rust — and the template to
copy when starting a new one. It exercises **every extension point a Claude Code
plugin has**, wired to a tested Rust crate rather than to shell scripts.

The plugin itself does something small and coherent: it tracks `TODO`, `FIXME`
and `HACK` markers, and enforces the house rule that every marker names an owner.

## What it demonstrates

| Extension point | File | What it does here |
|---|---|---|
| Manifest | `.claude-plugin/plugin.json` | Name, version, metadata |
| MCP server | `.mcp.json` → `bin/claude-example-mcp` | Two tools: `scan_todos`, `check_text` |
| Hook | `hooks/hooks.json` → `bin/claude-example-hook` | `PostToolUse` on `Write`/`Edit`, reports unowned markers |
| Skill (auto) | `skills/todo-conventions/SKILL.md` | The marker conventions, loaded when relevant |
| Skill (invoked) | `skills/todos/SKILL.md` | `/claude-example:todos` |
| Agent | `agents/todo-triager.md` | Triages a marker backlog into a plan |
| Settings | `examples/claude-example.local.md` | Per-project config via `.claude/*.local.md` |

> **No context file.** Unlike the Qwen extensions in this repo (`contextFileName`
> in `qwen-extension.json`), Claude Code plugins have no auto-loaded context
> file. Plugin documentation lives in this README; instructions meant for Claude
> belong in a skill.

## Layout

```text
claude-code/example/
├── .claude-plugin/plugin.json   # manifest (must be in .claude-plugin/)
├── .mcp.json                    # MCP server registration
├── hooks/hooks.json             # hook registration
├── skills/
│   ├── todo-conventions/SKILL.md
│   └── todos/SKILL.md
├── agents/todo-triager.md
├── examples/
│   └── claude-example.local.md  # settings template
├── bin/                         # built binaries (gitignored)
├── src/
│   ├── lib.rs                   # marker scanning + settings (pure, tested)
│   ├── hook.rs                  # hook decision logic (pure, tested)
│   └── bin/
│       ├── mcp.rs               # MCP server binary
│       └── hook.rs              # hook binary
└── tests/e2e.rs                 # drives both real binaries
```

Component directories sit at the plugin **root**, not inside `.claude-plugin/`.
Only the manifest goes in `.claude-plugin/`.

## Building

The config files point at `bin/`, so the binaries must be built before the
plugin does anything:

```bash
just build-claude-example
```

Windows developers build locally with that recipe. The other platforms come from
`cargo-dist`, which already picks both binaries up automatically — `dist plan`
lists `claude-example-mcp` and `claude-example-hook` for all five configured
targets, so no bespoke CI job is needed. `bin/` is gitignored; build artifacts
are not committed.

Note that `.mcp.json` and `hooks/hooks.json` reference the binaries **without a
file extension**:

```json
"command": "${CLAUDE_PLUGIN_ROOT}/bin/claude-example-mcp"
```

That is deliberate and portable: Windows `CreateProcess` appends `.exe` when the
path has no extension, so one string works on all three platforms.

## Installing

```bash
claude --plugin-dir /path/to/aiplugins/claude-code/example
```

Then confirm it loaded:

- `/mcp` → the `claude-example` server, with `scan_todos` and `check_text`
- `/help` → `/claude-example:todos`
- `claude --debug` → the `PostToolUse` hook firing on a `Write`

Hooks and MCP servers load at session start; after changing `hooks.json`,
`.mcp.json`, or rebuilding a binary, restart Claude Code.

## Configuring

Copy `examples/claude-example.local.md` to `.claude/claude-example.local.md` in
your project:

```markdown
---
require_owner: true
kinds: [TODO, FIXME, HACK]
max_results: 200
---
```

Every key is optional, and a malformed value falls back to its default rather
than breaking the session.

## Testing

```bash
cargo nextest run -p claude-example
```

41 tests, in two layers:

- **Unit tests** (`src/`) pin the decisions — marker parsing, owner detection,
  settings, and what the hook chooses to report.
- **E2E tests** (`tests/e2e.rs`) pin the contracts — the hook's stdout JSON, and
  the MCP protocol, driven through rmcp's own client against the real server
  binary. These catch the failures that compile perfectly and still break in
  production.

## Patterns worth copying

**Pure core, thin binaries.** All judgement lives in `src/lib.rs` and
`src/hook.rs`, which take strings and return values. The binaries only read
stdin, read files, and print. That is what makes the logic testable without
spawning a process.

**Hooks fail open.** The hook always exits 0 and degrades to silence on any
input it does not recognise. Exit 2 would feed stderr back to Claude as a
blocking error — the wrong answer to "you forgot an owner", and a hook that can
fail a session on its own bug is a bad neighbour.

**stdout is a protocol channel.** Both binaries log to stderr. In the MCP server
a stray `println!` corrupts the JSON-RPC stream and the server just appears to
hang; in the hook it corrupts the verdict.

**Tolerant input parsing.** Every field of the hook's input struct is optional,
so a payload that grows a field does not break the hook.

**Name your MCP server.** `ServerInfo` defaults to reporting `"rmcp"`. Note that
`Implementation::from_build_env()` does **not** fix this — its `env!` calls
expand inside rmcp's crate, so it also reports `"rmcp"`. Pass
`env!("CARGO_BIN_NAME")` from your own crate, as `src/bin/mcp.rs` does.

## Releasing

The workspace releases with `cargo-release` (version bumps), `git-cliff`
(changelog) and `cargo-dist` (cross-platform artifacts). One thing to know when
copying this plugin: the version lives in **two** places — `Cargo.toml` and
`.claude-plugin/plugin.json`. `Cargo.toml` wires them together so they cannot
drift:

```toml
[package.metadata.release]
pre-release-replacements = [
  { file = ".claude-plugin/plugin.json", search = '"version": "[0-9]+\.[0-9]+\.[0-9]+"', replace = '"version": "{{version}}"', exactly = 1 },
]
```

`exactly = 1` makes cargo-release fail loudly if the manifest is ever reshaped
so the pattern stops matching, rather than silently skipping the rewrite.

## License

PolyForm-Noncommercial-1.0.0, per the workspace.
