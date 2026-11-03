//! `plugin-footprint measure`, driven as the real binary.
//!
//! `tests/document.rs` pins `document::build`, but `measure` is what writes the
//! committed documents. A field added in `main.rs` after `build` returns would
//! pass every library test and surface only as a CI diff, so the written file is
//! pinned here, on a plugin that declares no MCP server (a complete measurement
//! of zero, which needs no built binary).

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_plugin-footprint");

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "plugin-footprint-measure-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(dir.join("plug").join(".claude-plugin")).expect("create fixture");
        std::fs::write(
            dir.join("plug").join(".claude-plugin").join("plugin.json"),
            r#"{"name":"plug"}"#,
        )
        .expect("write manifest");
        Self { dir }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

#[test]
fn a_measured_plugin_is_written_with_exactly_the_documented_keys_and_no_version() {
    let fixture = Fixture::new("keys");
    let out = fixture.dir.join("out.json");

    let status = Command::new(BIN)
        .arg("measure")
        .arg(fixture.dir.join("plug"))
        .arg("--out")
        .arg(&out)
        .status()
        .expect("run plugin-footprint");
    assert!(
        status.success(),
        "a plugin with no MCP server is a complete measurement"
    );

    let text = std::fs::read_to_string(&out).expect("the document was written");
    let value: serde_json::Value = serde_json::from_str(&text).expect("the document is JSON");
    let mut keys: Vec<&str> = value
        .as_object()
        .expect("a document is an object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();

    assert_eq!(
        keys,
        ["agent", "plugin", "probe", "schemaVersion", "tiers", "tree"],
        "the written document: {text}"
    );
    assert!(value.get("pluginVersion").is_none(), "{text}");
}
