//! `mock-rtk-cc` — a stand-in for the real `rtk`, used by the e2e tests.
//!
//! The plugin's contract is entirely about how it talks to an external binary:
//! what it sends, what it accepts back, and how it behaves when that binary
//! misbehaves. Testing that against the *real* rtk would make the suite depend
//! on rtk being installed (it is not, on CI) and on rtk's rules never changing.
//! So the tests point `RTK_BIN` at this instead.
//!
//! It reproduces the behaviours observed from rtk 0.45.0, including the one
//! that matters most: an already-rewritten command produces **empty stdout and
//! exit 0**, which is how rtk signals "nothing to do".
//!
//! `MOCK_RTK_MODE` forces the failure modes the plugin must survive:
//!   `fail`    — exit non-zero
//!   `garbage` — exit 0 but write prose to stdout instead of JSON
//!   `silent`  — exit 0 with no output at all

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = std::env::var("MOCK_RTK_MODE").unwrap_or_default();

    if mode == "fail" {
        eprintln!("mock-rtk-cc: simulated failure");
        std::process::exit(1);
    }

    // Global flags may precede the subcommand; ignore them when dispatching,
    // but echo them back so tests can assert they were forwarded.
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let flags: Vec<&String> = args.iter().filter(|a| a.starts_with("--")).collect();

    match positional.first().map(|s| s.as_str()) {
        Some("hook") => match positional.get(1).map(|s| s.as_str()) {
            Some("claude") => hook_claude(&mode),
            Some("check") => {
                let command = positional.get(2).map(|s| s.as_str()).unwrap_or("");
                println!("rtk {command}");
            }
            other => {
                eprintln!("mock-rtk-cc: unknown hook subcommand {other:?}");
                std::process::exit(2);
            }
        },
        Some("gain") => println!("MOCK gain {}", render(&positional[1..], &flags)),
        Some("discover") => println!("MOCK discover {}", render(&positional[1..], &flags)),
        Some("proxy") => println!("MOCK proxy {}", render(&positional[1..], &flags)),
        other => {
            eprintln!("mock-rtk-cc: unknown command {other:?}");
            std::process::exit(2);
        }
    }
}

/// Emulate `rtk hook claude`: read the PreToolUse event, rewrite `cmd` to
/// `rtk cmd` — unless it is already rewritten, in which case say nothing.
fn hook_claude(mode: &str) {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);

    if mode == "silent" {
        return;
    }
    if mode == "garbage" {
        println!("mock-rtk-cc: this is not JSON");
        return;
    }

    let Some(command) = extract_command(&input) else {
        return;
    };
    // The idempotency property probed against the real rtk: feeding back an
    // already-rewritten command is a no-op.
    if command.starts_with("rtk ") {
        return;
    }

    println!(
        r#"{{"hookSpecificOutput":{{"hookEventName":"PreToolUse","permissionDecisionReason":"RTK auto-rewrite","updatedInput":{{"command":"rtk {}"}}}}}}"#,
        command.replace('\\', "\\\\").replace('"', "\\\"")
    );
}

/// Pull `tool_input.command` out of the event without pulling in a JSON parser.
/// The mock only ever sees payloads the tests write, so a substring scan is
/// enough — and keeps the fixture free of assumptions about escaping.
fn extract_command(input: &str) -> Option<String> {
    let key = "\"command\":\"";
    let start = input.find(key)? + key.len();
    let rest = &input[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn render(positional: &[&String], flags: &[&String]) -> String {
    let mut parts: Vec<String> = positional.iter().map(|s| s.to_string()).collect();
    parts.extend(flags.iter().map(|s| s.to_string()));
    parts.join(" ")
}
