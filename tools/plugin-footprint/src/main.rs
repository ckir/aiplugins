//! `plugin-footprint` — measure what installing a plugin costs a context window.
//!
//! Maintainer tooling (spec: `docs/specs/2026-09-03-plugin-footprint.md`). It
//! emits one footprint document per plugin on stdout, in the pinned canonical
//! form, which is the stable interface the gate and the README generator read.
//!
//! Usage:
//!   plugin-footprint measure <plugin-dir> [--out <path>]
//!   plugin-footprint ratchet --measured <path> --budgets <path>

use plugin_footprint::canonical::canonical_json;
use plugin_footprint::document::{build, Tree};
use plugin_footprint::manifest::{looks_like_a_plugin, read_mcp_servers};
use plugin_footprint::probe::{probe, Limits, Outcome, Status};
use plugin_footprint::sources::read_file_sources;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("measure") if args.len() == 2 => measure(Path::new(&args[1]), None),
        Some("measure") if args.len() == 4 && args[2] == "--out" => {
            measure(Path::new(&args[1]), Some(Path::new(&args[3])))
        }
        Some("ratchet") if args.len() == 5 && args[1] == "--measured" && args[3] == "--budgets" => {
            ratchet(Path::new(&args[2]), Path::new(&args[4]))
        }
        _ => {
            // Usage goes to stderr: stdout carries the document, and a consumer
            // piping it into `jq` must never get prose mixed in.
            eprintln!("usage: plugin-footprint measure <plugin-dir> [--out <path>]");
            eprintln!("       plugin-footprint ratchet --measured <path> --budgets <path>");
            ExitCode::from(2)
        }
    }
}

fn measure(plugin_dir: &Path, out: Option<&Path>) -> ExitCode {
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

    // Read before probing, not after. This is the cheap check, and a plugin
    // whose skill layout is malformed should say so in milliseconds rather than
    // after a probe that may sit through its 120-second budget first.
    let files = match read_file_sources(plugin_dir) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("plugin-footprint: {e}");
            return ExitCode::from(1);
        }
    };

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
        &merged,
        &files,
    );

    let value = match serde_json::to_value(&document) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("plugin-footprint: serialising the document: {e}");
            return ExitCode::from(1);
        }
    };
    let text = canonical_json(&value);
    match out {
        // A trailing newline so the file is a well-formed text file and git does
        // not report "\ No newline at end of file" on every regeneration.
        Some(path) => {
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("plugin-footprint: creating {}: {e}", parent.display());
                    return ExitCode::from(1);
                }
            }
            if let Err(e) = std::fs::write(path, format!("{text}\n")) {
                eprintln!("plugin-footprint: writing {}: {e}", path.display());
                return ExitCode::from(1);
            }
        }
        None => println!("{text}"),
    }

    // A failed probe is reported as a document AND as a non-zero exit, so a
    // caller that only checks the status code cannot mistake it for a plugin
    // that costs nothing.
    match merged.status {
        Status::Ok => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    }
}

/// Seed a plugin's thresholds on first sight, and tighten them when a
/// measurement comes in well under the current ceiling.
///
/// The ratchet has HYSTERESIS on purpose, and its guarantee is BOUNDED — state
/// it precisely, because a vague promise here is the kind that quietly stops
/// being true.
///
/// Lowering on every improvement would mean a change saving 5 bytes tightens the
/// ceiling by 5, so reverting it fails a gate that was green a day earlier. The
/// ceiling therefore only follows a measurement that is a further `HEADROOM`
/// below it: it moves when `measured <= current - 2*HEADROOM`, and moves to
/// `measured + HEADROOM`.
///
/// What that buys, exactly: after a lowering, the new ceiling sits `HEADROOM`
/// above the new measurement, so reverting a saving of up to `HEADROOM` bytes
/// still passes. A saving LARGER than `HEADROOM` is banked, and reverting it
/// will fail the ceiling — which is what a ratchet is for, and is not a bug. Do
/// not describe this as "a revert always stays green"; it is "a revert of a
/// saving up to HEADROOM stays green".
fn ratchet(measured_path: &Path, budgets_path: &Path) -> ExitCode {
    const HEADROOM: u64 = 2_000;
    const DELTA: u64 = 500;

    let Ok(text) = std::fs::read_to_string(measured_path) else {
        eprintln!("plugin-footprint: cannot read {}", measured_path.display());
        return ExitCode::from(1);
    };
    let Ok(document) = serde_json::from_str::<serde_json::Value>(&text) else {
        eprintln!(
            "plugin-footprint: {} does not parse",
            measured_path.display()
        );
        return ExitCode::from(1);
    };

    // A failed measurement must never move a threshold. Its tiers are absent,
    // and treating that as a footprint of zero would ratchet every budget to
    // the floor and fail every subsequent change.
    if document["probe"]["status"].as_str() != Some("ok") {
        eprintln!(
            "plugin-footprint: refusing to ratchet from a probe that did not succeed ({})",
            document["probe"]["status"].as_str().unwrap_or("absent")
        );
        return ExitCode::from(1);
    }

    let plugin = document["plugin"].as_str().unwrap_or_default().to_string();
    let measured = document["tiers"]["resident"]["bytes"].as_u64().unwrap_or(0);

    let mut budgets: serde_json::Value = std::fs::read_to_string(budgets_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let current = budgets[&plugin]["residentBytes"].as_u64();
    let target = measured + HEADROOM;
    let next = match current {
        // Seed: the budget starts where the plugin already is, plus headroom.
        None => target,
        // Tighten only when the measurement is a further headroom below the
        // ceiling — the hysteresis described above.
        Some(current) if target + HEADROOM <= current => target,
        Some(current) => current,
    };

    budgets[&plugin] = serde_json::json!({
        "residentBytes": next,
        "headroomBytes": HEADROOM,
        "deltaBytes": DELTA,
    });

    let text = canonical_json(&budgets);
    if let Err(e) = std::fs::write(budgets_path, format!("{text}\n")) {
        eprintln!("plugin-footprint: writing {}: {e}", budgets_path.display());
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
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
