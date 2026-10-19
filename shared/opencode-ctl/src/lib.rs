//! Shared engine for the opencode-ctl MCP servers.

pub mod cli;
pub mod clock;
pub mod compact;
pub mod created_set;
pub mod engine;
pub mod error;
pub mod host;
pub mod liveness;
pub mod logging;
pub mod openapi;
pub mod pairing;
pub mod server;
pub mod settings;
pub mod startup;
pub mod sweep;
pub mod testing;
pub mod tools;
pub mod transport;
