//! Reading `.mcp.json` and turning it into something launchable (spec §4.2).
//!
//! The launch command is manifest-driven so that a new plugin is measurable with
//! no change to this tool. That only holds if the substitution and the
//! confinement rule are both right, and confinement is the half with teeth: the
//! manifest is repo content, so on a pull request it is contributor-authored
//! input that this tool is about to execute.

use plugin_footprint::manifest::{read_mcp_servers, ManifestError};
use std::path::{Path, PathBuf};

/// A throwaway plugin directory carrying a `.mcp.json`.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(tag: &str, manifest: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "plugin-footprint-manifest-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("create fixture");
        std::fs::write(dir.join(".mcp.json"), manifest).expect("write manifest");
        Self { dir }
    }

    fn empty(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "plugin-footprint-manifest-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("create fixture");
        Self { dir }
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

#[test]
fn reads_command_args_and_env_and_substitutes_the_plugin_root() {
    let fx = Fixture::new(
        "basic",
        r#"{
          "mcpServers": {
            "re-ghidra-mcp-cc": {
              "command": "${CLAUDE_PLUGIN_ROOT}/bin/re-ghidra-cc-mcp",
              "args": ["serve"],
              "env": { "RUST_LOG": "info" }
            }
          }
        }"#,
    );

    let servers = read_mcp_servers(fx.path()).expect("manifest reads");

    assert_eq!(servers.len(), 1);
    let s = &servers[0];
    assert_eq!(s.name, "re-ghidra-mcp-cc");
    assert_eq!(s.args, vec!["serve".to_string()]);
    assert_eq!(s.env.get("RUST_LOG").map(String::as_str), Some("info"));
    assert_eq!(
        s.command,
        fx.path().join("bin").join("re-ghidra-cc-mcp"),
        "${{CLAUDE_PLUGIN_ROOT}} must resolve to the plugin directory"
    );
}

#[test]
fn a_plugin_with_no_manifest_declares_no_servers() {
    // A hooks-only or skills-only plugin has no `.mcp.json` at all. That is not a
    // failure — it has no MCP footprint — and must not be reported as one, or the
    // gate's fatal probe assertion would fire on a perfectly good plugin.
    let fx = Fixture::empty("nomanifest");

    let servers = read_mcp_servers(fx.path()).expect("a missing manifest is not an error");

    assert!(servers.is_empty());
}

#[test]
fn every_declared_server_is_returned() {
    let fx = Fixture::new(
        "two",
        r#"{
          "mcpServers": {
            "beta":  { "command": "${CLAUDE_PLUGIN_ROOT}/bin/b" },
            "alpha": { "command": "${CLAUDE_PLUGIN_ROOT}/bin/a" }
          }
        }"#,
    );

    let servers = read_mcp_servers(fx.path()).expect("manifest reads");

    // Sorted by name so the resulting footprint document is stable across runs
    // rather than inheriting whatever order the JSON happened to be written in.
    let names: Vec<&str> = servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "beta"]);
}

#[test]
fn a_command_escaping_the_plugin_root_is_refused() {
    let fx = Fixture::new(
        "escape",
        r#"{
          "mcpServers": {
            "evil": { "command": "${CLAUDE_PLUGIN_ROOT}/../../../evil" }
          }
        }"#,
    );

    let err =
        read_mcp_servers(fx.path()).expect_err("a command outside the plugin root is refused");

    assert!(
        matches!(err, ManifestError::CommandEscapesPluginRoot { .. }),
        "expected a confinement refusal, got: {err}"
    );
}

#[test]
fn an_absolute_command_outside_the_plugin_root_is_refused() {
    // The escape does not need `..`. A bare absolute path is the simpler form,
    // and a `PATH`-resolved name like "node" lands here too.
    let fx = Fixture::new(
        "absolute",
        r#"{
          "mcpServers": {
            "evil": { "command": "/usr/bin/evil" }
          }
        }"#,
    );

    let err = read_mcp_servers(fx.path()).expect_err("an outside command is refused");

    assert!(
        matches!(err, ManifestError::CommandEscapesPluginRoot { .. }),
        "expected a confinement refusal, got: {err}"
    );
}

#[test]
fn malformed_json_is_an_error_not_an_empty_result() {
    // Reporting "no servers" for a manifest we failed to parse would understate
    // the footprint and pass the gate. It has to be loud.
    let fx = Fixture::new("malformed", "{ this is not json");

    let err = read_mcp_servers(fx.path()).expect_err("malformed JSON must not be silently empty");

    assert!(
        matches!(err, ManifestError::Parse { .. }),
        "expected a parse error, got: {err}"
    );
}

#[test]
fn a_server_without_a_command_is_an_error() {
    let fx = Fixture::new(
        "nocommand",
        r#"{ "mcpServers": { "broken": { "args": ["serve"] } } }"#,
    );

    let err = read_mcp_servers(fx.path()).expect_err("a server with no command cannot be launched");

    assert!(
        matches!(err, ManifestError::MissingCommand { .. }),
        "expected a missing-command error, got: {err}"
    );
}

#[test]
fn a_sibling_whose_name_merely_starts_with_the_root_is_refused() {
    // The classic confinement bypass: `/tmp/plug-evil` is not inside `/tmp/plug`,
    // but a naive string-prefix check says it is. `Path::starts_with` compares
    // whole components, which is what makes this refusal correct — a test that
    // fails loudly if anyone ever "simplifies" it to a string comparison.
    let fx = Fixture::new(
        "sibling",
        r#"{
          "mcpServers": {
            "evil": { "command": "${CLAUDE_PLUGIN_ROOT}/../plugin-footprint-manifest-sibling-evil/x" }
          }
        }"#,
    );

    let err = read_mcp_servers(fx.path()).expect_err("a sibling directory is not inside the root");

    assert!(
        matches!(err, ManifestError::CommandEscapesPluginRoot { .. }),
        "expected a confinement refusal, got: {err}"
    );
}
