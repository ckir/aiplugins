// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The list file (spec 8.1) and the version-skew rules (8.4).

use crate::clock::{parse_rfc3339, rfc3339};
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The newest list format this engine knows.
pub const FORMAT: u32 = 1;

const SUBJECT_MAX: usize = 200;
const DESCRIPTION_MAX: usize = 8000;
const OWNER_MAX: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    InProgress,
    Completed,
}

/// An optional member is omitted when absent; an explicit null is a broken rule.
fn not_null<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub subject: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "not_null"
    )]
    pub description: Option<String>,
    pub status: Status,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "not_null"
    )]
    pub owner: Option<String>,
    pub blocked_by: Vec<u64>,
    pub created_at: String,
    pub updated_at: String,
    pub updated_by: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    pub format: u32,
    pub next_id: u64,
    pub tasks: Vec<Task>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn invalid(reason: impl Into<String>) -> Error {
    Error::ListInvalid {
        reason: reason.into(),
    }
}

fn is_control(c: char) -> bool {
    c.is_control() && (c <= '\u{1f}' || c == '\u{7f}')
}

/// Trim, then require 1 to 200 characters and no control character.
pub fn validate_subject(s: &str) -> Result<String> {
    let t = s.trim();
    let n = t.chars().count();
    if n == 0 {
        return Err(invalid("a subject must not be blank"));
    }
    if n > SUBJECT_MAX {
        return Err(invalid(format!(
            "a subject is at most {SUBJECT_MAX} characters, got {n}"
        )));
    }
    if t.chars().any(is_control) {
        return Err(invalid("a subject must not contain a control character"));
    }
    Ok(t.to_string())
}

/// At most 64 characters and no control character.
pub fn validate_owner(s: &str) -> Result<String> {
    let n = s.chars().count();
    if n > OWNER_MAX {
        return Err(invalid(format!(
            "an owner is at most {OWNER_MAX} characters, got {n}"
        )));
    }
    if s.chars().any(is_control) {
        return Err(invalid("an owner must not contain a control character"));
    }
    Ok(s.to_string())
}

/// At most 8,000 characters; line feed and tab allowed, no other control character.
pub fn validate_description(s: &str) -> Result<String> {
    let n = s.chars().count();
    if n > DESCRIPTION_MAX {
        return Err(invalid(format!(
            "a description is at most {DESCRIPTION_MAX} characters, got {n}"
        )));
    }
    if s.chars().any(|c| is_control(c) && c != '\n' && c != '\t') {
        return Err(invalid(
            "a description must not contain a control character other than line feed and tab",
        ));
    }
    Ok(s.to_string())
}

fn validate_timestamp(field: &str, id: u64, s: &str) -> Result<()> {
    match parse_rfc3339(s) {
        Some(secs) if rfc3339(secs) == s => Ok(()),
        _ => Err(invalid(format!(
            "task {id}: {field} '{s}' is not RFC 3339 UTC in whole seconds ending in Z"
        ))),
    }
}

impl List {
    pub fn empty() -> List {
        List {
            format: FORMAT,
            next_id: 1,
            tasks: Vec::new(),
            extra: Map::new(),
        }
    }

    /// Parse and validate. A `format` above the newest known fails before the rest
    /// of the layout is read, because a newer layout may differ.
    pub fn parse(bytes: &[u8]) -> Result<List> {
        let value: Value =
            serde_json::from_slice(bytes).map_err(|e| invalid(format!("not valid JSON: {e}")))?;
        let format = value
            .get("format")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("'format' must be an integer"))?;
        if format > u64::from(FORMAT) {
            return Err(Error::NewerFormat {
                found: format,
                newest: FORMAT,
            });
        }
        let list: List = serde_json::from_value(value).map_err(|e| invalid(e.to_string()))?;
        list.validate()?;
        Ok(list)
    }

    pub fn validate(&self) -> Result<()> {
        if self.format != FORMAT {
            return Err(invalid(format!(
                "'format' must be {FORMAT}, got {}",
                self.format
            )));
        }
        let mut prev: Option<u64> = None;
        for t in &self.tasks {
            if t.id == 0 {
                return Err(invalid("a task id must be positive"));
            }
            if let Some(p) = prev {
                if t.id <= p {
                    return Err(invalid(format!(
                        "task ids must be unique and ascending: {} follows {p}",
                        t.id
                    )));
                }
            }
            prev = Some(t.id);
        }
        if let Some(max) = prev {
            if self.next_id <= max {
                return Err(invalid(format!(
                    "'next_id' {} must be above every id (highest is {max})",
                    self.next_id
                )));
            }
        }
        for t in &self.tasks {
            let id = t.id;
            if t.subject.chars().any(is_control) {
                return Err(invalid(format!(
                    "task {id}: a subject must not contain a control character"
                )));
            }
            validate_subject(&t.subject).map_err(|e| invalid(format!("task {id}: {e}")))?;
            if let Some(d) = &t.description {
                validate_description(d).map_err(|e| invalid(format!("task {id}: {e}")))?;
            }
            if let Some(o) = &t.owner {
                validate_owner(o).map_err(|e| invalid(format!("task {id}: {e}")))?;
            }
            for w in t.blocked_by.windows(2) {
                if w[0] >= w[1] {
                    return Err(invalid(format!(
                        "task {id}: blocked_by must be ascending without duplicates"
                    )));
                }
            }
            for b in &t.blocked_by {
                if *b == id {
                    return Err(invalid(format!(
                        "task {id}: blocked_by names the task itself"
                    )));
                }
                if !self.tasks.iter().any(|o| o.id == *b) {
                    return Err(invalid(format!(
                        "task {id}: blocked_by names unknown task {b}"
                    )));
                }
            }
            validate_timestamp("created_at", id, &t.created_at)?;
            validate_timestamp("updated_at", id, &t.updated_at)?;
        }
        Ok(())
    }

    /// Indented JSON with one line break at the end.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = serde_json::to_vec_pretty(self).unwrap_or_default();
        out.push(b'\n');
        out
    }
}
