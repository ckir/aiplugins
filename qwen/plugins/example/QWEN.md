# Example Extension

This is an example Qwen Code extension demonstrating a Rust-based MCP server,
commands, skills, and agents.

## Available capabilities

- **`/example:polish <text>`** — proofread and tighten a passage while keeping
  its meaning and tone.
- **The `synonyms` skill** — suggest alternative words and phrasings with notes
  on nuance and formality.
- **The `diary-writer` subagent** — expand brief notes into a full journal
  entry.
- **The `count_words` MCP tool** — count words and characters in a passage.
  Powered by a Rust binary compiled via Cargo.
