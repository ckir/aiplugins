use serde::Serialize;
use serde_json::json;
use std::env;

#[derive(Serialize)]
struct PreInvocationOutput {
    #[serde(rename = "injectSteps")]
    inject_steps: Vec<serde_json::Value>,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rtk-hook-preinvocation {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "rtk-hook-preinvocation {} — Antigravity PreInvocation hook",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        println!("Usage: rtk-hook-preinvocation");
        println!("  Outputs an injectSteps JSON array to stdout instructing the agent to use the");
        println!("  rtk MCP tool instead of the native run_command tool.");
        return;
    }

    let output = PreInvocationOutput {
        inject_steps: vec![json!({
            "ephemeralMessage": "CRITICAL: Do NOT use the native `run_command` tool. You MUST use the `call_mcp_tool` with ServerName='rtkmcp' and ToolName='rtk_run' for all shell commands to ensure token-optimized execution."
        })],
    };

    if let Ok(json_str) = serde_json::to_string(&output) {
        println!("{}", json_str);
    }
}
