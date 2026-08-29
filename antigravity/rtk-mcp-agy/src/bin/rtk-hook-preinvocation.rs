//! `rtk-hook-preinvocation` — the plugin's `PreInvocation` hook.
//!
//! Antigravity runs this at the start of an execution loop and reads an
//! `injectSteps` array from stdout. The payload lives in [`rtk_mcp_agy`]; this
//! file only serializes it.

use rtk_mcp_agy::preinvocation_payload;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rtk-hook-preinvocation {}", rtk_mcp_agy::version());
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "rtk-hook-preinvocation {} — Antigravity PreInvocation hook",
            rtk_mcp_agy::version()
        );
        println!();
        println!("Usage: rtk-hook-preinvocation");
        println!("  Outputs an injectSteps JSON array to stdout instructing the agent to use the");
        println!("  rtk MCP tool instead of the native run_command tool.");
        return;
    }

    if let Ok(json_str) = serde_json::to_string(&preinvocation_payload()) {
        println!("{json_str}");
    }
}
