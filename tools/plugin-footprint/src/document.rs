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
use crate::sources::{FileSource, FileSources};
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
    pub invocation: Tier,
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
/// Deliberately carries NO timestamp. The byte counts are a pure function of the
/// plugin tree, so the git commit that holds this document already records when
/// it was measured — more reliably than a field inside it. Recording a wall
/// clock here would also make two identical regenerations differ, and §6's
/// freshness layer is exactly `regenerate, then require no diff`: a
/// non-deterministic document turns that layer permanently red and teaches
/// everyone to special-case it. The one measurement that IS separated in time is
/// the oracle's cached token count (§4.3.1), produced by a run that is not this
/// one; when that lands, the timestamp belongs on `Oracle`, describing the age
/// of the cache rather than of the document.
pub fn build(
    plugin: &str,
    plugin_dir: &Path,
    tree: Tree,
    plugin_version: Option<&str>,
    outcome: &Outcome,
    files: &FileSources,
) -> Document {
    let (status, detail) = match &outcome.status {
        Status::Ok => ("ok", None),
        Status::NoServer => ("no_server", None),
        Status::Failed(why) => ("failed", Some(why.clone())),
        Status::TimedOut(why) => ("timed_out", Some(why.clone())),
    };

    // `NoServer` carries tiers too: the plugin's cost is entirely file-backed,
    // and omitting the tiers would report a real footprint as no footprint.
    let tiers = matches!(outcome.status, Status::Ok | Status::NoServer).then(|| Tiers {
        resident: resident_tier(outcome, &files.resident),
        invocation: tier_from_files(&files.invocation),
    });

    Document {
        schema_version: SCHEMA_VERSION,
        plugin: plugin.to_string(),
        agent: "claude-code",
        tree,
        plugin_version: plugin_version.map(str::to_string),
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
/// session (spec §3). MCP schemas and prompts, plus the frontmatter the host
/// reads to decide what a skill or agent is for.
///
/// Hook output is still not counted, in either tier: measuring it means
/// executing contributor-authored code against a synthetic event (spec §4.5),
/// and that is a price this tool does not pay. A plugin whose hooks emit a large
/// SessionStart preamble is therefore under-reported, which is the one honest
/// gap left in the measurement.
fn resident_tier(outcome: &Outcome, files: &[FileSource]) -> Tier {
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
        .chain(files.iter().map(from_file))
        .collect();

    // Sorted so the document does not inherit the order a server happened to
    // list its tools in, which would churn the committed copy for no reason.
    // The key is (kind, id) rather than id alone: two sources can now share an
    // id — a skill's frontmatter and its body — and sorting on the id alone
    // would leave their relative order to the sort's stability, not to the data.
    sources.sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
    total(sources)
}

/// A tier built only from file-backed sources.
fn tier_from_files(files: &[FileSource]) -> Tier {
    let mut sources: Vec<Source> = files.iter().map(from_file).collect();
    sources.sort_by(|a, b| (a.kind, &a.id).cmp(&(b.kind, &b.id)));
    total(sources)
}

fn total(sources: Vec<Source>) -> Tier {
    Tier {
        bytes: sources.iter().map(|s| s.bytes).sum(),
        tokens: None,
        sources,
    }
}

fn from_file(file: &FileSource) -> Source {
    Source {
        kind: file.kind,
        id: file.id.clone(),
        bytes: file.bytes,
        tokens: None,
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
        .and_then(|root| strip_root(&root, binary))
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

/// Strip `root` from `binary`, comparing components case-insensitively on
/// Windows.
///
/// `Path::strip_prefix` compares byte-for-byte, so a plugin directory given as
/// `c:\plugin` fails to strip from a command resolved as `C:\plugin\bin\x`
/// purely on the drive letter's case — and the caller's fallback then throws
/// away the whole internal path (`bin/`) that a reader needs. Windows paths are
/// case-insensitive, so matching them that way here is not a loosening; it is
/// what the platform already means.
fn strip_root(root: &Path, binary: &Path) -> Option<PathBuf> {
    let mut rest = binary.components();
    for expected in root.components() {
        let actual = rest.next()?;
        let (actual, expected) = (actual.as_os_str(), expected.as_os_str());
        let same = actual == expected
            || (cfg!(windows)
                // Case-folded only when BOTH components are valid UTF-8. A lossy
                // conversion maps every invalid sequence onto U+FFFD, so two
                // genuinely different components would compare equal and this
                // would strip a root the binary was never under.
                && match (actual.to_str(), expected.to_str()) {
                    (Some(a), Some(b)) => a.to_lowercase() == b.to_lowercase(),
                    _ => false,
                });
        if !same {
            return None;
        }
    }
    Some(rest.collect())
}
