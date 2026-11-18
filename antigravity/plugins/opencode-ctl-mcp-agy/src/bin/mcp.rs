// Copyright 2026 AI Plugins Marketplace Contributors
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 together with the
// additional terms (evaluation grant, anti-circumvention) that apply to this
// repository. See the LICENSE file at the repository root.

//! `opencode-ctl-agy-mcp`: the Antigravity front of opencode-ctl-mcp. All behaviour lives in
//! the `opencode-ctl` engine; this file only names the host, the product and the version.

use opencode_ctl::cli::dispatch;
use opencode_ctl::host::Host;

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
