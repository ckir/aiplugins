//! `session_changes` and `workspace_status`.

use std::path::Path;

use serde_json::{json, Map, Value};

use crate::clock::Clock;
use crate::engine::{DiffKey, Engine};
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::transport::Transport;

use super::{SessionChangesArgs, WorkspaceStatusArgs};

/// Bytes kept free under the cap for the markers `finish` may add.
const RESERVE: usize = 64;
/// `,"patch":` plus the quotes and separator around a patch value.
const PATCH_OVERHEAD: usize = 12;
/// `,"patch_truncated":true`
const FLAG_OVERHEAD: usize = 23;

fn serialized_len(v: &Value) -> usize {
    v.to_string().len()
}

fn escaped_len(c: char) -> usize {
    match c {
        '"' | '\\' | '\n' | '\r' | '\t' | '\u{8}' | '\u{c}' => 2,
        c if (c as u32) < 0x20 => 6,
        c => c.len_utf8(),
    }
}

/// The longest prefix of `s` whose JSON-escaped form is at most `max` bytes.
fn cut_escaped(s: &str, max: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for c in s.chars() {
        let n = escaped_len(c);
        if used + n > max {
            break;
        }
        used += n;
        out.push(c);
    }
    out
}

fn range_error(e: ToolError) -> ToolError {
    let tag = e
        .details
        .as_ref()
        .and_then(|d| d.get("tag"))
        .and_then(Value::as_str);
    if matches!(tag, Some("MessageNotFoundError" | "InvalidRequestError")) {
        ToolError {
            code: ErrorCode::InvalidRequest,
            ..e
        }
    } else {
        e
    }
}

fn meta_of(file: &Value) -> Map<String, Value> {
    let mut m = Map::new();
    for k in ["file", "status", "additions", "deletions"] {
        m.insert(k.to_string(), file.get(k).cloned().unwrap_or(Value::Null));
    }
    m
}

/// Fit the diff under `max` bytes: patches go first (`omitted_patches`), then file entries
/// (their names move to `omitted_files`); one patch too big for the whole budget is cut.
fn pack(head: &Map<String, Value>, files: &[Value], patches: bool, max: usize) -> Value {
    let budget = max.saturating_sub(RESERVE);
    let metas: Vec<Map<String, Value>> = files.iter().map(meta_of).collect();
    let meta_cost: Vec<usize> = metas
        .iter()
        .map(|m| serialized_len(&Value::Object(m.clone())) + 1)
        .collect();
    let name_cost: Vec<usize> = metas
        .iter()
        .map(|m| serialized_len(&m["file"]) + 1)
        .collect();
    let mut skeleton = head.clone();
    skeleton.insert("files".into(), json!([]));
    skeleton.insert("omitted_patches".into(), json!(files.len()));
    skeleton.insert("omitted_files".into(), json!([]));
    let base = serialized_len(&Value::Object(skeleton));
    let mut keep = files.len();
    let mut total = base + meta_cost.iter().sum::<usize>();
    while keep > 0 && total > budget {
        keep -= 1;
        total = total - meta_cost[keep] + name_cost[keep];
    }
    let mut listed: Vec<Value> = Vec::new();
    let mut with_patch = 0usize;
    let full = budget.saturating_sub(total);
    let mut remaining = full;
    for (i, m) in metas.iter().take(keep).enumerate() {
        let mut entry = m.clone();
        if patches {
            if let Some(p) = files[i].get("patch").and_then(Value::as_str) {
                let need = serialized_len(&Value::String(p.to_string())) + PATCH_OVERHEAD;
                if need <= remaining {
                    entry.insert("patch".into(), json!(p));
                    remaining -= need;
                    with_patch += 1;
                } else if need > full {
                    // Cannot fit even alone: cut it to what is left.
                    let room = remaining.saturating_sub(PATCH_OVERHEAD + FLAG_OVERHEAD);
                    if room > 0 {
                        entry.insert("patch".into(), json!(cut_escaped(p, room)));
                        entry.insert("patch_truncated".into(), json!(true));
                        with_patch += 1;
                        remaining = 0;
                    }
                }
            }
        }
        listed.push(Value::Object(entry));
    }
    let omitted_files: Vec<Value> = metas.iter().skip(keep).map(|m| m["file"].clone()).collect();
    let omitted_patches = if patches {
        keep - with_patch
    } else {
        files.len()
    };
    let mut out = head.clone();
    out.insert("files".into(), Value::Array(listed));
    out.insert("omitted_patches".into(), json!(omitted_patches));
    out.insert("omitted_files".into(), Value::Array(omitted_files));
    Value::Object(out)
}

impl<T: Transport, C: Clock> Engine<T, C> {
    async fn first_user_message(&self, id: &str, order: &str) -> Result<Option<String>, ToolError> {
        let params = vec![
            ("sessionID".to_string(), id.to_string()),
            ("type".to_string(), "user".to_string()),
            ("limit".to_string(), "1".to_string()),
            ("order".to_string(), order.to_string()),
        ];
        let (data, _, _) = self.call_json("session.message.list", params, None).await?;
        Ok(data
            .as_array()
            .and_then(|a| a.first())
            .and_then(|m| m.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    async fn session_directory(&self, id: &str) -> Result<Value, ToolError> {
        if self.env.session.as_deref() == Some(id) {
            if let Some(d) = &self.env.directory {
                return Ok(json!(d.to_string_lossy()));
            }
        }
        let (s, _, _) = self
            .call_json(
                "session.get",
                vec![("sessionID".to_string(), id.to_string())],
                None,
            )
            .await?;
        Ok(s.pointer("/location/directory")
            .cloned()
            .unwrap_or(Value::Null))
    }

    pub async fn session_changes(&self, a: SessionChangesArgs) -> ToolResult {
        let id = self.resolve_session(a.session_id.as_deref())?;
        let directory = self.session_directory(&id).await?;
        let from = match a.from {
            Some(f) => Some(f),
            None => self.first_user_message(&id, "asc").await?,
        };
        let to = match a.to {
            Some(t) => Some(t),
            None => self.first_user_message(&id, "desc").await?,
        };
        let mut head = Map::new();
        head.insert("directory".into(), directory);
        head.insert("from".into(), json!(from));
        head.insert("to".into(), json!(to));
        let (Some(from), Some(to)) = (from, to) else {
            // A session without a user message has no turns to diff.
            return Ok(pack(&head, &[], true, self.settings.output_max_bytes));
        };
        let key: DiffKey = (id.clone(), from.clone(), to.clone(), a.context);
        let diff = match self.diff_cache_get(&key) {
            Some(d) => d,
            None => {
                let mut params = vec![
                    ("sessionID".to_string(), id),
                    ("from".to_string(), from),
                    ("to".to_string(), to),
                ];
                if let Some(c) = a.context {
                    params.push(("context".to_string(), c.to_string()));
                }
                let (d, _, _) = self
                    .call_json("session.diff", params, None)
                    .await
                    .map_err(range_error)?;
                self.diff_cache_put(key, &d);
                d
            }
        };
        let mut files = diff.as_array().cloned().unwrap_or_default();
        if let Some(paths) = &a.paths {
            files.retain(|f| {
                f.get("file")
                    .and_then(Value::as_str)
                    .is_some_and(|n| paths.iter().any(|p| p == n))
            });
        }
        Ok(pack(
            &head,
            &files,
            a.patches.unwrap_or(true),
            self.settings.output_max_bytes,
        ))
    }

    pub async fn workspace_status(&self, a: WorkspaceStatusArgs) -> ToolResult {
        let dir = self
            .resolve_directory(a.directory.as_deref().map(Path::new))
            .to_string_lossy()
            .to_string();
        let params = || vec![("location[directory]".to_string(), dir.clone())];
        let (info, _, _) = self.call_json("vcs.get", params(), None).await?;
        let provider = info.get("provider").filter(|p| !p.is_null()).cloned();
        let Some(provider) = provider else {
            return Ok(json!({"directory": dir, "vcs": null}));
        };
        let (status, _, meta) = self.call_json("vcs.status", params(), None).await?;
        let entries: Vec<Value> = status
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| {
                        json!({
                            "path": e.get("path").cloned().unwrap_or(Value::Null),
                            "kind": e.get("kind").cloned().unwrap_or(Value::Null),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut out = Map::new();
        out.insert("directory".into(), json!(dir));
        out.insert("vcs".into(), provider);
        out.insert("status".into(), Value::Array(entries));
        if let Some(m) = meta {
            out.insert("meta".into(), m);
        }
        Ok(Value::Object(out))
    }
}
