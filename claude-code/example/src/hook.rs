//! Decision logic for the `PostToolUse` hook.
//!
//! Kept separate from the binary so it can be unit-tested without spawning a
//! process or touching stdin. The binary in `src/bin/hook.rs` supplies the two
//! things this module deliberately does not do: read stdin, and read files.

use crate::{scan_text, Config, Marker};
use serde::{Deserialize, Serialize};

// ── Input ──────────────────────────────────────────────────────────────
//
// Claude Code sends the hook a JSON object on stdin. Only the fields this hook
// actually uses are modelled, and every one of them is optional: an input shape
// that grows a field must not break the hook.

/// The subset of the `PostToolUse` payload this hook reads.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
pub struct HookInput {
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub tool_input: ToolInput,
}

/// The tool arguments, covering both `Write` and `Edit` payload shapes.
#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
pub struct ToolInput {
    /// Present for both Write and Edit.
    pub file_path: Option<String>,
    /// Present for `Write`: the whole new file body.
    pub content: Option<String>,
    /// Present for `Edit`: only the replacement text.
    pub new_string: Option<String>,
}

// ── Output ─────────────────────────────────────────────────────────────

/// What the hook prints on stdout.
///
/// Field names are camelCase on the wire — that is the harness's contract, not
/// a style choice, so the rename is load-bearing.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    /// Surfaced to Claude as context. `None` means "say nothing".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
    /// Keep the transcript clean when there is nothing to report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppress_output: Option<bool>,
}

impl HookOutput {
    /// The no-op result: nothing to say, nothing shown.
    pub fn silent() -> Self {
        Self {
            system_message: None,
            suppress_output: Some(true),
        }
    }

    /// A result carrying a message back to Claude.
    pub fn message(text: impl Into<String>) -> Self {
        Self {
            system_message: Some(text.into()),
            suppress_output: None,
        }
    }

    /// Serialize, falling back to an empty object so the hook can never emit
    /// malformed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ── Decision ───────────────────────────────────────────────────────────

/// Tool names this hook reacts to.
const WATCHED_TOOLS: [&str; 2] = ["Write", "Edit"];

/// Decide what to report for one hook invocation.
///
/// `read_file` is injected so tests need no file system: it is called only when
/// the payload carries no inline content.
///
/// This function never returns an error. A hook that fails loudly on a payload
/// it did not expect turns every surprise into a broken session, so anything
/// unrecognised degrades to [`HookOutput::silent`].
pub fn evaluate(
    input_json: &str,
    config: &Config,
    read_file: impl FnOnce(&str) -> Option<String>,
) -> HookOutput {
    let Ok(input) = serde_json::from_str::<HookInput>(input_json) else {
        return HookOutput::silent();
    };

    if !WATCHED_TOOLS.contains(&input.tool_name.as_str()) {
        return HookOutput::silent();
    }
    if !config.require_owner {
        return HookOutput::silent();
    }

    // Prefer the text the tool actually wrote; fall back to the file on disk,
    // which is what makes this work for tools whose payload shape differs.
    let content = input
        .tool_input
        .content
        .or(input.tool_input.new_string)
        .or_else(|| input.tool_input.file_path.as_deref().and_then(read_file));

    let Some(content) = content else {
        return HookOutput::silent();
    };

    let unowned: Vec<Marker> = scan_text(&content, &config.kinds)
        .into_iter()
        .filter(Marker::is_unowned)
        .collect();

    if unowned.is_empty() {
        return HookOutput::silent();
    }

    let file = input.tool_input.file_path.as_deref().unwrap_or("the file");
    HookOutput::message(format_report(file, &unowned))
}

/// Build the human-readable report shown to Claude.
fn format_report(file: &str, unowned: &[Marker]) -> String {
    let plural = if unowned.len() == 1 {
        "marker"
    } else {
        "markers"
    };
    let mut out = format!(
        "{} unowned {} in {}. House convention is `KIND(owner): note` — \
         add an owner or resolve them:",
        unowned.len(),
        plural,
        file
    );
    for m in unowned {
        let note = if m.note.is_empty() {
            String::new()
        } else {
            format!(" {}", m.note)
        };
        out.push_str(&format!(
            "\n  line {}: {}:{}",
            m.line,
            m.kind.keyword(),
            note
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MarkerKind;

    fn never_read(_: &str) -> Option<String> {
        panic!("read_file must not be called when inline content is present");
    }

    fn write_event(content: &str) -> String {
        serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": { "file_path": "src/main.rs", "content": content }
        })
        .to_string()
    }

    #[test]
    fn flags_unowned_todo_in_a_write() {
        let out = evaluate(
            &write_event("// TODO: fix later\n"),
            &Config::default(),
            never_read,
        );
        let msg = out.system_message.expect("expected a message");
        assert!(msg.contains("1 unowned marker"), "got: {msg}");
        assert!(msg.contains("src/main.rs"), "got: {msg}");
        assert!(msg.contains("line 1"), "got: {msg}");
    }

    #[test]
    fn stays_silent_when_every_marker_has_an_owner() {
        let out = evaluate(
            &write_event("// TODO(alice): fix later\n"),
            &Config::default(),
            never_read,
        );
        assert_eq!(out, HookOutput::silent());
    }

    #[test]
    fn stays_silent_on_clean_content() {
        let out = evaluate(
            &write_event("fn main() {}\n"),
            &Config::default(),
            never_read,
        );
        assert_eq!(out, HookOutput::silent());
    }

    #[test]
    fn counts_multiple_unowned_markers() {
        let content = "// TODO: one\n// FIXME: two\n// TODO(bob): owned\n";
        let out = evaluate(&write_event(content), &Config::default(), never_read);
        let msg = out.system_message.unwrap();
        assert!(msg.contains("2 unowned markers"), "got: {msg}");
        assert!(msg.contains("line 1"), "got: {msg}");
        assert!(msg.contains("line 2"), "got: {msg}");
        assert!(!msg.contains("line 3"), "owned marker leaked: {msg}");
    }

    #[test]
    fn reads_new_string_for_edit_events() {
        let event = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": "src/lib.rs",
                "old_string": "fn a() {}",
                "new_string": "fn a() {} // TODO: rename"
            }
        })
        .to_string();
        let out = evaluate(&event, &Config::default(), never_read);
        assert!(out.system_message.unwrap().contains("1 unowned marker"));
    }

    #[test]
    fn falls_back_to_reading_the_file_when_payload_has_no_text() {
        let event = serde_json::json!({
            "tool_name": "Write",
            "tool_input": { "file_path": "src/main.rs" }
        })
        .to_string();
        let out = evaluate(&event, &Config::default(), |path| {
            assert_eq!(path, "src/main.rs");
            Some("// FIXME: from disk\n".to_string())
        });
        assert!(out.system_message.unwrap().contains("1 unowned marker"));
    }

    #[test]
    fn ignores_tools_it_does_not_watch() {
        let event = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": { "content": "// TODO: not a file write" }
        })
        .to_string();
        assert_eq!(
            evaluate(&event, &Config::default(), never_read),
            HookOutput::silent()
        );
    }

    #[test]
    fn respects_require_owner_false() {
        let config = Config {
            require_owner: false,
            ..Config::default()
        };
        let out = evaluate(&write_event("// TODO: fix later\n"), &config, never_read);
        assert_eq!(out, HookOutput::silent());
    }

    #[test]
    fn respects_the_configured_kinds() {
        let config = Config {
            kinds: vec![MarkerKind::Fixme],
            ..Config::default()
        };
        let out = evaluate(&write_event("// TODO: ignored\n"), &config, never_read);
        assert_eq!(out, HookOutput::silent());
    }

    #[test]
    fn malformed_json_is_silent_not_fatal() {
        assert_eq!(
            evaluate("not json at all", &Config::default(), never_read),
            HookOutput::silent()
        );
    }

    #[test]
    fn unknown_extra_fields_do_not_break_parsing() {
        let event = serde_json::json!({
            "tool_name": "Write",
            "session_id": "abc",
            "some_future_field": { "nested": true },
            "tool_input": { "file_path": "a.rs", "content": "// TODO: x", "extra": 1 }
        })
        .to_string();
        assert!(evaluate(&event, &Config::default(), never_read)
            .system_message
            .is_some());
    }

    #[test]
    fn silent_output_serializes_without_a_message() {
        let json: serde_json::Value =
            serde_json::from_str(&HookOutput::silent().to_json()).unwrap();
        assert!(json.get("systemMessage").is_none());
        assert_eq!(json["suppressOutput"], true);
    }

    #[test]
    fn message_output_uses_camel_case_on_the_wire() {
        let json: serde_json::Value =
            serde_json::from_str(&HookOutput::message("hi").to_json()).unwrap();
        assert_eq!(json["systemMessage"], "hi");
    }
}
