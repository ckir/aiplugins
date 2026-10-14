use serde::{Deserialize, Serialize};

// ── Qwen PreToolUse input ──────────────────────────────────────────────
#[derive(Debug, Deserialize, PartialEq)]
pub struct QwenInput {
    pub tool_name: String,
    pub tool_input: ToolInput,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct ToolInput {
    pub command: Option<String>,
}

// ── Qwen PreToolUse output ─────────────────────────────────────────────
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct QwenOutput {
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HookSpecificOutput {
    pub hook_event_name: String,
    pub permission_decision: String,
    pub permission_decision_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<UpdatedInput>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct UpdatedInput {
    pub command: String,
}

impl QwenOutput {
    pub fn pass_through(reason: &str) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreToolUse".into(),
                permission_decision: "allow".into(),
                permission_decision_reason: reason.into(),
                updated_input: None,
            },
        }
    }

    pub fn rewritten(original: &str, rewritten: &str) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreToolUse".into(),
                permission_decision: "allow".into(),
                permission_decision_reason: format!(
                    "RTK rewrite applied: {} -> {}",
                    original, rewritten
                ),
                updated_input: Some(UpdatedInput {
                    command: rewritten.into(),
                }),
            },
        }
    }

    /// Serialize to JSON string, returning None on failure.
    pub fn to_json(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }

    /// Parse from JSON string.
    pub fn from_json(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }
}

/// The rtk executable used when `RTK_BIN` says nothing.
pub const DEFAULT_RTK_BIN: &str = "rtk";

/// Decide which rtk executable to invoke.
///
/// A blank value counts as unset. `RTK_BIN=` is how a shell commonly spells
/// "clear this", and `env::var` reports it as `Ok("")` rather than an error —
/// so taking it literally means trying to spawn the empty string, which fails
/// with a confusing OS error instead of quietly using rtk from `PATH`.
///
/// The variable is deliberately un-prefixed: the sibling `rtk-mcp-agy` and
/// `rtk-mcp-cc` plugins honour the same one, and all three resolve it the same
/// way, so relocating rtk takes one variable rather than three.
pub fn resolve_rtk_bin(override_value: Option<String>) -> String {
    override_value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_RTK_BIN.to_string())
}

/// Process a single hook input and return the output.
///
/// If `rtk_rewrite_fn` returns Some, the command is rewritten.
/// Otherwise the original command passes through.
pub fn process_hook(
    input_json: &str,
    rtk_rewrite_fn: impl FnOnce(&str) -> Option<String>,
) -> QwenOutput {
    // Parse input
    let data: QwenInput = match serde_json::from_str(input_json) {
        Ok(v) => v,
        Err(_) => return QwenOutput::pass_through("JSON parse error, passing through"),
    };

    // Only process run_shell_command
    if data.tool_name != "run_shell_command" {
        return QwenOutput::pass_through("Non-shell command, passing through");
    }

    let command = match data.tool_input.command {
        Some(cmd) if !cmd.trim().is_empty() => cmd,
        _ => return QwenOutput::pass_through("No command in input"),
    };

    // Ask RTK to rewrite the command
    match rtk_rewrite_fn(&command) {
        Some(rewritten) => QwenOutput::rewritten(&command, &rewritten),
        None => QwenOutput::pass_through(&format!("No RTK rewrite available for: {}", command)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtk_bin_defaults_to_the_path_lookup() {
        assert_eq!(resolve_rtk_bin(None), DEFAULT_RTK_BIN);
    }

    #[test]
    fn test_rtk_bin_honours_an_override() {
        assert_eq!(
            resolve_rtk_bin(Some("/opt/rtk/bin/rtk".to_string())),
            "/opt/rtk/bin/rtk"
        );
        assert_eq!(resolve_rtk_bin(Some("  rtk-dev  ".to_string())), "rtk-dev");
    }

    #[test]
    fn test_blank_rtk_bin_does_not_erase_the_default() {
        // `env::var` returns Ok("") for `RTK_BIN=`, so without this the hook
        // would try to spawn the empty string rather than fall back to PATH.
        for blank in ["", "   ", "\t", "\n"] {
            assert_eq!(
                resolve_rtk_bin(Some(blank.to_string())),
                DEFAULT_RTK_BIN,
                "blank value {blank:?} must count as unset"
            );
        }
    }

    #[test]
    fn test_pass_through_output() {
        let output = QwenOutput::pass_through("test reason");
        assert_eq!(output.hook_specific_output.hook_event_name, "PreToolUse");
        assert_eq!(output.hook_specific_output.permission_decision, "allow");
        assert_eq!(
            output.hook_specific_output.permission_decision_reason,
            "test reason"
        );
        assert!(output.hook_specific_output.updated_input.is_none());
    }

    #[test]
    fn test_rewritten_output() {
        let output = QwenOutput::rewritten("ls", "ls -la");
        assert_eq!(output.hook_specific_output.permission_decision, "allow");
        assert!(output.hook_specific_output.updated_input.is_some());
        assert_eq!(
            output
                .hook_specific_output
                .updated_input
                .as_ref()
                .unwrap()
                .command,
            "ls -la"
        );
    }

    #[test]
    fn test_process_hook_json_parse_error() {
        let result = process_hook("not json", |_| None);
        assert_eq!(
            result.hook_specific_output.permission_decision_reason,
            "JSON parse error, passing through"
        );
    }

    #[test]
    fn test_process_hook_non_shell_command() {
        let input = r#"{"tool_name": "read_file", "tool_input": {}}"#;
        let result = process_hook(input, |_| None);
        assert_eq!(
            result.hook_specific_output.permission_decision_reason,
            "Non-shell command, passing through"
        );
    }

    #[test]
    fn test_process_hook_no_command() {
        let input = r#"{"tool_name": "run_shell_command", "tool_input": {}}"#;
        let result = process_hook(input, |_| None);
        assert_eq!(
            result.hook_specific_output.permission_decision_reason,
            "No command in input"
        );
    }

    #[test]
    fn test_process_hook_empty_command() {
        let input = r#"{"tool_name": "run_shell_command", "tool_input": {"command": "  "}}"#;
        let result = process_hook(input, |_| None);
        assert_eq!(
            result.hook_specific_output.permission_decision_reason,
            "No command in input"
        );
    }

    #[test]
    fn test_process_hook_with_rewrite() {
        let input = r#"{"tool_name": "run_shell_command", "tool_input": {"command": "ls"}}"#;
        let result = process_hook(input, |_| Some("ls -la --color".to_string()));
        assert!(result.hook_specific_output.updated_input.is_some());
        assert_eq!(
            result
                .hook_specific_output
                .updated_input
                .as_ref()
                .unwrap()
                .command,
            "ls -la --color"
        );
    }

    #[test]
    fn test_process_hook_no_rewrite() {
        let input = r#"{"tool_name": "run_shell_command", "tool_input": {"command": "ls"}}"#;
        let result = process_hook(input, |_| None);
        assert!(result.hook_specific_output.updated_input.is_none());
        assert_eq!(
            result.hook_specific_output.permission_decision_reason,
            "No RTK rewrite available for: ls"
        );
    }

    /// The keys actually emitted under `hook_specific_output`.
    fn emitted_fields(output: &QwenOutput) -> serde_json::Map<String, serde_json::Value> {
        let json = output.to_json().expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        value["hook_specific_output"]
            .as_object()
            .expect("hook_specific_output object")
            .clone()
    }

    #[test]
    fn test_pass_through_omits_updated_input_from_the_wire() {
        // `skip_serializing_if = "Option::is_none"` is a wire contract: a
        // pass-through must omit the key, not send `"updated_input": null`.
        //
        // The roundtrip tests below cannot check this. `Option` deserializes a
        // missing key and an explicit null identically, so serialize →
        // deserialize → compare stays equal whether or not the attribute is
        // there. Only the emitted keys reveal it.
        let fields = emitted_fields(&QwenOutput::pass_through("reason"));
        assert!(!fields.contains_key("updated_input"), "got: {fields:?}");
        // The three fields that must always be present.
        for key in [
            "hook_event_name",
            "permission_decision",
            "permission_decision_reason",
        ] {
            assert!(fields.contains_key(key), "missing {key}: {fields:?}");
        }
    }

    #[test]
    fn test_rewrite_includes_updated_input_on_the_wire() {
        let fields = emitted_fields(&QwenOutput::rewritten("ls", "ls -la"));
        assert_eq!(
            fields["updated_input"]["command"], "ls -la",
            "got: {fields:?}"
        );
    }

    #[test]
    fn test_output_json_roundtrip() {
        let output = QwenOutput::rewritten("ls", "ls -la");
        let json = output.to_json().expect("serialize");
        let parsed = QwenOutput::from_json(&json).expect("deserialize");
        assert_eq!(output, parsed);
    }

    #[test]
    fn test_output_json_pass_through() {
        let output = QwenOutput::pass_through("reason");
        let json = output.to_json().expect("serialize");
        let parsed = QwenOutput::from_json(&json).expect("deserialize");
        assert_eq!(output, parsed);
    }
}
