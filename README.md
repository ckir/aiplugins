# AI Plugins Marketplace

Welcome to the **AI Plugins Marketplace**, a monorepo designed to host, manage, and distribute plugins for various autonomous AI agents.

## Supported Agents (Initial)

Initially, this marketplace provides and supports plugins for the following agents:
- **Claude Code**
- **Antigravity**
- **Qwen**

## Repository Structure

This repository is structured as a monorepo. It will contain specialized packages, shared resources, and individual plugins tailored to the agents they support.

```text
aiplugins/
├── claude-code/   # Plugins specifically for Claude Code
├── antigravity/   # Plugins specifically for Antigravity
├── qwen/          # Plugins specifically for Qwen
├── shared/        # Agent-agnostic crates that several plugins front
├── scripts/       # Repository checks run by CI and `just check`
└── README.md      # Project overview
```

`shared/` holds engines rather than plugins. A crate belongs there when more
than one agent's plugin would otherwise reimplement it — the agent-specific
half (manifest, skills, hooks, settings format) stays in that agent's directory
and is usually a thin front end.

| Crate | What it is |
|---|---|
| `shared/ghidra-ipc` | Wire protocol, framing and error envelopes for the Ghidra worker channel |
| `shared/ghidra-worker-ctl` | Headless Ghidra JVM lifecycle: boot, launch, connection, Windows Job Object containment |
| `shared/ghidra-mcp` | The MCP server and its 19 reverse-engineering tools, plus the embedded Ghidra worker script and driver skill |

Fronted today by [`claude-code/re-ghidra-mcp-cc`](claude-code/re-ghidra-mcp-cc/).

*More details and plugin guidelines will be added soon.*
