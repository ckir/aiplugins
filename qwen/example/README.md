# qwen-example

Example Qwen Code extension demonstrating a Rust-based MCP server.

## Structure

```
qwen/example/
├── qwen-extension.json   # Extension manifest
├── Cargo.toml            # Rust workspace crate
├── src/
│   └── main.rs           # MCP server binary (stdio JSON-RPC)
├── QWEN.md               # Extension context
├── commands/
│   └── example/
│       └── polish.md     # /example:polish slash command
├── skills/
│   └── synonyms/
│       └── SKILL.md      # Synonyms skill
└── agents/
    └── diary-writer.md   # Diary writer subagent
```

## Building

```bash
cargo build -p qwen-example --bin qwen-example-mcp
```

## Linking

```bash
qwen extensions link ./qwen/example
```

## MCP Tools

- `count_words` — count words, characters, and characters without spaces in a text passage.
