# Antigravity (agy) re-ghidra-mcp Design

## Understanding Summary
- **What is being built:** A Rust-based integration (`antigravity/re-ghidra-mcp-agy`) that attaches a persistent headless Ghidra JVM to your session and exposes it as reverse-engineering tools over MCP.
- **Why it exists:** Ghidra startup is expensive. Launching one headless JVM and holding it for the life of the MCP session allows tool calls to hit an already-warm process instead of paying startup cost per call.
- **Who it is for:** The Antigravity (agy) agent.
- **Key constraints:** Needs to interact with Java (Ghidra JVM) from an MCP server while avoiding cross-process lifecycle leaks.

## Architecture & Data Flow

```text
Antigravity ──stdio JSON-RPC──> re-ghidra-agy-mcp ──loopback TCP──> Ghidra JVM
                                (Rust, shared/)                     (GhidraMcpWorker.java)
```

The worker is a GhidraScript embedded in the binary and extracted at boot into a directory keyed by its own content hash, so the extracted copy can never be a different version from the binary that wrote it.

It **attaches** to a Ghidra project you have already created and fully analyzed in the GUI, then closed. It does not import or analyze binaries itself.

## Configuration
The tool is packaged as an Antigravity Plugin. Installing the plugin (e.g. via `agy plugin install https://github.com/ckir/aiplugins/tree/main/antigravity/re-ghidra-mcp-agy`) automatically discovers and loads the hooks (`hooks.json`) and MCP server configuration (`mcp_config.json`) contained within the plugin bundle.
