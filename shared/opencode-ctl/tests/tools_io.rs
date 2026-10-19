//! Prompt, messages, changes, workspace status, permission and form tools over the `FakeTransport` seam.
#![allow(clippy::field_reassign_with_default)]

mod common;

use common::*;
use opencode_ctl::clock::TestClock;
use opencode_ctl::error::ErrorCode;
use opencode_ctl::host::Host;
use opencode_ctl::settings::Settings;
use opencode_ctl::testing::fake::{FakeTransport, RecordedCall};
use opencode_ctl::testing::temp_home;
use opencode_ctl::transport::TransportError;
use serde_json::{json, Value};

struct Rig {
    fake: FakeTransport,
    clock: TestClock,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    _home: opencode_ctl::testing::TempHome,
}

async fn rig(settings: Settings, paired: bool, setup: impl FnOnce(&FakeTransport)) -> Rig {
    let home = temp_home();
    let clock = TestClock::new(1_000);
    let fake = FakeTransport::new().with_clock(clock.clone());
    setup(&fake);
    let env = if paired {
        let mut env = env_paired(&home, SID, None);
        env.directory = Some("/paired/dir".into());
        env
    } else {
        env_unpaired(&home)
    };
    let e = engine(fake.clone(), clock.clone(), Host::Claude, settings, env);
    let client = client_for(e).await;
    Rig {
        fake,
        clock,
        client,
        _home: home,
    }
}

async fn plain(setup: impl FnOnce(&FakeTransport)) -> Rig {
    rig(Settings::default(), false, setup).await
}

fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

fn data(v: Value) -> Result<Value, TransportError> {
    Ok(json!({ "data": v }))
}

fn tx_err(code: ErrorCode) -> Result<Value, TransportError> {
    Err(TransportError {
        code,
        message: "scripted".into(),
        hint: None,
        details: None,
    })
}

fn calls_of(r: &Rig, op: &str) -> Vec<RecordedCall> {
    r.fake.calls().into_iter().filter(|c| c.op == op).collect()
}

fn param<'a>(c: &'a RecordedCall, k: &str) -> Option<&'a str> {
    c.params
        .iter()
        .find(|(n, _)| n == k)
        .map(|(_, v)| v.as_str())
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

fn want(ks: &[&str]) -> Vec<String> {
    let mut k: Vec<String> = ks.iter().map(|s| s.to_string()).collect();
    k.sort();
    k
}

// ---------------------------------------------------------------- session_prompt

fn prompt_ok(f: &FakeTransport) {
    f.on("session.prompt", vec![data(json!({"id": "msg_abc"}))]);
}

#[tokio::test]
async fn prompt_returns_inbox_id_and_hint() {
    let r = plain(prompt_ok).await;
    let out = call(
        &r.client,
        "session_prompt",
        json!({"session_id": SID, "text": "do it"}),
    )
    .await;
    assert_eq!(out["message_id"], "msg_abc", "{out}");
    assert_eq!(
        out["hint"],
        "call session_wait with after=<message_id> on EVERY slice"
    );
    assert_eq!(keys(&out), want(&["message_id", "delivery", "hint"]));
}

#[tokio::test]
async fn prompt_default_delivery_is_steer() {
    let r = plain(prompt_ok).await;
    let out = call(
        &r.client,
        "session_prompt",
        json!({"session_id": SID, "text": "x"}),
    )
    .await;
    assert_eq!(out["delivery"], "steer");
    let c = &calls_of(&r, "session.prompt")[0];
    assert_eq!(c.body, Some(json!({"text": "x", "delivery": "steer"})));
    assert_eq!(param(c, "sessionID"), Some(SID));
}

#[tokio::test]
async fn prompt_explicit_queue_forwarded() {
    let r = plain(prompt_ok).await;
    let out = call(
        &r.client,
        "session_prompt",
        json!({"session_id": SID, "text": "x", "delivery": "queue"}),
    )
    .await;
    assert_eq!(out["delivery"], "queue");
    let c = &calls_of(&r, "session.prompt")[0];
    assert_eq!(c.body, Some(json!({"text": "x", "delivery": "queue"})));
    let bad = call(
        &r.client,
        "session_prompt",
        json!({"session_id": SID, "text": "x", "delivery": "later"}),
    )
    .await;
    assert_eq!(code(&bad), "INVALID_ARGUMENT", "{bad}");
}

#[tokio::test]
async fn prompt_body_too_large() {
    let mut s = Settings::default();
    s.body_max_bytes = 1024;
    let r = rig(s, false, prompt_ok).await;
    let out = call(
        &r.client,
        "session_prompt",
        json!({"session_id": SID, "text": "a".repeat(2000)}),
    )
    .await;
    assert_eq!(code(&out), "BODY_TOO_LARGE", "{out}");
    assert!(calls_of(&r, "session.prompt").is_empty());
}

// ---------------------------------------------------------------- session_messages

fn user(id: &str, text: &str) -> Value {
    json!({"id": id, "type": "user", "time": {"created": 5}, "text": text})
}

fn assistant(id: &str) -> Value {
    json!({"id": id, "type": "assistant", "time": {"created": 6}, "content": [
        {"type": "reasoning", "text": "thinking"},
        {"type": "text", "text": "answer"},
        {"type": "tool", "name": "bash", "state": {"status": "completed", "output": "big"}}
    ]})
}

fn idle(id: &str) -> Value {
    json!({"id": id, "type": "idle", "outcome": "succeeded", "time": {"created": 7}})
}

fn messages_script(f: &FakeTransport) {
    f.on(
        "session.message.list",
        vec![Ok(json!({
            "data": [user("msg_1", "hi"), assistant("msg_2"), idle("msg_3")],
            "cursor": {"next": "CUR"}
        }))],
    );
}

#[tokio::test]
async fn messages_order_forwarded_only_without_cursor() {
    let r = plain(messages_script).await;
    let out = call(&r.client, "session_messages", json!({"session_id": SID})).await;
    assert_eq!(out["next_cursor"], "CUR", "{out}");
    assert_eq!(out["truncated"], false);
    call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "order": "asc"}),
    )
    .await;
    call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "order": "asc", "cursor": "CUR"}),
    )
    .await;
    let c = calls_of(&r, "session.message.list");
    assert_eq!(param(&c[0], "order"), Some("desc"));
    assert_eq!(param(&c[1], "order"), Some("asc"));
    assert_eq!(param(&c[2], "order"), None);
    assert_eq!(param(&c[2], "cursor"), Some("CUR"));
    assert_eq!(param(&c[0], "sessionID"), Some(SID));
    let bad = call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "order": "sideways"}),
    )
    .await;
    assert_eq!(code(&bad), "INVALID_ARGUMENT", "{bad}");
}

#[tokio::test]
async fn messages_limit_clamped_200() {
    let r = plain(messages_script).await;
    call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "limit": 5000}),
    )
    .await;
    call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "limit": 0}),
    )
    .await;
    call(&r.client, "session_messages", json!({"session_id": SID})).await;
    let c = calls_of(&r, "session.message.list");
    assert_eq!(param(&c[0], "limit"), Some("200"));
    assert_eq!(param(&c[1], "limit"), Some("1"));
    assert_eq!(param(&c[2], "limit"), Some("20"));
}

#[tokio::test]
async fn messages_idle_rows_kept_with_outcome() {
    let r = plain(messages_script).await;
    let out = call(&r.client, "session_messages", json!({"session_id": SID})).await;
    let m = out["messages"].as_array().unwrap();
    assert_eq!(m.len(), 3);
    assert_eq!(keys(&m[0]), want(&["id", "type", "text", "tools"]));
    assert_eq!(keys(&m[1]), want(&["id", "type", "text", "tools"]));
    assert_eq!(
        keys(&m[2]),
        want(&["id", "type", "outcome", "text", "tools"])
    );
    assert_eq!(m[2]["outcome"], "succeeded");
    assert_eq!(m[0]["text"], "hi");
    assert_eq!(m[1]["text"], "answer");
}

#[tokio::test]
async fn messages_type_forwarded_and_tools_modes() {
    let r = plain(messages_script).await;
    let out = call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "type": "user"}),
    )
    .await;
    assert!(out.get("messages").is_some(), "{out}");
    assert_eq!(
        param(&calls_of(&r, "session.message.list")[0], "type"),
        Some("user")
    );
    let tools_of = |out: &Value| out["messages"][1]["tools"].clone();
    let summary = call(&r.client, "session_messages", json!({"session_id": SID})).await;
    assert_eq!(
        tools_of(&summary),
        json!([{"name": "bash", "status": "completed"}])
    );
    let none = call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "tools": "none"}),
    )
    .await;
    assert_eq!(tools_of(&none), json!([]));
    let full = call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "tools": "full"}),
    )
    .await;
    assert_eq!(tools_of(&full)[0]["state"]["output"], "big");
    let reasoning = call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "reasoning": true}),
    )
    .await;
    assert_eq!(reasoning["messages"][1]["text"], "thinkinganswer");
    let bad = call(
        &r.client,
        "session_messages",
        json!({"session_id": SID, "tools": "lots"}),
    )
    .await;
    assert_eq!(code(&bad), "INVALID_ARGUMENT", "{bad}");

    // The cap cuts: truncated is true.
    let mut s = Settings::default();
    s.output_max_bytes = 1024;
    let big = rig(s, false, |f| {
        let rows: Vec<Value> = (0..40)
            .map(|i| user(&format!("msg_{i}"), &"w".repeat(100)))
            .collect();
        f.on("session.message.list", vec![data(json!(rows))]);
    })
    .await;
    let out = call(&big.client, "session_messages", json!({"session_id": SID})).await;
    assert_eq!(out["truncated"], true, "{out}");
    assert!(out.to_string().len() <= 1024, "{}", out.to_string().len());
}

// ---------------------------------------------------------------- workspace_status

#[tokio::test]
async fn envelope_meta_kept() {
    let r = plain(|f| {
        f.on("vcs.get", vec![data(json!({"provider": "git"}))]);
        f.on(
            "vcs.status",
            vec![Ok(json!({
                "location": {"directory": "/w"},
                "data": [{"path": "a.rs", "kind": "modified"}]
            }))],
        );
    })
    .await;
    let out = call(&r.client, "workspace_status", json!({"directory": "/w"})).await;
    assert_eq!(out["vcs"], "git", "{out}");
    assert_eq!(out["directory"], "/w");
    assert_eq!(out["status"], json!([{"path": "a.rs", "kind": "modified"}]));
    assert_eq!(out["meta"], json!({"location": {"directory": "/w"}}));
}

#[tokio::test]
async fn workspace_status_deep_object_param() {
    let r = plain(|f| {
        f.on("vcs.get", vec![data(json!({"provider": "git"}))]);
        f.on("vcs.status", vec![data(json!([]))]);
    })
    .await;
    let out = call(&r.client, "workspace_status", json!({"directory": "/w"})).await;
    assert!(out.get("meta").is_none(), "{out}");
    for op in ["vcs.get", "vcs.status"] {
        let c = &calls_of(&r, op)[0];
        assert_eq!(param(c, "location[directory]"), Some("/w"), "{op}");
    }
}

#[tokio::test]
async fn workspace_status_non_git_null() {
    let r = plain(|f| {
        f.on("vcs.get", vec![data(json!({}))]);
        f.on("vcs.status", vec![data(json!([]))]);
    })
    .await;
    let out = call(&r.client, "workspace_status", json!({"directory": "/w"})).await;
    assert_eq!(out, json!({"directory": "/w", "vcs": null}));
    assert!(calls_of(&r, "vcs.status").is_empty());
}

// ---------------------------------------------------------------- session_changes

fn file(name: &str, status: &str, patch: &str) -> Value {
    json!({"file": name, "status": status, "additions": 1, "deletions": 2, "patch": patch})
}

fn users(f: &FakeTransport) {
    f.on_matching(
        "session.message.list",
        &[("order", "asc")],
        vec![data(json!([user("msg_first", "a")]))],
    );
    f.on_matching(
        "session.message.list",
        &[("order", "desc")],
        vec![data(json!([user("msg_last", "b")]))],
    );
    f.on(
        "session.get",
        vec![data(json!({"id": SID, "location": {"directory": "/w"}}))],
    );
}

#[tokio::test]
async fn changes_default_range_ends_at_user_message() {
    let r = plain(|f| {
        users(f);
        f.on(
            "session.diff",
            vec![data(json!([file("a.rs", "added", "@@ p")]))],
        );
    })
    .await;
    let out = call(&r.client, "session_changes", json!({"session_id": SID})).await;
    assert_eq!(out["from"], "msg_first", "{out}");
    assert_eq!(out["to"], "msg_last");
    assert_eq!(out["directory"], "/w");
    assert_eq!(out["omitted_patches"], 0);
    assert_eq!(out["omitted_files"], json!([]));
    let reads = calls_of(&r, "session.message.list");
    assert_eq!(reads.len(), 2);
    for c in &reads {
        assert_eq!(param(c, "type"), Some("user"));
        assert_eq!(param(c, "limit"), Some("1"));
    }
    assert_eq!(param(&reads[0], "order"), Some("asc"));
    assert_eq!(param(&reads[1], "order"), Some("desc"));
    let d = &calls_of(&r, "session.diff")[0];
    assert_eq!(param(d, "from"), Some("msg_first"));
    assert_eq!(param(d, "to"), Some("msg_last"));
    assert_eq!(param(d, "context"), None);
    assert_eq!(
        out["files"][0],
        json!({"file": "a.rs", "status": "added", "additions": 1, "deletions": 2, "patch": "@@ p"})
    );
}

#[tokio::test]
async fn changes_paired_directory_comes_from_env() {
    let r = rig(Settings::default(), true, |f| {
        f.on("session.diff", vec![data(json!([]))]);
    })
    .await;
    let out = call(
        &r.client,
        "session_changes",
        json!({"from": "msg_1", "to": "msg_2"}),
    )
    .await;
    assert_eq!(out["directory"], "/paired/dir", "{out}");
    assert!(calls_of(&r, "session.get").is_empty());
}

#[tokio::test]
async fn changes_paths_filter_engine_side() {
    let r = plain(|f| {
        users(f);
        f.on(
            "session.diff",
            vec![data(json!([
                file("a.rs", "added", "pa"),
                file("b.rs", "modified", "pb"),
                file("c.rs", "deleted", "pc")
            ]))],
        );
    })
    .await;
    let out = call(
        &r.client,
        "session_changes",
        json!({"session_id": SID, "paths": ["b.rs", "c.rs"]}),
    )
    .await;
    let names: Vec<&str> = out["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["b.rs", "c.rs"]);
    let d = &calls_of(&r, "session.diff")[0];
    assert!(d.params.iter().all(|(k, _)| k != "paths"));
}

fn small_cap() -> Settings {
    let mut s = Settings::default();
    s.output_max_bytes = 2048;
    s
}

#[tokio::test]
async fn changes_drop_patches_before_files() {
    let patch = "x".repeat(900);
    let r = rig(small_cap(), false, |f| {
        users(f);
        f.on(
            "session.diff",
            vec![data(json!([
                file("a.rs", "added", &patch),
                file("b.rs", "added", &patch),
                file("c.rs", "added", &patch),
                file("d.rs", "added", &patch)
            ]))],
        );
    })
    .await;
    let out = call(&r.client, "session_changes", json!({"session_id": SID})).await;
    assert_eq!(out["files"].as_array().unwrap().len(), 4, "{out}");
    assert_eq!(out["omitted_files"], json!([]));
    let with_patch = out["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f.get("patch").is_some())
        .count();
    let omitted = out["omitted_patches"].as_u64().unwrap() as usize;
    assert!(omitted > 0, "{out}");
    assert_eq!(with_patch + omitted, 4);
    assert!(out.get("truncated").is_none(), "{out}");
    assert!(out.to_string().len() <= 2048);
}

#[tokio::test]
async fn changes_files_truncated_lists_names() {
    let r = rig(small_cap(), false, |f| {
        users(f);
        let files: Vec<Value> = (0..60)
            .map(|i| file(&format!("src/dir/file_{i:03}.rs"), "modified", "p"))
            .collect();
        f.on("session.diff", vec![data(json!(files))]);
    })
    .await;
    let out = call(&r.client, "session_changes", json!({"session_id": SID})).await;
    let listed = out["files"].as_array().unwrap().len();
    let omitted: Vec<&str> = out["omitted_files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(listed > 0 && !omitted.is_empty(), "{out}");
    assert_eq!(listed + omitted.len(), 60);
    assert_eq!(omitted[0], format!("src/dir/file_{listed:03}.rs"));
    assert!(out.to_string().len() <= 2048);
}

#[tokio::test]
async fn changes_single_patch_truncated_flag() {
    let r = rig(small_cap(), false, |f| {
        users(f);
        f.on(
            "session.diff",
            vec![data(json!([file(
                "big.rs",
                "modified",
                &"y".repeat(20_000)
            )]))],
        );
    })
    .await;
    let out = call(&r.client, "session_changes", json!({"session_id": SID})).await;
    let f = &out["files"][0];
    assert_eq!(f["patch_truncated"], true, "{out}");
    let len = f["patch"].as_str().unwrap().len();
    assert!(len > 0 && len < 2048, "{len}");
    assert_eq!(out["omitted_patches"], 0);
    assert!(out.to_string().len() <= 2048);
}

#[tokio::test]
async fn changes_patches_false_names_and_counts() {
    let r = plain(|f| {
        users(f);
        f.on(
            "session.diff",
            vec![data(json!([
                file("a.rs", "added", "pa"),
                file("b.rs", "added", "pb")
            ]))],
        );
    })
    .await;
    let out = call(
        &r.client,
        "session_changes",
        json!({"session_id": SID, "patches": false}),
    )
    .await;
    assert_eq!(out["omitted_patches"], 2);
    assert!(out["files"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| f.get("patch").is_none()));
}

#[tokio::test]
async fn changes_status_enum_three_values() {
    let r = plain(|f| {
        users(f);
        f.on(
            "session.diff",
            vec![data(json!([
                file("a", "added", "p"),
                file("b", "modified", "p"),
                file("c", "deleted", "p")
            ]))],
        );
    })
    .await;
    let out = call(&r.client, "session_changes", json!({"session_id": SID})).await;
    let st: Vec<&str> = out["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["status"].as_str().unwrap())
        .collect();
    assert_eq!(st, ["added", "modified", "deleted"]);
}

#[tokio::test]
async fn changes_invalid_range_is_invalid_request() {
    let r = plain(|f| {
        users(f);
        f.on("session.diff", vec![tx_err(ErrorCode::InvalidRequest)]);
    })
    .await;
    let out = call(&r.client, "session_changes", json!({"session_id": SID})).await;
    assert_eq!(code(&out), "INVALID_REQUEST", "{out}");
    // A MessageNotFoundError envelope maps to the same code.
    let r = plain(|f| {
        users(f);
        f.on(
            "session.diff",
            vec![Ok(
                json!({"_tag": "MessageNotFoundError", "message": "no such message"}),
            )],
        );
    })
    .await;
    let out = call(
        &r.client,
        "session_changes",
        json!({"session_id": SID, "from": "msg_x", "to": "msg_y"}),
    )
    .await;
    assert_eq!(code(&out), "INVALID_REQUEST", "{out}");
}

#[tokio::test]
async fn diff_cache_lru_bound() {
    let r = plain(|f| {
        f.on(
            "session.diff",
            vec![data(json!([file("a.rs", "added", "p")]))],
        );
        f.on(
            "session.get",
            vec![data(json!({"id": SID, "location": {"directory": "/w"}}))],
        );
    })
    .await;
    let range = |n: u32, ctx: Option<u32>| {
        let mut a =
            json!({"session_id": SID, "from": format!("msg_f{n}"), "to": format!("msg_t{n}")});
        if let Some(c) = ctx {
            a["context"] = json!(c);
        }
        a
    };
    let diffs = |r: &Rig| calls_of(r, "session.diff").len();
    for n in [1, 2, 3] {
        let out = call(&r.client, "session_changes", range(n, None)).await;
        assert!(out.get("files").is_some(), "{out}");
    }
    assert_eq!(diffs(&r), 3);
    // Cache holds {2, 3}. A hit on 3 then on 2 promotes: LRU order is now 3, 2 (2 newest).
    call(&r.client, "session_changes", range(3, None)).await;
    call(&r.client, "session_changes", range(2, None)).await;
    assert_eq!(diffs(&r), 3, "range 3 and 2 must hit");
    // Range 1 is a miss and evicts the least recently used, range 3.
    call(&r.client, "session_changes", range(1, None)).await;
    assert_eq!(diffs(&r), 4);
    // Range 2 still cached (a FIFO cache would have evicted it).
    call(&r.client, "session_changes", range(2, None)).await;
    assert_eq!(diffs(&r), 4, "range 2 must hit");
    // Range 3 was evicted: miss, evicting range 1.
    call(&r.client, "session_changes", range(3, None)).await;
    assert_eq!(diffs(&r), 5);
    // TTL: after 11 s range 2 is expired.
    r.clock.advance(11_000);
    call(&r.client, "session_changes", range(2, None)).await;
    assert_eq!(diffs(&r), 6, "expired entry must refetch");
    // A different context is a different key.
    call(&r.client, "session_changes", range(2, Some(5))).await;
    assert_eq!(diffs(&r), 7);
    let d = calls_of(&r, "session.diff");
    assert_eq!(param(&d[6], "context"), Some("5"));
}

// ---------------------------------------------------------------- permission_reply

fn perm(id: &str, save: Value) -> Value {
    json!({"id": id, "action": "bash", "resources": ["*"], "save": save})
}

fn perm_script(f: &FakeTransport, save: Value) {
    f.on(
        "session.permission.list",
        vec![data(json!([perm("per_1", save)]))],
    );
    f.on("session.permission.reply", vec![Ok(json!({"ok": true}))]);
}

#[tokio::test]
async fn permission_reply_body_is_decision() {
    let r = plain(|f| perm_script(f, json!([]))).await;
    let out = call(
        &r.client,
        "permission_reply",
        json!({"session_id": SID, "request_id": "per_1", "decision": "once"}),
    )
    .await;
    assert_eq!(out, json!({"replied": true, "saved": false}));
    let c = &calls_of(&r, "session.permission.reply")[0];
    assert_eq!(c.body, Some(json!({"decision": "once"})));
    assert_eq!(param(c, "sessionID"), Some(SID));
    assert_eq!(param(c, "requestID"), Some("per_1"));
    call(
        &r.client,
        "permission_reply",
        json!({"session_id": SID, "request_id": "per_1", "decision": "reject", "message": "no"}),
    )
    .await;
    let c = &calls_of(&r, "session.permission.reply")[1];
    assert_eq!(c.body, Some(json!({"decision": "reject", "message": "no"})));
    let bad = call(
        &r.client,
        "permission_reply",
        json!({"session_id": SID, "request_id": "per_1", "decision": "maybe"}),
    )
    .await;
    assert_eq!(code(&bad), "INVALID_ARGUMENT", "{bad}");
}

#[tokio::test]
async fn permission_reply_always_locked() {
    let mut s = Settings::default();
    s.permissions_locked = true;
    let r = rig(s, false, |f| perm_script(f, json!([{"x": 1}]))).await;
    let out = call(
        &r.client,
        "permission_reply",
        json!({"session_id": SID, "request_id": "per_1", "decision": "always", "confirm": true}),
    )
    .await;
    assert_eq!(code(&out), "PERMISSIONS_LOCKED", "{out}");
    assert!(calls_of(&r, "session.permission.reply").is_empty());
    // once is still allowed under the lock.
    let ok = call(
        &r.client,
        "permission_reply",
        json!({"session_id": SID, "request_id": "per_1", "decision": "once"}),
    )
    .await;
    assert_eq!(ok["replied"], true, "{ok}");
}

#[tokio::test]
async fn permission_reply_always_needs_confirm() {
    let r = plain(|f| perm_script(f, json!([{"x": 1}]))).await;
    for args in [
        json!({"session_id": SID, "request_id": "per_1", "decision": "always"}),
        json!({"session_id": SID, "request_id": "per_1", "decision": "always", "confirm": false}),
    ] {
        let out = call(&r.client, "permission_reply", args).await;
        assert_eq!(code(&out), "CONFIRM_REQUIRED", "{out}");
    }
    assert!(calls_of(&r, "session.permission.reply").is_empty());
}

#[tokio::test]
async fn permission_reply_vanished_is_conflict() {
    let r = plain(|f| {
        f.on("session.permission.list", vec![data(json!([]))]);
        f.on("session.permission.reply", vec![Ok(json!({"ok": true}))]);
    })
    .await;
    let out = call(
        &r.client,
        "permission_reply",
        json!({"session_id": SID, "request_id": "per_1", "decision": "once"}),
    )
    .await;
    assert_eq!(code(&out), "CONFLICT", "{out}");
    assert!(calls_of(&r, "session.permission.reply").is_empty());
}

#[tokio::test]
async fn permission_reply_saved_inference() {
    let always =
        json!({"session_id": SID, "request_id": "per_1", "decision": "always", "confirm": true});
    let r = plain(|f| perm_script(f, json!([{"action": "bash", "resource": "ls"}]))).await;
    let out = call(&r.client, "permission_reply", always.clone()).await;
    assert_eq!(out, json!({"replied": true, "saved": true}), "{out}");
    assert_eq!(
        calls_of(&r, "session.permission.reply")[0].body,
        Some(json!({"decision": "always"}))
    );
    let r = plain(|f| perm_script(f, json!([]))).await;
    let out = call(&r.client, "permission_reply", always).await;
    assert_eq!(out, json!({"replied": true, "saved": false}), "{out}");
    let r = plain(|f| perm_script(f, json!([{"a": 1}]))).await;
    let out = call(
        &r.client,
        "permission_reply",
        json!({"session_id": SID, "request_id": "per_1", "decision": "once"}),
    )
    .await;
    assert_eq!(out["saved"], false, "once never saves: {out}");
}

// ---------------------------------------------------------------- form_reply

#[tokio::test]
async fn form_reply_204_ok() {
    let r = plain(|f| f.on("session.form.reply", vec![Ok(json!({"ok": true}))])).await;
    let out = call(
        &r.client,
        "form_reply",
        json!({"session_id": SID, "form_id": "frm_1", "answer": {"name": "x"}}),
    )
    .await;
    assert_eq!(out, json!({"replied": true}));
    let c = &calls_of(&r, "session.form.reply")[0];
    assert_eq!(c.body, Some(json!({"answer": {"name": "x"}})));
    assert_eq!(param(c, "sessionID"), Some(SID));
    assert_eq!(param(c, "formID"), Some("frm_1"));
}

#[tokio::test]
async fn form_reply_settled_409_conflict() {
    let r = plain(|f| f.on("session.form.reply", vec![tx_err(ErrorCode::Conflict)])).await;
    let out = call(
        &r.client,
        "form_reply",
        json!({"session_id": SID, "form_id": "frm_1", "answer": {}}),
    )
    .await;
    assert_eq!(code(&out), "CONFLICT", "{out}");
}

// ---------------------------------------------------------------- saved_permissions

fn saved(f: &FakeTransport) {
    f.on(
        "permission.saved.list",
        vec![data(json!([
            {"id": "sav_1", "action": "bash"},
            {"id": "sav_2", "action": "edit"}
        ]))],
    );
    f.on("permission.saved.remove", vec![Ok(json!({"ok": true}))]);
}

#[tokio::test]
async fn saved_permissions_remove_needs_confirm() {
    let r = plain(saved).await;
    for args in [
        json!({"remove_id": "sav_1"}),
        json!({"remove_id": "sav_1", "confirm": false}),
    ] {
        let out = call(&r.client, "saved_permissions", args).await;
        assert_eq!(code(&out), "CONFIRM_REQUIRED", "{out}");
    }
    assert!(calls_of(&r, "permission.saved.remove").is_empty());
}

#[tokio::test]
async fn saved_permissions_list_passes_project_id() {
    let r = plain(saved).await;
    let out = call(
        &r.client,
        "saved_permissions",
        json!({"project_id": "prj_1"}),
    )
    .await;
    assert_eq!(out["rules"].as_array().unwrap().len(), 2, "{out}");
    assert_eq!(keys(&out), want(&["rules"]));
    assert_eq!(
        param(&calls_of(&r, "permission.saved.list")[0], "projectID"),
        Some("prj_1")
    );
    call(&r.client, "saved_permissions", json!({})).await;
    assert_eq!(
        param(&calls_of(&r, "permission.saved.list")[1], "projectID"),
        None
    );
}

#[tokio::test]
async fn saved_permissions_remove_with_confirm_executes() {
    let r = plain(saved).await;
    let out = call(
        &r.client,
        "saved_permissions",
        json!({"remove_id": "sav_1", "confirm": true}),
    )
    .await;
    let c = calls_of(&r, "permission.saved.remove");
    assert_eq!(c.len(), 1, "{out}");
    assert_eq!(param(&c[0], "id"), Some("sav_1"));
    assert_eq!(out["rules"], json!([{"id": "sav_2", "action": "edit"}]));
}
