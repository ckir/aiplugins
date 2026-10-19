//! In-process MCP protocol tests over the `FakeTransport` seam (no opencode needed).
#![allow(clippy::field_reassign_with_default)]

mod common;

use std::path::PathBuf;

use common::*;
use opencode_ctl::cli::{blank_is_unset, env_nonempty};
use opencode_ctl::clock::TestClock;
use opencode_ctl::engine::EnvSnapshot;
use opencode_ctl::error::{ErrorCode, ToolError};
use opencode_ctl::host::Host;
use opencode_ctl::settings::{PermissionPolicy, Settings};
use opencode_ctl::testing::fake::FakeTransport;
use opencode_ctl::testing::{env_map, temp_home};
use opencode_ctl::transport::{home_required, Lifecycle, TransportError};
use serde_json::{json, Value};

const TWENTY: [&str; 20] = [
    "service_status",
    "service_start",
    "service_stop",
    "service_restart",
    "session_list",
    "session_create",
    "session_get",
    "session_prompt",
    "session_wait",
    "session_messages",
    "session_changes",
    "workspace_status",
    "session_interrupt",
    "session_delete",
    "permission_reply",
    "form_reply",
    "saved_permissions",
    "opencode_operations",
    "opencode_api",
    "pair_status",
];

/// Ruling 1: the tools whose bodies land in later tasks, with a minimal valid argument object each.
/// A later task deletes its line(s) here and nowhere else.
const NOT_YET_IMPLEMENTED: [(&str, &str); 2] =
    [("session_wait", r#"{}"#), ("pair_status", r#"{}"#)];

fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

fn info_ok() -> Value {
    json!({"version":"2.0.32","pid":4242,"urls":["http://127.0.0.1:49374"],"paths":{"tmp":"/t"}})
}

fn sess(id: &str) -> Value {
    json!({"id": id, "projectID":"p", "cost": 0.5,
        "tokens":{"input":1,"output":2,"reasoning":0,"cache":{"read":0,"write":0}},
        "time":{"created":1,"updated":2,"idle":3}, "title":"T",
        "location":{"directory":"/w"}, "agent":"build",
        "model":{"providerID":"openai","id":"gpt-5"}, "outcome":"succeeded"})
}

struct Rig {
    fake: FakeTransport,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    _home: opencode_ctl::testing::TempHome,
}

async fn rig(
    host: Host,
    settings: Settings,
    paired: bool,
    setup: impl FnOnce(&FakeTransport),
) -> Rig {
    let home = temp_home();
    let fake = FakeTransport::new();
    setup(&fake);
    let env = if paired {
        env_paired(&home, SID, None)
    } else {
        env_unpaired(&home)
    };
    let e = engine(fake.clone(), TestClock::new(1_000), host, settings, env);
    let client = client_for(e).await;
    Rig {
        fake,
        client,
        _home: home,
    }
}

async fn plain(setup: impl FnOnce(&FakeTransport)) -> Rig {
    rig(Host::Claude, Settings::default(), false, setup).await
}

#[tokio::test]
async fn serve_handshakes_in_process_without_opencode() {
    let r = plain(|_| {}).await;
    let tools = r.client.list_all_tools().await.expect("list_tools");
    let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    names.sort();
    let mut want: Vec<String> = TWENTY.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(names, want);

    let out = call(&r.client, "service_status", json!({})).await;
    assert!(out.get("error").is_some(), "{out}");

    for (name, args) in NOT_YET_IMPLEMENTED {
        let out = call(&r.client, name, serde_json::from_str(args).unwrap()).await;
        assert_eq!(code(&out), "CLI_ERROR", "{name}: {out}");
        assert!(
            out["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not implemented"),
            "{name}: {out}"
        );
    }
}

#[tokio::test]
async fn tools_declare_token_cost() {
    let r = plain(|_| {}).await;
    let tools = r.client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 20);
    for t in &tools {
        let d = t.description.as_deref().unwrap_or("");
        let want = match &*t.name {
            "session_prompt" => "Spends worker model tokens: yes.",
            "opencode_api" => "Spends worker model tokens: depends.",
            _ => "Spends worker model tokens: no.",
        };
        assert!(d.ends_with(want), "{}: {d}", t.name);
    }
    let table = opencode_ctl::server::tool_descriptions();
    assert_eq!(table.len(), 20);
    for (n, d) in table {
        assert!(TWENTY.contains(&n));
        assert!(d.contains("Spends worker model tokens: "));
    }
    let stop = tools.iter().find(|t| t.name == "service_stop").unwrap();
    assert!(stop
        .description
        .as_deref()
        .unwrap()
        .contains("stops the daemon for EVERY session on this machine; any later session tool restarts it; to stop one turn use session_interrupt"));
}

#[tokio::test]
async fn confirm_required_on_destructive() {
    let r = plain(|_| {}).await;
    for (tool, args) in [
        ("service_stop", json!({"confirm": false})),
        ("service_restart", json!({})),
        (
            "session_delete",
            json!({"session_id": SID, "confirm": false}),
        ),
    ] {
        let out = call(&r.client, tool, args).await;
        assert_eq!(code(&out), "CONFIRM_REQUIRED", "{tool}: {out}");
    }
    assert!(r.fake.lifecycle_calls().is_empty());
    assert!(r.fake.calls().is_empty());
}

#[tokio::test]
async fn host_unknown_refuses_lifecycle() {
    for host in [Host::Unknown, Host::Opencode] {
        let r = rig(host, Settings::default(), false, |_| {}).await;
        for tool in ["service_stop", "service_restart"] {
            for confirm in [true, false] {
                let out = call(&r.client, tool, json!({"confirm": confirm})).await;
                assert_eq!(code(&out), "REFUSED_ON_HOST", "{host:?} {tool}: {out}");
            }
        }
        assert!(r.fake.lifecycle_calls().is_empty());
    }
}

#[tokio::test]
async fn session_create_agent_locked() {
    let mut s = Settings::default();
    s.permissions_locked = true;
    let r = rig(Host::Claude, s, false, |f| {
        f.on("session.create", vec![Ok(json!({"data": sess(SID)}))])
    })
    .await;
    let out = call(&r.client, "session_create", json!({"agent": "plan"})).await;
    assert_eq!(code(&out), "PERMISSIONS_LOCKED", "{out}");
    let out = call(
        &r.client,
        "session_create",
        json!({"permissions": [{"action":"*","resource":"*","effect":"allow"}]}),
    )
    .await;
    assert_eq!(code(&out), "PERMISSIONS_LOCKED", "{out}");
    assert!(r.fake.calls().iter().all(|c| c.op != "session.create"));
}

#[tokio::test]
async fn session_create_policy_expansion() {
    for (policy, effect) in [
        (PermissionPolicy::Allow, Some("allow")),
        (PermissionPolicy::Ask, Some("ask")),
        (PermissionPolicy::Deny, Some("deny")),
        (PermissionPolicy::Inherit, None),
    ] {
        let mut s = Settings::default();
        s.permission_policy = policy;
        let r = rig(Host::Claude, s, false, |f| {
            f.on("session.create", vec![Ok(json!({"data": sess(SID)}))])
        })
        .await;
        let out = call(&r.client, "session_create", json!({})).await;
        assert!(out.get("error").is_none(), "{out}");
        let body = r.fake.calls().last().unwrap().body.clone().unwrap();
        match effect {
            Some(e) => assert_eq!(
                body["permissions"],
                json!([{"action":"*","resource":"*","effect":e}])
            ),
            None => assert!(body.get("permissions").is_none(), "{body}"),
        }
    }
}

#[tokio::test]
async fn session_create_model_ref_split() {
    let r = plain(|f| f.on("session.create", vec![Ok(json!({"data": sess(SID)}))])).await;
    let cases = [
        (
            "openai/gpt-5#high",
            json!({"providerID":"openai","id":"gpt-5","variant":"high"}),
        ),
        ("openai/gpt-5", json!({"providerID":"openai","id":"gpt-5"})),
        (
            "openrouter/anthropic/claude#x",
            json!({"providerID":"openrouter","id":"anthropic/claude","variant":"x"}),
        ),
    ];
    for (m, want) in cases {
        let out = call(&r.client, "session_create", json!({"model": m})).await;
        assert!(out.get("error").is_none(), "{out}");
        let body = r.fake.calls().last().unwrap().body.clone().unwrap();
        assert_eq!(body["model"], want, "{m}");
    }
    let before = r.fake.calls().len();
    for bad in ["gpt-5", "/x", "a/", "a/b#"] {
        let out = call(&r.client, "session_create", json!({"model": bad})).await;
        assert_eq!(code(&out), "INVALID_ARGUMENT", "{bad}: {out}");
    }
    assert_eq!(r.fake.calls().len(), before);
}

#[tokio::test]
async fn session_create_directory_defaults() {
    let home = temp_home();
    let cwd_env = env_unpaired(&home);
    let cwd = cwd_env.cwd.clone();
    let mut with_dir = env_unpaired(&home);
    with_dir.directory = Some(PathBuf::from("/paired/dir"));
    for (env, want) in [
        (cwd_env, cwd.to_string_lossy().to_string()),
        (with_dir, "/paired/dir".to_string()),
    ] {
        let fake = FakeTransport::new();
        fake.on("session.create", vec![Ok(json!({"data": sess(SID)}))]);
        let e = engine(
            fake.clone(),
            TestClock::new(0),
            Host::Claude,
            Settings::default(),
            env,
        );
        let client = client_for(e).await;
        let out = call(&client, "session_create", json!({})).await;
        assert!(out.get("error").is_none(), "{out}");
        let body = fake.calls().last().unwrap().body.clone().unwrap();
        assert_eq!(body["location"]["directory"], json!(want));
        let out = call(&client, "session_create", json!({"directory": "/x/y"})).await;
        assert!(out.get("error").is_none(), "{out}");
        let body = fake.calls().last().unwrap().body.clone().unwrap();
        assert_eq!(body["location"]["directory"], json!("/x/y"));
    }
}

#[tokio::test]
async fn session_create_defaults_from_settings() {
    let mut s = Settings::default();
    s.default_model = Some("openai/gpt-5#high".into());
    s.default_agent = Some("build".into());
    let r = rig(Host::Claude, s, false, |f| {
        f.on("session.create", vec![Ok(json!({"data": sess(SID)}))])
    })
    .await;
    let out = call(
        &r.client,
        "session_create",
        json!({"parent_id": SID2, "title": "t"}),
    )
    .await;
    assert!(out.get("error").is_none(), "{out}");
    assert_eq!(out["session_id"], json!(SID));
    assert_eq!(out["directory"], json!("/w"));
    let body = r.fake.calls().last().unwrap().body.clone().unwrap();
    assert_eq!(body["agent"], json!("build"));
    assert_eq!(body["parentID"], json!(SID2));
    assert_eq!(body["title"], json!("t"));
    assert_eq!(
        body["model"],
        json!({"providerID":"openai","id":"gpt-5","variant":"high"})
    );
    // a per-call permissions array passes through unlocked
    let rules = json!([{"action":"bash","resource":"*","effect":"deny"}]);
    let out = call(&r.client, "session_create", json!({"permissions": rules})).await;
    assert!(out.get("error").is_none(), "{out}");
    let body = r.fake.calls().last().unwrap().body.clone().unwrap();
    assert_eq!(body["permissions"], rules);
}

#[tokio::test]
async fn session_list_fields_exactly_route() {
    let r = plain(|f| {
        f.on(
            "session.list",
            vec![Ok(
                json!({"data": [sess(SID)], "cursor": {"previous": null, "next": "cur2"}}),
            )],
        )
    })
    .await;
    let out = call(&r.client, "session_list", json!({})).await;
    assert_eq!(out["next_cursor"], json!("cur2"));
    assert_eq!(
        out["sessions"][0],
        json!({"id": SID, "title": "T", "directory": "/w", "outcome": "succeeded",
            "updated": 2, "idle": 3,
            "tokens": {"input":1,"output":2,"reasoning":0,"cache":{"read":0,"write":0}},
            "cost": 0.5})
    );
    assert!(r.fake.calls().iter().all(|c| c.op != "session.get"));
}

#[tokio::test]
async fn session_list_all_search_limit_cursor() {
    let r = plain(|f| {
        f.on(
            "session.list",
            vec![Ok(
                json!({"data": [], "cursor": {"previous": null, "next": null}}),
            )],
        )
    })
    .await;
    let get = |r: &Rig| r.fake.calls().last().unwrap().params.clone();
    let has =
        |p: &Vec<(String, String)>, k: &str| p.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());

    let out = call(&r.client, "session_list", json!({})).await;
    assert_eq!(out["next_cursor"], Value::Null);
    let p = get(&r);
    assert_eq!(has(&p, "limit").as_deref(), Some("20"));
    assert!(has(&p, "directory").is_some());

    call(
        &r.client,
        "session_list",
        json!({"all": true, "search": "foo", "limit": 500, "cursor": "abc"}),
    )
    .await;
    let p = get(&r);
    assert_eq!(has(&p, "directory"), None);
    assert_eq!(has(&p, "search").as_deref(), Some("foo"));
    assert_eq!(has(&p, "limit").as_deref(), Some("200"));
    assert_eq!(has(&p, "cursor").as_deref(), Some("abc"));

    call(
        &r.client,
        "session_list",
        json!({"limit": 0, "directory": "/d"}),
    )
    .await;
    let p = get(&r);
    assert_eq!(has(&p, "limit").as_deref(), Some("1"));
    assert_eq!(has(&p, "directory").as_deref(), Some("/d"));
}

#[tokio::test]
async fn session_get_pending_counts() {
    let r = rig(Host::Claude, Settings::default(), true, |f| {
        f.on("session.get", vec![Ok(json!({"data": sess(SID)}))]);
        f.on(
            "session.permission.list",
            vec![Ok(json!({"data": [
                {"id":"per_1","sessionID":SID,"action":"bash","resources":["ls"],"save":["bash:*"],"source":{"x":1}},
                {"id":"per_2","sessionID":SID,"action":"edit","resources":["a"]}]}))],
        );
        f.on(
            "session.form.list",
            vec![Ok(json!({"data": [
                {"id":"frm_1","sessionID":SID,"title":"Q","fields":[{"key":"k","type":"string","title":"K","extra":1}]}]}))],
        );
    })
    .await;
    let out = call(&r.client, "session_get", json!({})).await;
    assert!(out.get("error").is_none(), "{out}");
    assert_eq!(out["id"], json!(SID));
    assert_eq!(out["directory"], json!("/w"));
    assert_eq!(out["pending_permissions"].as_array().unwrap().len(), 2);
    assert_eq!(out["pending_permissions"][0]["action"], json!("bash"));
    assert!(out["pending_permissions"][0].get("source").is_none());
    assert_eq!(out["pending_forms"].as_array().unwrap().len(), 1);
    assert_eq!(out["pending_forms"][0]["id"], json!("frm_1"));
    assert!(out["pending_forms"][0]["fields"][0].get("extra").is_none());
    let ops: Vec<String> = r.fake.calls().iter().map(|c| c.op.clone()).collect();
    assert_eq!(
        ops,
        [
            "session.get",
            "session.permission.list",
            "session.form.list"
        ]
    );
}

#[tokio::test]
async fn session_delete_never_defaults() {
    let r = rig(Host::Claude, Settings::default(), true, |f| {
        f.on("session.remove", vec![Ok(json!({"ok": true}))])
    })
    .await;
    // session_id is required: a paired session is never deleted by omission
    let out = call(&r.client, "session_delete", json!({"confirm": true})).await;
    assert!(out.get("error").is_some(), "{out}");
    assert!(r.fake.calls().is_empty());
    let out = call(
        &r.client,
        "session_delete",
        json!({"session_id": SID2, "confirm": true}),
    )
    .await;
    assert_eq!(out, json!({"deleted": true}));
    let c = r.fake.calls();
    assert_eq!(c.len(), 1);
    assert!(c[0]
        .params
        .contains(&("sessionID".to_string(), SID2.to_string())));
}

#[tokio::test]
async fn session_delete_with_confirm_returns_deleted() {
    let r = plain(|f| f.on("session.remove", vec![Ok(json!({"ok": true}))])).await;
    let out = call(
        &r.client,
        "session_delete",
        json!({"session_id": SID, "confirm": true}),
    )
    .await;
    assert_eq!(out, json!({"deleted": true}));
}

#[tokio::test]
async fn session_interrupt_resume_query_param() {
    let r = rig(Host::Claude, Settings::default(), true, |f| {
        f.on("session.interrupt", vec![Ok(json!({"interrupted": true}))])
    })
    .await;
    let out = call(&r.client, "session_interrupt", json!({"resume": true})).await;
    assert_eq!(out, json!({"interrupted": true}));
    let c = r.fake.calls();
    assert!(c[0].params.contains(&("resume".into(), "true".into())));
    assert!(c[0].params.contains(&("sessionID".into(), SID.into())));
    assert!(c[0].body.is_none());
    call(&r.client, "session_interrupt", json!({})).await;
    let c = r.fake.calls();
    assert!(c[1].params.contains(&("resume".into(), "false".into())));
}

#[tokio::test]
async fn session_interrupt_idle_noop_false() {
    let r = rig(Host::Claude, Settings::default(), true, |f| {
        f.on("session.interrupt", vec![Ok(json!({"interrupted": false}))])
    })
    .await;
    let out = call(&r.client, "session_interrupt", json!({})).await;
    assert_eq!(out, json!({"interrupted": false}));
}

#[tokio::test]
async fn no_session_without_pairing() {
    let r = plain(|_| {}).await;
    for tool in ["session_get", "session_interrupt"] {
        let out = call(&r.client, tool, json!({})).await;
        assert_eq!(code(&out), "NO_SESSION", "{tool}: {out}");
    }
    let out = call(&r.client, "session_get", json!({"session_id": "nope"})).await;
    assert_eq!(code(&out), "SESSION_NOT_FOUND", "{out}");
    assert!(out["error"]["hint"].is_string());
    assert!(r.fake.calls().is_empty());
}

#[tokio::test]
async fn redaction_applied_to_every_result() {
    let r = plain(|f| {
        let mut s = sess(SID);
        s["title"] = json!("see https://user:pw@host.example/x");
        f.on("session.list", vec![Ok(json!({"data": [s], "cursor": {}}))]);
        f.on(
            "server.info",
            vec![Ok(json!({"version":"2.0.32","pid":1,"urls":["http://u:p@127.0.0.1:1"],"paths":{"tmp":"/t"}}))],
        );
        f.on(
            "session.get",
            vec![Err(TransportError {
                code: ErrorCode::CliError,
                message: "boom at http://a:b@h/ end".into(),
                hint: Some("try http://c:d@h2/".into()),
                details: None,
            })],
        );
    })
    .await;
    let list = call(&r.client, "session_list", json!({})).await.to_string();
    assert!(
        !list.contains("pw@") && list.contains("host.example"),
        "{list}"
    );
    let st = call(&r.client, "service_status", json!({}))
        .await
        .to_string();
    assert!(!st.contains("u:p@") && st.contains("127.0.0.1"), "{st}");
    let err = call(&r.client, "session_get", json!({"session_id": SID}))
        .await
        .to_string();
    assert!(!err.contains("a:b@") && !err.contains("c:d@"), "{err}");
}

#[test]
fn env_snapshot_empty_values_are_none() {
    for blank in ["", " "] {
        let all = [
            "OPENCODE_CTL_SESSION",
            "OPENCODE_CTL_DIR",
            "OPENCODE_CTL_SERVER",
            "OPENCODE_CTL_PAIR",
            "OPENCODE_CTL_HOST",
            "OPENCODE_SERVER_PASSWORD",
        ];
        let pairs: Vec<(&str, &str)> = all.iter().map(|k| (*k, blank)).collect();
        let get = env_map(&pairs);
        let e = EnvSnapshot::from_lookup(&get, PathBuf::from("/cwd"), PathBuf::from("/home"));
        assert_eq!(e.session, None);
        assert_eq!(e.directory, None);
        assert_eq!(e.server, None);
        assert_eq!(e.pair, None);
        assert_eq!(e.host_env, None);
        assert!(!e.server_password_present);
        assert_eq!(e.cwd, PathBuf::from("/cwd"));
        assert_eq!(e.home, PathBuf::from("/home"));
    }
    let get = env_map(&[
        ("OPENCODE_CTL_SESSION", " ses_x "),
        ("OPENCODE_SERVER_PASSWORD", " pw "),
    ]);
    let e = EnvSnapshot::from_lookup(&get, PathBuf::new(), PathBuf::new());
    assert_eq!(e.session.as_deref(), Some(" ses_x "));
    assert!(e.server_password_present);
}

#[test]
fn env_nonempty_keeps_value_verbatim() {
    assert_eq!(blank_is_unset(Some(" pw ".into())).as_deref(), Some(" pw "));
    assert_eq!(blank_is_unset(Some("  ".into())), None);
    assert_eq!(blank_is_unset(Some("".into())), None);
    assert_eq!(blank_is_unset(None), None);
    assert_eq!(
        env_nonempty("OPENCODE_CTL_TEST_SURELY_UNSET_VARIABLE"),
        None
    );
}

#[test]
fn dispatch_without_home_refuses_except_version_usage_emit_skill() {
    use opencode_ctl::cli::Route;
    let sub = |n: &str| Route::Sub {
        name: n.to_string(),
        rest: vec![],
    };
    let none = env_map(&[]);
    let blank = env_map(&[("HOME", " ")]);
    for get in [&none as &dyn Fn(&str) -> Option<String>, &blank] {
        for ok in [Route::Version, Route::Usage, sub("emit-skill")] {
            assert_eq!(home_required(&ok, get).unwrap(), PathBuf::new());
        }
        for bad in [
            sub("serve"),
            sub("gc"),
            sub("doctor-json"),
            sub("install-launcher"),
            Route::Pair(vec![]),
            sub("resume"),
            sub("attach"),
        ] {
            let e: ToolError = home_required(&bad, get).unwrap_err();
            assert_eq!(e.code, ErrorCode::IoError);
            assert!(e.message.starts_with("no home directory"), "{}", e.message);
        }
    }
    let h = env_map(&[("HOME", "/h")]);
    for r in [
        Route::Version,
        sub("serve"),
        Route::Pair(vec![]),
        sub("emit-skill"),
    ] {
        assert_eq!(home_required(&r, &h).unwrap(), PathBuf::from("/h"));
    }
    let u = env_map(&[("USERPROFILE", "/u")]);
    assert_eq!(
        home_required(&sub("serve"), &u).unwrap(),
        PathBuf::from("/u")
    );
    let hu = env_map(&[("HOME", " "), ("USERPROFILE", "/u")]);
    assert_eq!(
        home_required(&sub("serve"), &hu).unwrap(),
        PathBuf::from("/u")
    );
    let both = env_map(&[("HOME", "/h"), ("USERPROFILE", "/u")]);
    assert_eq!(
        home_required(&sub("serve"), &both).unwrap(),
        PathBuf::from("/h")
    );
}

#[tokio::test]
async fn service_status_unpaired_shape() {
    let r = plain(|f| f.on("server.info", vec![Ok(info_ok())])).await;
    let out = call(&r.client, "service_status", json!({})).await;
    assert_eq!(
        out,
        json!({"running": true, "urls": ["http://127.0.0.1:49374"], "pid": 4242,
            "version": "2.0.32", "reachable": true, "paired": null})
    );

    let r = plain(|f| f.on_lifecycle(Lifecycle::Status, Ok("stopped".into()))).await;
    let out = call(&r.client, "service_status", json!({})).await;
    assert_eq!(out["running"], json!(false));
    assert_eq!(out["reachable"], json!(false));
    assert!(r.fake.calls().iter().all(|c| c.op != "server.info"));
    assert_eq!(
        r.fake.openapi_fetches(),
        0,
        "a stopped service must not be touched"
    );

    let r = plain(|f| {
        f.on(
            "server.info",
            vec![Err(TransportError {
                code: ErrorCode::ServiceDown,
                message: "down".into(),
                hint: None,
                details: None,
            })],
        )
    })
    .await;
    let out = call(&r.client, "service_status", json!({})).await;
    assert_eq!(out["reachable"], json!(false));
    assert!(out.get("error").is_none());
}

#[tokio::test]
async fn service_status_paired_shape_without_pair_id() {
    let r = rig(Host::Claude, Settings::default(), true, |f| {
        f.on("server.info", vec![Ok(info_ok())])
    })
    .await;
    let out = call(&r.client, "service_status", json!({})).await;
    let p = &out["paired"];
    assert_eq!(p["pair_id"], Value::Null);
    assert_eq!(p["session_id"], json!(SID));
    assert_eq!(p["tui"], json!("none"));
    assert_eq!(p["claude"], json!("none"));
    for k in ["directory", "server_url"] {
        assert!(p.get(k).is_some(), "{k} missing");
    }

    let home = temp_home();
    let fake = FakeTransport::new();
    fake.on("server.info", vec![Ok(info_ok())]);
    let e = engine(
        fake,
        TestClock::new(0),
        Host::Claude,
        Settings::default(),
        env_paired(&home, SID, Some("0123456789ab")),
    );
    let client = client_for(e).await;
    let out = call(&client, "service_status", json!({})).await;
    assert_eq!(out["paired"]["pair_id"], json!("0123456789ab"));
    assert_eq!(out["paired"]["tui"], json!("unknown"));
    assert_eq!(out["paired"]["claude"], json!("unknown"));
}

fn auth_failed() -> TransportError {
    TransportError {
        code: ErrorCode::AuthFailed,
        message: "401".into(),
        hint: Some("set OPENCODE_SERVER_PASSWORD".into()),
        details: None,
    }
}

#[tokio::test]
async fn service_status_propagates_auth_failed() {
    for url in [None, Some("http://127.0.0.1:9")] {
        let mut s = Settings::default();
        s.server_url = url.map(str::to_string);
        let r = rig(Host::Claude, s, false, |f| {
            f.on("server.info", vec![Err(auth_failed())])
        })
        .await;
        let out = call(&r.client, "service_status", json!({})).await;
        assert_eq!(code(&out), "AUTH_FAILED", "{out}");
        assert!(out["error"]["hint"].is_string());
    }
}

#[tokio::test]
async fn service_status_server_url_calls_info() {
    let mut s = Settings::default();
    s.server_url = Some("http://10.0.0.2:4096".into());
    let r = rig(Host::Claude, s.clone(), false, |f| {
        f.on("server.info", vec![Ok(info_ok())])
    })
    .await;
    let out = call(&r.client, "service_status", json!({})).await;
    assert_eq!(out["running"], json!(true));
    assert_eq!(out["reachable"], json!(true));
    assert!(r.fake.lifecycle_calls().is_empty());
    assert!(r.fake.calls().iter().any(|c| c.op == "server.info"));

    let r = rig(Host::Claude, s, false, |f| {
        f.on(
            "server.info",
            vec![Err(TransportError {
                code: ErrorCode::ServiceDown,
                message: "down".into(),
                hint: None,
                details: None,
            })],
        )
    })
    .await;
    let out = call(&r.client, "service_status", json!({})).await;
    assert_eq!(out["running"], json!(false));
    assert_eq!(out["reachable"], json!(false));
    assert!(r.fake.lifecycle_calls().is_empty());
}

#[tokio::test]
async fn lifecycle_verbs_with_server_url() {
    let mut s = Settings::default();
    s.server_url = Some("http://10.0.0.2:4096".into());
    let r = rig(Host::Claude, s, false, |f| {
        f.on("server.info", vec![Ok(info_ok())])
    })
    .await;
    let out = call(&r.client, "service_start", json!({})).await;
    assert_eq!(out["running"], json!(true));
    assert!(out.get("paired").is_some());
    assert!(r.fake.calls().iter().any(|c| c.op == "server.info"));
    for tool in ["service_stop", "service_restart"] {
        let out = call(&r.client, tool, json!({"confirm": true})).await;
        assert_eq!(code(&out), "REFUSED_ON_HOST", "{out}");
        assert!(
            out["error"]["message"]
                .as_str()
                .unwrap()
                .starts_with("server_url is set"),
            "{out}"
        );
    }
    assert!(r.fake.lifecycle_calls().is_empty());
}

#[tokio::test]
async fn service_start_returns_status() {
    let r = plain(|f| f.on("server.info", vec![Ok(info_ok())])).await;
    let out = call(&r.client, "service_start", json!({})).await;
    assert_eq!(out["running"], json!(true));
    assert_eq!(out["pid"], json!(4242));
    let verbs: Vec<Lifecycle> = r.fake.lifecycle_calls().iter().map(|(v, _)| *v).collect();
    assert_eq!(verbs, [Lifecycle::Start, Lifecycle::Status]);
}

#[tokio::test]
async fn service_stop_and_restart_with_confirm() {
    let r = plain(|f| f.on("server.info", vec![Ok(info_ok())])).await;
    let out = call(&r.client, "service_stop", json!({"confirm": true})).await;
    assert_eq!(out, json!({"stopped": true}));
    let out = call(&r.client, "service_restart", json!({"confirm": true})).await;
    assert_eq!(out["running"], json!(true));
    let verbs: Vec<Lifecycle> = r.fake.lifecycle_calls().iter().map(|(v, _)| *v).collect();
    assert_eq!(
        verbs,
        [Lifecycle::Stop, Lifecycle::Restart, Lifecycle::Status]
    );
}
