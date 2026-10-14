//! The oracle nobody had: `re-ghidra-qwen-mcp emit-skill` must reproduce the
//! COMMITTED plugin copy byte for byte.
//!
//! Nothing compared them before. `skill_asset.rs` compares the embedded const
//! to the CANONICAL, and the ship-guard is a `.contains(...)` substring check
//! that passes whether the frontmatter sits at line 1 or line 11 - so a
//! regenerated copy carrying a stray header would have shipped unnoticed.
//!
//! This is the FAST tier (`just test`, pure Rust, no Ghidra and no JVM):
//! `emit-skill` only prints an embedded `include_str!` const. Putting it behind
//! the live-Ghidra gate would tie the emit contract to GHIDRA_INSTALL_DIR +
//! JDK 21, so it would almost never run.
//!
//! It lives in the PLUGIN rather than in `shared/ghidra-mcp` because the
//! artifact under test is this plugin's committed copy. The canonical skill is
//! shared; each agent plugin owns — and must pin — its own generated copy of it.

use std::path::PathBuf;
use std::process::Command;

/// The committed plugin copy - the artifact `emit-skill` is supposed to regenerate.
fn plugin_copy() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills/ghidra-re-driver/SKILL.md")
}

/// Run `emit-skill` on the real binary and return its stdout.
///
/// `env!("CARGO_BIN_EXE_re-ghidra-qwen-mcp")` is the built binary's path,
/// resolved by cargo at compile time - no `cargo run` indirection, and no
/// dependence on anything being installed on PATH.
fn emit() -> Vec<u8> {
    let out = Command::new(env!("CARGO_BIN_EXE_re-ghidra-qwen-mcp"))
        .arg("emit-skill")
        .output()
        .expect("run `re-ghidra-qwen-mcp emit-skill`");
    assert!(
        out.status.success(),
        "`emit-skill` exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

#[test]
fn emit_reproduces_the_committed_plugin_copy_byte_for_byte() {
    let stdout = emit();
    let expected = std::fs::read(plugin_copy()).expect("read the committed plugin copy");

    // Compare as text so a failure is readable, but assert on the exact bytes.
    if stdout != expected {
        let got = String::from_utf8_lossy(&stdout);
        let want = String::from_utf8_lossy(&expected);
        let first_diff = got
            .lines()
            .zip(want.lines())
            .position(|(a, b)| a != b)
            .map(|i| format!("first differing line: {}", i + 1))
            .unwrap_or_else(|| "no differing line; lengths differ".to_string());
        panic!(
            "`emit-skill` no longer reproduces skills/ghidra-re-driver/SKILL.md.\n\
             emitted {} bytes / {} lines, committed copy {} bytes / {} lines\n{}\n\
             Regenerate with: just emit-ghidra-skill",
            stdout.len(),
            got.lines().count(),
            expected.len(),
            want.lines().count(),
            first_diff
        );
    }
}

#[test]
fn emit_output_does_not_carry_the_maintainer_header() {
    // A second, independent lens on the same contract: even if the copy above
    // were regenerated WRONG and committed, this row still fails. It encodes
    // what the header IS rather than what the copy happens to contain, so the
    // two rows cannot go green together on a bad copy.
    let text = String::from_utf8(emit()).expect("emit output is UTF-8");

    assert!(
        text.starts_with("---\n") || text.starts_with("---\r\n"),
        "emit must begin at the frontmatter `---`, got: {:?}",
        text.chars().take(40).collect::<String>()
    );
    // STRUCTURAL first: the emit must not begin inside an HTML comment. This
    // survives the header's text being reworded, which the content check below
    // does not - replace the SPDX line with a differently-worded copyright and
    // a leaked header would sail past a content-only assertion.
    assert!(
        !text.starts_with("<!--"),
        "emit begins inside an HTML comment - the maintainer header leaked"
    );
    // Content check kept as a second, independent signal against today's
    // specific header.
    assert!(
        !text.contains("SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0"),
        "the maintainer-facing HTML comment header leaked into the emitted copy"
    );
}
