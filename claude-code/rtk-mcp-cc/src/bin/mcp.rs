//! `rtk-cc-mcp` — the plugin's MCP server, spoken over stdio.
//!
//! Claude Code launches this binary via `.mcp.json` and talks JSON-RPC to it on
//! stdin/stdout. Two rules follow from that, and both are easy to get wrong:
//!
//! 1. **stdout belongs to the protocol.** Every log line goes to stderr. A
//!    stray `println!` corrupts the stream and the server appears to hang.
//! 2. **The process is long-lived.** It stays up until Claude Code closes the
//!    pipe, so `waiting()` is what keeps it alive.
//!
//! The tools here are rtk's *analytics* surface. Three of them are read-only.
//! The fourth, `rtk_proxy`, executes — see its doc comment.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData, ServerHandler, ServiceExt,
};
use rtk_mcp_cc::{
    check_args, discover_args, gain_args, proxy_args, Config, DiscoverOptions, GainOptions,
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

// ── Tool parameter types ───────────────────────────────────────────────
//
// Each `#[tool]` takes one `Parameters<T>`; `T`'s JsonSchema derive becomes the
// tool's input schema, and the doc comments become the field descriptions the
// model reads. Describe fields properly — this is the model's only documentation.

/// Arguments for the `rtk_gain` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GainRequest {
    /// Include the recent per-command history rather than the summary alone.
    pub history: Option<bool>,
    /// Restrict the statistics to the current working project.
    pub project_only: Option<bool>,
    /// Output format: `text` (default), `json`, or `csv`.
    pub format: Option<String>,
}

/// Arguments for the `rtk_discover` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DiscoverRequest {
    /// Filter to projects whose path contains this substring.
    pub project: Option<String>,
    /// Maximum commands reported per section. rtk's default is 15.
    pub limit: Option<u32>,
    /// Only consider sessions from the last N days. rtk's default is 30.
    pub since: Option<u32>,
    /// Scan every project rather than only the current one.
    pub all_projects: Option<bool>,
    /// Output format: `text` (default) or `json`.
    pub format: Option<String>,
}

/// Arguments for the `rtk_check` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckRequest {
    /// The shell command to analyze, e.g. `grep -rn foo src/`. It is never
    /// executed — this reports only how rtk would rewrite it.
    pub command: String,
}

/// Arguments for the `rtk_proxy` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ProxyRequest {
    /// The command as an already-split argument vector, program first —
    /// e.g. `["git", "status", "--short"]`. This is not a shell command line:
    /// pipes, redirects, globs and `;` are not interpreted, and each element is
    /// passed through verbatim as one argument.
    pub argv: Vec<String>,
}

// ── Server ─────────────────────────────────────────────────────────────

/// The MCP server. Holds the generated tool router plus the resolved settings.
#[derive(Debug, Clone)]
pub struct RtkServer {
    // Read by the code `#[tool_handler]` generates, which dead-code analysis
    // does not see through. rmcp's own test suite annotates this same field.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    config: Config,
}

impl RtkServer {
    /// Build a server, loading settings relative to `project_dir`.
    pub fn new(project_dir: &std::path::Path) -> Self {
        Self {
            tool_router: Self::tool_router(),
            config: Config::load(project_dir),
        }
    }

    /// Run rtk with `args` and return its output as the tool result.
    ///
    /// stderr is folded into the result rather than dropped: rtk puts its
    /// advisory notices there, and a tool that silently discards the only
    /// explanation of an empty result is worse than a noisy one.
    fn run(&self, args: &[String]) -> Result<String, ErrorData> {
        let output = Command::new(&self.config.rtk_bin)
            .args(args)
            .output()
            .map_err(|e| {
                ErrorData::internal_error(
                    format!(
                        "could not run {} ({e}). Install rtk, or set RTK_BIN to its path.",
                        self.config.rtk_bin
                    ),
                    None,
                )
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string();
        let stderr = String::from_utf8_lossy(&output.stderr)
            .trim_end()
            .to_string();

        if !output.status.success() {
            return Err(ErrorData::internal_error(
                format!(
                    "rtk exited with {}: {}",
                    output.status,
                    if stderr.is_empty() { &stdout } else { &stderr }
                ),
                None,
            ));
        }

        Ok(match (stdout.is_empty(), stderr.is_empty()) {
            (true, true) => "(rtk produced no output)".to_string(),
            (false, true) => stdout,
            (true, false) => stderr,
            (false, false) => format!("{stdout}\n\n[rtk stderr]\n{stderr}"),
        })
    }
}

/// Turn an argument-building error into an MCP invalid-params error.
fn bad_params(message: String) -> ErrorData {
    ErrorData::invalid_params(message, None)
}

#[tool_router]
impl RtkServer {
    /// Token-savings analytics.
    #[tool(
        name = "rtk_gain",
        description = "Report rtk's token-savings statistics: how many tokens rtk has saved, and \
                       optionally the recent per-command history. Read-only."
    )]
    async fn rtk_gain(
        &self,
        Parameters(GainRequest {
            history,
            project_only,
            format,
        }): Parameters<GainRequest>,
    ) -> Result<String, ErrorData> {
        let opts = GainOptions {
            history: history.unwrap_or(false),
            project_only: project_only.unwrap_or(false),
            format,
        };
        let args = gain_args(&opts, &self.config).map_err(bad_params)?;
        info!(?args, "rtk_gain");
        self.run(&args)
    }

    /// Missed-optimization analysis over past sessions.
    #[tool(
        name = "rtk_discover",
        description = "Analyze past Claude Code session history for commands that rtk could have \
                       optimized but did not, i.e. missed token savings. Read-only."
    )]
    async fn rtk_discover(
        &self,
        Parameters(DiscoverRequest {
            project,
            limit,
            since,
            all_projects,
            format,
        }): Parameters<DiscoverRequest>,
    ) -> Result<String, ErrorData> {
        let opts = DiscoverOptions {
            project,
            limit,
            since,
            all_projects: all_projects.unwrap_or(false),
            format,
        };
        let args = discover_args(&opts, &self.config).map_err(bad_params)?;
        info!(?args, "rtk_discover");
        self.run(&args)
    }

    /// Dry-run the rewrite engine.
    #[tool(
        name = "rtk_check",
        description = "Show how rtk would rewrite a given shell command, without running it. Use \
                       this to explain why a command was or was not optimized. Read-only: the \
                       command is analyzed, never executed."
    )]
    async fn rtk_check(
        &self,
        Parameters(CheckRequest { command }): Parameters<CheckRequest>,
    ) -> Result<String, ErrorData> {
        let args = check_args(&command, &self.config).map_err(bad_params)?;
        info!(?args, "rtk_check");
        self.run(&args)
    }

    /// Execute a command unoptimized, but still counted.
    ///
    /// This is the one tool here that runs something. It exists for the case
    /// rtk's own docs call out — debugging rtk itself, when you need a command
    /// to bypass the rewrite rules while still being tracked in the statistics.
    /// It takes an argv vector and spawns it directly, so no shell metacharacter
    /// is ever interpreted.
    #[tool(
        name = "rtk_proxy",
        description = "Execute a command through `rtk proxy`: it runs UNFILTERED (no rewriting) \
                       but is still counted in rtk's statistics. Takes an argument vector, not a \
                       shell line — no pipes, redirects or globbing. This tool EXECUTES the \
                       command; prefer the ordinary Bash tool unless you specifically need to \
                       bypass rtk's rewriting while keeping it tracked."
    )]
    async fn rtk_proxy(
        &self,
        Parameters(ProxyRequest { argv }): Parameters<ProxyRequest>,
    ) -> Result<String, ErrorData> {
        let args = proxy_args(&argv, &self.config).map_err(bad_params)?;
        info!(?args, "rtk_proxy");
        self.run(&args)
    }
}

#[tool_handler]
impl ServerHandler for RtkServer {
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
                "Analytics for rtk (Rust Token Killer), which rewrites shell commands into \
                 token-cheaper equivalents. Use rtk_gain for how much has been saved, \
                 rtk_discover for savings being missed, and rtk_check to explain how one \
                 particular command would be rewritten. rtk_proxy executes a command \
                 unfiltered and is rarely what you want.",
            )
    }
}

// ── Entry point ────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rtk-cc-mcp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "rtk-cc-mcp {} — MCP server exposing rtk analytics",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        println!("Usage: rtk-cc-mcp");
        println!("  Speaks MCP over stdio. Launched by Claude Code via .mcp.json;");
        println!("  running it by hand is only useful for debugging.");
        println!();
        println!("Tools: rtk_gain, rtk_discover, rtk_check, rtk_proxy");
        println!();
        println!("Environment:");
        println!("  RTK_BIN  Path to the rtk executable (default: `rtk` on PATH)");
        return Ok(());
    }

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

    info!(project_dir = %project_dir.display(), "starting rtk-cc-mcp");

    let service = RtkServer::new(&project_dir).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
