// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

use repo_tasks::model::{validate_description, validate_owner, validate_subject, List};
use serde_json::{json, Value};

const TS: &str = "2026-11-18T10:12:00Z";

fn task(id: u64) -> Value {
    json!({"id": id, "subject": "s", "status": "pending", "blocked_by": [],
        "created_at": TS, "updated_at": TS, "updated_by": "cli"})
}

fn list(next_id: u64, tasks: Vec<Value>) -> Value {
    json!({"format": 1, "next_id": next_id, "tasks": tasks})
}

fn with(mut t: Value, key: &str, v: Value) -> Value {
    t[key] = v;
    t
}

fn parse(v: &Value) -> repo_tasks::error::Result<List> {
    List::parse(&serde_json::to_vec(v).unwrap())
}

#[test]
fn empty_list_is_valid() {
    let l = List::empty();
    assert_eq!((l.format, l.next_id, l.tasks.len()), (1, 1, 0));
    l.validate().unwrap();
    List::parse(&l.to_bytes()).unwrap();
}

#[test]
fn round_trip_keeps_unknown_members() {
    let mut v = list(2, vec![with(task(1), "future_task", json!({"a": [1, 2]}))]);
    v["future_top"] = json!("kept");
    let l = parse(&v).unwrap();
    let out: Value = serde_json::from_slice(&l.to_bytes()).unwrap();
    assert_eq!(out["future_top"], json!("kept"));
    assert_eq!(out["tasks"][0]["future_task"], json!({"a": [1, 2]}));
}

#[test]
fn to_bytes_ends_with_one_line_break() {
    let b = List::empty().to_bytes();
    assert!(b.ends_with(b"\n") && !b.ends_with(b"\n\n"));
}

#[test]
fn omitted_not_null() {
    let l = parse(&list(2, vec![task(1)])).unwrap();
    let out: Value = serde_json::from_slice(&l.to_bytes()).unwrap();
    let t = out["tasks"][0].as_object().unwrap();
    assert!(!t.contains_key("owner") && !t.contains_key("description"));
}

#[test]
fn newer_format_names_both_numbers() {
    let mut v = list(1, vec![]);
    v["format"] = json!(2);
    let msg = parse(&v).unwrap_err().to_string();
    assert!(msg.contains('2') && msg.contains('1'), "{msg}");
    assert!(msg.contains("update the plugin"), "{msg}");
    assert!(!msg.contains('\n'));
}

#[test]
fn newer_format_with_unknown_layout_still_reports_format() {
    let msg = List::parse(br#"{"format": 7, "tasks": "different"}"#)
        .unwrap_err()
        .to_string();
    assert!(
        msg.contains('7') && msg.contains("update the plugin"),
        "{msg}"
    );
}

#[test]
fn rejects_each_rule() {
    let long = |n: usize| "a".repeat(n);
    let mut f0 = list(1, vec![]);
    f0["format"] = json!(0);
    let mut fstr = list(1, vec![]);
    fstr["format"] = json!("1");
    let mut nonum = list(1, vec![]);
    nonum["next_id"] = json!("1");
    let rows: Vec<(&str, Value)> = vec![
        ("format 0", f0),
        ("format string", fstr),
        ("next_id string", nonum),
        ("invalid json shape", json!([1])),
        ("ids not ascending", list(3, vec![task(2), task(1)])),
        ("duplicate id", list(3, vec![task(1), task(1)])),
        ("id zero", list(3, vec![task(0)])),
        ("next_id equals max id", list(2, vec![task(2)])),
        ("next_id below max id", list(2, vec![task(5)])),
        (
            "blank subject",
            list(2, vec![with(task(1), "subject", json!("  \t "))]),
        ),
        (
            "empty subject",
            list(2, vec![with(task(1), "subject", json!(""))]),
        ),
        (
            "subject 201",
            list(2, vec![with(task(1), "subject", json!(long(201)))]),
        ),
        (
            "subject control",
            list(2, vec![with(task(1), "subject", json!("a\u{1b}b"))]),
        ),
        (
            "subject newline",
            list(2, vec![with(task(1), "subject", json!("a\nb"))]),
        ),
        (
            "subject del",
            list(2, vec![with(task(1), "subject", json!("a\u{7f}"))]),
        ),
        (
            "description 8001",
            list(2, vec![with(task(1), "description", json!(long(8001)))]),
        ),
        (
            "description control",
            list(2, vec![with(task(1), "description", json!("a\u{1b}b"))]),
        ),
        (
            "description cr",
            list(2, vec![with(task(1), "description", json!("a\rb"))]),
        ),
        (
            "owner 65",
            list(2, vec![with(task(1), "owner", json!(long(65)))]),
        ),
        (
            "owner control",
            list(2, vec![with(task(1), "owner", json!("a\tb"))]),
        ),
        (
            "status unknown",
            list(2, vec![with(task(1), "status", json!("done"))]),
        ),
        (
            "blocked_by unsorted",
            list(
                4,
                vec![task(1), task(2), with(task(3), "blocked_by", json!([2, 1]))],
            ),
        ),
        (
            "blocked_by duplicate",
            list(3, vec![task(1), with(task(2), "blocked_by", json!([1, 1]))]),
        ),
        (
            "blocked_by self",
            list(2, vec![with(task(1), "blocked_by", json!([1]))]),
        ),
        (
            "blocked_by unknown",
            list(2, vec![with(task(1), "blocked_by", json!([9]))]),
        ),
        (
            "created_at +00:00",
            list(
                2,
                vec![with(
                    task(1),
                    "created_at",
                    json!("2026-11-18T10:12:00+00:00"),
                )],
            ),
        ),
        (
            "updated_at +00:00",
            list(
                2,
                vec![with(
                    task(1),
                    "updated_at",
                    json!("2026-11-18T10:12:00+00:00"),
                )],
            ),
        ),
        (
            "created_at fraction",
            list(
                2,
                vec![with(task(1), "created_at", json!("2026-11-18T10:12:00.5Z"))],
            ),
        ),
        (
            "created_at garbage",
            list(2, vec![with(task(1), "created_at", json!("yesterday"))]),
        ),
        (
            "created_at number",
            list(2, vec![with(task(1), "created_at", json!(1795000000))]),
        ),
        (
            "owner null",
            list(2, vec![with(task(1), "owner", Value::Null)]),
        ),
    ];
    for (name, v) in rows {
        assert!(parse(&v).is_err(), "row accepted: {name}");
    }
    assert!(List::parse(b"not json").is_err());
    assert!(List::parse(b"").is_err());
}

#[test]
fn accepts_the_boundaries() {
    let long = |n: usize| "a".repeat(n);
    let t = with(task(1), "subject", json!(long(200)));
    let t = with(t, "owner", json!(long(64)));
    let t = with(t, "description", json!(format!("{}\n\tx", long(7990))));
    let t2 = with(task(2), "blocked_by", json!([1]));
    let l = parse(&list(3, vec![t, t2])).unwrap();
    assert_eq!(l.tasks.len(), 2);
    // 200 two-byte characters count as 200
    let t = with(task(1), "subject", json!("é".repeat(200)));
    parse(&list(2, vec![t])).unwrap();
    let t = with(task(1), "subject", json!("é".repeat(201)));
    assert!(parse(&list(2, vec![t])).is_err());
}

#[test]
fn validators_trim_and_check() {
    assert_eq!(validate_subject("  hi \n").unwrap(), "hi");
    assert!(validate_subject("   ").is_err());
    assert!(validate_subject("a\nb").is_err());
    assert!(validate_owner(&"o".repeat(64)).is_ok());
    assert!(validate_owner(&"o".repeat(65)).is_err());
    assert!(validate_owner("a\u{0}b").is_err());
    assert!(validate_description("a\n\tb").is_ok());
    assert!(validate_description("a\u{7f}").is_err());
    assert!(validate_description(&"d".repeat(8001)).is_err());
}
