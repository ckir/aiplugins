// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Test binary: the repo-tasks MCP server with the host and the roots window taken from
//! `REPO_TASKS_TEST_HOST` (`claude` or `agy`) and `REPO_TASKS_TEST_ROOTS_WINDOW_MS`. These two
//! switches exist only here: the engine reads neither.

use repo_tasks::host::Host;
use repo_tasks::resolve::Env;
use repo_tasks::server::{serve_with, ServeOptions};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let host = match std::env::var("REPO_TASKS_TEST_HOST").as_deref() {
        Ok("agy") => Host::Agy,
        _ => Host::Claude,
    };
    let mut opts = ServeOptions::default();
    if let Some(ms) = std::env::var("REPO_TASKS_TEST_ROOTS_WINDOW_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        opts.roots_window = Duration::from_millis(ms);
    }
    if let Err(e) = serve_with(host, "test", Env::from_process(), opts).await {
        eprintln!("repo-tasks: {e}");
        std::process::exit(1);
    }
}
