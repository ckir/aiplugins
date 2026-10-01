//! `re-ghidra-agy-mcp emit-skill` must reproduce the COMMITTED plugin copy byte for byte.

use std::path::PathBuf;
use std::process::Command;

/// The committed plugin copy - the artifact `emit-skill` is supposed to regenerate.
fn plugin_copy() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills/ghidra-re-driver/SKILL.md")
}

/// Run `emit-skill` on the real binary and return its stdout.
fn emit() -> Vec<u8> {
    let out = Command::new(env!("CARGO_BIN_EXE_re-ghidra-agy-mcp"))
        .arg("emit-skill")
        .output()
        .expect("run `re-ghidra-agy-mcp emit-skill`");
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
    let text = String::from_utf8(emit()).expect("emit output is UTF-8");

    assert!(
        text.starts_with("---\n") || text.starts_with("---\r\n"),
        "emit must begin at the frontmatter `---`, got: {:?}",
        text.chars().take(40).collect::<String>()
    );
    assert!(
        !text.starts_with("<!--"),
        "emit begins inside an HTML comment - the maintainer header leaked"
    );
    assert!(
        !text.contains("SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0"),
        "the maintainer-facing HTML comment header leaked into the emitted copy"
    );
}
