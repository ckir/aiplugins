//! `permission_reply`, `form_reply` and `saved_permissions`.

use serde_json::{json, Map, Value};

use crate::clock::Clock;
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::transport::Transport;

use super::{FormReplyArgs, PermissionReplyArgs, SavedPermissionsArgs};

fn non_empty(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        _ => false,
    }
}

impl<T: Transport, C: Clock> Engine<T, C> {
    pub async fn permission_reply(&self, a: PermissionReplyArgs) -> ToolResult {
        let id = self.resolve_session(a.session_id.as_deref())?;
        let decision = a.decision.as_str();
        if !matches!(decision, "once" | "always" | "reject") {
            return Err(ToolError::new(
                ErrorCode::InvalidArgument,
                format!("decision '{decision}' is not once, always or reject"),
            ));
        }
        if decision == "always" {
            if self.settings.permissions_locked {
                return Err(ToolError::new(
                    ErrorCode::PermissionsLocked,
                    "permissions are locked: a saved (always) rule is refused",
                )
                .hint("reply once, or ask the owner to unlock permissions"));
            }
            if a.confirm != Some(true) {
                return Err(ToolError::new(
                    ErrorCode::ConfirmRequired,
                    "decision always saves a standing rule and needs confirm: true",
                ));
            }
        }
        let (list, _, _) = self
            .call_json(
                "session.permission.list",
                vec![("sessionID".to_string(), id.clone())],
                None,
            )
            .await?;
        let request = list
            .as_array()
            .and_then(|l| {
                l.iter()
                    .find(|p| p.get("id").and_then(Value::as_str) == Some(a.request_id.as_str()))
            })
            .ok_or_else(|| {
                ToolError::new(
                    ErrorCode::Conflict,
                    format!("permission request {} is no longer pending", a.request_id),
                )
            })?;
        let saved = decision == "always" && non_empty(request.get("save"));
        let mut body = Map::new();
        body.insert("decision".into(), json!(decision));
        if let Some(m) = a.message {
            body.insert("message".into(), json!(m));
        }
        self.call_json(
            "session.permission.reply",
            vec![
                ("sessionID".to_string(), id),
                ("requestID".to_string(), a.request_id),
            ],
            Some(Value::Object(body)),
        )
        .await?;
        Ok(json!({"replied": true, "saved": saved}))
    }

    pub async fn form_reply(&self, a: FormReplyArgs) -> ToolResult {
        let id = self.resolve_session(a.session_id.as_deref())?;
        self.call_json(
            "session.form.reply",
            vec![
                ("sessionID".to_string(), id),
                ("formID".to_string(), a.form_id),
            ],
            Some(json!({"answer": a.answer})),
        )
        .await?;
        Ok(json!({"replied": true}))
    }

    pub async fn saved_permissions(&self, a: SavedPermissionsArgs) -> ToolResult {
        if a.remove_id.is_some() && a.confirm != Some(true) {
            return Err(ToolError::new(
                ErrorCode::ConfirmRequired,
                "removing a saved permission rule needs confirm: true",
            ));
        }
        let mut params = Vec::new();
        if let Some(p) = a.project_id {
            params.push(("projectID".to_string(), p));
        }
        let (data, _, _) = self
            .call_json("permission.saved.list", params, None)
            .await?;
        let mut rules = data.as_array().cloned().unwrap_or_default();
        if let Some(rid) = a.remove_id {
            self.call_json(
                "permission.saved.remove",
                vec![("id".to_string(), rid.clone())],
                None,
            )
            .await?;
            rules.retain(|r| r.get("id").and_then(Value::as_str) != Some(rid.as_str()));
        }
        Ok(json!({"rules": rules}))
    }
}
