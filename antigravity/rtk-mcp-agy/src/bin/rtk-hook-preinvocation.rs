use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct PreInvocationOutput {
    #[serde(rename = "injectSteps")]
    inject_steps: Vec<serde_json::Value>,
}

fn main() {
    let output = PreInvocationOutput {
        inject_steps: vec![json!({
            "ephemeralMessage": "CRITICAL: Do NOT use the native `run_command` tool. You MUST use the `call_mcp_tool` with ServerName='rtkmcp' and ToolName='rtk_run' for all shell commands to ensure token-optimized execution."
        })],
    };

    if let Ok(json_str) = serde_json::to_string(&output) {
        println!("{}", json_str);
    }
}
