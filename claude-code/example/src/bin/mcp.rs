//! `claude-example-mcp` — the plugin's MCP server, spoken over stdio.
//!
//! Claude Code launches this binary via `.mcp.json` and talks JSON-RPC to it on
//! stdin/stdout. Two rules follow from that, and both are easy to get wrong:
//!
//! 1. **stdout belongs to the protocol.** Every log line goes to stderr. A
//!    stray `println!` corrupts the stream and the server appears to hang.
//! 2. **The process is long-lived.** It stays up until Claude Code closes the
//!    pipe, so `waiting()` is what keeps it alive.

use claude_example::{scan_workspace, Config, FileMarker};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;
use tracing::info;

// ── Tool parameter types ───────────────────────────────────────────────
//
// Each `#[tool]` takes one `Parameters<T>`; `T`'s JsonSchema derive becomes the
// tool's input schema, and the doc comments become the field descriptions the
// model reads. Describe fields properly — this is the model's only documentation.

/// Arguments for the `scan_todos` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScanTodosRequest {
    /// Directory to scan. Defaults to the project root.
    pub path: Option<String>,
    /// When true, report only markers written without an `(owner)`.
    pub unowned_only: Option<bool>,
}

/// Arguments for the `check_text` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckTextRequest {
    /// The source text to inspect for TODO/FIXME/HACK markers.
    pub text: String,
}

// ── Server ─────────────────────────────────────────────────────────────

/// The MCP server. Holds the generated tool router plus the plugin config.
#[derive(Debug, Clone)]
pub struct TodoServer {
    // Read by the code `#[tool_handler]` generates, which dead-code analysis
    // does not see through. rmcp's own test suite annotates this same field.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    config: Config,
}

impl TodoServer {
    /// Build a server, loading settings relative to `project_dir`.
    pub fn new(project_dir: &std::path::Path) -> Self {
        Self {
            tool_router: Self::tool_router(),
            config: Config::load(project_dir),
        }
    }
}

#[tool_router]
impl TodoServer {
    /// Scan a directory tree for TODO/FIXME/HACK markers.
    #[tool(
        name = "scan_todos",
        description = "Scan a directory tree for TODO/FIXME/HACK markers, returning each marker's \
                       file, line, owner and note as JSON."
    )]
    async fn scan_todos(
        &self,
        Parameters(ScanTodosRequest { path, unowned_only }): Parameters<ScanTodosRequest>,
    ) -> Result<String, ErrorData> {
        let root = match path {
            Some(p) => PathBuf::from(p),
            None => std::env::current_dir().map_err(|e| {
                ErrorData::internal_error(format!("no working directory: {e}"), None)
            })?,
        };
        if !root.is_dir() {
            return Err(ErrorData::invalid_params(
                format!("not a directory: {}", root.display()),
                None,
            ));
        }

        let mut markers = scan_workspace(&root, &self.config.kinds, self.config.max_results);
        if unowned_only.unwrap_or(false) {
            markers.retain(|m| m.marker.is_unowned());
        }

        info!(count = markers.len(), root = %root.display(), "scan_todos");
        render(&markers)
    }

    /// Inspect a block of text without touching the file system.
    #[tool(
        name = "check_text",
        description = "Inspect a block of text for TODO/FIXME/HACK markers and report each one's \
                       line, owner and note as JSON."
    )]
    async fn check_text(
        &self,
        Parameters(CheckTextRequest { text }): Parameters<CheckTextRequest>,
    ) -> Result<String, ErrorData> {
        let markers: Vec<FileMarker> = claude_example::scan_text(&text, &self.config.kinds)
            .into_iter()
            .map(|marker| FileMarker {
                file: "<input>".to_string(),
                marker,
            })
            .collect();
        render(&markers)
    }
}

/// Serialize a marker list into the JSON payload returned to the model.
fn render(markers: &[FileMarker]) -> Result<String, ErrorData> {
    let unowned = markers.iter().filter(|m| m.marker.is_unowned()).count();
    let payload = serde_json::json!({
        "total": markers.len(),
        "unowned": unowned,
        "markers": markers,
    });
    serde_json::to_string_pretty(&payload)
        .map_err(|e| ErrorData::internal_error(format!("serialize failed: {e}"), None))
}

#[tool_handler]
impl ServerHandler for TodoServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            // Without this the server introduces itself as "rmcp" and is
            // indistinguishable from every other rmcp server in `/mcp`.
            //
            // Do not reach for `Implementation::from_build_env()` here: its
            // `env!` calls expand inside rmcp's own crate, so it reports
            // "rmcp" no matter who calls it. The `env!`s below expand here,
            // which is the whole point.
            .with_server_info(Implementation::new(
                env!("CARGO_BIN_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Finds TODO/FIXME/HACK markers in a codebase. Use scan_todos for files on disk \
                 and check_text for a snippet you already have in hand.",
            )
    }
}

// ── Entry point ────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stderr, never stdout: stdout is the JSON-RPC channel.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let project_dir = std::env::var("CLAUDE_PROJECT_DIR")
        .map(PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .unwrap_or_else(|_| PathBuf::from("."));

    info!(project_dir = %project_dir.display(), "starting claude-example-mcp");

    let service = TodoServer::new(&project_dir).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
