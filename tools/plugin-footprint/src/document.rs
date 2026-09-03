//! The footprint document (spec §5).
//!
//! One per plugin, and the stable interface between everything downstream: the
//! gate, the snapshot test and the README generator all read this and nothing
//! else parses plugin internals.
//!
//! Two rules shape the types more than anything else, and both are about what
//! the document must be unable to say:
//!
//! * A failed probe carries no tiers. Not zeroed tiers — absent ones. Every
//!   budget ceiling is trivially satisfied by zero, so a plugin whose binary
//!   will not start must not be representable as a plugin that costs nothing.
//! * Tokens are absent until an oracle has produced them. Bytes are computed
//!   hermetically on every run; token counts come from the exact counter and are
//!   cached here (§4.3.1). `Option` rather than a default is what keeps
//!   "not measured" distinct from "measured as none".

use crate::canonical::canonical_len;
use crate::manifest::absolutize;
use crate::probe::{Outcome, Status};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The schema version consumers check before trusting the shape.
///
/// A consumer reads any version up to and including its own and refuses only
/// versions above it. Strict equality would deadlock the very pull request that
/// bumps this: §6.2 has the gate read its baseline from the merge base, which by
/// definition still carries the previous version.
pub const SCHEMA_VERSION: u32 = 1;

/// Which tree was measured (spec §4.2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tree {
    /// The plugin directory as built in this repository. What the gate probes,
    /// because it is what a pull request can produce.
    Dev,
    /// An assembled release bundle. What the published figure must describe,
    /// because it is what a user installs.
    Bundle,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub schema_version: u32,
    pub plugin: String,
    /// Claude Code only for v1 (spec §8, Fork 3). The field exists so the
    /// contract is already multi-agent when Qwen arrives.
    pub agent: &'static str,
    pub tree: Tree,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_version: Option<String>,
    pub measured_at: String,
    pub probe: ProbeReport,
    /// Absent unless the probe succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiers: Option<Tiers>,
    /// Absent until an exact count has been taken.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oracle: Option<Oracle>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReport {
    pub status: &'static str,
    /// Why, for anything other than `ok`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Repo-relative, forward slashes, no executable suffix.
    pub binary: String,
    pub tool_count: usize,
    pub prompt_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tiers {
    pub resident: Tier,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tier {
    pub bytes: u64,
    /// Absent until an oracle has run; never defaulted to zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    /// Always itemised, never only a total. The per-source breakdown is what
    /// makes a failing gate actionable rather than merely red.
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub kind: &'static str,
    pub id: String,
    pub bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Oracle {
    pub kind: &'static str,
    pub id: String,
    /// Required for an exact count: the same payload counts differently across
    /// models, so a figure without its model is not reproducible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Assemble the document for one probed plugin.
///
/// `measured_at_epoch_secs` is passed in rather than read from the clock so the
/// library stays deterministic and testable; the CLI supplies the real time.
pub fn build(
    plugin: &str,
    plugin_dir: &Path,
    tree: Tree,
    plugin_version: Option<&str>,
    measured_at_epoch_secs: i64,
    outcome: &Outcome,
) -> Document {
    let (status, detail) = match &outcome.status {
        Status::Ok => ("ok", None),
        Status::Failed(why) => ("failed", Some(why.clone())),
        Status::TimedOut(why) => ("timed_out", Some(why.clone())),
    };

    let tiers = matches!(outcome.status, Status::Ok).then(|| Tiers {
        resident: resident_tier(outcome),
    });

    Document {
        schema_version: SCHEMA_VERSION,
        plugin: plugin.to_string(),
        agent: "claude-code",
        tree,
        plugin_version: plugin_version.map(str::to_string),
        measured_at: rfc3339_utc(measured_at_epoch_secs),
        probe: ProbeReport {
            status,
            detail,
            binary: normalise_binary(plugin_dir, &outcome.binary),
            tool_count: outcome.tools.len(),
            prompt_count: outcome.prompts.len(),
        },
        tiers,
        oracle: None,
    }
}

/// The resident tier: what the host holds in every request, for the whole
/// session (spec §3).
///
/// MCP tool schemas and prompt entries only. The file-backed resident sources
/// §3 also names — skill frontmatter, agent context files, command and agent
/// descriptions — are not measured yet, so this tier is a LOWER BOUND and §7
/// must render it as one until §4.5 is implemented.
fn resident_tier(outcome: &Outcome) -> Tier {
    let mut sources: Vec<Source> = outcome
        .tools
        .iter()
        .map(|tool| source("mcp_tool_schema", tool))
        .chain(
            outcome
                .prompts
                .iter()
                .map(|prompt| source("mcp_prompt", prompt)),
        )
        .collect();

    // Sorted by id so the document does not inherit the order a server happened
    // to list its tools in, which would churn the committed copy for no reason.
    sources.sort_by(|a, b| a.id.cmp(&b.id));

    Tier {
        bytes: sources.iter().map(|s| s.bytes).sum(),
        tokens: None,
        sources,
    }
}

fn source(kind: &'static str, value: &serde_json::Value) -> Source {
    Source {
        kind,
        id: value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<unnamed>")
            .to_string(),
        bytes: canonical_len(value) as u64,
        tokens: None,
    }
}

/// Relative to the plugin, with forward slashes and no executable suffix.
///
/// Relative to the PLUGIN rather than to the working directory, so the recorded
/// path is the same whether the tool was invoked as `measure claude-code/x` or
/// with an absolute path. These documents are committed (§4.3.1), and a
/// machine-specific path in one would churn on whoever regenerated it.
///
/// Forward slashes and no `.exe` because the gate may run on Linux while the
/// published figure is regenerated on Windows (§8, Fork 7); snapshotting an
/// un-normalised path would flake on the platform rather than on the footprint.
/// When the prefix cannot be stripped — a symlinked plugin directory, a `\\?\`
/// prefix on one side only, a working directory that moved between the two
/// resolutions — this degrades to the file NAME rather than to the whole
/// absolute path. Falling back to the full path would write
/// `C:/Users/<someone>/...` into a committed document, silently, and it would
/// differ on every machine that regenerated it. A bare name is still useful
/// provenance and can leak nothing.
fn normalise_binary(plugin_dir: &Path, binary: &Path) -> String {
    let relative = absolutize(plugin_dir)
        .and_then(|root| binary.strip_prefix(&root).ok().map(Path::to_path_buf))
        // Stripping the root from itself leaves nothing, and `binary: ""` names
        // no file at all — least of all the directory that was probed.
        .filter(|relative| relative.components().next().is_some())
        .unwrap_or_else(|| {
            binary
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| binary.to_path_buf())
        });

    // Separator rewriting is Windows-only. A backslash is a legal character in a
    // Unix filename, so rewriting it there would turn the single file
    // `my\binary` into `my/binary` — a file inside a directory, which is not
    // what was probed.
    let text = if cfg!(windows) {
        relative.to_string_lossy().replace('\\', "/")
    } else {
        relative.to_string_lossy().into_owned()
    };
    text.strip_suffix(".exe").unwrap_or(&text).to_string()
}

/// Format a Unix timestamp as RFC 3339 in UTC.
///
/// Hand-rolled rather than pulling in a date crate for one field. The civil-date
/// conversion is Howard Hinnant's `civil_from_days`, which is the standard
/// algorithm and is pinned by tests against known epochs.
fn rfc3339_utc(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs_of_day = epoch_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (h, m, s) = (
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
    );
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Days since 1970-01-01 to a civil (year, month, day).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_epochs_format_correctly() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_000_000_000), "2001-09-09T01:46:40Z");
        // A leap day, which is where a hand-rolled conversion goes wrong.
        assert_eq!(rfc3339_utc(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(rfc3339_utc(1_756_857_600), "2025-09-03T00:00:00Z");
    }
}
