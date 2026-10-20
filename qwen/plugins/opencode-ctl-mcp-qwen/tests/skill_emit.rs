//! The committed skill copies equal `emit-skill <name> --host qwen` byte for byte.

use std::path::Path;
use std::process::Command;

fn check(name: &str) {
    let out = Command::new(env!("CARGO_BIN_EXE_opencode-ctl-qwen-mcp"))
        .args(["emit-skill", name, "--host", "qwen"])
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
        "skills/{name}/SKILL.md differs from `emit-skill {name} --host qwen`"
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
