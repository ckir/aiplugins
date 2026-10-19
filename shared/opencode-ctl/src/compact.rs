//! Output compaction, byte caps and redaction.

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::error::{ErrorCode, ToolError};

/// Bytes reserved inside the budget for the `truncated` / `original_bytes` keys (and a wrapper for a non-object root).
const MARKER_RESERVE: usize = 64;
/// A string at or below this length is never shortened (the marker would not save bytes).
const MIN_SHORTEN_LEN: usize = 64;
/// Extra bytes removed past the excess when shortening a string, so the `...[+N bytes]` tail always nets a saving.
const SHORTEN_SLACK: usize = 40;

#[derive(Debug, Clone)]
pub struct Cap {
    pub max_bytes: usize,
    pub exempt_arrays: &'static [&'static str],
    pub exempt_entry_max: usize,
    pub exempt_len_max: usize,
}

fn scalar_len(v: &Value) -> usize {
    serde_json::to_string(v).map(|s| s.len()).unwrap_or(0)
}

/// Serialised length of `v`, counting an array under an exempt key as `[]`.
fn size(v: &Value, skip: &[&str]) -> usize {
    match v {
        Value::Array(a) => {
            2 + a.iter().map(|e| size(e, skip)).sum::<usize>() + a.len().saturating_sub(1)
        }
        Value::Object(o) => {
            2 + o
                .iter()
                .map(|(k, e)| {
                    let key = scalar_len(&Value::String(k.clone())) + 1;
                    if e.is_array() && skip.contains(&k.as_str()) {
                        key + 2
                    } else {
                        key + size(e, skip)
                    }
                })
                .sum::<usize>()
                + o.len().saturating_sub(1)
        }
        other => scalar_len(other),
    }
}

/// Visit every child value that is not an exempt array.
fn children<'a>(v: &'a Value, skip: &[&str], out: &mut Vec<&'a Value>) {
    match v {
        Value::Array(a) => out.extend(a.iter()),
        Value::Object(o) => out.extend(
            o.iter()
                .filter(|(k, e)| !(e.is_array() && skip.contains(&k.as_str())))
                .map(|(_, e)| e),
        ),
        _ => {}
    }
}

fn largest(v: &Value, skip: &[&str], pick: &dyn Fn(&Value) -> Option<usize>, best: &mut usize) {
    if let Some(n) = pick(v) {
        *best = (*best).max(n);
    }
    let mut kids = Vec::new();
    children(v, skip, &mut kids);
    for k in kids {
        largest(k, skip, pick, best);
    }
}

fn for_first_mut(
    v: &mut Value,
    skip: &[&str],
    pick: &dyn Fn(&Value) -> Option<usize>,
    target: usize,
    act: &mut dyn FnMut(&mut Value),
) -> bool {
    if pick(v) == Some(target) {
        act(v);
        return true;
    }
    match v {
        Value::Array(a) => a
            .iter_mut()
            .any(|e| for_first_mut(e, skip, pick, target, act)),
        Value::Object(o) => o
            .iter_mut()
            .filter(|(k, e)| !(e.is_array() && skip.contains(&k.as_str())))
            .any(|(_, e)| for_first_mut(e, skip, pick, target, act)),
        _ => false,
    }
}

fn cut_array_tail(a: &mut Vec<Value>, excess: usize, skip: &[&str]) {
    let mut removed = 0usize;
    while removed < excess {
        match a.pop() {
            Some(e) => removed += size(&e, skip) + 1,
            None => break,
        }
    }
}

fn shorten_string(s: &mut String, excess: usize) {
    let removed = (excess + SHORTEN_SLACK).min(s.len());
    let mut keep = s.len() - removed;
    while !s.is_char_boundary(keep) {
        keep -= 1;
    }
    let gone = s.len() - keep;
    s.truncate(keep);
    s.push_str(&format!("...[+{gone} bytes]"));
}

fn cut_object_tail(o: &mut Map<String, Value>, excess: usize, skip: &[&str]) {
    let mut removed = 0usize;
    while removed < excess {
        match o.keys().next_back().cloned() {
            Some(k) => {
                if let Some(e) = o.remove(&k) {
                    removed += size(&e, skip) + scalar_len(&Value::String(k)) + 2;
                }
            }
            None => break,
        }
    }
}

/// Reduce `v` until `size(v, skip) <= budget`: arrays cut at element boundaries, then the longest strings shortened,
/// then (last resort, so the bound always holds) object entries dropped.
fn shrink(v: &mut Value, budget: usize, skip: &[&str]) {
    let arr_len = |v: &Value| match v {
        Value::Array(a) if !a.is_empty() => Some(size(v, skip)),
        _ => None,
    };
    let str_len = |v: &Value| match v {
        Value::String(s) if s.len() > MIN_SHORTEN_LEN => Some(s.len()),
        _ => None,
    };
    let obj_len = |v: &Value| match v {
        Value::Object(o) if !o.is_empty() => Some(size(v, skip)),
        _ => None,
    };
    loop {
        let cur = size(v, skip);
        if cur <= budget {
            return;
        }
        let excess = cur - budget;
        let mut best = 0usize;
        largest(v, skip, &arr_len, &mut best);
        if best > 0 {
            for_first_mut(v, skip, &arr_len, best, &mut |n| {
                if let Value::Array(a) = n {
                    cut_array_tail(a, excess, skip)
                }
            });
            continue;
        }
        largest(v, skip, &str_len, &mut best);
        if best > 0 {
            for_first_mut(v, skip, &str_len, best, &mut |n| {
                if let Value::String(s) = n {
                    shorten_string(s, excess)
                }
            });
            continue;
        }
        largest(v, skip, &obj_len, &mut best);
        if best > 0 {
            for_first_mut(v, skip, &obj_len, best, &mut |n| {
                if let Value::Object(o) = n {
                    cut_object_tail(o, excess, skip)
                }
            });
            continue;
        }
        // Nothing reducible is left (scalars only): replace the root with a placeholder.
        *v = Value::Null;
        return;
    }
}

/// Reduce every exempt array found under `v`; returns whether anything changed.
fn reduce_exempt(v: &mut Value, cap: &Cap) -> bool {
    let mut changed = false;
    match v {
        Value::Object(o) => {
            for (k, e) in o.iter_mut() {
                if cap.exempt_arrays.contains(&k.as_str()) {
                    if let Value::Array(a) = e {
                        if a.len() > cap.exempt_len_max {
                            a.truncate(cap.exempt_len_max);
                            changed = true;
                        }
                        for entry in a.iter_mut() {
                            if scalar_or_full_len(entry) > cap.exempt_entry_max {
                                shrink(entry, cap.exempt_entry_max, &[]);
                                changed = true;
                            }
                        }
                        continue;
                    }
                }
                changed |= reduce_exempt(e, cap);
            }
        }
        Value::Array(a) => {
            for e in a.iter_mut() {
                changed |= reduce_exempt(e, cap);
            }
        }
        _ => {}
    }
    changed
}

fn scalar_or_full_len(v: &Value) -> usize {
    size(v, &[])
}

/// Cap `value` at `cap.max_bytes` (exempt arrays aside) and keep it valid JSON. A reduced result carries
/// `"truncated": true` and `"original_bytes": n`.
pub fn cap_json(mut value: Value, cap: &Cap) -> Value {
    let original = size(&value, &[]);
    let reduced = reduce_exempt(&mut value, cap);
    let over = size(&value, cap.exempt_arrays) > cap.max_bytes;
    if !reduced && !over {
        return value;
    }
    if over {
        shrink(
            &mut value,
            cap.max_bytes.saturating_sub(MARKER_RESERVE),
            cap.exempt_arrays,
        );
    }
    let mut obj = match value {
        Value::Object(o) => o,
        other => {
            let mut o = Map::new();
            o.insert("data".to_string(), other);
            o
        }
    };
    obj.insert("truncated".to_string(), json!(true));
    obj.insert("original_bytes".to_string(), json!(original));
    Value::Object(obj)
}

/// Replace every container nested deeper than `max_depth` (the root is depth 0) with `"<depth>"`.
pub fn depth_cap(value: Value, max_depth: usize) -> Value {
    fn walk(v: Value, depth: usize, max: usize) -> Value {
        match v {
            Value::Array(_) | Value::Object(_) if depth > max => {
                Value::String("<depth>".to_string())
            }
            Value::Array(a) => {
                Value::Array(a.into_iter().map(|e| walk(e, depth + 1, max)).collect())
            }
            Value::Object(o) => Value::Object(
                o.into_iter()
                    .map(|(k, e)| (k, walk(e, depth + 1, max)))
                    .collect(),
            ),
            other => other,
        }
    }
    walk(value, 0, max_depth)
}

fn sensitive_key(key: &str) -> bool {
    let k = key.to_lowercase().replace('-', "_");
    if k.contains("tokens") {
        return false;
    }
    [
        "header",
        "apikey",
        "api_key",
        "token",
        "secret",
        "password",
        "auth",
        "credential",
    ]
    .iter()
    .any(|n| k.contains(n))
}

/// Redact secret-bearing keys and strip URL userinfo from every string.
pub fn redact(value: &mut Value) {
    match value {
        Value::Object(o) => {
            for (k, e) in o.iter_mut() {
                if sensitive_key(k) {
                    *e = Value::String("<redacted>".to_string());
                } else {
                    redact(e);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(redact),
        Value::String(s) => {
            let t = redact_text(s);
            if t != *s {
                *s = t;
            }
        }
        _ => {}
    }
}

/// Remove `user[:pass]@` from every `scheme://user:pass@host` in `s`.
pub fn redact_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '/' | '?' | '#' | '"' | '\'' | '<' | '>')
            })
            .unwrap_or(tail.len());
        let authority = &tail[..end];
        match authority.rfind('@') {
            Some(at)
                if head[..head.len() - 3].ends_with(|c: char| {
                    c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')
                }) =>
            {
                out.push_str(&authority[at + 1..]);
            }
            _ => out.push_str(authority),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolsMode {
    None,
    Summary,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Message {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip)]
    pub created: i64,
    pub text: String,
    #[serde(skip)]
    pub last_text: Option<String>,
    pub tools: Vec<Value>,
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

pub fn compact_message(raw: &Value, reasoning: bool, tools: ToolsMode) -> Message {
    let r#type = str_of(raw, "type").unwrap_or_default();
    let mut m = Message {
        id: str_of(raw, "id").unwrap_or_default(),
        r#type: r#type.clone(),
        outcome: None,
        created: raw
            .pointer("/time/created")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        text: String::new(),
        last_text: None,
        tools: Vec::new(),
    };
    match r#type.as_str() {
        "user" | "synthetic" => m.text = str_of(raw, "text").unwrap_or_default(),
        "idle" => m.outcome = str_of(raw, "outcome"),
        "assistant" => {
            for part in raw
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let part_text = || str_of(part, "text").unwrap_or_default();
                match part.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        let t = part_text();
                        m.text.push_str(&t);
                        m.last_text = Some(t);
                    }
                    Some("reasoning") if reasoning => m.text.push_str(&part_text()),
                    Some("tool") => match tools {
                        ToolsMode::None => {}
                        ToolsMode::Summary => m.tools.push(json!({
                            "name": part.get("name").cloned().unwrap_or(Value::Null),
                            "status": part.pointer("/state/status").cloned().unwrap_or(Value::Null),
                        })),
                        ToolsMode::Full => m.tools.push(part.clone()),
                    },
                    _ => {}
                }
            }
        }
        _ => {}
    }
    m
}

fn pick(raw: &Value, keys: &[&str]) -> Map<String, Value> {
    keys.iter()
        .filter_map(|k| raw.get(*k).map(|v| ((*k).to_string(), v.clone())))
        .collect()
}

/// Keep the form-field keys the agent needs (`packages/schema/src/form.ts:53-119`).
pub fn compact_form_field(raw: &Value) -> Value {
    let mut keys = vec![
        "key",
        "type",
        "title",
        "description",
        "required",
        "hidden",
        "when",
    ];
    let extra: &[&str] = match raw.get("type").and_then(Value::as_str) {
        Some("string") => &[
            "format",
            "minLength",
            "maxLength",
            "pattern",
            "placeholder",
            "default",
            "options",
            "custom",
        ],
        Some("number" | "integer") => &["minimum", "maximum", "default"],
        Some("boolean") => &["default"],
        Some("multiselect") => &["options", "minItems", "maxItems", "custom", "default"],
        Some("external") => &["url"],
        _ => &[],
    };
    keys.extend_from_slice(extra);
    Value::Object(pick(raw, &keys))
}

pub fn compact_permission(raw: &Value) -> Value {
    Value::Object(pick(
        raw,
        &["id", "action", "resources", "save", "message", "metadata"],
    ))
}

/// Split a CLI envelope into `(data, cursor, meta)`; a `_tag` envelope is a `CLI_ERROR`.
pub fn unwrap_envelope(raw: Value) -> Result<(Value, Option<Value>, Option<Value>), ToolError> {
    if let Some(tag) = raw.get("_tag").and_then(Value::as_str) {
        let message = raw
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or(tag)
            .to_string();
        return Err(ToolError::new(ErrorCode::CliError, message).details(json!({ "tag": tag })));
    }
    match raw {
        Value::Object(mut o) if o.contains_key("data") => {
            let data = o.remove("data").unwrap_or(Value::Null);
            let cursor = o.remove("cursor").filter(|c| !c.is_null());
            let meta = if o.is_empty() {
                None
            } else {
                Some(Value::Object(o))
            };
            Ok((data, cursor, meta))
        }
        other => Ok((other, None, None)),
    }
}
