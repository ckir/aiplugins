use std::io::Write;
use std::process::{Command, Stdio};

fn run_bridge(input: &str, use_mock: bool) -> (String, String, std::process::ExitStatus) {
    let bin_path = env!("CARGO_BIN_EXE_rtk-mcp-qwen");
    let mut cmd = Command::new(bin_path);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if use_mock {
        // Point RTK_BIN to our mock binary
        let mock_bin = env!("CARGO_BIN_EXE_mock-rtk");
        cmd.env("RTK_BIN", mock_bin);
    } else {
        // Force a non-existent binary so we reliably test the spawn failure path
        cmd.env("RTK_BIN", "this-binary-does-not-exist-12345");
    }

    let mut child = cmd.spawn().expect("Failed to spawn process");
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .expect("Failed to write to stdin");
    }

    let output = child.wait_with_output().expect("Failed to read output");
    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in stdout");
    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF-8 in stderr");

    (stdout, stderr, output.status)
}

fn assert_json_decision(stdout: &str, decision: &str, reason_contains: &str) -> serde_json::Value {
    let json: serde_json::Value = serde_json::from_str(stdout).expect("Valid JSON");
    assert_eq!(
        json["hook_specific_output"]["permission_decision"],
        decision
    );
    let reason = json["hook_specific_output"]["permission_decision_reason"]
        .as_str()
        .unwrap();
    assert!(
        reason.contains(reason_contains),
        "Expected reason to contain '{}', got: '{}'",
        reason_contains,
        reason
    );
    json
}

/// Assert a decision leaves the command untouched.
///
/// The key must be *absent*, not null. `updated_input` is annotated
/// `skip_serializing_if = "Option::is_none"`, and a serialize/deserialize
/// roundtrip cannot verify that: `Option` reads missing and null identically,
/// so the roundtrip stays equal either way. Checking the emitted keys is the
/// only thing that pins it, and this is the wire the agent actually reads.
fn assert_no_rewrite(json: &serde_json::Value) {
    let fields = json["hook_specific_output"]
        .as_object()
        .expect("hook_specific_output object");
    assert!(
        !fields.contains_key("updated_input"),
        "a pass-through must omit updated_input entirely, not send null; got: {json}"
    );
}

#[test]
fn test_e2e_empty_input() {
    let (stdout, _stderr, status) = run_bridge("   \n\t", false);
    assert!(status.success());
    assert_json_decision(&stdout, "allow", "Empty input");
}

#[test]
fn test_e2e_invalid_json() {
    let (stdout, _stderr, status) = run_bridge("this is not json", false);
    assert!(status.success());
    let json = assert_json_decision(&stdout, "allow", "JSON parse error");

    // What actually matters on an unreadable payload: the command must be left
    // alone. The previous assertion here was `stderr.contains("Failed to read
    // stdin") || stderr.is_empty()` — nothing ever writes that message on a
    // parse error, so the disjunction always passed on its second half and
    // asserted nothing at all.
    assert_no_rewrite(&json);
}

#[test]
fn test_e2e_non_shell_command() {
    let input = r#"{"tool_name": "read_file", "tool_input": {}}"#;
    let (stdout, _stderr, status) = run_bridge(input, false);
    assert!(status.success());
    assert_json_decision(&stdout, "allow", "Non-shell command, passing through");
}

#[test]
fn test_e2e_no_command_in_input() {
    let input = r#"{"tool_name": "run_shell_command", "tool_input": {}}"#;
    let (stdout, _stderr, status) = run_bridge(input, false);
    assert!(status.success());
    assert_json_decision(&stdout, "allow", "No command in input");
}

#[test]
fn test_e2e_rtk_spawn_failure_fallback() {
    let input = r#"{"tool_name": "run_shell_command", "tool_input": {"command": "ls"}}"#;
    let (stdout, stderr, status) = run_bridge(input, false); // use_mock = false points to non-existent bin
    assert!(status.success());
    assert_json_decision(&stdout, "allow", "No RTK rewrite available");
    assert!(stderr.contains("Failed to spawn rtk"));
}

#[test]
fn test_e2e_rtk_successful_rewrite() {
    let input = r#"{"tool_name": "run_shell_command", "tool_input": {"command": "ls"}}"#;
    let (stdout, _stderr, status) = run_bridge(input, true); // Use mock
    assert!(status.success());
    let json = assert_json_decision(&stdout, "allow", "RTK rewrite applied");
    assert_eq!(
        json["hook_specific_output"]["updated_input"]["command"],
        "ls -la"
    );
}

#[test]
fn test_e2e_rtk_failure_exit_code() {
    let input = r#"{"tool_name": "run_shell_command", "tool_input": {"command": "fail"}}"#;
    let (stdout, stderr, status) = run_bridge(input, true);
    assert!(status.success()); // The bridge succeeds and falls back
    assert_json_decision(&stdout, "allow", "No RTK rewrite available");
    assert!(stderr.contains("rtk exited with code"));
    assert!(stderr.contains("mock simulated rtk error"));
}

#[test]
fn test_e2e_rtk_empty_output() {
    let input = r#"{"tool_name": "run_shell_command", "tool_input": {"command": "empty"}}"#;
    let (stdout, _stderr, status) = run_bridge(input, true);
    assert!(status.success());
    assert_json_decision(&stdout, "allow", "No RTK rewrite available");
}

#[test]
fn test_e2e_rtk_accepts_a_rewrite_on_exit_code_3() {
    // Exit 3 is a success code for rtk, like 0: a rewrite on stdout must be
    // applied. This is the assertion that actually pins the `code != Some(3)`
    // branch in rtk_rewrite — deleting that branch makes this test fail.
    //
    // Its predecessor did not: it used a mock that exited 3 with *no* output,
    // so the None came from the empty-stdout branch and the special case could
    // be removed with the whole suite still green.
    let input =
        r#"{"tool_name": "run_shell_command", "tool_input": {"command": "code_3_with_rewrite"}}"#;
    let (stdout, _stderr, status) = run_bridge(input, true);
    assert!(status.success());
    let json = assert_json_decision(&stdout, "allow", "RTK rewrite applied");
    assert_eq!(
        json["hook_specific_output"]["updated_input"]["command"],
        "rtk rewritten-on-code-3"
    );
}

#[test]
fn test_e2e_rtk_exit_code_3_without_output_is_a_pass_through() {
    // The other half of the pair: a success code carrying nothing to apply.
    let input =
        r#"{"tool_name": "run_shell_command", "tool_input": {"command": "code_3_no_output"}}"#;
    let (stdout, _stderr, status) = run_bridge(input, true);
    assert!(status.success());
    let json = assert_json_decision(&stdout, "allow", "No RTK rewrite available");
    assert_no_rewrite(&json);
}
