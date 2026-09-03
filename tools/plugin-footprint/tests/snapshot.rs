//! A readable diff of what each published plugin is made of (spec §6).
//!
//! The budget says whether a footprint is too big. This says WHAT CHANGED, which
//! is the part a reviewer can act on.
//!
//! Only the per-source breakdown is snapshotted. `probe.binary` differs by
//! platform, so including it would make this fail for reasons having nothing to
//! do with a footprint. (The document carries no timestamp at all — see
//! `document::build` — so there is nothing else to exclude.)

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn breakdown(plugin: &str) -> Option<serde_json::Value> {
    let path = repo_root()
        .join("docs")
        .join("footprints")
        .join(format!("{plugin}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    let document: serde_json::Value =
        serde_json::from_str(&text).expect("committed document parses");
    Some(serde_json::json!({
        "resident": document["tiers"]["resident"],
        "invocation": document["tiers"]["invocation"],
    }))
}

#[test]
fn rtk_mcp_cc_breakdown() {
    let Some(value) = breakdown("rtk-mcp-cc") else {
        eprintln!("SKIP: docs/footprints/rtk-mcp-cc.json absent; run `just footprint-regen`");
        return;
    };
    insta::assert_json_snapshot!(value);
}

#[test]
fn re_ghidra_mcp_cc_breakdown() {
    let Some(value) = breakdown("re-ghidra-mcp-cc") else {
        eprintln!("SKIP: docs/footprints/re-ghidra-mcp-cc.json absent; run `just footprint-regen`");
        return;
    };
    insta::assert_json_snapshot!(value);
}
