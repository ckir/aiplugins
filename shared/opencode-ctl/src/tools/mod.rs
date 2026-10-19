//! Tool argument structs (all twenty, final schemas) and tool bodies.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::clock::Clock;
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::transport::Transport;

pub mod service;
pub mod session;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServiceStatusArgs {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServiceStartArgs {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConfirmArgs {
    /// Must be true; the flag is friction, not a guard.
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionListArgs {
    /// Workspace directory filter (default: the paired directory, else the host working directory).
    pub directory: Option<String>,
    /// Drop the directory filter.
    pub all: Option<bool>,
    pub search: Option<String>,
    /// 1..200, default 20.
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionCreateArgs {
    pub title: Option<String>,
    pub directory: Option<String>,
    /// Refused with PERMISSIONS_LOCKED when permissions are locked.
    pub agent: Option<String>,
    /// `provider/model[#variant]`.
    pub model: Option<String>,
    /// Permission rules `{action, resource, effect}`; refused with PERMISSIONS_LOCKED when locked.
    pub permissions: Option<Vec<Value>>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionGetArgs {
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionPromptArgs {
    pub session_id: Option<String>,
    pub text: String,
    /// `steer` (default) or `queue`.
    pub delivery: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionWaitArgs {
    pub session_id: Option<String>,
    /// The message id returned by session_prompt.
    pub after: Option<String>,
    pub timeout_s: Option<u64>,
    pub poll_s: Option<u64>,
    pub hard: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionMessagesArgs {
    pub session_id: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    /// `asc` or `desc` (default `desc`); ignored when `cursor` is given.
    pub order: Option<String>,
    pub r#type: Option<String>,
    pub reasoning: Option<bool>,
    /// `none`, `summary` (default) or `full`.
    pub tools: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionChangesArgs {
    pub session_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    /// Workspace-relative exact paths.
    pub paths: Option<Vec<String>>,
    pub patches: Option<bool>,
    pub context: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorkspaceStatusArgs {
    pub directory: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionInterruptArgs {
    pub session_id: Option<String>,
    /// Let pending steering input continue (default false).
    pub resume: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionDeleteArgs {
    /// Required: never defaults to the paired session.
    pub session_id: String,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PermissionReplyArgs {
    pub session_id: Option<String>,
    pub request_id: String,
    /// `once`, `always` or `reject`.
    pub decision: String,
    pub message: Option<String>,
    pub confirm: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FormReplyArgs {
    pub session_id: Option<String>,
    pub form_id: String,
    pub answer: Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SavedPermissionsArgs {
    pub project_id: Option<String>,
    pub remove_id: Option<String>,
    pub confirm: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpencodeOperationsArgs {
    pub filter: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpencodeApiArgs {
    /// An operationId such as `session.get`.
    pub operation: String,
    pub params: Option<Value>,
    pub body: Option<Value>,
    pub allow_mutation: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PairStatusArgs {}

fn not_implemented(tool: &str) -> ToolResult {
    Err(ToolError::new(
        ErrorCode::CliError,
        format!("{tool}: not implemented"),
    ))
}

/// Tools whose bodies land in later tasks; each moves out of here when its task lands.
impl<T: Transport, C: Clock> Engine<T, C> {
    pub async fn session_prompt(&self, _a: SessionPromptArgs) -> ToolResult {
        not_implemented("session_prompt")
    }
    pub async fn session_wait(&self, _a: SessionWaitArgs) -> ToolResult {
        not_implemented("session_wait")
    }
    pub async fn session_messages(&self, _a: SessionMessagesArgs) -> ToolResult {
        not_implemented("session_messages")
    }
    pub async fn session_changes(&self, _a: SessionChangesArgs) -> ToolResult {
        not_implemented("session_changes")
    }
    pub async fn workspace_status(&self, _a: WorkspaceStatusArgs) -> ToolResult {
        not_implemented("workspace_status")
    }
    pub async fn permission_reply(&self, _a: PermissionReplyArgs) -> ToolResult {
        not_implemented("permission_reply")
    }
    pub async fn form_reply(&self, _a: FormReplyArgs) -> ToolResult {
        not_implemented("form_reply")
    }
    pub async fn saved_permissions(&self, _a: SavedPermissionsArgs) -> ToolResult {
        not_implemented("saved_permissions")
    }
    pub async fn opencode_operations(&self, _a: OpencodeOperationsArgs) -> ToolResult {
        not_implemented("opencode_operations")
    }
    pub async fn opencode_api(&self, _a: OpencodeApiArgs) -> ToolResult {
        not_implemented("opencode_api")
    }
    pub async fn pair_status(&self, _a: PairStatusArgs) -> ToolResult {
        not_implemented("pair_status")
    }
}
