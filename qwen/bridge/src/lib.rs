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
                permission_decision_reason: format!("RTK rewrite applied: {} -> {}", original, rewritten),
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
        None => QwenOutput::pass_through(&format!(
            "No RTK rewrite available for: {}",
            command
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pass_through_output() {
        let output = QwenOutput::pass_through("test reason");
        assert_eq!(output.hook_specific_output.hook_event_name, "PreToolUse");
        assert_eq!(output.hook_specific_output.permission_decision, "allow");
        assert_eq!(output.hook_specific_output.permission_decision_reason, "test reason");
        assert!(output.hook_specific_output.updated_input.is_none());
    }

    #[test]
    fn test_rewritten_output() {
        let output = QwenOutput::rewritten("ls", "ls -la");
        assert_eq!(output.hook_specific_output.permission_decision, "allow");
        assert!(output.hook_specific_output.updated_input.is_some());
        assert_eq!(
            output.hook_specific_output.updated_input.as_ref().unwrap().command,
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
            result.hook_specific_output.updated_input.as_ref().unwrap().command,
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
