//! The OpenCode-host created-set guard (Task 14).

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::*;
use opencode_ctl::clock::TestClock;
use opencode_ctl::created_set::{key_for, strip_verbatim, CreatedSet, SWEEP_MAX_AGE_MS};
use opencode_ctl::host::Host;
use opencode_ctl::settings::Settings;
use opencode_ctl::testing::fake::FakeTransport;
use opencode_ctl::testing::{temp_home, TempHome};
use opencode_ctl::transport::TransportError;
use serde_json::{json, Value};

const SID3: &str = "ses_0123456789abcdefghijklmnop";

fn sess(id: &str, dir: &str) -> Value {
    json!({"id": id, "projectID":"p", "cost": 0.5,
        "tokens":{"input":1,"output":2,"reasoning":0,"cache":{"read":0,"write":0}},
        "time":{"created":1,"updated":2,"idle":3}, "title":"T",
        "location":{"directory":dir}, "agent":"build",
        "model":{"providerID":"openai","id":"gpt-5"}, "outcome":"succeeded"})
}

fn not_found() -> TransportError {
    TransportError {
        code: opencode_ctl::error::ErrorCode::SessionNotFound,
        message: "gone".into(),
        hint: None,
        details: None,
    }
}

fn service_down() -> TransportError {
    TransportError {
        code: opencode_ctl::error::ErrorCode::ServiceDown,
        message: "down".into(),
        hint: None,
        details: None,
    }
}

struct Rig {
    fake: FakeTransport,
    engine: Arc<TestEngine>,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
}

async fn rig_on(home: &TempHome, host: Host, setup: impl FnOnce(&FakeTransport)) -> Rig {
    let clock = TestClock::new(1_000);
    let fake = FakeTransport::new().with_clock(clock.clone());
    setup(&fake);
    let e = engine(
        fake.clone(),
        clock,
        host,
        Settings::default(),
        env_unpaired(home),
    );
    let client = client_for(e.clone()).await;
    Rig {
        fake,
        engine: e,
        client,
    }
}

fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

fn dir(home: &TempHome, name: &str) -> PathBuf {
    let d = home.path().join(name);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Every tool of the server, classified. Session-scoped = takes a session id.
/// Guarded = every session-scoped tool except session_list/get/messages/changes/workspace_status.
const GUARDED: [&str; 6] = [
    "session_prompt",
    "session_wait",
    "session_interrupt",
    "session_delete",
    "permission_reply",
    "form_reply",
];
const EXEMPT: [&str; 14] = [
    "service_status",
    "service_start",
    "service_stop",
    "service_restart",
    "session_list",
    "session_create",
    "session_get",
    "session_messages",
    "session_changes",
    "workspace_status",
    "saved_permissions",
    "opencode_operations",
    "opencode_api",
    "pair_status",
];

fn args_for(tool: &str, sid: &str) -> Value {
    match tool {
        "session_prompt" => json!({"session_id": sid, "text": "hi"}),
        "session_wait" => json!({"session_id": sid}),
        "session_interrupt" => json!({"session_id": sid}),
        "session_delete" => json!({"session_id": sid, "confirm": true}),
        "permission_reply" => json!({"session_id": sid, "request_id": "per_1", "decision": "once"}),
        "form_reply" => json!({"session_id": sid, "form_id": "frm_1", "answer": {}}),
        "session_get" | "session_messages" | "session_changes" => json!({"session_id": sid}),
        _ => json!({}),
    }
}

#[tokio::test]
async fn tool_classification_covers_every_tool() {
    let home = temp_home();
    let r = rig_on(&home, Host::Opencode, |_| {}).await;
    let mut listed: Vec<String> = r
        .client
        .list_all_tools()
        .await
        .unwrap()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    listed.sort();
    let mut known: Vec<String> = GUARDED
        .iter()
        .chain(EXEMPT.iter())
        .map(|s| s.to_string())
        .collect();
    known.sort();
    assert_eq!(
        listed, known,
        "a new tool must be classified guarded or exempt"
    );
}

#[tokio::test]
async fn opencode_host_refuses_foreign_session() {
    let home = temp_home();
    let r = rig_on(&home, Host::Opencode, |_| {}).await;
    for tool in GUARDED {
        let out = call(&r.client, tool, args_for(tool, SID)).await;
        assert_eq!(code(&out), "REFUSED_ON_HOST", "{tool}: {out}");
        assert_eq!(
            out["error"]["hint"], "create the worker session first",
            "{tool}: {out}"
        );
    }
    assert!(r.fake.calls().is_empty(), "{:?}", r.fake.calls());
    for tool in ["session_get", "session_messages", "session_changes"] {
        let out = call(&r.client, tool, args_for(tool, SID)).await;
        assert_ne!(code(&out), "REFUSED_ON_HOST", "{tool} is exempt: {out}");
    }
    for tool in [
        "session_list",
        "workspace_status",
        "saved_permissions",
        "opencode_operations",
        "service_status",
    ] {
        let out = call(&r.client, tool, args_for(tool, SID)).await;
        assert_ne!(code(&out), "REFUSED_ON_HOST", "{tool} is exempt: {out}");
    }
}

#[tokio::test]
async fn opencode_host_created_session_is_accepted_and_survives_restart() {
    let home = temp_home();
    let work = dir(&home, "work");
    let setup = |f: &FakeTransport| {
        f.on(
            "session.create",
            vec![Ok(json!({"data": sess(SID, work.to_str().unwrap())}))],
        );
        f.on(
            "session.interrupt",
            vec![Ok(json!({"data": {"interrupted": true}}))],
        );
    };
    let r = rig_on(&home, Host::Opencode, setup).await;
    let out = call(
        &r.client,
        "session_create",
        json!({"directory": work.to_str().unwrap()}),
    )
    .await;
    assert_eq!(out["session_id"], SID, "{out}");
    let out = call(&r.client, "session_interrupt", json!({"session_id": SID})).await;
    assert_eq!(out["interrupted"], true, "{out}");
    drop(r);
    // a brand-new engine on the same home (server restart): still accepted; others still refused
    let r2 = rig_on(&home, Host::Opencode, setup).await;
    let out = call(&r2.client, "session_interrupt", json!({"session_id": SID})).await;
    assert_eq!(out["interrupted"], true, "{out}");
    let out = call(&r2.client, "session_interrupt", json!({"session_id": SID2})).await;
    assert_eq!(code(&out), "REFUSED_ON_HOST", "{out}");
}

#[tokio::test]
async fn opencode_host_created_set_survives_restart() {
    // created in another directory than the cwd: found by scanning every file; file re-read per call
    let home = temp_home();
    let r = rig_on(&home, Host::Opencode, |f| {
        f.on(
            "session.interrupt",
            vec![Ok(json!({"data": {"interrupted": true}}))],
        );
    })
    .await;
    let out = call(&r.client, "session_interrupt", json!({"session_id": SID})).await;
    assert_eq!(code(&out), "REFUSED_ON_HOST", "{out}");
    CreatedSet::new(home.path().to_path_buf())
        .append(Path::new("/elsewhere/not/cwd"), SID)
        .unwrap();
    let out = call(&r.client, "session_interrupt", json!({"session_id": SID})).await;
    assert_eq!(out["interrupted"], true, "re-read after a refusal: {out}");
}

#[tokio::test]
async fn opencode_host_hatch_session_mutation_gated() {
    let home = temp_home();
    let r = rig_on(&home, Host::Opencode, |f| {
        f.on(
            "server.info",
            vec![Ok(
                json!({"data": {"version": "2.0.32", "pid": 1, "urls": []}}),
            )],
        );
        f.on("session.get", vec![Ok(json!({"data": sess(SID, "/w")}))]);
        f.on("session.compact", vec![Ok(json!({"data": true}))]);
    })
    .await;
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.compact", "params": {"sessionID": SID}, "allow_mutation": true}),
    )
    .await;
    assert_eq!(code(&out), "REFUSED_ON_HOST", "{out}");
    assert!(r.fake.calls().iter().all(|c| c.op != "session.compact"));
    // a read of the same foreign session stays open
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.get", "params": {"sessionID": SID}}),
    )
    .await;
    assert_ne!(code(&out), "REFUSED_ON_HOST", "{out}");
    assert!(r.fake.calls().iter().any(|c| c.op == "session.get"));
    // once created, the mutation goes through
    CreatedSet::new(home.path().to_path_buf())
        .append(Path::new("/w"), SID)
        .unwrap();
    let out = call(
        &r.client,
        "opencode_api",
        json!({"operation": "session.compact", "params": {"sessionID": SID}, "allow_mutation": true}),
    )
    .await;
    assert_ne!(code(&out), "REFUSED_ON_HOST", "{out}");
    assert!(r.fake.calls().iter().any(|c| c.op == "session.compact"));
}

#[tokio::test]
async fn claude_host_has_no_created_guard() {
    let home = temp_home();
    let r = rig_on(&home, Host::Claude, |f| {
        f.on(
            "session.interrupt",
            vec![Ok(json!({"data": {"interrupted": true}}))],
        );
        f.on("session.create", vec![Ok(json!({"data": sess(SID, "/w")}))]);
    })
    .await;
    let out = call(&r.client, "session_interrupt", json!({"session_id": SID})).await;
    assert_eq!(out["interrupted"], true, "{out}");
    call(&r.client, "session_create", json!({})).await;
    assert!(
        std::fs::read_dir(home.path())
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with("created.")),
        "no created.* file on a non-OpenCode host"
    );
    assert!(r.engine.guard_created(SID2).is_ok());
}

#[tokio::test]
async fn created_set_append_probes_at_most_five() {
    let home = temp_home();
    let work = dir(&home, "work");
    let set = CreatedSet::new(home.path().to_path_buf());
    let stored: Vec<String> = (0..7).map(|i| format!("ses_{:0>26}", i)).collect();
    for id in &stored {
        set.append(&work, id).unwrap();
    }
    let r = rig_on(&home, Host::Opencode, |f| {
        f.on(
            "session.create",
            vec![Ok(json!({"data": sess(SID, work.to_str().unwrap())}))],
        );
        f.on("session.get", vec![Ok(json!({"data": sess(SID2, "/w")}))]);
    })
    .await;
    call(
        &r.client,
        "session_create",
        json!({"directory": work.to_str().unwrap()}),
    )
    .await;
    let probes: Vec<_> = r
        .fake
        .calls()
        .into_iter()
        .filter(|c| c.op == "session.get")
        .collect();
    assert_eq!(probes.len(), 5, "{probes:?}");
    assert!(
        probes
            .iter()
            .all(|c| c.params.iter().all(|(_, v)| v != SID)),
        "the new id is not probed"
    );
    assert_eq!(set.ids().len(), 8);
}

#[tokio::test]
async fn created_set_prune_only_on_not_found() {
    let home = temp_home();
    let work = dir(&home, "work");
    let set = CreatedSet::new(home.path().to_path_buf());
    let gone = format!("ses_gone{:0>22}", "a");
    let gone = gone.as_str();
    let down = format!("ses_down{:0>22}", "b");
    let down = down.as_str();
    let boom = format!("ses_boom{:0>22}", "c");
    let boom = boom.as_str();
    for id in [gone, down, boom] {
        set.append(&work, id).unwrap();
    }
    let r = rig_on(&home, Host::Opencode, |f| {
        f.on(
            "session.create",
            vec![Ok(json!({"data": sess(SID, work.to_str().unwrap())}))],
        );
        f.on_matching(
            "session.get",
            &[("sessionID", gone)],
            vec![Err(not_found())],
        );
        f.on_matching(
            "session.get",
            &[("sessionID", down)],
            vec![Err(service_down())],
        );
        // `boom` is unscripted: the fake answers CLI_ERROR
    })
    .await;
    call(
        &r.client,
        "session_create",
        json!({"directory": work.to_str().unwrap()}),
    )
    .await;
    let ids = set.ids();
    assert!(!ids.contains(&gone.to_string()), "{ids:?}");
    for kept in [down, boom, SID] {
        assert!(ids.contains(&kept.to_string()), "{kept} kept: {ids:?}");
    }
}

#[test]
fn created_set_merge_under_lock() {
    let home = temp_home();
    let work = dir(&home, "work");
    let set = Arc::new(CreatedSet::new(home.path().to_path_buf()));
    let ids: Vec<String> = (0..8).map(|i| format!("ses_{:0>26}", i)).collect();
    let hs: Vec<_> = ids
        .iter()
        .cloned()
        .map(|id| {
            let (set, work) = (set.clone(), work.clone());
            std::thread::spawn(move || set.append(&work, &id).unwrap())
        })
        .collect();
    hs.into_iter().for_each(|h| h.join().unwrap());
    let mut got = set.ids();
    got.sort();
    assert_eq!(got, ids);
}

#[test]
fn created_set_prune_locks() {
    let home = temp_home();
    let work = dir(&home, "work");
    let set = Arc::new(CreatedSet::new(home.path().to_path_buf()));
    let old: Vec<String> = (0..4).map(|i| format!("ses_old{:0>23}", i)).collect();
    let new: Vec<String> = (0..4).map(|i| format!("ses_new{:0>23}", i)).collect();
    for id in &old {
        set.append(&work, id).unwrap();
    }
    let mut hs = Vec::new();
    for id in new.clone() {
        let (set, work) = (set.clone(), work.clone());
        hs.push(std::thread::spawn(move || set.append(&work, &id).unwrap()));
    }
    for id in old.clone() {
        let set = set.clone();
        hs.push(std::thread::spawn(move || set.prune(&id).unwrap()));
    }
    hs.into_iter().for_each(|h| h.join().unwrap());
    let mut got = set.ids();
    got.sort();
    assert_eq!(got, new);
}

#[test]
fn created_set_canonical_key() {
    let home = temp_home();
    let a = dir(&home, "a");
    let k = key_for(&a);
    assert_eq!(k.len(), 12);
    assert!(k
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(key_for(&a.join(".").join("..").join("a")), k);
    // not existing: lexically normalised
    let ghost = home.path().join("ghost").join("x");
    assert_eq!(
        key_for(&home.path().join("ghost").join("y").join("..").join("x")),
        key_for(&ghost)
    );
    assert_ne!(key_for(&ghost), k);
    #[cfg(unix)]
    {
        let link = home.path().join("link");
        std::os::unix::fs::symlink(&a, &link).unwrap();
        assert_eq!(key_for(&link), k);
    }
    assert_eq!(strip_verbatim(r"\\?\C:\work\x"), r"C:\work\x");
    assert_eq!(strip_verbatim(r"\\?\UNC\host\share\x"), r"\\host\share\x");
    assert_eq!(strip_verbatim("/plain/path"), "/plain/path");
}

#[test]
fn created_set_sweep_30_days() {
    let home = temp_home();
    let set = CreatedSet::new(home.path().to_path_buf());
    let (old_dir, fresh_dir) = (dir(&home, "old"), dir(&home, "fresh"));
    set.append(&old_dir, SID).unwrap();
    set.append(&fresh_dir, SID2).unwrap();
    let data = |d: &Path| home.path().join(format!("created.{}.json", key_for(d)));
    let lock = |d: &Path| home.path().join(format!("created.{}.lock", key_for(d)));
    let now = std::time::SystemTime::now();
    let ago = |days: u64| now - std::time::Duration::from_secs(days * 86_400);
    let touch = |p: PathBuf, t: std::time::SystemTime| {
        std::fs::OpenOptions::new()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    };
    touch(data(&old_dir), ago(31));
    touch(lock(&fresh_dir), ago(31)); // fresh data file, old lock: keep both
    let now_ms = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let removed = set.sweep(now_ms, SWEEP_MAX_AGE_MS);
    assert_eq!(removed, vec![data(&old_dir)]);
    assert!(!data(&old_dir).exists() && !lock(&old_dir).exists());
    assert!(data(&fresh_dir).exists() && lock(&fresh_dir).exists());
    // a lone lock is removed only when itself older than 30 days
    let lone = home.path().join("created.aaaaaaaaaaaa.lock");
    std::fs::write(&lone, b"").unwrap();
    assert!(set.sweep(now_ms, SWEEP_MAX_AGE_MS).is_empty());
    touch(lone.clone(), ago(31));
    assert_eq!(set.sweep(now_ms, SWEEP_MAX_AGE_MS), vec![lone.clone()]);
    assert!(!lone.exists());
}

#[test]
fn created_set_unparsable_and_vanished_files_are_skipped() {
    let home = temp_home();
    let set = CreatedSet::new(home.path().to_path_buf());
    std::fs::write(home.path().join("created.badbadbadbad.json"), b"{not json").unwrap();
    assert_eq!(set.contains(SID).unwrap(), None);
    let work = dir(&home, "work");
    set.append(&work, SID).unwrap();
    assert_eq!(set.contains(SID).unwrap(), Some(work.clone()));
    // an unparsable file for the SAME key reads as empty and is overwritten by the next append
    std::fs::write(
        home.path().join(format!("created.{}.json", key_for(&work))),
        b"junk",
    )
    .unwrap();
    set.append(&work, SID3).unwrap();
    assert_eq!(set.ids(), vec![SID3.to_string()]);
}
