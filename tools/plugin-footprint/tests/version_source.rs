//! pluginVersion must come from each package's own manifest, never from a
//! global. Uses real in-tree plugins (real_plugin.rs precedent), so no
//! fixtures can drift from reality.
use plugin_footprint::manifest::plugin_version;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

#[test]
fn each_host_reports_its_own_manifest_version() {
    let root = repo_root();
    let cases = [
        ("claude-code/rtk-mcp-cc", ".claude-plugin/plugin.json"),
        ("qwen/rtk-mcp-qwen", "qwen-extension.json"),
        ("opencode/rtk-mcp-opencode", "package.json"),
        ("antigravity/rtk-mcp-agy", "plugin.json"),
    ];
    for (dir, manifest) in cases {
        let plugin = root.join(dir);
        let text = std::fs::read_to_string(plugin.join(manifest))
            .unwrap_or_else(|_| panic!("fixture manifest missing: {dir}/{manifest}"));
        let want: serde_json::Value = serde_json::from_str(&text).unwrap();
        let want = want.get("version").and_then(|v| v.as_str()).unwrap();
        assert_eq!(
            plugin_version(&plugin).as_deref(),
            Some(want),
            "pluginVersion for {dir} must equal its {manifest}"
        );
    }
}
