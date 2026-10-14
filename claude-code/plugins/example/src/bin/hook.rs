//! `claude-example-hook` — the plugin's `PostToolUse` hook.
//!
//! Claude Code runs this binary after a `Write` or `Edit`, hands it the event
//! as JSON on stdin, and reads a JSON verdict from stdout. All the judgement
//! lives in [`claude_example::hook`]; this file only moves bytes.
//!
//! The hook **always exits 0**. Exit 2 would feed stderr back to Claude as a
//! blocking error, which is the wrong response to "you forgot an owner" — and a
//! hook that can fail the session on its own bug is a bad neighbour. Reporting
//! happens through `systemMessage` instead.

use claude_example::hook::{evaluate, HookOutput};
use claude_example::Config;
use std::io::Read;
use std::path::PathBuf;

fn main() {
    // stderr only: stdout carries the hook's JSON verdict.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        tracing::warn!("could not read hook input from stdin");
        print!("{}", HookOutput::silent().to_json());
        return;
    }

    let project_dir = std::env::var("CLAUDE_PROJECT_DIR")
        .map(PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .unwrap_or_else(|_| PathBuf::from("."));

    let config = Config::load(&project_dir);
    let output = evaluate(&input, &config, |path| read_target(&project_dir, path));

    print!("{}", output.to_json());
}

/// Read the edited file so markers can still be found when the event payload
/// carries no inline text. Relative paths resolve against the project root.
fn read_target(project_dir: &std::path::Path, path: &str) -> Option<String> {
    let candidate = PathBuf::from(path);
    let full = if candidate.is_absolute() {
        candidate
    } else {
        project_dir.join(candidate)
    };
    std::fs::read_to_string(full).ok()
}
