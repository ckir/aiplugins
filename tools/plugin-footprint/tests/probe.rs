//! The live probe (spec §4.1, §4.4).
//!
//! Every case here runs a real process and a real line-delimited JSON-RPC
//! conversation over real pipes, against `probe-target` — a server this crate
//! ships purely so that pagination, method-not-found, hangs, early death and
//! oversized payloads can be produced on demand. The real plugin servers cannot
//! be asked for any of those, and they are the paths that decide whether a
//! failed measurement is reported as a failure or as a zero.

use plugin_footprint::manifest::ServerSpec;
use plugin_footprint::probe::{probe, Limits, Status};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

fn target(args: &[&str]) -> ServerSpec {
    ServerSpec {
        name: "probe-target".to_string(),
        command: PathBuf::from(env!("CARGO_BIN_EXE_probe-target")),
        args: args.iter().map(|a| (*a).to_string()).collect(),
        env: BTreeMap::new(),
    }
}

/// Short deadlines so the timeout cases finish in seconds rather than minutes.
/// The production defaults are 30s per method and 120s per plugin (spec §4.4).
fn quick() -> Limits {
    Limits {
        per_method: Duration::from_millis(600),
        per_plugin: Duration::from_secs(10),
        ..Limits::default()
    }
}

#[test]
fn reads_the_tools_and_prompts_a_server_reports() {
    let outcome = probe(&target(&[]), &quick());

    assert!(
        matches!(outcome.status, Status::Ok),
        "expected a clean probe, got: {:?}",
        outcome.status
    );
    assert_eq!(outcome.tools.len(), 1);
    assert_eq!(outcome.tools[0]["name"], "tool_page_0");
    assert_eq!(outcome.prompts.len(), 1);
}

#[test]
fn follows_cursors_until_the_server_stops_offering_them() {
    // A single-page read of a paginating server undercounts, and an undercount
    // reads as a footprint *improvement* — the gate would reward the defect.
    let outcome = probe(&target(&["--pages", "3"]), &quick());

    assert!(matches!(outcome.status, Status::Ok), "{:?}", outcome.status);
    let names: Vec<&str> = outcome
        .tools
        .iter()
        .map(|t| t["name"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(names, vec!["tool_page_0", "tool_page_1", "tool_page_2"]);
}

#[test]
fn a_server_without_prompts_is_not_a_failed_probe() {
    // `prompts/list` is optional. A method-not-found means zero prompt tokens,
    // which is a real measurement — quite different from a probe that failed.
    let outcome = probe(&target(&["--no-prompts"]), &quick());

    assert!(
        matches!(outcome.status, Status::Ok),
        "an unimplemented optional method is not a failure, got: {:?}",
        outcome.status
    );
    assert!(outcome.prompts.is_empty());
    assert_eq!(outcome.tools.len(), 1, "tools were still measured");
}

#[test]
fn a_server_that_never_answers_times_out_rather_than_reporting_zero() {
    let outcome = probe(&target(&["--hang"]), &quick());

    match &outcome.status {
        Status::TimedOut(why) => assert!(
            why.contains("tools/list"),
            "the timeout should name the method it waited on, got: {why}"
        ),
        other => panic!("expected a timeout, got: {other:?}"),
    }
    assert!(
        outcome.tools.is_empty(),
        "a timed-out probe reports no tools, and the caller must not read that as zero"
    );
}

#[test]
fn a_server_that_dies_mid_handshake_is_failed_not_zero() {
    let outcome = probe(&target(&["--die"]), &quick());

    assert!(
        matches!(outcome.status, Status::Failed(_)),
        "expected a failure, got: {:?}",
        outcome.status
    );
}

#[test]
fn a_command_that_does_not_exist_is_failed() {
    // The common case in CI: the gate ran before the binaries were built.
    let mut spec = target(&[]);
    spec.command = spec.command.with_file_name("no-such-binary-at-all");

    let outcome = probe(&spec, &quick());

    assert!(
        matches!(outcome.status, Status::Failed(_)),
        "expected a failure, got: {:?}",
        outcome.status
    );
}

#[test]
fn an_endless_cursor_trips_the_page_cap_and_says_so() {
    // Cursor-following is a loop driven by the thing being measured. A circular
    // cursor would otherwise run until memory or the plugin deadline gave out.
    let limits = Limits {
        max_pages: 4,
        ..quick()
    };

    let outcome = probe(&target(&["--loop-cursor"]), &limits);

    match &outcome.status {
        Status::Failed(why) => assert!(
            why.contains("page"),
            "the failure must name which limit tripped, got: {why}"
        ),
        other => panic!("expected a failure, got: {other:?}"),
    }
}

#[test]
fn an_oversized_payload_trips_the_size_cap_and_says_so() {
    let limits = Limits {
        max_bytes: 4096,
        max_pages: 1000,
        ..quick()
    };

    // Each page carries a ~2 KiB description, so the cap trips after a few.
    let outcome = probe(&target(&["--pages", "100", "--huge", "2048"]), &limits);

    match &outcome.status {
        Status::Failed(why) => assert!(
            why.contains("byte"),
            "the failure must name which limit tripped, got: {why}"
        ),
        other => panic!("expected a failure, got: {other:?}"),
    }
}

#[test]
fn the_probed_process_does_not_outlive_the_probe() {
    // Every outcome reaps, including the successful one — the path that runs
    // every time. Nothing in MCP obliges a server to exit on stdin EOF, so a
    // prober that merely drops its pipes leaks one process per plugin per run.
    let started = std::time::Instant::now();
    let outcome = probe(&target(&["--hang"]), &quick());
    let elapsed = started.elapsed();

    assert!(matches!(outcome.status, Status::TimedOut(_)));
    assert!(
        outcome.reaped,
        "a hung server must be killed, not abandoned"
    );

    // `reaped` alone is a field, and a field can be set. This bound is what
    // makes the claim mean something: the target sleeps in 60s increments, so
    // dropping the `kill()` and merely waiting would park here far past this.
    // Coarse on purpose — it distinguishes "killed" from "waited", and is not a
    // performance measurement.
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "the probe should return as soon as it gives up, took {elapsed:?}"
    );
}

#[test]
fn a_result_without_the_expected_array_is_failed_not_an_empty_success() {
    // The worst outcome this tool can produce is not a crash, it is a WRONG
    // NUMBER that people trust. A server answering `tools/list` with its tools
    // under some other key must not read as "this plugin has no tools": that
    // would sail through the gate as a spectacular footprint reduction and
    // publish zero for a plugin that costs whatever it costs.
    let outcome = probe(&target(&["--wrong-key"]), &quick());

    match &outcome.status {
        Status::Failed(why) => assert!(
            why.contains("tools"),
            "the failure must name the method and the key it wanted, got: {why}"
        ),
        other => panic!("expected a failure, got: {other:?}"),
    }
    assert!(outcome.tools.is_empty());
}
