//! The gate's layers (spec §6).
//!
//! `check` takes two parsed documents and a budget. It reads no files and runs
//! no processes, so every layer is testable without a filesystem, a git
//! checkout or a built binary — and so the layer ORDER, which is the part that
//! matters, is pinned by tests rather than by a comment.
//!
//! The order is a contract. Layer 1 is fatal because every later layer would
//! otherwise be reasoning about numbers that were never measured: a plugin whose
//! binary will not start measures nothing, and nothing satisfies every ceiling.
//!
//! Four of §6's five layers live here. The fifth — freshness — deliberately does
//! not, and getting that wrong is easy: `check` receives the COMMITTED document,
//! so asking it to verify freshness would have it compare that file against the
//! merge base's copy, which answers "did this change" and not "is this still
//! true". Freshness is a question about the world, not about two files, so it is
//! enforced by regenerating the document and requiring no diff — which is what
//! makes the document's determinism (no wall clock, see `document::build`) load
//! bearing rather than tidy.

use serde_json::Value;

/// What this crate can read. A baseline written by a NEWER version is refused
/// rather than guessed at; an older one is read, because the pull request that
/// bumps the version necessarily reads a baseline written by the previous one.
pub const SUPPORTED_SCHEMA: u64 = 1;

#[derive(Debug, Clone)]
pub struct Budget {
    /// The ceiling on `tiers.resident.bytes`.
    pub resident_bytes: u64,
    /// How far below the ceiling a measurement must fall before the ceiling is
    /// lowered to follow it (spec §6). Recorded here so the gate and the
    /// ratchet share one number.
    pub headroom_bytes: u64,
    /// The most one pull request may add.
    pub delta_bytes: u64,
}

/// What the committed thresholds say about one plugin.
///
/// The distinction between `Absent` and `Malformed` is the whole point of this
/// type. MEASURED during the capstone review, against a real committed ref: with
/// `residentBytes` written as the STRING `"23670"`, the gate printed "has no
/// budget yet; measuring without a ceiling" and passed the plugin uncapped, exit
/// 0 — a one-character typo in a merged `budgets.json` silently disabling the
/// gate, with a message that reads like normal operation for a new plugin.
///
/// Absent means "there is nothing to compare against", which is legitimate and
/// common: every plugin looks like this on the run that introduces it.
/// Malformed means "a ceiling exists and this tool cannot read it", which is a
/// failure. Collapsing the second into the first is the false-zero rule turned
/// on the gate's own thresholds.
#[derive(Debug)]
pub enum BudgetLookup {
    Absent,
    Malformed(String),
    Found(Budget),
}

/// Resolve one plugin's budget out of the committed thresholds.
pub fn budget_for(budgets: &Value, plugin: &str) -> BudgetLookup {
    let Some(entry) = budgets.get(plugin) else {
        return BudgetLookup::Absent;
    };

    let field = |name: &str| -> Result<u64, String> {
        match entry.get(name) {
            None => Err(format!("`{name}` is missing")),
            Some(value) => value.as_u64().ok_or_else(|| {
                format!("`{name}` is {value}, which is not a whole number of bytes")
            }),
        }
    };

    match (
        field("residentBytes"),
        field("headroomBytes"),
        field("deltaBytes"),
    ) {
        (Ok(resident_bytes), Ok(headroom_bytes), Ok(delta_bytes)) => BudgetLookup::Found(Budget {
            resident_bytes,
            headroom_bytes,
            delta_bytes,
        }),
        (r, h, d) => BudgetLookup::Malformed(
            [r.err(), h.err(), d.err()]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("; "),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// Every reason, not just the first — a reviewer fixing one should be able
    /// to see the others in the same run.
    Fail(Vec<String>),
}

pub fn check(measured: &Value, baseline: Option<&Value>, budget: &Budget) -> Verdict {
    // Layer 1: the probe. Fatal, and alone — nothing later is meaningful.
    if let Some(reason) = probe_failed(measured) {
        return Verdict::Fail(vec![reason]);
    }

    let mut reasons = Vec::new();

    // The schema check runs before anything uses the baseline: a baseline this
    // tool cannot read is a baseline it must not compare against.
    let comparable = match baseline {
        Some(baseline) => match schema_of(baseline) {
            Some(version) if version <= schema_of(measured).unwrap_or(SUPPORTED_SCHEMA) => true,
            Some(version) => {
                reasons.push(format!(
                    "the baseline's schemaVersion {version} is newer than this tool understands; \
                     upgrade the tool rather than comparing shapes it cannot read"
                ));
                false
            }
            None => {
                reasons.push("the baseline carries no schemaVersion".to_string());
                false
            }
        },
        None => false,
    };

    let measured_bytes = resident_bytes(measured);

    if comparable {
        let baseline = baseline.expect("comparable implies a baseline");

        // The per-change delta cap. `saturating_sub` so a plugin that got
        // SMALLER does not underflow into apparent enormous growth.
        let growth = measured_bytes.saturating_sub(resident_bytes(baseline));
        if growth > budget.delta_bytes {
            reasons.push(format!(
                "this change adds {growth} resident bytes, over the per-change delta cap of {}",
                budget.delta_bytes
            ));
        }
    }

    // The budget ceiling.
    if measured_bytes > budget.resident_bytes {
        reasons.push(format!(
            "resident footprint {measured_bytes} bytes exceeds the budget of {} bytes",
            budget.resident_bytes
        ));
    }

    if reasons.is_empty() {
        Verdict::Pass
    } else {
        Verdict::Fail(reasons)
    }
}

fn probe_failed(measured: &Value) -> Option<String> {
    let probe = measured.get("probe")?;
    let status = probe
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("absent");
    // A complete measurement of a plugin that has no server to ask. The
    // tool-count check below exists to catch a server that launched and
    // returned nothing; there was no server here, so it does not apply.
    if status == "no_server" {
        return None;
    }
    if status != "ok" {
        let detail = probe
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or("no detail recorded");
        return Some(format!("the probe did not succeed ({status}): {detail}"));
    }
    if probe.get("toolCount").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return Some(
            "the probe succeeded but reported no tools at all; a measurement of nothing \
             satisfies every ceiling and must not be treated as one"
                .to_string(),
        );
    }
    None
}

fn schema_of(document: &Value) -> Option<u64> {
    document.get("schemaVersion")?.as_u64()
}

fn resident_bytes(document: &Value) -> u64 {
    document
        .get("tiers")
        .and_then(|t| t.get("resident"))
        .and_then(|r| r.get("bytes"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}
