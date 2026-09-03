//! The footprint gate (spec §6), as one command for CI to run.
//!
//! Usage: `footprint-gate <base-ref>` — for example `footprint-gate origin/main`.

use plugin_footprint::gate::{check, Budget, Verdict};
use std::process::{Command, ExitCode};

/// Where the thresholds live. Read from the BASE REF, never from the working
/// tree (spec §6.2): CI checks out the pull request, so a budget read from that
/// checkout is authored by the very change the gate is judging, and a failing
/// change could raise its own ceiling with a diff that looks like a routine
/// regeneration.
const BUDGETS_PATH: &str = "docs/footprints/budgets.json";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(base_ref) = args.first() else {
        eprintln!("usage: footprint-gate <base-ref>");
        return ExitCode::from(2);
    };

    // The plugin list comes from the marketplace manifest, the same source
    // `just smoke` and the bundle-smoke job iterate. Reading it from the working
    // tree is deliberate: a change that ADDS a plugin must have it measured.
    let plugins = match published_plugins() {
        Ok(plugins) => plugins,
        Err(e) => {
            eprintln!("footprint-gate: {e}");
            return ExitCode::from(1);
        }
    };

    // Budgets from the base ref. Absent on the very first run, which is not a
    // failure — every plugin is then new and only the probe layer applies.
    let budgets = show(base_ref, BUDGETS_PATH).unwrap_or_else(|| serde_json::json!({}));

    let mut failed = false;
    for plugin in &plugins {
        let budget = match budget_for(&budgets, plugin) {
            Some(budget) => budget,
            None => {
                println!(
                    "footprint-gate: {plugin} has no budget at {base_ref} yet; \
                     measuring without a ceiling"
                );
                Budget {
                    resident_bytes: u64::MAX,
                    headroom_bytes: 0,
                    delta_bytes: u64::MAX,
                }
            }
        };
        let path = format!("docs/footprints/{plugin}.json");

        let measured = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(e) => {
                    eprintln!("footprint-gate: {path} does not parse: {e}");
                    failed = true;
                    continue;
                }
            },
            Err(e) => {
                eprintln!(
                    "footprint-gate: {path} is missing ({e}). Run `just footprint-regen` and \
                     commit the result."
                );
                failed = true;
                continue;
            }
        };

        // Absent for a plugin added by this change, which is not a failure —
        // there is simply nothing to compare against yet.
        let baseline = show(base_ref, &path);

        match check(&measured, baseline.as_ref(), &budget) {
            Verdict::Pass => println!("footprint-gate: {plugin} ok"),
            Verdict::Fail(reasons) => {
                failed = true;
                eprintln!("footprint-gate: {plugin} FAILED");
                for reason in reasons {
                    eprintln!("  - {reason}");
                }
                report_growth(&measured, baseline.as_ref());
            }
        }
    }

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// The plugins the marketplace actually publishes.
///
/// The same iteration source `just smoke` and the `bundle-smoke` CI job use, so
/// the gate cannot drift from what ships. Note the interaction with §6.2's
/// bypass: a change that removes a plugin from this manifest removes it from
/// the gate. `scripts/check-marketplace.sh` is what keeps the manifest honest
/// about the plugins present, and it already runs in CI.
fn published_plugins() -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(".claude-plugin/marketplace.json")
        .map_err(|e| format!("reading the marketplace manifest: {e}"))?;
    let manifest: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("parsing the marketplace manifest: {e}"))?;
    Ok(manifest["plugins"]
        .as_array()
        .ok_or("the marketplace manifest declares no plugins array")?
        .iter()
        .filter_map(|p| p["name"].as_str().map(str::to_string))
        .collect())
}

fn budget_for(budgets: &serde_json::Value, plugin: &str) -> Option<Budget> {
    let entry = budgets.get(plugin)?;
    Some(Budget {
        resident_bytes: entry.get("residentBytes")?.as_u64()?,
        headroom_bytes: entry.get("headroomBytes")?.as_u64()?,
        delta_bytes: entry.get("deltaBytes")?.as_u64()?,
    })
}

/// Read a path as it exists at `base_ref`, or `None` when it is not there.
fn show(base_ref: &str, path: &str) -> Option<serde_json::Value> {
    let output = Command::new("git")
        .args(["show", &format!("{base_ref}:{path}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

/// Name the sources that grew most, so a red build says what to look at.
///
/// The JSON is the interface for tools; a CI log is the interface for a person,
/// and "over budget" without a culprit is merely red.
fn report_growth(measured: &serde_json::Value, baseline: Option<&serde_json::Value>) {
    let sources = |doc: &serde_json::Value| -> Vec<(String, u64)> {
        doc["tiers"]["resident"]["sources"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|s| {
                        (
                            format!(
                                "{}/{}",
                                s["kind"].as_str().unwrap_or("?"),
                                s["id"].as_str().unwrap_or("?")
                            ),
                            s["bytes"].as_u64().unwrap_or(0),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };

    let now = sources(measured);
    let before: std::collections::BTreeMap<String, u64> = baseline
        .map(|b| sources(b).into_iter().collect())
        .unwrap_or_default();

    let mut grown: Vec<(String, i64)> = now
        .iter()
        .map(|(id, bytes)| {
            let was = before.get(id).copied().unwrap_or(0);
            (id.clone(), *bytes as i64 - was as i64)
        })
        .filter(|(_, delta)| *delta != 0)
        .collect();
    grown.sort_by_key(|(_, delta)| -*delta);

    if grown.is_empty() {
        return;
    }
    eprintln!("  largest changes:");
    for (id, delta) in grown.iter().take(3) {
        eprintln!("    {id}: {delta:+} bytes");
    }
}
