use qwen_bridge::{process_hook, resolve_rtk_bin, QwenOutput};
use std::env;
use std::io::{self, Read};
use std::process::Command;
use tracing::error;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Handle --version and --help before setting up stdin
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("rtk-mcp-qwen {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "rtk-mcp-qwen {} — PreToolUse hook for run_shell_command rewriting",
            env!("CARGO_PKG_VERSION")
        );
        println!();
        println!("Usage: rtk-mcp-qwen");
        println!("  Reads JSON hook input from stdin, calls `rtk rewrite` on the command,");
        println!("and writes the rewritten (or pass-through) decision to stdout.");
        return;
    }

    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::try_new("info").unwrap()),
        )
        .init();

    let mut input_json = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input_json) {
        error!("Failed to read stdin: {}", e);
        output(&QwenOutput::pass_through(
            "stdin read error, passing through",
        ));
        return;
    }

    if input_json.trim().is_empty() {
        output(&QwenOutput::pass_through("Empty input"));
        return;
    }

    let result = process_hook(&input_json, rtk_rewrite);
    output(&result);
}

fn rtk_rewrite(command: &str) -> Option<String> {
    let rtk_bin = resolve_rtk_bin(env::var("RTK_BIN").ok());
    let output = match Command::new(&rtk_bin).arg("rewrite").arg(command).output() {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to spawn rtk (bin={}): {}", rtk_bin, e);
            return None;
        }
    };
    let code = output.status.code();
    if code != Some(0) && code != Some(3) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("rtk exited with code {:?}: {}", code, stderr.trim());
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

fn output(result: &QwenOutput) {
    match result.to_json() {
        Some(json) => print!("{}", json),
        None => {
            error!("Failed to serialize hook output");
            print!("{{}}");
        }
    }
}
