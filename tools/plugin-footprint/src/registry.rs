//! The plugin registry (`agents.json`) as the footprint tooling's iteration
//! source: registry keys name folders and documents, published names do not.

/// The registry keys of the `claude-code` agent's plugins, in file order.
///
/// Keys, not published names: a skills-only plugin is published under a name
/// that differs from its folder and its `docs/footprints/<key>.json`.
pub fn published_plugin_keys(agents_json: &str) -> Result<Vec<String>, String> {
    let registry: serde_json::Value =
        serde_json::from_str(agents_json).map_err(|e| format!("parsing agents.json: {e}"))?;
    let agent = registry["agents"]
        .as_array()
        .and_then(|agents| agents.iter().find(|a| a["id"] == "claude-code"))
        .ok_or("agents.json has no claude-code agent")?;
    Ok(agent["plugins"]
        .as_array()
        .ok_or("the claude-code agent in agents.json declares no plugins array")?
        .iter()
        .filter_map(|p| p.as_str().map(str::to_string))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::published_plugin_keys;

    #[test]
    fn returns_plugins_in_file_order() {
        let json = r#"{"agents":[{"id":"qwen","plugins":["q"]},
            {"id":"claude-code","plugins":["b-cc","a-cc"]}]}"#;
        assert_eq!(published_plugin_keys(json).unwrap(), vec!["b-cc", "a-cc"]);
    }

    #[test]
    fn returns_keys_not_published_names() {
        let json = r#"{"agents":[{"id":"claude-code","plugins":["x-cc","devtools-cc"],
            "publishedNames":{"devtools-cc":"devtools"}}]}"#;
        assert_eq!(
            published_plugin_keys(json).unwrap(),
            vec!["x-cc", "devtools-cc"]
        );
    }

    #[test]
    fn errors_without_claude_code_agent() {
        let json = r#"{"agents":[{"id":"qwen","plugins":["q"]}]}"#;
        assert!(published_plugin_keys(json)
            .unwrap_err()
            .contains("claude-code"));
    }

    #[test]
    fn errors_without_plugins_array() {
        let json = r#"{"agents":[{"id":"claude-code"}]}"#;
        assert!(published_plugin_keys(json).unwrap_err().contains("plugins"));
    }

    #[test]
    fn errors_on_non_json() {
        assert!(published_plugin_keys("not json").is_err());
    }
}
