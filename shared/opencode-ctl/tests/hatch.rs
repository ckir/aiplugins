//! `opencode_operations` and `opencode_api` over the `FakeTransport` seam.
#![allow(clippy::field_reassign_with_default)]

mod common;

use common::*;
use opencode_ctl::clock::TestClock;
use opencode_ctl::host::Host;
use opencode_ctl::settings::Settings;
use opencode_ctl::testing::fake::{FakeTransport, RecordedCall};
use opencode_ctl::testing::temp_home;
use serde_json::{json, Value};

struct Rig {
    fake: FakeTransport,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    _home: opencode_ctl::testing::TempHome,
}

async fn rig_with(settings: Settings, setup: impl FnOnce(&FakeTransport)) -> Rig {
    let home = temp_home();
    let clock = TestClock::new(1_000);
    let fake = FakeTransport::new().with_clock(clock.clone());
    setup(&fake);
    let e = engine(
        fake.clone(),
        clock,
        Host::Claude,
        settings,
        env_unpaired(&home),
    );
    let client = client_for(e).await;
    Rig {
        fake,
        client,
        _home: home,
    }
}

async fn rig(setup: impl FnOnce(&FakeTransport)) -> Rig {
    rig_with(Settings::default(), setup).await
}

fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

fn info(version: &str, pid: u64) -> Value {
    json!({"data": {"version": version, "pid": pid, "urls": ["http://127.0.0.1:49374"]}})
}

fn with_info(f: &FakeTransport) {
    f.on("server.info", vec![Ok(info("2.0.32", 1))]);
}

fn ops(r: &Rig) -> Vec<String> {
    r.fake.calls().into_iter().map(|c| c.op).collect()
}

fn calls_of(r: &Rig, op: &str) -> Vec<RecordedCall> {
    r.fake.calls().into_iter().filter(|c| c.op == op).collect()
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/openapi.json")).unwrap()
}

#[tokio::test]
async fn hatch_get_runs() {
    let r = rig(|f| {
        f.on(
            "session.get",
            vec![Ok(json!({"data": {"id": SID, "title": "t"}}))],
        )
    })
    .await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.get", "params": {"sessionID": SID}}),
    )
    .await;
    assert_eq!(out["source"], "live", "{out}");
    assert_eq!(out["data"]["title"], "t");
    assert!(out.get("warning").is_none());
    let c = calls_of(&r, "session.get");
    assert_eq!(c.len(), 1);
    assert_eq!(
        c[0].params,
        vec![("sessionID".to_string(), SID.to_string())]
    );
    assert_eq!(
        ops(&r),
        vec!["session.get"],
        "a live GET makes no extra call"
    );
}

#[tokio::test]
async fn hatch_mutation_needs_flag() {
    let r = rig(|f| {
        with_info(f);
        f.on("session.compact", vec![Ok(json!({"data": {"ok": 1}}))]);
    })
    .await;
    let args = json!({"operation": "session.compact", "params": {"sessionID": SID}});
    let out = call(&r.client, "opencode_api", args).await;
    assert_eq!(code(&out), "MUTATION_BLOCKED", "{out}");
    assert!(calls_of(&r, "session.compact").is_empty());

    let args = json!({"operation": "session.compact", "params": {"sessionID": SID}, "allow_mutation": true});
    let out = call(&r.client, "opencode_api", args).await;
    assert!(out.get("error").is_none(), "{out}");
    assert_eq!(calls_of(&r, "session.compact").len(), 1);
}

#[tokio::test]
async fn hatch_non_allowlisted_mutation_blocked_with_flag() {
    let r = rig(with_info).await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.update", "params": {"sessionID": SID}, "body": {}, "allow_mutation": true}),
    )
    .await;
    assert_eq!(code(&out), "BLOCKED_OPERATION", "{out}");
    assert_eq!(ops(&r), vec!["server.info"]);
}

#[tokio::test]
async fn hatch_204_is_ok_true() {
    let r = rig(|f| {
        with_info(f);
        f.on("session.view", vec![Ok(Value::Null)]);
    })
    .await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.view", "params": {"sessionID": SID}, "allow_mutation": true}),
    )
    .await;
    assert_eq!(out["data"], json!({"ok": true}), "{out}");
    assert_eq!(out["source"], "live");
}

#[tokio::test]
async fn hatch_never_forwards_header() {
    for key in ["-H", "header", "Header", "header[x]"] {
        let r = rig(|_| {}).await;
        let out = call(
            &r.client,
            "opencode_api",
            json!({"operation": "session.get", "params": {"sessionID": SID, key: "Authorization: x"}}),
        )
        .await;
        assert_eq!(code(&out), "INVALID_ARGUMENT", "{key}: {out}");
        assert!(r.fake.calls().is_empty(), "{key}: {:?}", r.fake.calls());
    }
}

#[tokio::test]
async fn hatch_tool_rejects_newline_param_value() {
    let r = rig(|_| {}).await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.list", "params": {"search": "a\nb"}}),
    )
    .await;
    assert_eq!(code(&out), "INVALID_ARGUMENT", "{out}");
    assert!(r.fake.calls().is_empty());
}

#[tokio::test]
async fn hatch_bad_session_id_param() {
    let r = rig(|_| {}).await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.get", "params": {"sessionID": "nope"}}),
    )
    .await;
    assert_eq!(code(&out), "SESSION_NOT_FOUND", "{out}");
    assert!(r.fake.calls().is_empty());
}

#[tokio::test]
async fn hatch_redacts_secret_keys() {
    let r = rig(|f| {
        f.on(
            "session.get",
            vec![Ok(json!({"data": {"apiKey": "sk-1", "nested": {"password": "p", "url": "http://u:pw@h/x"}, "ok": 1}}))],
        )
    })
    .await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.get", "params": {"sessionID": SID}}),
    )
    .await;
    let s = out.to_string();
    assert!(!s.contains("sk-1") && !s.contains("pw@"), "{s}");
    assert_eq!(out["data"]["apiKey"], "<redacted>");
    assert_eq!(out["data"]["ok"], 1);
}

#[tokio::test]
async fn hatch_live_openapi_wins() {
    let mut doc = fixture();
    let op = doc["paths"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .flat_map(|p| p.as_object_mut().unwrap().values_mut())
        .find(|o| o["operationId"] == "session.get")
        .unwrap();
    op["summary"] = json!("LIVE SUMMARY");
    let r = rig(|f| f.set_openapi(Some(doc))).await;
    let out = call(
        &r.client,
        "opencode_operations",
        json!({"filter": "session.get"}),
    )
    .await;
    assert_eq!(out["source"], "live", "{out}");
    let hit = out["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == "session.get")
        .unwrap();
    assert_eq!(hit["summary"], "LIVE SUMMARY");
}

#[tokio::test]
async fn hatch_snapshot_fallback_reports_source() {
    let r = rig(|f| {
        f.set_openapi(None);
        f.on("server.info", vec![Ok(info("2.1.0", 1))]);
        f.on("session.get", vec![Ok(json!({"data": {"id": SID}}))]);
    })
    .await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.get", "params": {"sessionID": SID}}),
    )
    .await;
    assert_eq!(out["source"], "snapshot(v2.0.32)", "{out}");
    assert_eq!(out["warning"], "snapshot v2.0.32 differs from server 2.1.0");
    assert_eq!(out["data"]["id"], SID);
}

#[tokio::test]
async fn hatch_snapshot_mutation_is_service_down() {
    let r = rig(|f| {
        f.set_openapi(None);
        with_info(f);
    })
    .await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.compact", "params": {"sessionID": SID}, "allow_mutation": true}),
    )
    .await;
    assert_eq!(code(&out), "SERVICE_DOWN", "{out}");
    assert!(calls_of(&r, "session.compact").is_empty());
}

#[tokio::test]
async fn hatch_cache_invalidated_on_pid_change() {
    let r = rig(|f| {
        f.on(
            "server.info",
            vec![Ok(info("2.0.32", 1)), Ok(info("2.0.32", 2))],
        );
        f.on("session.compact", vec![Ok(json!({"data": {}}))]);
    })
    .await;
    let args = json!({"operation": "session.compact", "params": {"sessionID": SID}, "allow_mutation": true});
    call(&r.client, "opencode_api", args.clone()).await;
    assert_eq!(r.fake.openapi_fetches(), 1);
    let out = call(&r.client, "opencode_api", args).await;
    assert!(out.get("error").is_none(), "{out}");
    assert_eq!(
        r.fake.openapi_fetches(),
        2,
        "a new pid drops the live table"
    );
}

#[tokio::test]
async fn hatch_server_info_reread_before_mutation() {
    let r = rig(|f| {
        with_info(f);
        f.on("session.compact", vec![Ok(json!({"data": {}}))]);
    })
    .await;
    call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.compact", "params": {"sessionID": SID}, "allow_mutation": true}),
    )
    .await;
    assert_eq!(ops(&r), vec!["server.info", "session.compact"]);
}

#[tokio::test]
async fn hatch_depth_cap_applied() {
    let deep = json!({"a": {"b": {"c": {"d": {"e": {"f": {"g": {"h": 1}}}}}}}});
    let r = rig(|f| f.on("session.get", vec![Ok(json!({"data": deep}))])).await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.get", "params": {"sessionID": SID}}),
    )
    .await;
    // data is depth 1 under the result root; the cap measures from `data` itself.
    let s = out["data"].to_string();
    assert!(s.contains("<depth>"), "{s}");
    assert!(!s.contains("\"h\""), "{s}");
}

#[tokio::test]
async fn hatch_unknown_operation_hint_three_nearest() {
    let r = rig(|_| {}).await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.gett"}),
    )
    .await;
    assert_eq!(code(&out), "UNKNOWN_OPERATION", "{out}");
    let hint = out["error"]["hint"].as_str().unwrap();
    assert!(hint.starts_with("nearest: "), "{hint}");
    assert_eq!(hint["nearest: ".len()..].split(", ").count(), 3, "{hint}");
    assert!(hint.contains("session.get"));
    assert!(r.fake.calls().is_empty());
}

#[tokio::test]
async fn hatch_method_path_form_rejected() {
    let r = rig(|_| {}).await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "GET /api/session"}),
    )
    .await;
    assert_eq!(code(&out), "INVALID_ARGUMENT", "{out}");
    assert!(r.fake.calls().is_empty());
}

#[tokio::test]
async fn hatch_params_must_be_string_map() {
    let r = rig(|_| {}).await;
    for p in [json!({"limit": 5}), json!(["a"]), json!("x")] {
        let out = call(
            &r.client,
            "opencode_api",
            json!({"operation": "session.list", "params": p}),
        )
        .await;
        assert_eq!(code(&out), "INVALID_ARGUMENT", "{out}");
    }
    assert!(r.fake.calls().is_empty());
}

#[tokio::test]
async fn hatch_switch_agent_locked() {
    let mut s = Settings::default();
    s.permissions_locked = true;
    let r = rig_with(s, with_info).await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.switchAgent", "params": {"sessionID": SID}, "body": {"agent": "x"}, "allow_mutation": true}),
    )
    .await;
    assert_eq!(code(&out), "PERMISSIONS_LOCKED", "{out}");
    assert!(calls_of(&r, "session.switchAgent").is_empty());
}

#[tokio::test]
async fn opencode_operations_shape_and_source() {
    let r = rig(|_| {}).await;
    let out = call(&r.client, "opencode_operations", json!({})).await;
    assert_eq!(out["source"], "live", "{out}");
    assert!(out.get("warning").is_none());
    let list = out["operations"].as_array().unwrap();
    assert!(!list.is_empty());
    let get = list.iter().find(|o| o["id"] == "session.get").unwrap();
    let mut k: Vec<String> = get.as_object().unwrap().keys().cloned().collect();
    k.sort();
    assert_eq!(k, ["id", "method", "mutating", "path", "summary"]);
    assert_eq!(get["method"], "GET");
    assert_eq!(get["mutating"], false);
    let compact = list.iter().find(|o| o["id"] == "session.compact").unwrap();
    assert_eq!(compact["mutating"], true);
    assert!(r.fake.calls().is_empty());

    let r = rig(|f| {
        f.set_openapi(None);
        f.on("server.info", vec![Ok(info("2.1.0", 1))]);
    })
    .await;
    let out = call(&r.client, "opencode_operations", json!({})).await;
    assert_eq!(out["source"], "snapshot(v2.0.32)", "{out}");
    assert_eq!(out["warning"], "snapshot v2.0.32 differs from server 2.1.0");
}

#[tokio::test]
async fn opencode_operations_filter_substring() {
    let r = rig(|_| {}).await;
    let out = call(
        &r.client,
        "opencode_operations",
        json!({"filter": "permission"}),
    )
    .await;
    let list = out["operations"].as_array().unwrap();
    assert!(!list.is_empty());
    for o in list {
        let hay = format!("{} {}", o["id"], o["summary"]).to_lowercase();
        assert!(hay.contains("permission"), "{o}");
    }
    let all = call(&r.client, "opencode_operations", json!({})).await;
    assert!(all["operations"].as_array().unwrap().len() > list.len());
}

// ---- path-parameter segments: a `..` value would be collapsed by the CLI's URL handling into a blocked route.

#[tokio::test]
async fn hatch_dotdot_path_param_rejected() {
    let r = rig(|f| {
        with_info(f);
        f.on("session.inbox.update", vec![Ok(json!({"data": {}}))]);
        f.on("session.inbox.cancel", vec![Ok(json!({"data": {}}))]);
        f.on("session.form.cancel", vec![Ok(json!({"data": {}}))]);
    })
    .await;
    for (op, key) in [
        ("session.inbox.update", "inboxID"),
        ("session.inbox.cancel", "inboxID"),
        ("session.form.cancel", "formID"),
    ] {
        for bad in [
            "..", ".", "a/b", "a?b", "a#b", "a%2e", "a b", "a\tb", "a\\b",
        ] {
            let out = call(
                &r.client,
                "opencode_api",
                json!({"operation": op, "allow_mutation": true,
                       "params": {"sessionID": SID, key: bad},
                       "body": {"permissions": []}}),
            )
            .await;
            assert_eq!(code(&out), "INVALID_ARGUMENT", "{op} {bad:?}: {out}");
        }
    }
    assert!(
        r.fake
            .calls()
            .iter()
            .all(|c| !c.op.starts_with("session.inbox") && !c.op.starts_with("session.form")),
        "no rejected call reaches the transport"
    );
}

#[tokio::test]
async fn hatch_real_id_shapes_still_pass() {
    let r = rig(|f| {
        with_info(f);
        f.on("session.inbox.cancel", vec![Ok(json!({"data": {}}))]);
        f.on("session.form.get", vec![Ok(json!({"data": {}}))]);
        f.on("provider.get", vec![Ok(json!({"data": {}}))]);
    })
    .await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.inbox.cancel", "allow_mutation": true,
               "params": {"sessionID": SID, "inboxID": "inb_01HZX-abc.def:1"}}),
    )
    .await;
    assert!(out.get("error").is_none(), "{out}");
    for (op, k, v) in [
        ("session.form.get", "formID", "frm_01ABCdef"),
        ("provider.get", "providerID", "github-copilot"),
    ] {
        let mut params = json!({ k: v });
        if k == "formID" {
            params["sessionID"] = json!(SID);
        }
        let out = call(
            &r.client,
            "opencode_api",
            json!({"operation": op, "params": params}),
        )
        .await;
        assert!(out.get("error").is_none(), "{op}: {out}");
    }
}

#[tokio::test]
async fn named_tools_reject_dotdot_ids() {
    let r = rig(|f| {
        f.on(
            "session.permission.list",
            vec![Ok(
                json!({"data": [{"id": "..", "action": "bash", "resources": ["*"], "save": []}]}),
            )],
        );
        f.on("session.permission.reply", vec![Ok(json!({"ok": true}))]);
        f.on("session.form.reply", vec![Ok(json!({"ok": true}))]);
        f.on("permission.saved.list", vec![Ok(json!({"data": []}))]);
        f.on("permission.saved.remove", vec![Ok(json!({"ok": true}))]);
    })
    .await;
    let out = call(
        &r.client,
        "permission_reply",
        json!({"session_id": SID, "request_id": "..", "decision": "once"}),
    )
    .await;
    assert_eq!(code(&out), "INVALID_ARGUMENT", "{out}");
    let out = call(
        &r.client,
        "form_reply",
        json!({"session_id": SID, "form_id": "..", "answer": {}}),
    )
    .await;
    assert_eq!(code(&out), "INVALID_ARGUMENT", "{out}");
    let out = call(
        &r.client,
        "saved_permissions",
        json!({"remove_id": "..", "confirm": true}),
    )
    .await;
    assert_eq!(code(&out), "INVALID_ARGUMENT", "{out}");
    for op in [
        "session.permission.reply",
        "session.form.reply",
        "permission.saved.remove",
    ] {
        assert!(calls_of(&r, op).is_empty(), "{op} must not be called");
    }
}
