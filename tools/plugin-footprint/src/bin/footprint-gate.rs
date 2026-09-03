//! The footprint gate (spec §6), as one command for CI to run.
//!
//! Usage: `footprint-gate <base-ref>` — for example `footprint-gate origin/main`.

use plugin_footprint::gate::{at_ref, budget_for, check, AtRef, Budget, BudgetLookup, Verdict};
use std::process::{Command, ExitCode};

/// A plugin with no baseline is measured but not compared, so it needs a budget
/// that constrains nothing. Named rather than inlined, because "no ceiling" is a
/// dangerous value and every place that produces it should be greppable.
const UNCAPPED: Budget = Budget {
    resident_bytes: u64::MAX,
    headroom_bytes: 0,
    delta_bytes: u64::MAX,
};

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

    // Checked BEFORE anything else, because every later step degrades silently
    // without it. MEASURED during the capstone review: run against
    // `origin/does-not-exist`, this printed "has no budget yet; measuring
    // without a ceiling" for every plugin and exited 0 — output byte-identical
    // to a legitimately new plugin. `git show` cannot tell "this ref does not
    // exist" from "this path is not at that ref", so a mistyped base branch, a
    // shallow clone, a renamed default branch or a missing `git` turned the gate
    // off with a green tick.
    if !resolves(base_ref) {
        eprintln!(
            "footprint-gate: {base_ref} does not resolve to a commit, so there is no baseline to \
             compare against. Refusing to report a pass: an unreadable baseline is not an absent \
             one. Check the ref, and that the checkout has enough history (fetch-depth: 0)."
        );
        return ExitCode::from(1);
    }

    // The plugin list comes from the marketplace manifest, the same source
    // `just smoke` and the bundle-smoke job iterate. Reading it from the working
    // tree is deliberate: a change that ADDS a plugin must have it measured.
    //
    // A plugin delisted here drops out of the gate — but not out of CI: measured,
    // `scripts/check-marketplace.sh` exits 1 with "exists in claude-code/ but is
    // not listed", and ci.yml runs it in the `wiring` job.
    let plugins = match published_plugins() {
        Ok(plugins) => plugins,
        Err(e) => {
            eprintln!("footprint-gate: {e}");
            return ExitCode::from(1);
        }
    };

    // Budgets from the base ref. Absent on the very first run, which is not a
    // failure — every plugin is then new and only the probe layer applies.
    // Unreadable is a different matter entirely, and used to be indistinguishable.
    let budgets = match at_ref(show(base_ref, BUDGETS_PATH).as_deref()) {
        AtRef::Found(budgets) => budgets,
        AtRef::Missing => serde_json::json!({}),
        AtRef::Unreadable(why) => {
            eprintln!(
                "footprint-gate: {BUDGETS_PATH} at {base_ref} does not parse ({why}). \
                 Refusing to measure every plugin without a ceiling: one syntax error \
                 must not disable the gate for the whole repository."
            );
            return ExitCode::from(1);
        }
    };

    let mut failed = false;
    for plugin in &plugins {
        let budget = match budget_for(&budgets, plugin) {
            BudgetLookup::Found(budget) => budget,
            // Legitimate and common: every plugin looks like this on the run
            // that introduces it.
            BudgetLookup::Absent => {
                println!(
                    "footprint-gate: {plugin} has no budget at {base_ref} yet; \
                     measuring without a ceiling"
                );
                UNCAPPED
            }
            // NOT the same thing. A ceiling that exists and cannot be read is a
            // failure, not an absence — see `BudgetLookup`.
            BudgetLookup::Malformed(why) => {
                eprintln!(
                    "footprint-gate: {plugin}'s budget at {base_ref} is unreadable ({why}). \
                     Refusing to measure it without a ceiling: that is how a typo disables the \
                     gate silently. Fix it ON {base_ref}, not in a pull request: §6.2 has the \
                     gate read thresholds from the base ref on purpose, so a branch cannot \
                     correct the thresholds its own run is judged against."
                );
                failed = true;
                continue;
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
        // there is simply nothing to compare against yet. Present-but-corrupt is
        // a failure: skipping the comparison would drop the delta cap silently.
        let baseline = match at_ref(show(base_ref, &path).as_deref()) {
            AtRef::Found(baseline) => Some(baseline),
            AtRef::Missing => None,
            AtRef::Unreadable(why) => {
                eprintln!(
                    "footprint-gate: {path} at {base_ref} does not parse ({why}). \
                     Refusing to compare against nothing: that silently drops the \
                     per-change delta cap."
                );
                failed = true;
                continue;
            }
        };

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

/// Whether `base_ref` names a commit that actually exists here.
///
/// Separate from `show`, and that separation is the point: `git show
/// <ref>:<path>` fails identically whether the ref is missing or merely does not
/// carry that path, and only the second is a legitimate "nothing to compare
/// against".
fn resolves(base_ref: &str) -> bool {
    Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base_ref}^{{commit}}"),
        ])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// The RAW bytes of a path as it exists at `base_ref`, or `None` when git could
/// not produce it.
///
/// Deliberately does not parse. Parsing here meant a corrupt file and an absent
/// one both arrived as `None`, and the caller then treated the corrupt one as
/// "nothing to compare against" — see `gate::AtRef`.
fn show(base_ref: &str, path: &str) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(["show", &format!("{base_ref}:{path}")])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
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
