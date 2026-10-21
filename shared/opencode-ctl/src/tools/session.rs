//! Basic `session_*` tools: list, create, get, interrupt, delete.

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::clock::Clock;
use crate::compact::{compact_form_field, compact_permission};
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::host::Host;
use crate::settings::PermissionPolicy;
use crate::transport::Transport;

use super::{
    SessionCreateArgs, SessionDeleteArgs, SessionGetArgs, SessionInterruptArgs, SessionListArgs,
};

fn field(v: &Value, ptr: &str) -> Value {
    v.pointer(ptr).cloned().unwrap_or(Value::Null)
}

/// `provider/model[#variant]` -> `Model.Ref`; the provider ends at the first `/`, the variant starts at the first `#`.
fn parse_model(raw: &str) -> Result<Value, ToolError> {
    let bad = || {
        ToolError::new(
            ErrorCode::InvalidArgument,
            format!("model '{raw}' is not provider/model[#variant]"),
        )
    };
    let (provider, rest) = raw.split_once('/').ok_or_else(bad)?;
    let (id, variant) = match rest.split_once('#') {
        Some((id, v)) => (id, Some(v)),
        None => (rest, None),
    };
    if provider.is_empty() || id.is_empty() || variant == Some("") {
        return Err(bad());
    }
    let mut m = Map::new();
    m.insert("providerID".into(), json!(provider));
    m.insert("id".into(), json!(id));
    if let Some(v) = variant {
        m.insert("variant".into(), json!(v));
    }
    Ok(Value::Object(m))
}

fn compact_form(raw: &Value) -> Value {
    let mut o = Map::new();
    for k in ["id", "title", "metadata"] {
        if let Some(v) = raw.get(k) {
            o.insert(k.to_string(), v.clone());
        }
    }
    if let Some(fields) = raw.get("fields").and_then(Value::as_array) {
        o.insert(
            "fields".into(),
            Value::Array(fields.iter().map(compact_form_field).collect()),
        );
    }
    Value::Object(o)
}

impl<T: Transport, C: Clock> Engine<T, C> {
    pub async fn session_list(&self, a: SessionListArgs) -> ToolResult {
        let mut params = Vec::new();
        if !a.all.unwrap_or(false) {
            let dir = self.resolve_directory(a.directory.as_deref().map(Path::new));
            params.push(("directory".to_string(), dir.to_string_lossy().to_string()));
        }
        if let Some(s) = a.search {
            params.push(("search".into(), s));
        }
        params.push((
            "limit".into(),
            a.limit.unwrap_or(20).clamp(1, 200).to_string(),
        ));
        if let Some(c) = a.cursor {
            params.push(("cursor".into(), c));
        }
        let (data, cursor, _) = self.call_json("session.list", params, None).await?;
        let sessions: Vec<Value> = data
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|s| {
                        json!({
                            "id": field(s, "/id"),
                            "title": field(s, "/title"),
                            "directory": field(s, "/location/directory"),
                            "outcome": field(s, "/outcome"),
                            "updated": field(s, "/time/updated"),
                            "idle": field(s, "/time/idle"),
                            "tokens": field(s, "/tokens"),
                            "cost": field(s, "/cost"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let next = cursor.as_ref().map_or(Value::Null, |c| field(c, "/next"));
        Ok(json!({"sessions": sessions, "next_cursor": next}))
    }

    pub async fn session_create(&self, a: SessionCreateArgs) -> ToolResult {
        let locked = self.settings.permissions_locked;
        if locked && a.agent.is_some() {
            return Err(ToolError::new(
                ErrorCode::PermissionsLocked,
                "permissions are locked: a per-call agent is refused",
            ));
        }
        if locked && a.permissions.is_some() {
            return Err(ToolError::new(
                ErrorCode::PermissionsLocked,
                "permissions are locked: per-call permissions are refused",
            ));
        }
        let directory = self.resolve_directory(a.directory.as_deref().map(Path::new));
        let mut body = Map::new();
        if let Some(t) = a.title {
            body.insert("title".into(), json!(t));
        }
        if let Some(p) = a.parent_id {
            body.insert("parentID".into(), json!(p));
        }
        if let Some(agent) = a.agent.or_else(|| self.settings.default_agent.clone()) {
            body.insert("agent".into(), json!(agent));
        }
        if let Some(model) = a.model.or_else(|| self.settings.default_model.clone()) {
            body.insert("model".into(), parse_model(&model)?);
        }
        body.insert(
            "location".into(),
            json!({"directory": directory.to_string_lossy()}),
        );
        let effect = match self.settings.permission_policy {
            PermissionPolicy::Inherit => None,
            PermissionPolicy::Allow => Some("allow"),
            PermissionPolicy::Ask => Some("ask"),
            PermissionPolicy::Deny => Some("deny"),
        };
        match (a.permissions, effect) {
            (Some(rules), _) => {
                body.insert("permissions".into(), Value::Array(rules));
            }
            (None, Some(e)) => {
                body.insert(
                    "permissions".into(),
                    json!([{"action": "*", "resource": "*", "effect": e}]),
                );
            }
            (None, None) => {}
        }
        let sent = Value::Object(body);
        let (data, _, _) = self
            .call_json("session.create", Vec::new(), Some(sent.clone()))
            .await?;
        let pick = |ptr: &str, fallback: &str| {
            data.pointer(ptr)
                .cloned()
                .or_else(|| sent.pointer(fallback).cloned())
                .unwrap_or(Value::Null)
        };
        if self.host == Host::Opencode {
            if let Some(new_id) = data.get("id").and_then(Value::as_str) {
                self.record_created(&directory, new_id).await?;
            }
        }
        Ok(json!({
            "session_id": field(&data, "/id"),
            "directory": pick("/location/directory", "/location/directory"),
            "agent": pick("/agent", "/agent"),
            "model": pick("/model", "/model"),
        }))
    }

    pub async fn session_get(&self, a: SessionGetArgs) -> ToolResult {
        let id = self.resolve_session(a.session_id.as_deref())?;
        let p = || vec![("sessionID".to_string(), id.clone())];
        let (s, _, _) = self.call_json("session.get", p(), None).await?;
        let (perms, _, _) = self.call_json("session.permission.list", p(), None).await?;
        let (forms, _, _) = self.call_json("session.form.list", p(), None).await?;
        let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
        let mut out = Map::new();
        out.insert("id".into(), field(&s, "/id"));
        out.insert("title".into(), field(&s, "/title"));
        out.insert("directory".into(), field(&s, "/location/directory"));
        out.insert("outcome".into(), field(&s, "/outcome"));
        for k in ["agent", "model"] {
            if let Some(v) = s.get(k) {
                out.insert(k.into(), v.clone());
            }
        }
        out.insert("tokens".into(), field(&s, "/tokens"));
        out.insert("cost".into(), field(&s, "/cost"));
        out.insert("time".into(), field(&s, "/time"));
        out.insert(
            "pending_permissions".into(),
            Value::Array(list(&perms).iter().map(compact_permission).collect()),
        );
        out.insert(
            "pending_forms".into(),
            Value::Array(list(&forms).iter().map(compact_form).collect()),
        );
        Ok(Value::Object(out))
    }

    pub async fn session_interrupt(&self, a: SessionInterruptArgs) -> ToolResult {
        let id = self.resolve_session(a.session_id.as_deref())?;
        self.guard_created(&id)?;
        let params = vec![
            ("sessionID".to_string(), id),
            ("resume".to_string(), a.resume.unwrap_or(false).to_string()),
        ];
        let (data, _, _) = self.call_json("session.interrupt", params, None).await?;
        Ok(
            json!({"interrupted": data.get("interrupted").and_then(Value::as_bool).unwrap_or(false)}),
        )
    }

    pub async fn session_delete(&self, a: SessionDeleteArgs) -> ToolResult {
        if !a.confirm {
            return Err(ToolError::new(
                ErrorCode::ConfirmRequired,
                "session_delete needs confirm: true",
            ));
        }
        let id = self.resolve_session(Some(&a.session_id))?;
        self.guard_created(&id)?;
        self.call_json("session.remove", vec![("sessionID".to_string(), id)], None)
            .await?;
        Ok(json!({"deleted": true}))
    }
}
