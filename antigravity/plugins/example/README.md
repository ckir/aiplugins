# agy-example

A complete, working Antigravity plugin implemented in Rust — and the template to
copy when starting a new one. It exercises **every extension point an Antigravity
plugin has**, wired to a tested Rust crate rather than to shell scripts.

The plugin itself tracks `TODO`, `FIXME` and `HACK` markers, and enforces
the convention that every marker names an owner.

## What it demonstrates

| Extension point | File | What it does here |
|---|---|---|
| Manifest | `plugin.json` | Name, version, description |
| MCP server | `mcp_config.json` → `bin/agy-example-mcp` | Two tools: `scan_todos`, `check_text` |
| Hook | `hooks.json` → `bin/agy-example-hook` | `PreInvocation` hook running `agy-example-hook` |
| Skill | `skills/example-skill/SKILL.md` | Procedural guidance for Antigravity |
| Agent | `agents/example-agent.md` | Example subagent configuration |
| Settings | `.agents/agy-example.local.md` | Per-project config via `.agents/*.local.md` |

## Layout

```text
antigravity/plugins/example/
├── plugin.json                  # manifest (at plugin root)
├── mcp_config.json              # MCP server registration
├── hooks.json                   # hook registration
├── skills/
│   └── example-skill/SKILL.md   # example skill
├── agents/
│   └── example-agent.md         # example agent
├── bin/                         # built binaries (staged by justfile)
├── src/
│   ├── lib.rs                   # marker scanning + settings (pure, tested)
│   ├── hook.rs                  # hook decision logic (pure, tested)
│   └── bin/
│       ├── mcp.rs               # MCP server binary
│       └── hook.rs              # hook binary
└── tests/e2e.rs                 # drives both real binaries
```

In Antigravity plugins, `plugin.json`, `mcp_config.json`, and `hooks.json` sit at the
plugin **root**.

## Building

The config files point at `bin/`, so the binaries must be built before the
plugin runs:

```bash
just build-agy-example
```

## Testing

```bash
cargo test -p agy-example
```

Tests run in two layers:

- **Unit tests** (`src/`) pin the decisions — marker parsing, owner detection,
  settings, and what the hook chooses to report.
- **E2E tests** (`tests/e2e.rs`) pin the contracts — the hook's stdout JSON, and
  the MCP protocol, driven through rmcp's own client against the real server
  binary.

## Patterns worth copying

**Pure core, thin binaries.** All judgement lives in `src/lib.rs` and
`src/hook.rs`, which take strings and return values. The binaries only read
stdin, read files, and print. That is what makes the logic testable without
spawning a process.

**Hooks fail open.** The hook always exits 0 and degrades to silence on any
input it does not recognise.

**stdout is a protocol channel.** Both binaries log to stderr. In the MCP server
a stray `println!` corrupts the JSON-RPC stream; in the hook it corrupts the verdict.

**Tolerant input parsing.** Every field of the hook's input struct is optional,
so a payload that grows a field does not break the hook.

**Name your MCP server.** `ServerInfo` defaults to reporting `"rmcp"`. Pass
`env!("CARGO_BIN_NAME")` from your own crate, as `src/bin/mcp.rs` does.

## License

PolyForm-Noncommercial-1.0.0, per the workspace.
