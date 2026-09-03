//! What the probed server is allowed to see of our environment (spec §4.2).
//!
//! This matters most in one place: the published figure is regenerated in a
//! release-time job because that is where `ANTHROPIC_API_KEY` lives, and that
//! same job launches plugin binaries. An inherited environment would hand the
//! key to every server it starts, during a handshake the server controls.
//!
//! The inherited environment is injected rather than read from the process, so
//! these tests never mutate global state — which races under a parallel test
//! runner, and is the same reason `ghidra_mcp::cli::layer` takes its getter.

use plugin_footprint::child_env::child_env;
use std::collections::BTreeMap;

fn inherited(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

fn declared(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    inherited(pairs)
}

#[test]
fn a_secret_in_the_parent_environment_is_not_passed_through() {
    let env = child_env(
        &declared(&[]),
        &inherited(&[
            ("ANTHROPIC_API_KEY", "sk-do-not-leak"),
            ("GITHUB_TOKEN", "ghp-do-not-leak"),
            ("PATH", "/usr/bin"),
        ]),
    );

    assert!(!env.contains_key("ANTHROPIC_API_KEY"));
    assert!(!env.contains_key("GITHUB_TOKEN"));
    assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin"));
}

#[test]
fn the_platform_allowlist_survives_because_an_empty_environment_breaks_the_probe() {
    // Not an empty environment: a process with no PATH cannot resolve anything it
    // shells out to, and on Windows one without SystemRoot fails before `main` in
    // most runtimes. The probe would then report `failed` for every plugin and the
    // gate's fatal probe assertion would make the build permanently red.
    let env = child_env(
        &declared(&[]),
        &inherited(&[
            ("PATH", "/usr/bin"),
            ("HOME", "/home/u"),
            ("TMPDIR", "/tmp"),
            ("LANG", "C.UTF-8"),
            ("SystemRoot", "C:\\Windows"),
            ("UNRELATED", "no"),
        ]),
    );

    for expected in ["PATH", "HOME", "TMPDIR", "LANG", "SystemRoot"] {
        assert!(env.contains_key(expected), "{expected} must survive");
    }
    assert!(!env.contains_key("UNRELATED"));
}

#[test]
fn what_the_manifest_declares_is_passed_through() {
    let env = child_env(
        &declared(&[("RUST_LOG", "info")]),
        &inherited(&[("PATH", "/usr/bin")]),
    );

    assert_eq!(env.get("RUST_LOG").map(String::as_str), Some("info"));
}

#[test]
fn a_declared_value_wins_over_the_inherited_one() {
    // The manifest is the plugin's own statement of what it needs; an ambient
    // value of the same name must not silently override it.
    let env = child_env(
        &declared(&[("PATH", "/plugin/bin")]),
        &inherited(&[("PATH", "/usr/bin")]),
    );

    assert_eq!(env.get("PATH").map(String::as_str), Some("/plugin/bin"));
}

#[test]
#[cfg(windows)]
fn windows_allowlist_matching_is_case_insensitive() {
    // Windows environment blocks are case-insensitive and a runner may expose
    // SYSTEMROOT rather than SystemRoot. A case-sensitive allowlist would strip
    // the variable it was written to keep, and the probe would die before `main`.
    let env = child_env(
        &declared(&[]),
        &inherited(&[("SYSTEMROOT", "C:\\Windows"), ("Path", "C:\\bin")]),
    );

    assert_eq!(env.len(), 2, "both should survive, got: {env:?}");
}

#[test]
#[cfg(windows)]
fn a_declared_value_wins_over_an_inherited_one_spelled_in_another_case() {
    // Windows environments routinely carry `Path`, not `PATH`. A case-sensitive
    // map keeps BOTH spellings, and `Command::envs` applies them in BTreeMap
    // order — "PATH" before "Path" before "path" — so on a case-INsensitive
    // environment block the last one applied wins. MEASURED: a map holding
    // {"FOO": declared, "foo": ambient} produced a child that saw the ambient
    // value, exactly reversing this module's documented contract.
    let env = child_env(
        &declared(&[("PATH", "/plugin/bin")]),
        &inherited(&[("Path", "/usr/bin")]),
    );

    assert_eq!(
        env.len(),
        1,
        "two spellings of one variable must not both survive: {env:?}"
    );
    assert_eq!(
        env.values().next().map(String::as_str),
        Some("/plugin/bin"),
        "the manifest's value must be the one that reaches the child"
    );
}

#[test]
#[cfg(windows)]
fn the_case_collision_guard_covers_non_ascii_names_too() {
    // Windows folds the whole of Unicode, not just ASCII. An `eq_ignore_ascii_case`
    // guard leaves `CAFÉ` and `café` as two surviving keys, and the ambient one
    // sorts later and therefore wins — the exact override the guard exists to stop.
    let env = child_env(
        &declared(&[("CAFÉ", "declared")]),
        &inherited(&[("café", "ambient")]),
    );

    assert_eq!(env.len(), 1, "one variable, one entry: {env:?}");
    assert_eq!(env.values().next().map(String::as_str), Some("declared"));
}
