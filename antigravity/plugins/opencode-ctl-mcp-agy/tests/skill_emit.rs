// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 together with the
// additional terms (evaluation grant, anti-circumvention) that apply to this
// repository. See the LICENSE file at the repository root.

//! The committed skill copies equal `emit-skill <name> --host agy` byte for byte.

use std::path::Path;
use std::process::Command;

fn check(name: &str) {
    let out = Command::new(env!("CARGO_BIN_EXE_opencode-ctl-agy-mcp"))
        .args(["emit-skill", name, "--host", "agy"])
        .env_clear()
        .output()
        .expect("spawn emit-skill");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let committed = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("skills")
            .join(name)
            .join("SKILL.md"),
    )
    .unwrap_or_else(|e| panic!("skills/{name}/SKILL.md: {e}"));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&committed),
        "skills/{name}/SKILL.md differs from `emit-skill {name} --host agy`"
    );
    assert_eq!(out.stdout, committed);
}

#[test]
fn committed_driver_skill_matches_emit_skill() {
    check("opencode-ctl-driver");
}

#[test]
fn committed_doctor_skill_matches_emit_skill() {
    check("opencode-ctl-doctor");
}
