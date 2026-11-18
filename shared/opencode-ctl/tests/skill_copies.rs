// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 together with the
// additional terms (evaluation grant, anti-circumvention) that apply to this
// repository. See the LICENSE file at the repository root.

//! The eight committed per-plugin skill copies equal `skill_asset::render` byte for byte.

use opencode_ctl::host::Host;
use opencode_ctl::skill_asset::{render, SkillName};
use std::path::Path;

const FRONTS: [(Host, &str); 4] = [
    (Host::Claude, "claude-code/plugins/opencode-ctl-mcp-cc"),
    (Host::Qwen, "qwen/plugins/opencode-ctl-mcp-qwen"),
    (Host::Agy, "antigravity/plugins/opencode-ctl-mcp-agy"),
    (Host::Opencode, "opencode/plugins/opencode-ctl-mcp-opencode"),
];
const NAMES: [SkillName; 2] = [SkillName::Driver, SkillName::Doctor];

#[test]
fn skill_emit_eight_committed_copies_match() {
    let mut checked = 0;
    for (host, dir) in FRONTS {
        for name in NAMES {
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(dir)
                .join("skills")
                .join(name.as_str())
                .join("SKILL.md");
            let committed =
                std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let rendered = render(name, host).expect("render");
            assert_eq!(
                rendered.as_bytes(),
                committed.as_slice(),
                "{} differs from render({}, {host:?})",
                path.display(),
                name.as_str()
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 8);
}
