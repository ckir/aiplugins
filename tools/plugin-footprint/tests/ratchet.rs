//! The ratchet (spec §6, Fork 2), driven as the real binary.
//!
//! Exercised through the compiled command rather than as a library call,
//! because every defect this file pins was in the I/O: what it does with a
//! budgets file that is absent, hand-edited, or not an object at all. Three of
//! them reached the capstone review precisely because this file did not exist.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_plugin-footprint");

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "plugin-footprint-ratchet-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("create fixture");
        Self { dir }
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.dir.join(name);
        std::fs::write(&path, contents).expect("write");
        path
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn read(&self, name: &str) -> serde_json::Value {
        let text = std::fs::read_to_string(self.dir.join(name)).expect("read");
        serde_json::from_str(&text).expect("parse")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn measured(plugin: &str, status: &str, bytes: u64) -> String {
    serde_json::json!({
        "schemaVersion": 1,
        "plugin": plugin,
        "probe": { "status": status, "toolCount": 4, "binary": "bin/x", "promptCount": 0 },
        "tiers": {
            "resident": { "bytes": bytes, "sources": [] },
            "invocation": { "bytes": 0, "sources": [] }
        }
    })
    .to_string()
}

fn ratchet(doc: &Path, budgets: &Path) -> std::process::Output {
    Command::new(BIN)
        .args([
            "ratchet",
            "--measured",
            &doc.to_string_lossy(),
            "--budgets",
            &budgets.to_string_lossy(),
        ])
        .output()
        .expect("run the ratchet")
}

#[test]
fn a_plugin_absent_from_the_budgets_file_is_seeded_at_measured_plus_headroom() {
    let fx = Fixture::new("seed");
    let doc = fx.write("m.json", &measured("newcomer", "ok", 10_000));

    // Deliberately no budgets file at all: this is the very first run.
    let out = ratchet(&doc, &fx.path("budgets.json"));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let budgets = fx.read("budgets.json");
    assert_eq!(budgets["newcomer"]["residentBytes"], 12_000);
    assert_eq!(budgets["newcomer"]["headroomBytes"], 2_000);
    assert_eq!(budgets["newcomer"]["deltaBytes"], 500);
}

#[test]
fn a_hand_set_policy_value_survives_regeneration() {
    // The plan's File Structure table gives the reason budgets.json is a
    // separate file at all: "a measurement and a policy are different things and
    // regenerating the first must not silently rewrite the second".
    //
    // MEASURED during capstone round 3: it did rewrite it. A maintainer raising
    // deltaBytes to 1500 to admit one legitimately large change had it reset to
    // 500 by the next `footprint-regen` — and because budgets.json lives under
    // docs/footprints/, the freshness check then failed the build as "stale".
    // The two mechanisms together made a reviewed threshold change impossible.
    let fx = Fixture::new("policy");
    let doc = fx.write("m.json", &measured("x", "ok", 10_000));
    let budgets = fx.write(
        "budgets.json",
        r#"{"x":{"residentBytes":12000,"headroomBytes":3000,"deltaBytes":1500}}"#,
    );

    let out = ratchet(&doc, &budgets);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let after = fx.read("budgets.json");
    assert_eq!(after["x"]["deltaBytes"], 1_500, "policy must survive");
    assert_eq!(after["x"]["headroomBytes"], 3_000, "policy must survive");
}

#[test]
fn the_hysteresis_uses_the_plugins_own_headroom_not_the_default() {
    // Once headroomBytes is a per-plugin policy value, the hysteresis has to
    // read it. Using the hardcoded default instead would silently apply a
    // different rule from the one the file states.
    let fx = Fixture::new("hyst");
    // headroom 3000, so the ceiling follows only a measurement 2*3000 below it.
    // 12000 - 6000 = 6000: a measurement of exactly 6000 moves it, to 9000.
    let doc = fx.write("m.json", &measured("x", "ok", 6_000));
    let budgets = fx.write(
        "budgets.json",
        r#"{"x":{"residentBytes":12000,"headroomBytes":3000,"deltaBytes":500}}"#,
    );

    assert!(ratchet(&doc, &budgets).status.success());

    assert_eq!(fx.read("budgets.json")["x"]["residentBytes"], 9_000);
}

#[test]
fn an_unchanged_measurement_does_not_move_the_ceiling() {
    // A ratchet that moved on every run would churn the committed file, and the
    // freshness check would then fail on nothing at all.
    let fx = Fixture::new("stable");
    let doc = fx.write("m.json", &measured("x", "ok", 10_000));
    let budgets = fx.write(
        "budgets.json",
        r#"{"x":{"residentBytes":12000,"headroomBytes":2000,"deltaBytes":500}}"#,
    );

    assert!(ratchet(&doc, &budgets).status.success());
    assert_eq!(fx.read("budgets.json")["x"]["residentBytes"], 12_000);
}

#[test]
fn a_budgets_file_that_is_not_an_object_is_refused_rather_than_panicking() {
    // MEASURED during capstone round 3: a budgets.json holding `[]` — valid
    // JSON, wrong shape — panicked inside serde_json with "cannot access key
    // ... in JSON array". Under `set -e` in footprint-regen that aborts the run
    // with a backtrace note and no statement of what is actually wrong.
    let fx = Fixture::new("notobject");
    let doc = fx.write("m.json", &measured("x", "ok", 10_000));
    let budgets = fx.write("budgets.json", "[]");

    let out = ratchet(&doc, &budgets);

    assert!(!out.status.success(), "must not succeed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("panicked"),
        "must fail with a message, not a panic: {stderr}"
    );
    assert!(
        stderr.contains("object"),
        "the message must say what shape was expected: {stderr}"
    );
}

#[test]
fn a_failed_probe_never_moves_a_threshold() {
    // Its tiers are absent, so the measurement reads as zero — and ratcheting
    // every budget to the floor would fail every subsequent change.
    let fx = Fixture::new("failed");
    let doc = fx.write("m.json", &measured("x", "failed", 0));
    let budgets = fx.write(
        "budgets.json",
        r#"{"x":{"residentBytes":12000,"headroomBytes":2000,"deltaBytes":500}}"#,
    );

    assert!(!ratchet(&doc, &budgets).status.success());
    assert_eq!(fx.read("budgets.json")["x"]["residentBytes"], 12_000);
}

#[test]
fn a_plugin_with_no_mcp_server_may_still_seed_a_threshold() {
    // `no_server` is a complete measurement (see probe::Status), so it earns a
    // budget like any other. Refusing it would leave skills-only plugins
    // permanently ungated.
    let fx = Fixture::new("noserver");
    let doc = fx.write("m.json", &measured("skillsonly", "no_server", 4_000));

    let out = ratchet(&doc, &fx.path("budgets.json"));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert_eq!(
        fx.read("budgets.json")["skillsonly"]["residentBytes"],
        6_000
    );
}
