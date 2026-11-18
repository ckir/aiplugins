// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! `repo-tasks-agy-mcp`: the Antigravity front of repo-tasks-mcp. All behaviour lives in
//! the `repo-tasks` engine; this file only names the host, the product and the version.

use repo_tasks::cli::dispatch;
use repo_tasks::host::Host;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = dispatch(
        Host::Agy,
        env!("CARGO_BIN_NAME"),
        env!("CARGO_PKG_VERSION"),
        std::env::args().next().as_deref(),
        args,
    )
    .await;
    if code != 0 {
        std::process::exit(code);
    }
}
