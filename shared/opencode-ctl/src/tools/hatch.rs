//! `opencode_operations` and the gated `opencode_api` escape hatch.

use serde_json::{json, Map, Value};

use crate::clock::Clock;
use crate::compact::depth_cap;
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::host::Host;
use crate::openapi::{
    gate, validate_params, validate_session_id, GateInput, OperationTable, TableSource,
    SNAPSHOT_VERSION,
};
use crate::transport::{Params, Transport};

use super::{OpencodeApiArgs, OpencodeOperationsArgs};

const DEPTH: usize = 6;

fn source_label(source: TableSource) -> String {
    match source {
        TableSource::Live => "live".to_string(),
        TableSource::Snapshot => format!("snapshot(v{SNAPSHOT_VERSION})"),
    }
}

/// `params` must be an object of string values; anything else is `INVALID_ARGUMENT`.
fn params_of(raw: Option<Value>) -> Result<Params, ToolError> {
    let bad = |m: String| ToolError::new(ErrorCode::InvalidArgument, m);
    match raw {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(map)) => map
            .into_iter()
            .map(|(k, v)| match v {
                Value::String(s) => Ok((k, s)),
                _ => Err(bad(format!("param {k} must be a string"))),
            })
            .collect(),
        Some(_) => Err(bad("params must be an object of strings".to_string())),
    }
}

impl<T: Transport, C: Clock> Engine<T, C> {
    /// The warning for a snapshot in use against a server of another version, else the table's own note.
    async fn hatch_warning(
        &self,
        source: TableSource,
        table_warning: Option<String>,
    ) -> Option<String> {
        if source == TableSource::Snapshot {
            if let Some(v) = self.observed_server_version().await {
                if v != SNAPSHOT_VERSION {
                    return Some(format!(
                        "snapshot v{SNAPSHOT_VERSION} differs from server {v}"
                    ));
                }
            }
        }
        table_warning
    }

    pub async fn opencode_operations(&self, a: OpencodeOperationsArgs) -> ToolResult {
        let (mut table, mut source, mut warning) = self.operation_table().await;
        if source == TableSource::Snapshot {
            // Learn the server version for the warning; a failure only means no comparison.
            if let Ok((info, _, _)) = self.call_json("server.info", Vec::new(), None).await {
                self.invalidate_if_changed(&info).await;
                (table, source, warning) = self.operation_table().await;
            }
        }
        let warning = self.hatch_warning(source, warning).await;
        let needle = a.filter.as_deref().map(str::to_lowercase);
        let operations: Vec<Value> = table
            .iter()
            .filter(|o| match &needle {
                Some(n) => o.id.to_lowercase().contains(n) || o.summary.to_lowercase().contains(n),
                None => true,
            })
            .map(|o| {
                json!({
                    "id": o.id,
                    "method": o.method.as_str(),
                    "path": o.path,
                    "summary": o.summary,
                    "mutating": o.mutating(),
                })
            })
            .collect();
        let mut out = Map::new();
        out.insert("source".into(), json!(source_label(source)));
        if let Some(w) = warning {
            out.insert("warning".into(), json!(w));
        }
        out.insert("operations".into(), Value::Array(operations));
        Ok(Value::Object(out))
    }

    pub async fn opencode_api(&self, a: OpencodeApiArgs) -> ToolResult {
        let params = params_of(a.params)?;
        let id = a.operation.as_str();
        let snapshot = OperationTable::snapshot();

        let (mut table, mut source, mut warning) = self.operation_table().await;
        let known_mutating = table
            .get(id)
            .or_else(|| snapshot.get(id))
            .is_some_and(|o| o.mutating());
        if known_mutating || source == TableSource::Snapshot {
            // The one transport call allowed before the gate: re-read server.info so a changed
            // pid or version drops the cached live table before a mutation is judged against it.
            match self.call_json("server.info", Vec::new(), None).await {
                Ok((info, _, _)) => {
                    self.invalidate_if_changed(&info).await;
                    (table, source, warning) = self.operation_table().await;
                }
                Err(e) if known_mutating => return Err(e),
                Err(_) => {}
            }
        }
        let warning = self.hatch_warning(source, warning).await;

        let server_version = self.observed_server_version().await;
        let live = (source == TableSource::Live).then_some(&table);
        let (op, source) = gate(GateInput {
            live,
            snapshot: &snapshot,
            id,
            allow_mutation: a.allow_mutation.unwrap_or(false),
            locked: self.settings.permissions_locked,
            server_version: server_version.as_deref(),
        })?;
        validate_params(&op, &params)?;
        for (name, value) in &params {
            if name == "sessionID" {
                validate_session_id(value)?;
            }
        }
        if self.host == Host::Opencode && op.mutating() {
            for (name, value) in &params {
                if name == "sessionID" {
                    self.guard_created(value)?;
                }
            }
        }

        let (data, cursor, meta) = self.call_json(id, params, a.body).await?;
        let data = if op.success_status == 204 {
            json!({ "ok": true })
        } else {
            data
        };
        let meta = match (meta, cursor) {
            (Some(Value::Object(mut m)), Some(c)) => {
                m.insert("cursor".into(), c);
                Some(Value::Object(m))
            }
            (None, Some(c)) => Some(json!({ "cursor": c })),
            (m, _) => m,
        };

        let mut out = Map::new();
        out.insert("source".into(), json!(source_label(source)));
        if let Some(w) = warning {
            out.insert("warning".into(), json!(w));
        }
        if let Some(m) = meta {
            out.insert("meta".into(), depth_cap(m, DEPTH));
        }
        out.insert("data".into(), depth_cap(data, DEPTH));
        Ok(Value::Object(out))
    }
}
