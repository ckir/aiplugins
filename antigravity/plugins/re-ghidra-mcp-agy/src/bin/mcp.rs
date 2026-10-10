//! `re-ghidra-agy-mcp` — the plugin's MCP server, spoken over stdio.
//!
//! Antigravity launches this binary via its MCP config and talks JSON-RPC to it on
//! stdin/stdout. It owns no reverse-engineering logic: the server, its 19
//! tools, the config contract and the embedded Ghidra worker all live in the
//! shared `ghidra-mcp` crate. What this binary contributes is the Antigravity-shaped part —
//! reading `.agents/re-ghidra-mcp-agy.local.md` as the lowest config layer.

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Antigravity launches MCP servers with the workspace as the working
    // directory, so this is where the project's settings file lives.
    let project_dir = std::env::current_dir().unwrap_or_default();
    let file_config = re_ghidra_mcp_agy::load_settings(&project_dir);

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
