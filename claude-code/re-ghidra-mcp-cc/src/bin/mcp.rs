//! `re-ghidra-cc-mcp` — the plugin's MCP server, spoken over stdio.
//!
//! Claude Code launches this binary via `.mcp.json` and talks JSON-RPC to it on
//! stdin/stdout. It owns no reverse-engineering logic: the server, its 19
//! tools, the config contract and the embedded Ghidra worker all live in the
//! shared `ghidra-mcp` crate, so the future agy and qwen plugins front exactly
//! the same code. What this binary contributes is the Claude-Code-shaped part —
//! reading `.claude/re-ghidra-mcp-cc.local.md` as the lowest config layer.
//!
//! Two rules follow from stdio being the protocol channel, and both are easy to
//! get wrong:
//!
//! 1. **stdout belongs to the protocol.** `serve` logs only to its per-instance
//!    file log; usage and configuration errors go to stderr.
//! 2. **The process is long-lived.** It stays up until Claude Code closes the
//!    pipe.

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Claude Code launches MCP servers with the workspace as the working
    // directory, so this is where the project's settings file lives.
    let project_dir = std::env::current_dir().unwrap_or_default();
    let file_config = re_ghidra_mcp_cc::load_settings(&project_dir);

    let code = ghidra_mcp::cli::dispatch(
        env!("CARGO_BIN_NAME"),
        env!("CARGO_PKG_VERSION"),
        args,
        file_config,
    )
    .await;
    if code != 0 {
        std::process::exit(code);
    }
}
