//! The agent-agnostic half of the Ghidra MCP stack.
//!
//! Everything here is shared by every agent's plugin: the MCP server and its 19
//! tools, the worker lifecycle glue, the config contract, and the embedded
//! `GhidraMcpWorker.java` and driver skill. An agent plugin contributes only a
//! binary that calls [`cli::dispatch`], plus its own manifest and skills.

pub mod cli;
pub mod config;
pub mod execute;
pub mod logging;
pub mod paths;
pub mod poison;
pub mod server;
pub mod skill_asset;
pub mod state;
pub mod tools;
pub mod worker_asset;

use std::path::Path;

/// Extract the embedded worker script into `dir` (panics on IO error — test/boot helper).
pub fn worker_asset_extract(dir: &Path) {
    worker_asset::extract_worker(dir).expect("extract worker script");
}
