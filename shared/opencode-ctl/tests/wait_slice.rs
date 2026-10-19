//! `session_wait` slice loop: wait child, stall-channel polls, deadline, errors and the result field matrix.
//! Every test runs on the default current-thread runtime with `TestClock::run_until_idle`; nothing sleeps for real.
#![allow(clippy::field_reassign_with_default)]

mod common;

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use common::*;
use opencode_ctl::clock::TestClock;
use opencode_ctl::engine::Engine;
use opencode_ctl::error::ErrorCode;
use opencode_ctl::host::Host;
use opencode_ctl::openapi::Operation;
use opencode_ctl::settings::Settings;
use opencode_ctl::testing::fake::{FakeTransport, RecordedCall};
use opencode_ctl::testing::{temp_home, FakeProbe};
use opencode_ctl::tools::SessionWaitArgs;
use opencode_ctl::transport::{Lifecycle, Params, Transport, TransportError, WaitOutcome};
use serde_json::{json, Value};

const NOW: i64 = 10_000_000;

struct Rig {
    fake: FakeTransport,
    clock: TestClock,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    _home: opencode_ctl::testing::TempHome,
}

#[derive(Clone, Copy, PartialEq)]
enum Pairing {
    Unpaired,
    PairedNoPairId,
    Paired,
}

fn data(v: Value) -> Result<Value, TransportError> {
    Ok(json!({ "data": v }))
}

fn paged(v: Value, next: Option<&str>, previous: Option<&str>) -> Result<Value, TransportError> {
    Ok(json!({ "data": v, "cursor": { "next": next, "previous": previous } }))
}

fn tx_err(code: ErrorCode) -> TransportError {
    TransportError {
        code,
        message: "scripted".into(),
        hint: None,
        details: None,
    }
}

fn ago(s: i64) -> i64 {
    NOW - s * 1000
}

fn user(id: &str, t: i64) -> Value {
    json!({"id": id, "type": "user", "text": "do it", "time": {"created": t}})
}

fn synth(id: &str, t: i64) -> Value {
    json!({"id": id, "type": "synthetic", "text": "restart notice", "time": {"created": t}})
}

fn idle(id: &str, t: i64, outcome: &str) -> Value {
    json!({"id": id, "type": "idle", "outcome": outcome, "time": {"created": t}})
}

fn asst(id: &str, t: i64) -> Value {
    json!({"id": id, "type": "assistant", "content": [], "time": {"created": t}})
}

/// Newest first, as `order=desc` returns it. Later (cursor) reads return an empty newer page.
fn base(fake: &FakeTransport, messages: Vec<Value>) {
    fake.on(
        "session.message.list",
        vec![paged(json!(messages), None, Some("prev0"))],
    );
    fake.on_matching(
        "session.message.list",
        &[("cursor", "prev0")],
        vec![paged(json!([]), None, Some("prev0"))],
    );
    fake.on("session.inbox.list", vec![data(json!([]))]);
    fake.on("session.active", vec![data(json!({}))]);
    fake.on("session.permission.list", vec![data(json!([]))]);
    fake.on("session.form.list", vec![data(json!([]))]);
    fake.on(
        "session.get",
        vec![data(json!({"id": SID, "tokens": {"input": 1}}))],
    );
    child_idle_at_once(fake);
}

fn child_idle_at_once(fake: &FakeTransport) {
    fake.on_wait(vec![(Ok(WaitOutcome::Idle), 0)]);
}

fn active(fake: &FakeTransport) {
    fake.on(
        "session.active",
        vec![data(json!({ SID: {"type": "running"} }))],
    );
}

async fn rig_full(settings: Settings, pairing: Pairing, setup: impl FnOnce(&FakeTransport)) -> Rig {
    let home = temp_home();
    let clock = TestClock::new(NOW);
    let fake = FakeTransport::new().with_clock(clock.clone());
    setup(&fake);
    let env = match pairing {
        Pairing::Unpaired => env_unpaired(&home),
        Pairing::PairedNoPairId => env_paired(&home, SID, None),
        Pairing::Paired => env_paired(&home, SID, Some("0123456789ab")),
    };
    let engine = engine(fake.clone(), clock.clone(), Host::Claude, settings, env);
    let client = client_for(engine).await;
    Rig {
        fake,
        clock,
        client,
        _home: home,
    }
}

async fn rig_with(settings: Settings, setup: impl FnOnce(&FakeTransport)) -> Rig {
    rig_full(settings, Pairing::Unpaired, setup).await
}

async fn rig(setup: impl FnOnce(&FakeTransport)) -> Rig {
    rig_with(Settings::default(), setup).await
}

/// Drive `fut` with virtual time.
async fn driven<F: Future>(clock: &TestClock, fut: F) -> F::Output {
    tokio::select! {
        out = fut => out,
        () = clock.run_until_idle() => unreachable!("run_until_idle never returns"),
    }
}

async fn run(r: &Rig, args: Value) -> Value {
    driven(&r.clock, call(&r.client, "session_wait", args)).await
}

/// Explicit anchor `msg_a`, so the anchor read is the inbox and the message pages.
async fn wait_a(r: &Rig, extra: Value) -> Value {
    let mut args = json!({"session_id": SID, "after": "msg_a"});
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        args[k] = v;
    }
    run(r, args).await
}

fn calls_of(r: &Rig, op: &str) -> Vec<RecordedCall> {
    r.fake.calls().into_iter().filter(|c| c.op == op).collect()
}

fn offsets(r: &Rig, op: &str) -> Vec<i64> {
    calls_of(r, op).iter().map(|c| c.at_ms - NOW).collect()
}

fn keys(v: &Value) -> BTreeSet<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

fn perm(id: &str) -> Value {
    json!({"id": id, "action": "bash", "resources": ["ls"]})
}

/// The always-present keys.
const BASE: [&str; 6] = [
    "state",
    "elapsed_s",
    "turn_s",
    "inbox_pending",
    "tokens",
    "last_assistant_text",
];

fn with(extra: &[&str]) -> BTreeSet<String> {
    BASE.iter()
        .chain(extra.iter())
        .map(|s| s.to_string())
        .collect()
}

fn base_a(fake: &FakeTransport) {
    base(fake, vec![user("msg_a", ago(60))]);
    active(fake);
}

// ------------------------------------------------------------------ arguments and tick schedule

#[tokio::test]
async fn wait_args_bounds() {
    let r = rig(base_a).await;
    for bad in [json!({"timeout_s": 0}), json!({"poll_s": 0})] {
        let out = wait_a(&r, bad).await;
        assert_eq!(out["error"]["code"], "INVALID_ARGUMENT", "{out}");
    }
    assert!(r.fake.calls().is_empty(), "rejected before any read");

    // timeout_s 9999 is capped at wait_max_s.
    let mut s = Settings::default();
    s.wait_max_s = 40;
    let r = rig_with(s, base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 9999})).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(out["elapsed_s"], 40, "{out}");

    // poll_s 50 with timeout_s 20: ticks at 0 and 20000 only (polls = 2).
    let r = rig(base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 20, "poll_s": 50})).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(offsets(&r, "session.permission.list"), vec![0, 20_000]);
    assert_eq!(offsets(&r, "session.form.list"), vec![0, 20_000]);
}

#[tokio::test]
async fn wait_default_timeout_120() {
    let r = rig(base_a).await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(out["elapsed_s"], 120, "{out}");
    let mut s = Settings::default();
    s.wait_timeout_s = 30;
    let r = rig_with(s, base_a).await;
    assert_eq!(wait_a(&r, json!({})).await["elapsed_s"], 30);
}

#[tokio::test]
async fn wait_polls_at_slice_start_and_end() {
    let r = rig(base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 25, "poll_s": 10})).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(
        offsets(&r, "session.permission.list"),
        vec![0, 10_000, 20_000, 25_000]
    );
    assert_eq!(
        offsets(&r, "session.form.list"),
        vec![0, 10_000, 20_000, 25_000]
    );
}

// ------------------------------------------------------------------ pending stall channels

#[tokio::test]
async fn wait_pending_poll_ends_slice_early() {
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.permission.list",
            vec![data(json!([])), data(json!([perm("per_1")]))],
        );
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 60, "poll_s": 10})).await;
    assert_eq!(out["state"], "awaiting_permission", "{out}");
    assert_eq!(out["elapsed_s"], 10, "surfaces at the second tick: {out}");
    assert_eq!(out["pending_permissions"][0]["id"], "per_1");
    assert_eq!(offsets(&r, "session.permission.list"), vec![0, 10_000]);
}

#[tokio::test]
async fn wait_awaiting_permission_shape() {
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.permission.list",
            vec![data(json!([perm("per_1")]))],
        );
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "awaiting_permission", "{out}");
    assert_eq!(keys(&out), with(&["pending_permissions"]), "{out}");
    assert_eq!(out["elapsed_s"], 0);
    assert_eq!(out["pending_permissions"][0]["action"], "bash");
}

#[tokio::test]
async fn wait_awaiting_input_fields() {
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.form.list",
            vec![data(json!([{
                "id": "frm_1",
                "title": "Name",
                "metadata": {"k": "v"},
                "fields": [{"key": "name", "type": "string", "title": "Your name", "required": true, "secret": "x"}]
            }]))],
        );
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "awaiting_input", "{out}");
    assert_eq!(keys(&out), with(&["pending_forms"]), "{out}");
    let form = &out["pending_forms"][0];
    assert_eq!(form["id"], "frm_1");
    assert_eq!(form["title"], "Name");
    assert_eq!(form["fields"][0]["key"], "name");
    assert_eq!(form["fields"][0]["required"], true);
    assert!(form["fields"][0].get("secret").is_none(), "{form}");
}

#[tokio::test]
async fn wait_permission_and_form_pending_together() {
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.permission.list",
            vec![data(json!([perm("per_1")]))],
        );
        f.on(
            "session.form.list",
            vec![data(json!([{"id": "frm_1", "title": "T", "fields": []}]))],
        );
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "awaiting_permission", "{out}");
    assert_eq!(out["pending_permissions"][0]["id"], "per_1");
    assert_eq!(out["pending_forms"][0]["id"], "frm_1");
}

#[tokio::test]
async fn wait_stall_arrays_survive_cap() {
    let mut s = Settings::default();
    s.output_max_bytes = 1024;
    let r = rig_with(s, |f| {
        base_a(f);
        let perms: Vec<Value> = (0..25)
            .map(
                |i| json!({"id": format!("per_{i}"), "action": "bash", "message": "x".repeat(300)}),
            )
            .collect();
        f.on("session.permission.list", vec![data(json!(perms))]);
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "awaiting_permission", "{out}");
    assert_eq!(out["pending_permissions"].as_array().unwrap().len(), 20);
    assert_eq!(out["more_permissions"], 5);
    assert!(out.get("more_forms").is_none());
}

#[tokio::test]
async fn wait_pair_present_when_paired_without_pair_id() {
    let r = rig_full(Settings::default(), Pairing::PairedNoPairId, base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 5})).await;
    assert_eq!(
        out["pair"],
        json!({"tui": "none", "claude": "none"}),
        "{out}"
    );
    let r = rig_full(Settings::default(), Pairing::Paired, base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 5})).await;
    assert_eq!(
        out["pair"],
        json!({"tui": "unknown", "claude": "unknown"}),
        "{out}"
    );
    let r = rig(base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 5})).await;
    assert!(out.get("pair").is_none(), "{out}");
}

// ------------------------------------------------------------------ the wait child

#[tokio::test]
async fn wait_no_spin_on_early_idle() {
    let r = rig(base_a).await;
    // Hand-drive: a child that returns at once must leave a poll_s sleeper, not restart.
    let call_fut = call(
        &r.client,
        "session_wait",
        json!({"session_id": SID, "after": "msg_a", "timeout_s": 35, "poll_s": 10}),
    );
    tokio::pin!(call_fut);
    let mut settled = false;
    for _ in 0..20_000 {
        tokio::select! {
            biased;
            _ = &mut call_fut => panic!("finished without virtual time"),
            () = tokio::task::yield_now() => {}
        }
        if r.fake.wait_calls().len() == 1 && r.clock.pending_sleepers() >= 1 {
            settled = true;
            break;
        }
    }
    assert!(settled, "first child never parked on the poll_s sleep");
    for _ in 0..300 {
        tokio::select! {
            biased;
            _ = &mut call_fut => panic!("finished without virtual time"),
            () = tokio::task::yield_now() => {}
        }
    }
    assert_eq!(r.fake.wait_calls().len(), 1, "no spin: still one child");
    let out = driven(&r.clock, call_fut).await;
    assert_eq!(out["state"], "running", "{out}");
    let at: Vec<i64> = r.fake.wait_calls().iter().map(|c| c.2).collect();
    assert!(at.len() >= 3, "{at:?}");
    for w in at.windows(2) {
        assert!(w[1] - w[0] >= 10_000, "spacing >= poll_s: {at:?}");
    }
}

#[tokio::test]
async fn wait_any_task_error_ends_slice() {
    let r = rig(|f| {
        base_a(f);
        f.on_wait(vec![(Err(tx_err(ErrorCode::AuthFailed)), 0)]);
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["error"]["code"], "AUTH_FAILED", "{out}");
    assert_eq!(
        out["elapsed_s"].as_i64(),
        None,
        "an error carries no state: {out}"
    );

    // An error from a read after a child return ends the slice too.
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.inbox.list",
            vec![data(json!([])), Err(tx_err(ErrorCode::ServiceDown))],
        );
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["error"]["code"], "SERVICE_DOWN", "{out}");
}

#[tokio::test]
async fn wait_one_failed_poll_is_retried() {
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.permission.list",
            vec![Err(tx_err(ErrorCode::ServiceDown)), data(json!([]))],
        );
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 25, "poll_s": 10})).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(
        offsets(&r, "session.permission.list"),
        vec![0, 10_000, 20_000, 25_000]
    );
}

#[tokio::test]
async fn wait_two_failed_polls_end_slice() {
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.permission.list",
            vec![Err(tx_err(ErrorCode::ServiceDown))],
        );
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 60, "poll_s": 10})).await;
    assert_eq!(out["error"]["code"], "SERVICE_DOWN", "{out}");
    assert_eq!(offsets(&r, "session.permission.list"), vec![0, 10_000]);

    // Either list failing counts, and the deadline tick is a tick: 20 s fails, 25 s fails again.
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.form.list",
            vec![
                data(json!([])),
                data(json!([])),
                Err(tx_err(ErrorCode::AuthFailed)),
            ],
        );
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 25, "poll_s": 10})).await;
    assert_eq!(out["error"]["code"], "AUTH_FAILED", "{out}");
    assert_eq!(
        offsets(&r, "session.form.list"),
        vec![0, 10_000, 20_000, 25_000]
    );
}

#[tokio::test]
async fn wait_missing_wait_route_polls_only() {
    let mut doc: Value =
        serde_json::from_str(include_str!("fixtures/openapi.json")).expect("fixture");
    let paths = doc["paths"].as_object_mut().unwrap();
    let mut emptied = Vec::new();
    for (p, item) in paths.iter_mut() {
        let item = item.as_object_mut().unwrap();
        item.retain(|_, op| op["operationId"] != "experimental.session.wait");
        if item.is_empty() {
            emptied.push(p.clone());
        }
    }
    for p in emptied {
        paths.remove(&p);
    }
    let r = rig(|f| {
        base_a(f);
        f.set_openapi(Some(doc.clone()));
        f.on(
            "session.permission.list",
            vec![data(json!([])), data(json!([perm("per_1")]))],
        );
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 60, "poll_s": 10})).await;
    assert!(r.fake.wait_calls().is_empty(), "no wait child");
    assert_eq!(out["state"], "awaiting_permission", "{out}");
    assert_eq!(out["elapsed_s"], 10, "{out}");

    // Without a permission the poll alone carries the slice to its deadline.
    let r = rig(|f| {
        base_a(f);
        f.set_openapi(Some(doc.clone()));
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 25, "poll_s": 10})).await;
    assert!(r.fake.wait_calls().is_empty());
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(out["elapsed_s"], 25);
}

#[tokio::test]
async fn wait_restart_notice_then_running() {
    let r = rig(|f| {
        base(f, vec![synth("msg_s", ago(5)), user("msg_a", ago(60))]);
        active(f);
        f.on_wait(vec![(Err(tx_err(ErrorCode::ServiceDown)), 0)]);
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 30})).await;
    assert_eq!(out["error"]["code"], "SERVICE_DOWN", "{out}");
    // The next slice: the server resumed the turn; a synthetic restart notice is no marker.
    child_idle_at_once(&r.fake);
    let out = wait_a(&r, json!({"timeout_s": 30})).await;
    assert_eq!(out["state"], "running", "{out}");
    assert!(out.get("outcome").is_none());
}

#[tokio::test]
async fn wait_verify_hint_on_idle() {
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "succeeded"), user("msg_a", ago(60))],
        )
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "idle", "{out}");
    assert_eq!(
        out["verify_hint"],
        json!({"tool": "session_changes", "args": {"session_id": SID, "from": "msg_a"}}),
        "{out}"
    );
    assert_eq!(out["elapsed_s"], 0);
}

// ------------------------------------------------------------------ states at the deadline and field matrix

#[tokio::test]
async fn wait_hard_timeout_state() {
    let r = rig(base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 20, "hard": true})).await;
    assert_eq!(out["state"], "timeout", "{out}");
    assert_eq!(out["elapsed_s"], 20);
    // `hard` never overrides a real answer.
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "succeeded"), user("msg_a", ago(60))],
        )
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 20, "hard": true})).await;
    assert_eq!(out["state"], "idle", "{out}");
}

#[tokio::test]
async fn wait_result_field_matrix() {
    // idle
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "succeeded"), user("msg_a", ago(60))],
        )
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "idle");
    assert_eq!(
        keys(&out),
        with(&["outcome", "verify_hint", "concurrent_turn"]),
        "idle: {out}"
    );

    // failed
    let r = rig(|f| {
        let mut m = idle("msg_i", ago(10), "failed");
        m["error"] = json!({"name": "ProviderError", "message": "boom"});
        base(f, vec![m, user("msg_a", ago(60))])
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "failed");
    assert_eq!(
        keys(&out),
        with(&["outcome", "concurrent_turn", "error"]),
        "failed: {out}"
    );

    // interrupted
    let r = rig(|f| {
        base(
            f,
            vec![
                idle("msg_i", ago(10), "interrupted"),
                user("msg_a", ago(60)),
            ],
        )
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "interrupted");
    assert_eq!(
        keys(&out),
        with(&["outcome", "concurrent_turn"]),
        "interrupted: {out}"
    );

    // running (the slice deadline)
    let r = rig(base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 20})).await;
    assert_eq!(out["state"], "running");
    assert_eq!(keys(&out), with(&[]), "running: {out}");

    // awaiting_permission
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.permission.list",
            vec![data(json!([perm("per_1")]))],
        );
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "awaiting_permission");
    assert_eq!(
        keys(&out),
        with(&["pending_permissions"]),
        "permission: {out}"
    );

    // awaiting_input
    let r = rig(|f| {
        base_a(f);
        f.on(
            "session.form.list",
            vec![data(json!([{"id": "frm_1", "title": "T", "fields": []}]))],
        );
    })
    .await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "awaiting_input");
    assert_eq!(keys(&out), with(&["pending_forms"]), "input: {out}");

    // stalled: not active, newest message 60 s old, stall_s 90 -> after 30 s
    let r = rig(|f| base(f, vec![user("msg_a", ago(60))])).await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "stalled", "{out}");
    assert_eq!(keys(&out), with(&[]), "stalled: {out}");

    // budget_exceeded
    let mut s = Settings::default();
    s.max_turn_s = 65;
    let r = rig_with(s, base_a).await;
    let out = wait_a(&r, json!({})).await;
    assert_eq!(out["state"], "budget_exceeded", "{out}");
    assert_eq!(keys(&out), with(&[]), "budget: {out}");

    // timeout
    let r = rig(base_a).await;
    let out = wait_a(&r, json!({"timeout_s": 20, "hard": true})).await;
    assert_eq!(out["state"], "timeout");
    assert_eq!(keys(&out), with(&[]), "timeout: {out}");

    // parked inbox adds parked_s; pairing adds pair
    let r = rig_full(Settings::default(), Pairing::Paired, |f| {
        base(f, vec![user("msg_0", ago(200))]);
        active(f);
        f.on(
            "session.inbox.list",
            vec![data(
                json!([{"id": "ibx_a", "type": "user", "time": {"created": ago(30)}}]),
            )],
        );
    })
    .await;
    let out = run(
        &r,
        json!({"session_id": SID, "after": "ibx_a", "timeout_s": 5}),
    )
    .await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(
        keys(&out),
        with(&["parked_s", "pair"]),
        "parked+paired: {out}"
    );
}

// ------------------------------------------------------------------ transport-call bound

#[tokio::test]
async fn wait_spawn_bound_per_slice() {
    // 120 s slice, poll 10, four children of 30 s virtual, one page.
    let r = rig(|f| {
        base_a(f);
        f.on_wait(vec![(Ok(WaitOutcome::DeadlineHit), 30_000)]);
    })
    .await;
    let out = run(
        &r,
        json!({"session_id": SID, "timeout_s": 120, "poll_s": 10}),
    )
    .await;
    assert_eq!(out["state"], "running", "{out}");
    let children = r.fake.wait_calls().len();
    assert_eq!(children, 4);
    let pages = 1;
    let polls = 1 + (120 - 1) / 10 + 1;
    assert_eq!(polls, 13);
    let calls = r.fake.calls().len();
    assert!(
        calls <= 2 + 2 * polls + 4 * children + 3 + pages,
        "{calls} calls"
    );
    assert_eq!(
        r.fake.wait_calls().iter().map(|c| c.1).collect::<Vec<_>>(),
        vec![30_000; 4],
        "each child waits min(remaining, 30 s)"
    );

    // The parked-item worst case: children that answer at once, a poll_s sleep between them.
    let r = rig(base_a).await;
    let out = run(
        &r,
        json!({"session_id": SID, "timeout_s": 120, "poll_s": 10}),
    )
    .await;
    assert_eq!(out["state"], "running", "{out}");
    let children = r.fake.wait_calls().len();
    assert_eq!(children, 12);
    let calls = r.fake.calls().len();
    assert!(
        calls <= 2 + 2 * polls + 4 * children + 3 + pages,
        "{calls} calls"
    );
    assert!(calls <= 80, "{calls} calls");
}

#[tokio::test]
async fn wait_child_deadline_is_min_of_remaining_and_30s() {
    let r = rig(|f| {
        base_a(f);
        f.on_wait(vec![(Ok(WaitOutcome::DeadlineHit), 30_000)]);
    })
    .await;
    let out = wait_a(&r, json!({"timeout_s": 45})).await;
    assert_eq!(out["state"], "running", "{out}");
    let w = r.fake.wait_calls();
    assert_eq!(w.len(), 2, "{w:?}");
    assert_eq!((w[0].1, w[1].1), (30_000, 15_000));
}

// ------------------------------------------------------------------ the unknown-anchor stall (ruling 10)

#[tokio::test]
async fn wait_unknown_after_on_empty_session_stalls_after_stall_s() {
    let r = rig(|f| base(f, vec![])).await;
    let out = run(
        &r,
        json!({"session_id": SID, "after": "msg_unknown", "timeout_s": 120}),
    )
    .await;
    assert_eq!(out["state"], "stalled", "{out}");
    assert_eq!(out["elapsed_s"], 90, "stall_s after the call start: {out}");
    // Not active, and it never stalls early.
    let r = rig(|f| base(f, vec![])).await;
    let out = run(
        &r,
        json!({"session_id": SID, "after": "msg_unknown", "timeout_s": 60}),
    )
    .await;
    assert_eq!(out["state"], "running", "{out}");
}

// ------------------------------------------------------------------ a first read cut by the deadline

/// Every call advances the virtual clock, so a long history costs slice time.
struct Slow {
    inner: FakeTransport,
    clock: TestClock,
    per_call_ms: i64,
}

impl Transport for Slow {
    async fn call(
        &self,
        op: &Operation,
        params: &Params,
        body: Option<&Value>,
        deadline: Duration,
    ) -> Result<Value, TransportError> {
        self.clock.advance(self.per_call_ms);
        self.inner.call(op, params, body, deadline).await
    }

    async fn lifecycle(&self, verb: Lifecycle) -> Result<String, TransportError> {
        self.inner.lifecycle(verb).await
    }

    async fn wait_idle(
        &self,
        session_id: &str,
        deadline: Duration,
    ) -> Result<WaitOutcome, TransportError> {
        self.inner.wait_idle(session_id, deadline).await
    }

    async fn fetch_openapi(&self) -> Result<Value, TransportError> {
        self.inner.fetch_openapi().await
    }

    async fn cli_version(&self) -> Result<String, TransportError> {
        self.inner.cli_version().await
    }
}

#[tokio::test]
async fn wait_search_cut_by_deadline_is_running() {
    let home = temp_home();
    let clock = TestClock::new(NOW);
    let fake = FakeTransport::new().with_clock(clock.clone());
    active(&fake);
    fake.on("session.inbox.list", vec![data(json!([]))]);
    fake.on("session.permission.list", vec![data(json!([]))]);
    fake.on("session.form.list", vec![data(json!([]))]);
    fake.on(
        "session.get",
        vec![data(json!({"id": SID, "tokens": {"input": 1}}))],
    );
    child_idle_at_once(&fake);
    // Anchor lookup, then a four-page history with the anchor on the last page.
    fake.on_matching(
        "session.message.list",
        &[("type", "user")],
        vec![paged(json!([user("msg_a", ago(600))]), None, None)],
    );
    fake.on_matching(
        "session.message.list",
        &[("order", "desc")],
        vec![paged(
            json!([asst("msg_p1", ago(5))]),
            Some("c2"),
            Some("prev1"),
        )],
    );
    fake.on_matching(
        "session.message.list",
        &[("cursor", "c2")],
        vec![paged(json!([asst("msg_p2", ago(10))]), Some("c3"), None)],
    );
    fake.on_matching(
        "session.message.list",
        &[("cursor", "c3")],
        vec![paged(json!([asst("msg_p3", ago(15))]), Some("c4"), None)],
    );
    fake.on_matching(
        "session.message.list",
        &[("cursor", "c4")],
        vec![paged(json!([user("msg_a", ago(600))]), None, None)],
    );
    let slow = Slow {
        inner: fake.clone(),
        clock: clock.clone(),
        per_call_ms: 10_000,
    };
    let engine = Arc::new(Engine::new(
        Settings::default(),
        Vec::new(),
        Host::Claude,
        env_unpaired(&home),
        slow,
        clock.clone(),
        Arc::new(FakeProbe::new("boot")),
    ));
    let out = driven(
        &clock,
        engine.session_wait(SessionWaitArgs {
            session_id: Some(SID.into()),
            after: None,
            timeout_s: Some(25),
            poll_s: None,
            hard: None,
        }),
    )
    .await
    .expect("a cut search is a result, not an error");
    assert_eq!(out["state"], "running", "{out}");
    let pages = calls_of_fake(&fake, "session.message.list");
    assert!(
        pages < 5,
        "the search stopped at the deadline: {pages} reads"
    );
}

fn calls_of_fake(fake: &FakeTransport, op: &str) -> usize {
    fake.calls().iter().filter(|c| c.op == op).count()
}

#[tokio::test]
async fn wait_search_cut_result_has_running_key_set() {
    for hard in [false, true] {
        let home = temp_home();
        let clock = TestClock::new(NOW);
        let fake = FakeTransport::new().with_clock(clock.clone());
        active(&fake);
        fake.on("session.inbox.list", vec![data(json!([]))]);
        fake.on("session.permission.list", vec![data(json!([]))]);
        fake.on("session.form.list", vec![data(json!([]))]);
        fake.on(
            "session.get",
            vec![data(json!({"id": SID, "tokens": {"input": 1}}))],
        );
        child_idle_at_once(&fake);
        // Page 1 already holds a FAILED marker after the (known) anchor; the anchor's own page comes later.
        let mut marker = idle("msg_i", ago(5), "failed");
        marker["error"] = json!({"name": "ProviderError", "message": "boom"});
        fake.on_matching(
            "session.message.list",
            &[("type", "user"), ("order", "desc")],
            vec![paged(json!([user("msg_a", ago(600))]), None, None)],
        );
        fake.on_matching(
            "session.message.list",
            &[("order", "desc")],
            vec![paged(
                json!([user("msg_later", ago(2)), marker]),
                Some("c2"),
                Some("prev1"),
            )],
        );
        fake.on_matching(
            "session.message.list",
            &[("cursor", "c2")],
            vec![paged(json!([asst("msg_p2", ago(10))]), Some("c3"), None)],
        );
        fake.on_matching(
            "session.message.list",
            &[("cursor", "c3")],
            vec![paged(json!([user("msg_a", ago(600))]), None, None)],
        );
        let slow = Slow {
            inner: fake.clone(),
            clock: clock.clone(),
            per_call_ms: 10_000,
        };
        let engine = Arc::new(Engine::new(
            Settings::default(),
            Vec::new(),
            Host::Claude,
            env_unpaired(&home),
            slow,
            clock.clone(),
            Arc::new(FakeProbe::new("boot")),
        ));
        let out = driven(
            &clock,
            engine.session_wait(SessionWaitArgs {
                session_id: Some(SID.into()),
                after: None,
                timeout_s: Some(25),
                poll_s: None,
                hard: Some(hard),
            }),
        )
        .await
        .expect("a cut search is a result, not an error");
        let want = if hard { "timeout" } else { "running" };
        assert_eq!(out["state"], want, "{out}");
        assert_eq!(keys(&out), with(&[]), "same keys as a plain {want}: {out}");
        assert!(out["elapsed_s"].as_i64().unwrap() >= 25, "{out}");
        // The stretch started at the anchor (600 s before NOW) and runs to now, not to the page-1 marker (595).
        let elapsed = out["elapsed_s"].as_i64().unwrap();
        assert_eq!(out["turn_s"], 600 + elapsed, "turn_s runs to now: {out}");
        assert_ne!(out["turn_s"], 595, "not the marker-ended value: {out}");
        assert_eq!(calls_of_fake(&fake, "session.permission.list"), 0);
        assert_eq!(calls_of_fake(&fake, "session.form.list"), 0);
        assert!(calls_of_fake(&fake, "session.message.list") < 4);
    }
}
