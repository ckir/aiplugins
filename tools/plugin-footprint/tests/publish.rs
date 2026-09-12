//! Rendering the published figure into a README region (spec §7).
//!
//! §7 says the region is "generated, never hand-written, and verified fresh in
//! CI", so everything here is a pure function over parsed documents: no files,
//! no git, no plugin binaries.
//!
//! Most of these tests pin the HONESTY of the label rather than its prose. The
//! figure being published is bytes, not the exact token count Fork 6(b) makes
//! authoritative, and it excludes hook output — so a reader who takes it for a
//! total, or for a token count, has been misled by us. That is a correctness
//! property, not a style preference, and it is the reason a region is generated
//! at all rather than typed once and left to rot.

use plugin_footprint::publish::{comparison_region, per_plugin_region, splice, PublishError};
use serde_json::{json, Value};

fn doc(plugin: &str, resident: u64, invocation: u64) -> Value {
    json!({
        "schemaVersion": 1,
        "plugin": plugin,
        "probe": { "status": "ok", "toolCount": 4, "binary": "bin/x", "promptCount": 0 },
        "tiers": {
            "resident": { "bytes": resident, "sources": [] },
            "invocation": { "bytes": invocation, "sources": [] }
        }
    })
}

// --- the honest label ---

#[test]
fn the_region_says_the_figure_is_bytes_and_not_tokens() {
    // The whole point of publishing now. Fork 6(b) makes an EXACT token count
    // authoritative and it does not exist, so every `tokens` field in every
    // document is null. A reader deciding whether to install thinks in context,
    // and will read a bare number as tokens unless told otherwise.
    let region = per_plugin_region(&doc("x", 21_631, 36_903)).expect("renders");

    assert!(region.contains("not tokens"), "got: {region}");
    assert!(
        region.to_lowercase().contains("does not convert"),
        "it must say the number does not convert to tokens at a fixed rate: {region}"
    );
}

#[test]
fn the_region_renders_the_figure_as_a_lower_bound_and_names_what_is_excluded() {
    // §4.5's disclosure rule, verbatim: "Wherever a tier contains an unmeasured
    // source, Deliverable B (§7) MUST render the figure as a lower bound and
    // name what is excluded ... rather than printing N as if it were the total."
    // Hook output is that unmeasured source, permanently.
    let region = per_plugin_region(&doc("x", 21_631, 36_903)).expect("renders");

    assert!(region.contains("≥"), "must be a lower bound: {region}");
    assert!(
        region.to_lowercase().contains("hook"),
        "must name hook output as the exclusion: {region}"
    );
}

#[test]
fn the_region_carries_both_tiers_and_says_when_each_is_paid() {
    // "The three tiers, with the resident figure leading" (§7). Resident and
    // Invocation are the two that are measured; the distinction is the whole
    // value of the figure, because one is paid on every request and the other
    // only when something runs.
    let region = per_plugin_region(&doc("x", 21_631, 36_903)).expect("renders");

    assert!(region.contains("21,631"), "resident, formatted: {region}");
    assert!(region.contains("36,903"), "invocation, formatted: {region}");
    let resident_at = region.find("21,631").expect("resident present");
    let invocation_at = region.find("36,903").expect("invocation present");
    assert!(resident_at < invocation_at, "resident must lead: {region}");
}

#[test]
fn a_document_whose_probe_failed_is_refused_rather_than_published_as_zero() {
    // The rule this whole crate is built on, applied to the one surface a USER
    // reads. A failed probe carries no tiers; publishing it would render 0 and
    // advertise the cheapest plugin in the marketplace.
    let failed = json!({
        "schemaVersion": 1,
        "plugin": "x",
        "probe": { "status": "failed", "detail": "could not launch", "binary": "bin/x",
                   "toolCount": 0, "promptCount": 0 }
    });

    assert!(matches!(
        per_plugin_region(&failed),
        Err(PublishError::NotMeasured { .. })
    ));
}

// --- splicing into a README ---

const README: &str =
    "# Title\n\nintro\n\n<!-- footprint:begin -->\nOLD\n<!-- footprint:end -->\n\ntail\n";

#[test]
fn splice_replaces_only_what_is_between_the_markers() {
    let out = splice("r.md", README, "NEW").expect("splices");

    assert!(out.contains("# Title"), "prose before survives: {out}");
    assert!(out.contains("tail"), "prose after survives: {out}");
    assert!(out.contains("NEW"));
    assert!(!out.contains("OLD"));
    assert!(out.contains("<!-- footprint:begin -->"), "markers survive");
    assert!(out.contains("<!-- footprint:end -->"));
}

#[test]
fn splicing_is_idempotent() {
    // Regenerating twice with nothing changed must produce the same bytes, or
    // the freshness check fails on every run — the same property removing the
    // document's timestamp bought, extended to the README.
    let once = splice("r.md", README, "NEW").expect("splices");
    let twice = splice("r.md", &once, "NEW").expect("splices");

    assert_eq!(once, twice);
}

#[test]
fn a_missing_marker_is_a_hard_error_naming_the_file() {
    // §7: "a missing or unpaired marker is a hard error naming the file, never a
    // silent no-op: a checker that quietly passes on a README it could not find
    // its region in is the stale-copy failure §7 exists to prevent."
    for absent in [
        "# Title\n\nno markers at all\n",
        "# Title\n\n<!-- footprint:begin -->\nunclosed\n",
        "# Title\n\n<!-- footprint:end -->\nno opener\n",
    ] {
        let err = splice("claude-code/x/README.md", absent, "NEW")
            .expect_err("a missing marker must be loud");
        assert!(
            err.to_string().contains("claude-code/x/README.md"),
            "the error must name the file, got: {err}"
        );
    }
}

#[test]
fn markers_in_the_wrong_order_are_refused() {
    let inverted = "# Title\n\n<!-- footprint:end -->\nbody\n<!-- footprint:begin -->\n";

    assert!(splice("r.md", inverted, "NEW").is_err());
}

#[test]
fn a_second_marker_pair_is_refused_rather_than_guessed_between() {
    // Which pair is the region? Picking one silently rewrites half a README and
    // leaves the other half stale forever.
    let twice = format!("{README}\n{README}");

    let err = splice("r.md", &twice, "NEW").expect_err("ambiguous must be loud");
    assert!(err.to_string().contains("more than one"), "got: {err}");
}

// --- the comparison table ---

#[test]
fn the_comparison_table_lists_every_published_plugin_sorted() {
    // Sorted so the table does not churn on the order the marketplace manifest
    // happens to list plugins in.
    let region = comparison_region(&[
        ("rtk-mcp-cc".to_string(), doc("rtk-mcp-cc", 3_690, 6_091)),
        (
            "re-ghidra-mcp-cc".to_string(),
            doc("re-ghidra-mcp-cc", 21_631, 36_903),
        ),
    ])
    .expect("renders");

    let ghidra_at = region.find("re-ghidra-mcp-cc").expect("present");
    let rtk_at = region.find("rtk-mcp-cc").expect("present");
    assert!(ghidra_at < rtk_at, "sorted by name: {region}");
    assert!(region.contains("3,690") && region.contains("21,631"));
}

#[test]
fn the_comparison_table_carries_the_same_disclosures_as_the_per_plugin_region() {
    // A reader comparing plugins is making the install decision §7 exists to
    // inform, and is the LEAST likely to have read the per-plugin page first.
    // The caveats cannot live only on the page they might skip.
    let region = comparison_region(&[("x".to_string(), doc("x", 100, 200))]).expect("renders");

    assert!(region.contains("not tokens"), "got: {region}");
    assert!(region.contains("≥"), "got: {region}");
    assert!(region.to_lowercase().contains("hook"), "got: {region}");
}

#[test]
fn one_unmeasurable_plugin_fails_the_whole_table_rather_than_dropping_a_row() {
    // Silently omitting a row would publish a marketplace that looks complete
    // and understates itself by a whole plugin.
    let failed = json!({
        "schemaVersion": 1, "plugin": "broken",
        "probe": { "status": "failed", "toolCount": 0, "binary": "b", "promptCount": 0 }
    });

    let err = comparison_region(&[
        ("x".to_string(), doc("x", 100, 200)),
        ("broken".to_string(), failed),
    ])
    .expect_err("must not publish a partial table");
    assert!(err.to_string().contains("broken"), "got: {err}");
}
