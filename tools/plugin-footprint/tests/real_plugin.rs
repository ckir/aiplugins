//! The whole pipeline against a real, shipped plugin.
//!
//! Everything else in this crate's suite runs against `probe-target`, which
//! exists so the failure paths can be produced on demand. This is the check that
//! the tool measures *reality*: it reads the plugin's own committed `.mcp.json`,
//! resolves and confines the command, launches the actual binary and counts what
//! it advertises.
//!
//! Note what this test depends on. It needs `re-ghidra-cc-mcp` to answer
//! `tools/list` with no Ghidra installed and no configuration reachable — which
//! it only does because configuration resolution is deferred
//! (`shared/ghidra-mcp`, the §4.1.1 prerequisite). Before that landed, this test
//! could not have been written: the server exited 2 before the handshake.

use plugin_footprint::manifest::read_mcp_servers;
use plugin_footprint::probe::{probe, Limits, Status};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // `tools/plugin-footprint` -> `tools` -> repo root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

#[test]
fn measures_re_ghidra_mcp_cc_from_its_own_manifest() {
    let plugin = repo_root().join("claude-code").join("re-ghidra-mcp-cc");

    // `bin/` is gitignored and produced by `just build-re-ghidra-mcp-cc`. The
    // gate's CI job runs that build first (spec §8, Fork 7), so this
    // precondition is the same one the gate has; locally it can be absent.
    if !plugin.join("bin").is_dir() {
        eprintln!(
            "SKIP measures_re_ghidra_mcp_cc_from_its_own_manifest: {} has no staged bin/. \
             Run `just build-re-ghidra-mcp-cc` to exercise it.",
            plugin.display()
        );
        return;
    }

    let servers = read_mcp_servers(&plugin).expect("the shipped manifest reads and is confined");
    assert_eq!(servers.len(), 1, "re-ghidra-mcp-cc declares one MCP server");

    let outcome = probe(&servers[0], &Limits::default());

    assert_eq!(
        outcome.status,
        Status::Ok,
        "probing the real binary must succeed with no Ghidra present"
    );
    assert_eq!(
        outcome.tools.len(),
        19,
        "re-ghidra-mcp-cc advertises 19 tools; a change here is a real footprint change"
    );
    assert!(
        outcome.tools.iter().all(|t| t.get("inputSchema").is_some()),
        "every tool must carry the schema whose size is the thing being measured"
    );
    assert!(
        !outcome.child_still_running(),
        "the probed server must not outlive the probe"
    );
}
