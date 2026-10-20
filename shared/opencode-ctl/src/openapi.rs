//! OpenAPI operation table (v2.0.32 snapshot) and the escape-hatch gate.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::error::{ErrorCode, ToolError};

pub const SNAPSHOT_VERSION: &str = "2.0.32";

pub const MUTATION_ALLOW_LIST: [&str; 7] = [
    "session.compact",
    "session.switchAgent",
    "session.switchModel",
    "session.view",
    "session.inbox.cancel",
    "session.inbox.update",
    "session.form.cancel",
];

pub const GET_DENY_LIST: [&str; 13] = [
    "server.connect",
    "fs.*",
    "session.log",
    "event.subscribe",
    "shell.*",
    "pty.*",
    "persistentPty.*",
    "server.experimental.persistentPty.*",
    "credential.*",
    "config.*",
    "mcp.*",
    "integration.*",
    "experimental.session.export",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }

    fn parse(s: &str) -> Option<Method> {
        match s {
            "GET" => Some(Method::Get),
            "POST" => Some(Method::Post),
            "PUT" => Some(Method::Put),
            "PATCH" => Some(Method::Patch),
            "DELETE" => Some(Method::Delete),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamIn {
    Path,
    Query,
}

impl ParamIn {
    fn as_str(self) -> &'static str {
        match self {
            ParamIn::Path => "path",
            ParamIn::Query => "query",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDecl {
    pub name: String,
    pub location: ParamIn,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub id: String,
    pub method: Method,
    pub path: String,
    pub params: Vec<ParamDecl>,
    pub success_status: u16,
    pub json_response: bool,
    pub summary: String,
    pub body_required: Vec<String>,
}

impl Operation {
    pub fn mutating(&self) -> bool {
        self.method != Method::Get
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableSource {
    Live,
    Snapshot,
}

#[derive(Debug, Clone)]
pub struct OperationTable {
    pub source: TableSource,
    ops: BTreeMap<String, Operation>,
}

/// Resolve one level of `$ref` into `components.schemas`.
fn resolve_ref<'a>(schema: &'a Value, doc: &'a Value) -> &'a Value {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        if let Some(name) = r.strip_prefix("#/components/schemas/") {
            if let Some(target) = doc.pointer("/components/schemas").and_then(|s| s.get(name)) {
                return target;
            }
        }
    }
    schema
}

/// The `required` property names of an operation's JSON request body schema.
/// `op_schema` is the operation object (it holds `requestBody`).
pub fn body_required_of(op_schema: &Value, doc: &Value) -> Vec<String> {
    let Some(schema) = op_schema.pointer("/requestBody/content/application~1json/schema") else {
        return Vec::new();
    };
    resolve_ref(schema, doc)
        .get("required")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_operation(
    path: &str,
    method: Method,
    op: &Value,
    doc: &Value,
) -> Result<Operation, String> {
    let id = op
        .get("operationId")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{} {path}: no operationId", method.as_str()))?
        .to_string();
    let mut params = Vec::new();
    for p in op
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{id}: parameter without name"))?;
        let loc = p
            .get("in")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{id}: parameter {name} without location"))?;
        let location = match loc {
            "path" => ParamIn::Path,
            "query" => ParamIn::Query,
            _ => continue,
        };
        params.push(ParamDecl {
            name: name.to_string(),
            location,
        });
    }
    let responses = op
        .get("responses")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{id}: no responses"))?;
    let (status, resp) = responses
        .iter()
        .filter_map(|(k, v)| {
            k.parse::<u16>()
                .ok()
                .filter(|s| (200..300).contains(s))
                .map(|s| (s, v))
        })
        .min_by_key(|(s, _)| *s)
        .ok_or_else(|| format!("{id}: no 2xx response"))?;
    let json_response = status == 200 && resp.pointer("/content/application~1json").is_some();
    Ok(Operation {
        id,
        method,
        path: path.to_string(),
        params,
        success_status: status,
        json_response,
        summary: op
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        body_required: body_required_of(op, doc),
    })
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

impl OperationTable {
    pub fn from_openapi(doc: &Value, source: TableSource) -> Result<Self, String> {
        let paths = doc
            .get("paths")
            .and_then(Value::as_object)
            .ok_or("document has no paths")?;
        let mut ops = BTreeMap::new();
        for (path, item) in paths {
            let Some(item) = item.as_object() else {
                continue;
            };
            for (m, op) in item {
                let Some(method) = Method::parse(&m.to_ascii_uppercase()) else {
                    continue;
                };
                let op = parse_operation(path, method, op, doc)?;
                if ops.contains_key(&op.id) {
                    return Err(format!("duplicate operationId {}", op.id));
                }
                ops.insert(op.id.clone(), op);
            }
        }
        Ok(Self { source, ops })
    }

    pub fn snapshot() -> Self {
        let v: Value = serde_json::from_str(include_str!("openapi.table.json"))
            .expect("snapshot table is valid JSON");
        Self::from_json(&v, TableSource::Snapshot).expect("snapshot table is well formed")
    }

    fn from_json(v: &Value, source: TableSource) -> Result<Self, String> {
        let list = v
            .get("operations")
            .and_then(Value::as_array)
            .ok_or("no operations array")?;
        let mut ops = BTreeMap::new();
        for o in list {
            let s = |k: &str| {
                o.get(k)
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| format!("operation field {k} missing"))
            };
            let id = s("id")?;
            let method = Method::parse(&s("method")?).ok_or("bad method")?;
            let mut params = Vec::new();
            for p in o
                .get("params")
                .and_then(Value::as_array)
                .ok_or("no params")?
            {
                let name = p
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or("param name")?
                    .to_string();
                let location = match p.get("in").and_then(Value::as_str) {
                    Some("path") => ParamIn::Path,
                    Some("query") => ParamIn::Query,
                    _ => return Err("bad param location".into()),
                };
                params.push(ParamDecl { name, location });
            }
            let body_required = o
                .get("body_required")
                .and_then(Value::as_array)
                .ok_or("no body_required")?
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            let op = Operation {
                id: id.clone(),
                method,
                path: s("path")?,
                params,
                success_status: o
                    .get("success")
                    .and_then(Value::as_u64)
                    .ok_or("no success")? as u16,
                json_response: o.get("json").and_then(Value::as_bool).ok_or("no json")?,
                summary: s("summary")?,
                body_required,
            };
            ops.insert(id, op);
        }
        Ok(Self { source, ops })
    }

    pub fn get(&self, id: &str) -> Option<&Operation> {
        self.ops.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Operation> {
        self.ops.values()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// The `n` ids closest to `id` by Levenshtein distance, ties alphabetical.
    pub fn nearest(&self, id: &str, n: usize) -> Vec<String> {
        let mut scored: Vec<(usize, &String)> =
            self.ops.keys().map(|k| (levenshtein(id, k), k)).collect();
        scored.sort();
        scored.into_iter().take(n).map(|(_, k)| k.clone()).collect()
    }

    pub fn to_json(&self) -> Value {
        let ops: Vec<Value> = self
            .ops
            .values()
            .map(|o| {
                json!({
                    "id": o.id,
                    "method": o.method.as_str(),
                    "path": o.path,
                    "params": o.params.iter().map(|p| json!({"name": p.name, "in": p.location.as_str()})).collect::<Vec<_>>(),
                    "success": o.success_status,
                    "json": o.json_response,
                    "summary": o.summary,
                    "body_required": o.body_required,
                })
            })
            .collect();
        json!({ "version": SNAPSHOT_VERSION, "operations": ops })
    }
}

/// Exact match, or `prefix.*` where the trailing `*` matches the rest of the id.
pub fn glob_matches(pattern: &str, id: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => id.starts_with(prefix),
        None => pattern == id,
    }
}

pub struct GateInput<'a> {
    pub live: Option<&'a OperationTable>,
    pub snapshot: &'a OperationTable,
    pub id: &'a str,
    pub allow_mutation: bool,
    pub locked: bool,
    pub server_version: Option<&'a str>,
}

pub fn gate(input: GateInput) -> Result<(Operation, TableSource), ToolError> {
    let id = input.id;
    if id.is_empty() || id.contains('/') || id.chars().any(char::is_whitespace) {
        return Err(ToolError::new(
            ErrorCode::InvalidArgument,
            "operation ids only, not a method or path",
        )
        .hint("pass an operationId such as session.get"));
    }
    let in_snapshot = input.snapshot.get(id);
    let (op, source) = match input.live {
        Some(live) => match live.get(id) {
            Some(op) => {
                if in_snapshot.is_none() {
                    return Err(ToolError::new(
                        ErrorCode::BlockedOperation,
                        format!("operation {id} is unknown to the bundled snapshot"),
                    )
                    .details(json!({"reason": "unknown_to_snapshot"})));
                }
                (op, TableSource::Live)
            }
            None if in_snapshot.is_some() => {
                return Err(ToolError::new(
                    ErrorCode::UnknownOperation,
                    format!("unknown operation {id}"),
                )
                .hint(&format!(
                    "not in the live document (server {})",
                    input.server_version.unwrap_or("unknown")
                )));
            }
            None => return Err(unknown(live, id)),
        },
        None => match in_snapshot {
            Some(op) => (op, TableSource::Snapshot),
            None => return Err(unknown(input.snapshot, id)),
        },
    };
    if !op.mutating() {
        if GET_DENY_LIST.iter().any(|p| glob_matches(p, id)) {
            return Err(blocked(id, "read denied by the escape-hatch deny-list"));
        }
    } else {
        if source == TableSource::Snapshot {
            return Err(ToolError::new(
                ErrorCode::ServiceDown,
                "live document needed before a mutation",
            )
            .hint("start the OpenCode service and retry"));
        }
        if !MUTATION_ALLOW_LIST.contains(&id) {
            return Err(blocked(id, "mutation not on the escape-hatch allow-list"));
        }
        if id == "session.switchAgent" && input.locked {
            return Err(ToolError::new(
                ErrorCode::PermissionsLocked,
                "permissions are locked; switching agent is refused",
            ));
        }
        if !input.allow_mutation {
            return Err(ToolError::new(
                ErrorCode::MutationBlocked,
                format!("{id} mutates state; pass allow_mutation to run it"),
            ));
        }
    }
    if !(op.success_status == 204 || (op.success_status == 200 && op.json_response)) {
        return Err(ToolError::new(
            ErrorCode::NonJsonResponse,
            format!("{id} does not return JSON"),
        ));
    }
    Ok((op.clone(), source))
}

fn unknown(table: &OperationTable, id: &str) -> ToolError {
    ToolError::new(
        ErrorCode::UnknownOperation,
        format!("unknown operation {id}"),
    )
    .hint(&format!("nearest: {}", table.nearest(id, 3).join(", ")))
}

fn blocked(id: &str, why: &str) -> ToolError {
    ToolError::new(ErrorCode::BlockedOperation, format!("{id}: {why}"))
}

fn path_segment_ok(v: &str) -> bool {
    v != "."
        && v != ".."
        && !v.chars().any(|c| {
            c.is_control() || c.is_whitespace() || matches!(c, '/' | '\\' | '?' | '#' | '%')
        })
}

pub fn validate_params(op: &Operation, params: &[(String, String)]) -> Result<(), ToolError> {
    let bad = |m: String| ToolError::new(ErrorCode::InvalidArgument, m);
    for (name, value) in params {
        if name.is_empty() || value.is_empty() {
            return Err(bad("param name and value must be non-empty".into()));
        }
        if value.contains('\0') || value.contains('\n') {
            return Err(bad(format!(
                "param {name}: value must not contain NUL or newline"
            )));
        }
        if name.contains('\0') || name.contains('\n') {
            return Err(bad("param name must not contain NUL or newline".into()));
        }
        let base = name.split('[').next().unwrap_or(name);
        if base.is_empty() {
            return Err(bad("param name must not be empty before `[`".into()));
        }
        if base.eq_ignore_ascii_case("header")
            || base.eq_ignore_ascii_case("-H")
            || name.starts_with('-')
        {
            return Err(bad("headers cannot be set".into()));
        }
        let Some(decl) = op.params.iter().find(|p| p.name == base) else {
            return Err(
                bad(format!("param {base} is not declared by {}", op.id)).hint(&format!(
                    "declared: {}",
                    op.params
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            );
        };
        // A path value is one URL segment. The CLI builds the URL with `new URL(path, base)`, which collapses a
        // literal `.`/`..` segment, so `inboxID=..` would reach a different (blocked) route.
        if decl.location == ParamIn::Path && !path_segment_ok(value) {
            return Err(bad(format!(
                "param {base}: a path value must be a single plain segment (not `.`/`..`, and without `/`, `\\`, `?`, `#`, `%` or whitespace)"
            )));
        }
    }
    Ok(())
}

pub fn validate_session_id(id: &str) -> Result<(), ToolError> {
    let ok = id.len() == 30
        && id.starts_with("ses_")
        && id[4..].bytes().all(|b| b.is_ascii_alphanumeric());
    if ok {
        Ok(())
    } else {
        Err(ToolError::new(
            ErrorCode::SessionNotFound,
            format!("not a session id: {id}"),
        )
        .hint("session ids look like ses_ followed by 26 letters or digits"))
    }
}

pub fn validate_pair_id(id: &str) -> Result<(), ToolError> {
    if id.len() == 12 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        Ok(())
    } else {
        Err(
            ToolError::new(ErrorCode::InvalidArgument, format!("not a pair id: {id}"))
                .hint("pair ids are 12 lowercase hex characters"),
        )
    }
}
