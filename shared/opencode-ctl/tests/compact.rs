use opencode_ctl::compact::{
    cap_json, compact_form_field, compact_message, compact_permission, depth_cap, redact,
    redact_text, unwrap_envelope, Cap, ToolsMode,
};
use opencode_ctl::error::ErrorCode;
use proptest::prelude::*;
use serde_json::{json, Value};

fn cap(max_bytes: usize) -> Cap {
    Cap {
        max_bytes,
        exempt_arrays: &["pending_permissions", "pending_forms"],
        exempt_entry_max: 1024,
        exempt_len_max: 20,
    }
}

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        "\\PC{0,400}".prop_map(Value::from),
    ];
    leaf.prop_recursive(5, 256, 12, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..12).prop_map(Value::Array),
            prop::collection::hash_map("[a-z]{1,6}", inner, 0..12)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

proptest! {
    #[test]
    fn compact_valid_json_under_cap(v in arb_json(), max in 1024usize..4096) {
        let c = cap(max);
        let out = cap_json(v, &c);
        let s = serde_json::to_string(&out).unwrap();
        prop_assert!(serde_json::from_str::<Value>(&s).is_ok());
        prop_assert!(s.len() <= c.max_bytes + 40960, "len {} max {}", s.len(), max);
    }

    // The generator never emits `pending_*` keys, so no exempt allowance applies: the real bound is `max_bytes`
    // itself (the truncation markers are budgeted inside it by MARKER_RESERVE).
    #[test]
    fn compact_exempt_free_input_fits_max_bytes(v in arb_json(), max in 1024usize..4096) {
        let c = cap(max);
        let s = serde_json::to_string(&cap_json(v, &c)).unwrap();
        prop_assert!(s.len() <= c.max_bytes, "len {} max {}", s.len(), max);
    }
}

#[test]
fn compact_cuts_arrays_before_strings() {
    let items: Vec<Value> = (0..400)
        .map(|i| json!({"n": i, "pad": "xxxxxxxxxx"}))
        .collect();
    let note = "n".repeat(300);
    let v = json!({"items": items, "note": note.clone()});
    let out = cap_json(v, &cap(4096));
    assert_eq!(out["note"], json!(note));
    let kept = out["items"].as_array().unwrap().len();
    assert!(kept > 0 && kept < 400, "kept {kept}");
    assert!(serde_json::to_string(&out).unwrap().len() <= 4096);
    assert_eq!(out["items"][0]["n"], json!(0));
}

#[test]
fn compact_shortens_longest_strings_after_arrays() {
    let v = json!({"big": "a".repeat(20000), "small": "keep"});
    let out = cap_json(v, &cap(2048));
    let big = out["big"].as_str().unwrap();
    assert!(
        big.starts_with("aaaa") && big.contains("...[+") && big.ends_with(" bytes]"),
        "{big}"
    );
    assert_eq!(out["small"], json!("keep"));
    assert!(serde_json::to_string(&out).unwrap().len() <= 2048);
}

#[test]
fn compact_marks_truncated_with_original_bytes() {
    let v = json!({"items": (0..500).map(|i| format!("item-number-{i}")).collect::<Vec<_>>()});
    let original = serde_json::to_string(&v).unwrap().len();
    let out = cap_json(v, &cap(2048));
    assert_eq!(out["truncated"], json!(true));
    assert_eq!(out["original_bytes"], json!(original));
}

#[test]
fn compact_small_value_is_untouched() {
    let v = json!({"a": [1, 2, 3], "b": "x"});
    assert_eq!(cap_json(v.clone(), &cap(2048)), v);
}

#[test]
fn pending_arrays_ceiling() {
    let perms: Vec<Value> = (0..25)
        .map(|i| {
            json!({
                "id": format!("per_{i}"),
                "action": "edit",
                "message": "m".repeat(5000),
                "resources": (0..400).map(|r| format!("/some/path/file-{r}.rs")).collect::<Vec<_>>(),
            })
        })
        .collect();
    let v = json!({"status": "blocked", "pending_permissions": perms});
    let out = cap_json(v, &cap(16384));
    let arr = out["pending_permissions"].as_array().unwrap();
    assert_eq!(arr.len(), 20);
    for e in arr {
        assert!(serde_json::to_string(e).unwrap().len() <= 1024);
        assert!(e["id"].is_string());
    }
    let total = serde_json::to_string(&out).unwrap().len();
    assert!(total <= 16384 + 40960, "{total}");
    assert_eq!(out["truncated"], json!(true));
}

#[test]
fn pending_arrays_are_not_cut_below_twenty() {
    let forms: Vec<Value> = (0..20).map(|i| json!({"id": format!("frm_{i}")})).collect();
    let v = json!({"pending_forms": forms, "filler": "z".repeat(50000)});
    let out = cap_json(v, &cap(1024));
    assert_eq!(out["pending_forms"].as_array().unwrap().len(), 20);
}

#[test]
fn redaction_substring_and_url_userinfo() {
    let mut v = json!({
        "Authorization": "Bearer abc",
        "x-api-key": "k",
        "client_secret": "s",
        "tokens": 12,
        "accessToken": "t",
        "nested": {"Headers": {"a": 1}, "ok": "https://a:b@h/p"},
        "msg": "Error: Server at https://u:p@h rejected",
    });
    redact(&mut v);
    assert_eq!(v["Authorization"], json!("<redacted>"));
    assert_eq!(v["x-api-key"], json!("<redacted>"));
    assert_eq!(v["client_secret"], json!("<redacted>"));
    assert_eq!(v["accessToken"], json!("<redacted>"));
    assert_eq!(v["tokens"], json!(12));
    assert_eq!(v["nested"]["Headers"], json!("<redacted>"));
    assert_eq!(v["nested"]["ok"], json!("https://h/p"));
    assert_eq!(v["msg"], json!("Error: Server at https://h rejected"));
    assert_eq!(redact_text("https://a:b@h/p"), "https://h/p");
    assert_eq!(
        redact_text("see http://h/p@x and ftp://me@h2:21/z"),
        "see http://h/p@x and ftp://h2:21/z"
    );
}

#[test]
fn redaction_tokens_exemption_is_only_for_the_token_term() {
    let mut v = json!({"authTokens": 1, "secret_tokens": 2, "tokens": 3, "Tokens": 4, "token": 5, "Token": 6, "x-token": 7});
    redact(&mut v);
    for k in ["authTokens", "secret_tokens", "token", "Token", "x-token"] {
        assert_eq!(v[k], json!("<redacted>"), "{k}");
    }
    assert_eq!(v["tokens"], json!(3));
    assert_eq!(v["Tokens"], json!(4));
}

#[test]
fn depth_cap_six() {
    let v = json!({"a": {"b": {"c": {"d": {"e": {"f": {"g": {"h": 1}}}}}}}});
    let out = depth_cap(v, 6);
    assert_eq!(out["a"]["b"]["c"]["d"]["e"]["f"]["g"], json!("<depth>"));
    assert!(out["a"]["b"]["c"]["d"]["e"]["f"].is_object());
    let shallow = json!({"a": [1, {"b": 2}]});
    assert_eq!(depth_cap(shallow.clone(), 6), shallow);
}

#[test]
fn compact_message_last_text() {
    let two = json!({"id": "msg_1", "type": "assistant", "time": {"created": 5}, "content": [
        {"type": "text", "text": "first "},
        {"type": "tool", "name": "bash", "state": {"status": "completed"}},
        {"type": "text", "text": "second"},
    ]});
    let m = compact_message(&two, false, ToolsMode::Summary);
    assert_eq!(m.last_text.as_deref(), Some("second"));
    assert_eq!(m.text, "first second");
    assert_eq!(m.created, 5);

    let none = json!({"id": "m", "type": "assistant", "time": {"created": 1}, "content": [
        {"type": "tool", "name": "bash", "state": {"status": "running"}},
        {"type": "reasoning", "text": "thinking"},
    ]});
    assert_eq!(
        compact_message(&none, false, ToolsMode::Summary).last_text,
        None
    );
    assert_eq!(
        compact_message(&none, true, ToolsMode::Summary).last_text,
        None
    );

    let trailing = json!({"id": "m", "type": "assistant", "time": {"created": 1}, "content": [
        {"type": "text", "text": "answer"},
        {"type": "reasoning", "text": "afterthought"},
    ]});
    let m = compact_message(&trailing, true, ToolsMode::None);
    assert_eq!(m.last_text.as_deref(), Some("answer"));
    assert_eq!(m.text, "answerafterthought");

    for raw in [
        json!({"id": "u", "type": "user", "time": {"created": 1}, "text": "hello"}),
        json!({"id": "i", "type": "idle", "time": {"created": 1}, "outcome": "completed"}),
        json!({"id": "s", "type": "synthetic", "time": {"created": 1}, "text": "note"}),
    ] {
        assert_eq!(
            compact_message(&raw, true, ToolsMode::Full).last_text,
            None,
            "{raw}"
        );
    }
}

#[test]
fn compact_message_modes() {
    let mut content = Vec::new();
    for i in 0..18 {
        content.push(json!({"type": "reasoning", "text": format!("r{i};")}));
    }
    for i in 0..11 {
        content.push(json!({"type": "text", "text": format!("t{i};")}));
    }
    for i in 0..24 {
        content.push(json!({"type": "tool", "id": format!("t{i}"), "name": "read", "state": {"status": "completed", "output": "x".repeat(100)}, "time": {}, "executed": true}));
    }
    let raw =
        json!({"id": "msg_a", "type": "assistant", "time": {"created": 42}, "content": content});

    let m = compact_message(&raw, false, ToolsMode::Summary);
    assert_eq!(
        m.text,
        (0..11).map(|i| format!("t{i};")).collect::<String>()
    );
    assert_eq!(m.tools.len(), 24);
    assert_eq!(m.tools[0], json!({"name": "read", "status": "completed"}));
    assert_eq!(m.last_text.as_deref(), Some("t10;"));
    assert_eq!(
        serde_json::to_value(&m).unwrap(),
        json!({"id": "msg_a", "type": "assistant", "text": m.text, "tools": m.tools})
    );

    let m = compact_message(&raw, true, ToolsMode::None);
    assert!(m.text.starts_with("r0;r1;") && m.text.contains("t0;"));
    assert!(m.tools.is_empty());

    let m = compact_message(&raw, false, ToolsMode::Full);
    assert_eq!(m.tools.len(), 24);
    assert_eq!(m.tools[0]["state"]["output"], json!("x".repeat(100)));

    let user = json!({"id": "msg_u", "type": "user", "time": {"created": 7}, "text": "do it"});
    let m = compact_message(&user, false, ToolsMode::Summary);
    assert_eq!(
        (m.text.as_str(), m.created, m.outcome.clone()),
        ("do it", 7, None)
    );

    let idle =
        json!({"id": "msg_i", "type": "idle", "time": {"created": 8}, "outcome": "completed"});
    let m = compact_message(&idle, false, ToolsMode::Summary);
    assert_eq!(
        serde_json::to_value(&m).unwrap(),
        json!({"id": "msg_i", "type": "idle", "outcome": "completed", "text": "", "tools": []})
    );

    let other = json!({"id": "msg_c", "type": "compaction", "time": {"created": 9}});
    let m = compact_message(&other, false, ToolsMode::Summary);
    assert_eq!((m.r#type.as_str(), m.text.as_str()), ("compaction", ""));
}

#[test]
fn compact_form_field_per_type() {
    let base = |t: &str| {
        json!({"key": "k", "type": t, "title": "T", "description": "D", "required": true, "hidden": false,
        "when": [{"key": "a", "op": "eq", "value": 1}], "junk": 1})
    };
    let with = |t: &str, extra: Value| {
        let mut b = base(t);
        for (k, v) in extra.as_object().unwrap() {
            b[k] = v.clone();
        }
        b
    };
    let opts = json!([{"value": "a", "label": "A"}]);

    let s = compact_form_field(&with(
        "string",
        json!({"format": "email", "minLength": 1, "maxLength": 9, "pattern": "^x", "placeholder": "p", "default": "d", "options": opts, "custom": true, "minimum": 3}),
    ));
    assert_eq!(
        s,
        json!({"key": "k", "type": "string", "title": "T", "description": "D", "required": true, "hidden": false,
            "when": [{"key": "a", "op": "eq", "value": 1}], "format": "email", "minLength": 1, "maxLength": 9,
            "pattern": "^x", "placeholder": "p", "default": "d", "options": opts, "custom": true})
    );

    for t in ["number", "integer"] {
        let f = compact_form_field(&with(
            t,
            json!({"minimum": 1, "maximum": 5, "default": 2, "pattern": "x"}),
        ));
        assert_eq!(f["minimum"], json!(1));
        assert_eq!(f["maximum"], json!(5));
        assert_eq!(f["default"], json!(2));
        assert!(f.get("pattern").is_none() && f.get("junk").is_none());
    }

    let b = compact_form_field(&with("boolean", json!({"default": true, "minimum": 1})));
    assert_eq!(b["default"], json!(true));
    assert!(b.get("minimum").is_none());

    let m = compact_form_field(&with(
        "multiselect",
        json!({"options": opts, "minItems": 1, "maxItems": 2, "custom": false, "default": ["a"], "format": "x"}),
    ));
    assert_eq!(m["minItems"], json!(1));
    assert_eq!(m["maxItems"], json!(2));
    assert_eq!(m["custom"], json!(false));
    assert_eq!(m["default"], json!(["a"]));
    assert_eq!(m["options"], opts);
    assert!(m.get("format").is_none());

    let e = compact_form_field(
        &json!({"key": "k", "type": "external", "url": "https://x/y", "title": "T", "description": "D", "junk": 1}),
    );
    assert_eq!(
        e,
        json!({"key": "k", "type": "external", "url": "https://x/y", "title": "T", "description": "D"})
    );
}

#[test]
fn compact_permission_keeps_listed_keys() {
    let raw = json!({"id": "per_1", "action": "edit", "resources": ["a"], "save": ["b"], "message": "m", "metadata": {"x": 1}, "sessionID": "ses_x", "time": 1});
    assert_eq!(
        compact_permission(&raw),
        json!({"id": "per_1", "action": "edit", "resources": ["a"], "save": ["b"], "message": "m", "metadata": {"x": 1}})
    );
}

#[test]
fn unwrap_envelope_shapes() {
    let (d, c, m) = unwrap_envelope(json!({"data": [1, 2], "cursor": "abc", "extra": 3})).unwrap();
    assert_eq!(d, json!([1, 2]));
    assert_eq!(c, Some(json!("abc")));
    assert_eq!(m, Some(json!({"extra": 3})));

    let (d, c, m) = unwrap_envelope(json!({"data": {"a": 1}})).unwrap();
    assert_eq!((d, c, m), (json!({"a": 1}), None, None));

    let (d, c, m) = unwrap_envelope(json!({"id": "x"})).unwrap();
    assert_eq!((d, c, m), (json!({"id": "x"}), None, None));
    let (d, _, _) = unwrap_envelope(json!([1])).unwrap();
    assert_eq!(d, json!([1]));

    let e = unwrap_envelope(json!({"_tag": "SessionNotFoundError", "message": "no such session"}))
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::CliError);
    assert_eq!(e.message, "no such session");
    assert_eq!(e.details.unwrap()["tag"], json!("SessionNotFoundError"));
}
