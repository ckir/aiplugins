//! The gate's layers (spec §6). Every case is a pair of parsed documents, so
//! nothing here needs a filesystem, a git checkout or a built binary.

use plugin_footprint::gate::{check, Budget, Verdict};
use serde_json::{json, Value};

fn budget() -> Budget {
    Budget {
        resident_bytes: 20_000,
        headroom_bytes: 2_000,
        delta_bytes: 500,
    }
}

fn doc(status: &str, tools: u64, resident: u64) -> Value {
    json!({
        "schemaVersion": 1,
        "plugin": "x",
        "probe": { "status": status, "toolCount": tools, "binary": "bin/x", "promptCount": 0 },
        "tiers": {
            "resident": {
                "bytes": resident,
                "sources": [{ "kind": "mcp_tool_schema", "id": "t", "bytes": resident }]
            },
            "invocation": { "bytes": 0, "sources": [] }
        }
    })
}

fn failed_doc() -> Value {
    json!({
        "schemaVersion": 1,
        "plugin": "x",
        "probe": { "status": "failed", "detail": "could not launch", "binary": "bin/x",
                   "toolCount": 0, "promptCount": 0 }
    })
}

fn reasons(verdict: Verdict) -> Vec<String> {
    match verdict {
        Verdict::Pass => panic!("expected a failure"),
        Verdict::Fail(reasons) => reasons,
    }
}

#[test]
fn a_clean_measurement_matching_its_baseline_passes() {
    let d = doc("ok", 19, 18_000);
    assert_eq!(check(&d, Some(&d), &budget()), Verdict::Pass);
}

#[test]
fn a_failed_probe_fails_the_gate_before_anything_else_is_considered() {
    // Layer 1 is fatal precisely because every later layer would be reasoning
    // about numbers that were never measured.
    let verdict = reasons(check(&failed_doc(), None, &budget()));

    assert!(
        verdict.iter().any(|r| r.contains("probe")),
        "the probe failure must be named, got {verdict:?}"
    );
    assert_eq!(
        verdict.len(),
        1,
        "no later layer may also report, got {verdict:?}"
    );
}

#[test]
fn a_probe_reporting_no_tools_at_all_fails() {
    // The false-zero this whole design guards against: a plugin that measured
    // nothing must not satisfy a ceiling by costing nothing.
    let verdict = reasons(check(&doc("ok", 0, 0), None, &budget()));

    assert!(
        verdict.iter().any(|r| r.contains("no tools")),
        "{verdict:?}"
    );
}

#[test]
fn a_measurement_over_budget_fails_naming_the_numbers() {
    let over = doc("ok", 19, 25_000);

    let verdict = reasons(check(&over, Some(&over), &budget()));

    let joined = verdict.join(" ");
    assert!(joined.contains("25000"), "actual must be named: {joined}");
    assert!(joined.contains("20000"), "budget must be named: {joined}");
}

#[test]
fn growth_larger_than_the_delta_cap_fails_even_when_under_budget() {
    // 19000 is comfortably under the 20000 ceiling, so only the delta cap can
    // catch it. That is the whole reason the cap exists: the ceiling permits any
    // growth beneath it, including a single jump nobody intended.
    let baseline = doc("ok", 19, 18_000);
    let grown = doc("ok", 19, 19_000);

    let verdict = reasons(check(&grown, Some(&baseline), &budget()));

    assert!(
        verdict.iter().any(|r| r.contains("delta")),
        "a 1000-byte jump exceeds the 500-byte cap, got {verdict:?}"
    );
    assert!(
        !verdict.iter().any(|r| r.contains("budget")),
        "it is under the ceiling; only the delta may fire, got {verdict:?}"
    );
}

#[test]
fn shrinking_is_never_a_delta_failure() {
    // `saturating_sub` matters here: a plugin that got smaller must not read as
    // enormous growth through an underflow.
    let baseline = doc("ok", 19, 18_000);
    let smaller = doc("ok", 19, 12_000);

    assert_eq!(check(&smaller, Some(&baseline), &budget()), Verdict::Pass);
}

#[test]
fn a_baseline_from_a_newer_schema_is_refused_rather_than_guessed_at() {
    let measured = doc("ok", 19, 18_000);
    let mut newer = measured.clone();
    newer["schemaVersion"] = json!(2);

    let verdict = reasons(check(&measured, Some(&newer), &budget()));

    assert!(
        verdict.iter().any(|r| r.contains("schemaVersion")),
        "{verdict:?}"
    );
}

#[test]
fn an_older_baseline_schema_is_read_not_refused() {
    // The PR that bumps the version reads a baseline written by the previous
    // one. Refusing that deadlocks the bump: the merge base can only ever carry
    // the older version.
    let mut measured = doc("ok", 19, 18_000);
    measured["schemaVersion"] = json!(2);
    let older = doc("ok", 19, 18_000);

    let verdict = check(&measured, Some(&older), &budget());

    assert_eq!(verdict, Verdict::Pass, "got {verdict:?}");
}

#[test]
fn a_plugin_with_no_baseline_is_measured_but_not_compared() {
    // A newly added plugin has no merge-base document. It must still satisfy the
    // probe and budget layers; only the comparisons are skipped.
    let d = doc("ok", 19, 18_000);
    assert_eq!(check(&d, None, &budget()), Verdict::Pass);

    let over = doc("ok", 19, 25_000);
    assert!(matches!(check(&over, None, &budget()), Verdict::Fail(_)));
}

#[test]
fn a_failed_probe_reports_only_the_probe_even_when_the_document_claims_a_tier() {
    // This is what actually pins the ORDER, and `failed_doc()` alone does not.
    // A document with no tiers reads as 0 resident bytes, which is under every
    // ceiling — so moving the probe check to the END of `check` leaves the
    // earlier test green, because the budget layer had nothing to complain
    // about either way. MEASURED: that mutation passed all nine of the first
    // tests written for this module.
    //
    // Giving the failed document a tier that IS over budget separates the two
    // orderings: fatal-first reports one reason, fatal-last reports two.
    let mut failed = failed_doc();
    failed["tiers"] = json!({
        "resident": { "bytes": 25_000, "sources": [] },
        "invocation": { "bytes": 0, "sources": [] }
    });

    let verdict = reasons(check(&failed, None, &budget()));

    assert_eq!(
        verdict.len(),
        1,
        "the probe layer is fatal and alone; nothing later may report: {verdict:?}"
    );
    assert!(verdict[0].contains("probe"), "{verdict:?}");
}

#[test]
fn a_probe_that_listed_no_tools_reports_only_the_probe_even_when_over_budget() {
    // Not a contrived shape: a server that answers `tools/list` with an empty
    // array still yields a large resident tier when the plugin's skills and
    // agents are large, because those are read from disk and not from the
    // server. The gate must say the probe is wrong, not that the plugin is fat.
    let verdict = reasons(check(&doc("ok", 0, 25_000), None, &budget()));

    assert_eq!(
        verdict.len(),
        1,
        "the probe layer is fatal and alone; nothing later may report: {verdict:?}"
    );
    assert!(verdict[0].contains("no tools"), "{verdict:?}");
}

// --- budget resolution: absent is not the same as unreadable ---

use plugin_footprint::gate::{budget_for, BudgetLookup};

#[test]
fn a_plugin_with_no_entry_at_the_base_ref_is_absent_not_malformed() {
    // The legitimate case: a plugin this change adds. Measured, not compared.
    assert!(matches!(
        budget_for(&json!({}), "newcomer"),
        BudgetLookup::Absent
    ));
}

#[test]
fn a_well_formed_entry_is_read() {
    let budgets =
        json!({ "x": { "residentBytes": 20000, "headroomBytes": 2000, "deltaBytes": 500 } });

    match budget_for(&budgets, "x") {
        BudgetLookup::Found(b) => {
            assert_eq!(b.resident_bytes, 20_000);
            assert_eq!(b.delta_bytes, 500);
        }
        other => panic!("expected a budget, got {other:?}"),
    }
}

#[test]
fn an_entry_that_is_present_but_unreadable_is_malformed_not_absent() {
    // MEASURED during the capstone review, against a real committed ref: with
    // `residentBytes` as the STRING "23670" the gate printed "has no budget yet;
    // measuring without a ceiling" and passed the plugin with no ceiling and no
    // delta cap, exit 0. A one-character typo in a merged budgets.json silently
    // disables the gate, and the message reads like normal operation.
    //
    // Absent means "nothing to compare against". Unreadable means "the ceiling
    // exists and this tool cannot see it", and treating the second as the first
    // is the false-zero rule applied to the gate's own thresholds.
    for broken in [
        json!({ "x": { "residentBytes": "23670", "headroomBytes": 2000, "deltaBytes": 500 } }),
        json!({ "x": { "residentBytes": 23670.5, "headroomBytes": 2000, "deltaBytes": 500 } }),
        json!({ "x": { "headroomBytes": 2000, "deltaBytes": 500 } }),
        json!({ "x": { "residentBytes": 23670, "deltaBytes": 500 } }),
        json!({ "x": { "residentBytes": 23670, "headroomBytes": 2000 } }),
        json!({ "x": "23670" }),
    ] {
        assert!(
            matches!(budget_for(&broken, "x"), BudgetLookup::Malformed(_)),
            "must not read as absent: {broken}"
        );
    }
}

#[test]
fn a_plugin_that_declares_no_mcp_server_is_a_complete_measurement_not_a_failed_one() {
    // MEASURED during capstone round 2: a skills-only plugin — which `main.rs`
    // and `manifest.rs` both call perfectly ordinary — measures cleanly at
    // status "ok", toolCount 0, and 39 real resident bytes of skill
    // frontmatter. The gate then hard-failed it with "a measurement of nothing
    // satisfies every ceiling", which is exactly backwards: nothing was
    // measured wrong, there was simply no server to ask.
    //
    // The two situations must not share a representation. "A server answered
    // with an empty list" is broken. "No server was declared" is a whole
    // plugin's cost, sitting entirely in the file-backed tiers.
    let no_server = json!({
        "schemaVersion": 1,
        "plugin": "skillsonly",
        "probe": { "status": "no_server", "toolCount": 0, "binary": "", "promptCount": 0 },
        "tiers": {
            "resident": { "bytes": 39, "sources": [
                { "kind": "skill_frontmatter", "id": "advice", "bytes": 39 }] },
            "invocation": { "bytes": 57, "sources": [] }
        }
    });

    assert_eq!(check(&no_server, None, &budget()), Verdict::Pass);
}

#[test]
fn a_declared_server_that_answers_with_no_tools_is_still_fatal() {
    // The other half, and the reason the fix above is a new status rather than
    // dropping the tool-count check. A server that answers `tools/list` with an
    // empty array also reports status "ok"; accepting that would restore the
    // false zero the check exists for — a broken server passing every ceiling
    // by costing nothing. `binary` is non-empty precisely because a server WAS
    // launched.
    let answered_nothing = json!({
        "schemaVersion": 1,
        "plugin": "x",
        "probe": { "status": "ok", "toolCount": 0, "binary": "bin/x", "promptCount": 0 },
        "tiers": {
            "resident": { "bytes": 0, "sources": [] },
            "invocation": { "bytes": 0, "sources": [] }
        }
    });

    let verdict = reasons(check(&answered_nothing, None, &budget()));
    assert!(
        verdict.iter().any(|r| r.contains("no tools")),
        "{verdict:?}"
    );
}

// --- reading a path at the base ref: missing is not the same as unreadable ---

use plugin_footprint::gate::{at_ref, AtRef};

#[test]
fn a_path_git_could_not_produce_is_missing() {
    // Legitimate: the thresholds file on the very first run, or a document for a
    // plugin this change adds.
    assert!(matches!(at_ref(None), AtRef::Missing));
}

#[test]
fn a_path_that_is_there_but_does_not_parse_is_unreadable_not_missing() {
    // MEASURED during capstone round 3, against a real committed ref: a
    // budgets.json with a syntax error made the gate print "has no budget at
    // <ref> yet; measuring without a ceiling" for EVERY plugin and exit 0. One
    // stray comma silently disabled the gate for the whole repository.
    //
    // This is the third distinct place the same conflation appeared — after the
    // base ref itself and an individual budget entry — which is why it is now a
    // named type rather than an Option at each call site.
    assert!(matches!(
        at_ref(Some(b"{\"x\": {\"residentBytes\": 1,,,}")),
        AtRef::Unreadable(_)
    ));
    assert!(matches!(at_ref(Some(b"")), AtRef::Unreadable(_)));
    assert!(matches!(
        at_ref(Some(b"not json at all")),
        AtRef::Unreadable(_)
    ));
}

#[test]
fn a_path_that_parses_comes_back_with_its_value() {
    match at_ref(Some(br#"{"x":{"residentBytes":7}}"#)) {
        AtRef::Found(value) => assert_eq!(value["x"]["residentBytes"], 7),
        other => panic!("expected Found, got {other:?}"),
    }
}

#[test]
fn a_delta_cap_at_or_above_the_headroom_is_refused() {
    // Fork 2 decided the per-change cap D with `D < H`, and the spec states it
    // twice (§6 item 5, §8 Fork 2). Nothing enforced it.
    //
    // Round 3's own policy-preservation fix is what made this live: `ratchet`
    // used to rebuild the entry from its constants on every run, so a violating
    // value could not survive a regeneration. Now it is honoured, which is
    // right for a policy file — and means the invariant has to be checked
    // somewhere instead of accidentally reset.
    //
    // Why it matters rather than being pedantry: the ratchet reclaims slack
    // down to `headroom` above the low-water mark. A cap at or above that
    // headroom lets ONE change spend the entire allowance the ratchet exists to
    // reclaim, which is precisely the jump §6 says the ceiling is blind to.
    for bad in [
        json!({ "x": { "residentBytes": 20000, "headroomBytes": 2000, "deltaBytes": 2000 } }),
        json!({ "x": { "residentBytes": 20000, "headroomBytes": 2000, "deltaBytes": 5000 } }),
        json!({ "x": { "residentBytes": 20000, "headroomBytes": 0, "deltaBytes": 0 } }),
    ] {
        assert!(
            matches!(budget_for(&bad, "x"), BudgetLookup::Malformed(_)),
            "D >= H must be refused: {bad}"
        );
    }

    // And the shipped defaults must still be accepted: 500 < 2000.
    assert!(matches!(
        budget_for(
            &json!({ "x": { "residentBytes": 20000, "headroomBytes": 2000, "deltaBytes": 500 } }),
            "x"
        ),
        BudgetLookup::Found(_)
    ));
}
