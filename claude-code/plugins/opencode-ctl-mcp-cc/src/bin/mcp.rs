//! `opencode-ctl-cc-mcp`: the Claude Code front of opencode-ctl-mcp. All behaviour lives in
//! the `opencode-ctl` engine; this file only names the host, the product and the version.

use opencode_ctl::cli::dispatch;
use opencode_ctl::host::Host;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = dispatch(
        Host::Claude,
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
