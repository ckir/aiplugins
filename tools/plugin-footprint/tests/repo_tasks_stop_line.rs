// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The context-cost stop line for the repo-tasks wrappers (spec 13): a wrapper
//! whose resident tier measures 3,690 bytes or more fails, and the owner is
//! told before a budget is recorded.

use std::path::{Path, PathBuf};

/// The stop line in bytes (spec 13): 3,690 or more fails.
const STOP_LINE: u64 = 3690;

const DOCUMENTS: [&str; 3] = [
    "repo-tasks-mcp-cc.json",
    "repo-tasks-mcp-agy.json",
    "repo-tasks-mcp-opencode.json",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn footprints() -> PathBuf {
    repo_root().join("docs").join("footprints")
}

/// `tiers.resident.bytes` of a footprint document, or a message naming why not.
fn resident_bytes(document: &Path) -> Result<u64, String> {
    let text = std::fs::read_to_string(document)
        .map_err(|e| format!("cannot read {}: {e}", document.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not JSON: {e}", document.display()))?;
    value["tiers"]["resident"]["bytes"]
        .as_u64()
        .ok_or_else(|| format!("{} has no tiers.resident.bytes", document.display()))
}

/// The stop line itself: `Ok` below it, `Err` at or above it.
fn within_stop_line(document: &Path) -> Result<u64, String> {
    let bytes = resident_bytes(document)?;
    if bytes >= STOP_LINE {
        Err(format!(
            "{} measures {bytes} resident bytes, at or above the {STOP_LINE} stop line",
            document.display()
        ))
    } else {
        Ok(bytes)
    }
}

#[test]
fn stop_line_fails_at_3690_and_passes_below() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("repo-tasks-stop-line");

    let at = within_stop_line(&fixtures.join("at-3690.json"));
    assert!(
        at.is_err(),
        "3,690 bytes must fail the stop line, got {at:?}"
    );

    let below = within_stop_line(&fixtures.join("below-3689.json"));
    assert_eq!(below, Ok(3689), "3,689 bytes must pass the stop line");
}

#[test]
fn all_three_footprints_exist() {
    // A document that was never generated must not pass the stop line by being
    // absent. Qwen has no footprint document today, so none is required.
    for name in DOCUMENTS {
        let path = footprints().join(name);
        assert!(path.is_file(), "missing {}", path.display());
    }
}

#[test]
fn all_three_footprints_are_below_the_stop_line() {
    for name in DOCUMENTS {
        let path = footprints().join(name);
        if let Err(message) = within_stop_line(&path) {
            panic!("{message}");
        }
    }
}
