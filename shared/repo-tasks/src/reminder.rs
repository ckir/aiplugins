// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The session reminder (10): one line, wrapped the way each host expects.

use std::path::Path;

use serde_json::Value;

use crate::error::Error;
use crate::host::Host;
use crate::model::{List, Status};
use crate::resolve::{locate, start_dir, Entry, Env};
use crate::store::{read_list, Tuning};

/// The line of 10.1 for a list that cannot be used.
pub const NOTICE: &str =
    "repo-tasks: this repository's task list cannot be read. Call the task_list tool to see why.";

/// The host named by `reminder --event <host>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostEvent {
    ClaudeCode,
    Qwen,
    Antigravity,
    Opencode,
}

impl HostEvent {
    pub fn parse(s: &str) -> Option<HostEvent> {
        match s {
            "claude-code" => Some(HostEvent::ClaudeCode),
            "qwen" => Some(HostEvent::Qwen),
            "antigravity" => Some(HostEvent::Antigravity),
            "opencode" => Some(HostEvent::Opencode),
            _ => None,
        }
    }

    /// The host as 7.1 sees it: only Antigravity differs (no working-directory fallback).
    pub fn host(self) -> Host {
        match self {
            HostEvent::ClaudeCode => Host::Claude,
            HostEvent::Qwen => Host::Qwen,
            HostEvent::Antigravity => Host::Agy,
            HostEvent::Opencode => Host::Opencode,
        }
    }

    /// Whether the host's event arrives as JSON on standard input.
    pub fn reads_stdin(self) -> bool {
        self != HostEvent::Opencode
    }
}

/// Why the list could not be read: the one failure that speaks, or every other one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadFailure {
    /// The list file exists and cannot be used, or 7.2 stopped on an unfollowable `.git`.
    Unusable,
    /// Anything else: the reminder prints nothing.
    Silent,
}

/// A failure while resolving the store: only an unfollowable `.git` entry speaks.
pub fn resolve_failure(e: &Error) -> ReadFailure {
    match e {
        Error::GitEntry { .. } => ReadFailure::Unusable,
        _ => ReadFailure::Silent,
    }
}

/// A failure while reading the list file: it exists and cannot be used.
pub fn read_failure(_e: &Error) -> ReadFailure {
    ReadFailure::Unusable
}

/// The line of 10.1, the notice, or `None`. No subject, description or owner appears.
pub fn line(outcome: Result<List, ReadFailure>) -> Option<String> {
    match outcome {
        Err(ReadFailure::Unusable) => Some(NOTICE.to_string()),
        Err(ReadFailure::Silent) => None,
        Ok(list) => {
            let open = list
                .tasks
                .iter()
                .filter(|t| t.status != Status::Completed)
                .count();
            let doing = list
                .tasks
                .iter()
                .filter(|t| t.status == Status::InProgress)
                .count();
            (open > 0).then(|| {
                format!(
                    "repo-tasks: {open} open in this repository's task list, {doing} in progress. Call the task_list tool to see them."
                )
            })
        }
    }
}

/// Each host's output form (10.2). The hooks' keys are those the existing `re-ghidra` hooks
/// print; the workspace plays no part in the output.
pub fn wrap(host_event: HostEvent, line: &str, _workspace: Option<&str>) -> String {
    match host_event {
        HostEvent::ClaudeCode => serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": line,
            }
        })
        .to_string(),
        HostEvent::Qwen => serde_json::json!({
            "systemMessage": line,
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": line,
            }
        })
        .to_string(),
        HostEvent::Antigravity => serde_json::json!({
            "injectSteps": [
                {
                    "ephemeralMessage": line
                }
            ]
        })
        .to_string(),
        HostEvent::Opencode => line.to_string(),
    }
}

/// Variables the hosts set for a hook, read once by the caller.
#[derive(Debug, Clone, Default)]
pub struct HookEnv {
    pub claude_project_dir: Option<String>,
    pub qwen_project_dir: Option<String>,
}

impl HookEnv {
    pub fn from_process() -> HookEnv {
        let get = |k: &str| std::env::var(k).ok();
        HookEnv {
            claude_project_dir: get("CLAUDE_PROJECT_DIR"),
            qwen_project_dir: get("QWEN_PROJECT_DIR"),
        }
    }
}

fn non_blank(s: Option<&str>) -> Option<String> {
    s.filter(|v| !v.trim().is_empty()).map(str::to_string)
}

/// What the event on standard input says: the workspace path, and whether the host asks
/// for nothing (Antigravity after its first invocation).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventInfo {
    pub workspace: Option<String>,
    pub skip: bool,
}

pub fn read_event(event: HostEvent, raw: &str, hook_env: &HookEnv) -> EventInfo {
    let v: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    let cwd = non_blank(v.get("cwd").and_then(Value::as_str));
    match event {
        HostEvent::Opencode => EventInfo::default(),
        HostEvent::ClaudeCode => EventInfo {
            workspace: cwd.or_else(|| non_blank(hook_env.claude_project_dir.as_deref())),
            skip: false,
        },
        HostEvent::Qwen => EventInfo {
            workspace: cwd.or_else(|| non_blank(hook_env.qwen_project_dir.as_deref())),
            skip: false,
        },
        HostEvent::Antigravity => {
            let skip = v
                .get("invocationNum")
                .and_then(Value::as_u64)
                .is_some_and(|n| n > 1);
            let first = non_blank(
                v.get("workspacePaths")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_str),
            );
            EventInfo {
                workspace: first.or(cwd),
                skip,
            }
        }
    }
}

/// The reminder's whole decision: the text to print, already wrapped, or `None`.
/// `workspace_arg` is `--workspace`; `raw_event` is standard input for the three hooks.
pub fn output(
    event: HostEvent,
    env: &Env,
    hook_env: &HookEnv,
    raw_event: &str,
    workspace_arg: Option<&str>,
) -> Option<String> {
    let info = read_event(event, raw_event, hook_env);
    if info.skip {
        return None;
    }
    let workspace = non_blank(workspace_arg).or(info.workspace);
    let outcome = read_outcome(event, env, workspace.as_deref());
    let l = line(outcome)?;
    Some(wrap(event, &l, workspace.as_deref()))
}

fn read_outcome(event: HostEvent, env: &Env, workspace: Option<&str>) -> Result<List, ReadFailure> {
    let start = start_dir(
        Entry::Reminder,
        env,
        None,
        None,
        workspace.map(Path::new),
        event.host(),
    )
    .map_err(|e| resolve_failure(&e))?;
    let store = locate(&start.path, env).map_err(|e| resolve_failure(&e))?;
    read_list(&store, &Tuning::default()).map_err(|e| read_failure(&e))
}
