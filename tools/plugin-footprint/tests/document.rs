//! The output contract (spec §5).
//!
//! This document is the stable interface: the gate, the snapshot test and the
//! README generator all consume it, and nothing else parses plugin internals.
//! What these tests pin is mostly what the document must REFUSE to say — a
//! failed probe must not be representable as a measurement of zero, and a tier
//! holding an unmeasured source must not present itself as a total.

use plugin_footprint::document::{build, Tree};
use plugin_footprint::probe::{Outcome, Status};
use serde_json::json;
use std::path::{Path, PathBuf};

fn outcome(status: Status, tools: Vec<serde_json::Value>, binary: &str) -> Outcome {
    Outcome {
        status,
        tools,
        prompts: Vec::new(),
        binary: PathBuf::from(binary),
        reaped: true,
    }
}

fn ok_outcome() -> Outcome {
    Outcome {
        prompts: vec![json!({ "name": "a_prompt", "description": "p" })],
        ..outcome(
            Status::Ok,
            vec![
                json!({ "name": "beta", "description": "b", "inputSchema": { "type": "object" } }),
                json!({ "name": "alpha", "description": "a", "inputSchema": { "type": "object" } }),
            ],
            "claude-code/x/bin/x-mcp",
        )
    }
}

#[test]
fn a_successful_probe_itemises_every_source_it_measured() {
    let doc = build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        Some("0.6.4"),
        0,
        &ok_outcome(),
    );

    let value = serde_json::to_value(&doc).expect("serialises");
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["plugin"], "x");
    assert_eq!(value["agent"], "claude-code");
    assert_eq!(value["tree"], "dev");
    assert_eq!(value["probe"]["status"], "ok");
    assert_eq!(value["probe"]["toolCount"], 2);
    assert_eq!(value["probe"]["promptCount"], 1);

    // Itemised, never only a total: a failing gate has to be able to say which
    // source grew, or it is merely red.
    let sources = value["tiers"]["resident"]["sources"]
        .as_array()
        .expect("resident sources are itemised");
    assert_eq!(sources.len(), 3, "two tools and one prompt");

    // Sorted by id so the document does not inherit the order a server happened
    // to list its tools in.
    let ids: Vec<&str> = sources.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["a_prompt", "alpha", "beta"]);
}

#[test]
fn tier_bytes_are_the_sum_of_the_sources() {
    let doc = build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        Some("0.6.4"),
        0,
        &ok_outcome(),
    );
    let value = serde_json::to_value(&doc).expect("serialises");

    let sources = value["tiers"]["resident"]["sources"].as_array().unwrap();
    let summed: u64 = sources.iter().map(|s| s["bytes"].as_u64().unwrap()).sum();

    assert_eq!(value["tiers"]["resident"]["bytes"].as_u64(), Some(summed));
    assert!(summed > 0, "a measured tier is not empty");
}

#[test]
fn a_failed_probe_omits_the_tiers_entirely_rather_than_reporting_zero() {
    // The failure this whole contract exists to prevent. A plugin whose binary
    // will not start must not serialise as `resident.tokens = 0`, because every
    // budget ceiling is trivially satisfied by zero.
    let failed = outcome(
        Status::Failed("could not launch".to_string()),
        Vec::new(),
        "bin/x",
    );

    let value = serde_json::to_value(build("x", Path::new("plug"), Tree::Dev, None, 0, &failed))
        .expect("serialises");

    assert_eq!(value["probe"]["status"], "failed");
    assert!(
        value.get("tiers").is_none() || value["tiers"].is_null(),
        "a failed probe must carry no tiers at all, got: {}",
        value["tiers"]
    );
    assert!(
        value["probe"]["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("could not launch"),
        "the reason must survive into the document"
    );
}

#[test]
fn a_timed_out_probe_is_distinguishable_from_a_failed_one() {
    let timed_out = outcome(
        Status::TimedOut("no response to tools/list".to_string()),
        Vec::new(),
        "bin/x",
    );

    let value = serde_json::to_value(build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        None,
        0,
        &timed_out,
    ))
    .expect("serialises");

    assert_eq!(value["probe"]["status"], "timed_out");
}

#[test]
fn tokens_are_absent_until_an_oracle_has_produced_them() {
    // Bytes are computed hermetically on every run; tokens come from the exact
    // counter and are cached into the committed document (spec §4.3.1). An
    // un-run oracle must leave the field absent, never 0 — the same
    // failure-is-not-a-zero rule the probe status follows.
    let value = serde_json::to_value(build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        None,
        0,
        &ok_outcome(),
    ))
    .unwrap();

    assert!(value["tiers"]["resident"]["bytes"].is_u64());
    assert!(
        value["tiers"]["resident"].get("tokens").is_none()
            || value["tiers"]["resident"]["tokens"].is_null(),
        "no oracle has run, so there is no token count to report"
    );
    assert!(
        value.get("oracle").is_none() || value["oracle"].is_null(),
        "and no oracle to attribute one to"
    );
}

#[test]
fn the_binary_path_is_recorded_normalised_and_platform_independent() {
    // Provenance (spec §5): a document naming a version its binary does not
    // report measured a stale binary — which happened for real during this
    // tool's development. Recorded with forward slashes and no `.exe`, because
    // the gate may run on Linux while the published figure is regenerated on
    // Windows, and the path must not differ on that alone.
    // The probed command is absolute (see `ServerSpec::command`), and native
    // separators are whatever this platform uses.
    let plugin_dir = std::env::current_dir().expect("cwd").join("plug");
    let native = plugin_dir.join("bin").join("x-mcp.exe");
    let probed = outcome(
        Status::Ok,
        vec![json!({ "name": "t" })],
        &native.to_string_lossy(),
    );

    let value = serde_json::to_value(build("x", &plugin_dir, Tree::Dev, None, 0, &probed)).unwrap();

    // Plugin-relative, forward slashes, no `.exe` — so a relative and an
    // absolute invocation produce the same document, and Linux and Windows
    // produce the same document.
    assert_eq!(value["probe"]["binary"], "bin/x-mcp");
}

#[test]
fn the_document_serialises_in_the_pinned_canonical_form() {
    let doc = build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        Some("0.6.4"),
        0,
        &ok_outcome(),
    );
    let text = plugin_footprint::canonical::canonical_json(&serde_json::to_value(&doc).unwrap());

    // Sorted keys, so a committed document does not churn on field order.
    assert!(
        text.starts_with(r#"{"agent":"claude-code","measuredAt""#),
        "expected canonical ordering, got: {}",
        &text[..text.len().min(80)]
    );
}

#[test]
fn a_binary_outside_the_plugin_degrades_to_a_name_not_a_machine_path() {
    // `normalise_binary` strips the plugin prefix. If that ever fails — a
    // symlinked plugin directory, a `\?\` prefix on one side, a cwd that moved
    // between the two resolutions — falling back to the WHOLE absolute path
    // would write `C:/Users/<someone>/...` into a document that gets committed
    // (§4.3.1), and it would do it silently. The file name is still useful
    // provenance and cannot leak a home directory or churn between machines.
    let elsewhere = outcome(
        Status::Ok,
        vec![json!({ "name": "t" })],
        "/somewhere/else/entirely/bin/x-mcp",
    );

    let value = serde_json::to_value(build(
        "x",
        Path::new("plug"),
        Tree::Dev,
        None,
        0,
        &elsewhere,
    ))
    .unwrap();

    assert_eq!(value["probe"]["binary"], "x-mcp");
}

#[test]
fn a_command_that_is_the_plugin_root_is_not_recorded_as_an_empty_path() {
    // Stripping the root from itself leaves nothing. `binary: ""` names no file
    // at all, and least of all the directory that was actually probed.
    let plugin_dir = std::env::current_dir().expect("cwd").join("plug");
    let probed = outcome(
        Status::Failed("is a directory".to_string()),
        Vec::new(),
        &plugin_dir.to_string_lossy(),
    );

    let value = serde_json::to_value(build("x", &plugin_dir, Tree::Dev, None, 0, &probed)).unwrap();

    assert_eq!(value["probe"]["binary"], "plug");
}

#[test]
#[cfg(unix)]
fn a_backslash_in_a_unix_filename_is_not_turned_into_a_directory() {
    // A backslash is a legal character in a Unix filename. Rewriting it to `/`
    // for cross-platform tidiness would make the document name `my/binary` — a
    // file `binary` inside a directory `my` — when what was probed was a single
    // file called `my\binary`.
    let plugin_dir = std::env::current_dir().expect("cwd").join("plug");
    let probed = outcome(
        Status::Ok,
        vec![json!({ "name": "t" })],
        &plugin_dir.join(r"my\binary").to_string_lossy(),
    );

    let value = serde_json::to_value(build("x", &plugin_dir, Tree::Dev, None, 0, &probed)).unwrap();

    assert_eq!(value["probe"]["binary"], r"my\binary");
}
