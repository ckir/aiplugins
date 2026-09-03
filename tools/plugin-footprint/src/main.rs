//! `plugin-footprint` — measure what installing a plugin costs a context window.
//!
//! Maintainer tooling (spec: `docs/specs/2026-09-03-plugin-footprint.md`). It
//! emits one footprint document per plugin on stdout, in the pinned canonical
//! form, which is the stable interface the gate and the README generator read.
//!
//! Usage:
//!   plugin-footprint measure <plugin-dir>

use plugin_footprint::canonical::canonical_json;
use plugin_footprint::document::{build, Tree};
use plugin_footprint::manifest::{looks_like_a_plugin, read_mcp_servers};
use plugin_footprint::probe::{probe, Limits, Outcome, Status};
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("measure") if args.len() == 2 => measure(Path::new(&args[1])),
        _ => {
            // Usage goes to stderr: stdout carries the document, and a consumer
            // piping it into `jq` must never get prose mixed in.
            eprintln!("usage: plugin-footprint measure <plugin-dir>");
            ExitCode::from(2)
        }
    }
}

fn measure(plugin_dir: &Path) -> ExitCode {
    // Checked before anything is measured. A plugin with no `.mcp.json` is
    // legitimate — hooks or skills only — so "no servers" cannot tell a
    // hooks-only plugin from a mistyped path, and the wrong path would otherwise
    // produce a confident `ok, 0 bytes`.
    if !looks_like_a_plugin(plugin_dir) {
        eprintln!(
            "plugin-footprint: {} is not a plugin directory (no .claude-plugin/plugin.json)",
            plugin_dir.display()
        );
        return ExitCode::from(2);
    }

    let servers = match read_mcp_servers(plugin_dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("plugin-footprint: {e}");
            return ExitCode::from(1);
        }
    };

    // Refused rather than merged. Merging several servers' tools into one tier
    // identifies each source by tool name alone, so two servers exposing the
    // same name would land as two sources with the same id — an ambiguous
    // document that a reviewer reading a diff cannot attribute. No plugin in
    // this marketplace declares more than one server today; when one does, the
    // fix is to qualify source ids by server, and this error says so rather
    // than quietly producing a shape nobody designed.
    if servers.len() > 1 {
        eprintln!(
            "plugin-footprint: {} declares {} MCP servers; measuring more than one into a single              document needs server-qualified source ids, which is not implemented",
            plugin_dir.display(),
            servers.len()
        );
        return ExitCode::from(1);
    }

    let mut merged = Outcome {
        status: Status::Ok,
        tools: Vec::new(),
        prompts: Vec::new(),
        binary: servers
            .first()
            .map(|s| s.command.clone())
            .unwrap_or_default(),
        reaped: true,
    };

    for spec in &servers {
        let outcome = probe(spec, &Limits::default());
        match outcome.status {
            Status::Ok => {
                merged.tools.extend(outcome.tools);
                merged.prompts.extend(outcome.prompts);
            }
            // The first failure decides the document: a partial measurement is
            // not a measurement, and reporting the servers that did answer would
            // understate the plugin while looking like a complete result.
            other => {
                merged.status = other;
                merged.tools.clear();
                merged.prompts.clear();
                merged.binary = outcome.binary;
                break;
            }
        }
    }

    let name = plugin_name(plugin_dir);
    let document = build(
        &name,
        plugin_dir,
        Tree::Dev,
        plugin_version(plugin_dir).as_deref(),
        now_epoch_secs(),
        &merged,
    );

    let value = match serde_json::to_value(&document) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("plugin-footprint: serialising the document: {e}");
            return ExitCode::from(1);
        }
    };
    println!("{}", canonical_json(&value));

    // A failed probe is reported as a document AND as a non-zero exit, so a
    // caller that only checks the status code cannot mistake it for a plugin
    // that costs nothing.
    match merged.status {
        Status::Ok => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    }
}

/// The plugin's declared name, falling back to its directory.
fn plugin_name(plugin_dir: &Path) -> String {
    plugin_json(plugin_dir)
        .and_then(|v| v.get("name")?.as_str().map(str::to_string))
        .unwrap_or_else(|| {
            plugin_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "<unnamed>".to_string())
        })
}

fn plugin_version(plugin_dir: &Path) -> Option<String> {
    plugin_json(plugin_dir)?
        .get("version")?
        .as_str()
        .map(str::to_string)
}

fn plugin_json(plugin_dir: &Path) -> Option<serde_json::Value> {
    let text =
        std::fs::read_to_string(plugin_dir.join(".claude-plugin").join("plugin.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn now_epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}
