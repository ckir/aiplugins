//! `session_wait` core: anchor resolution, the paged marker search and the terminal-state order (one evaluation per call).
#![allow(clippy::field_reassign_with_default)]

mod common;

use common::*;
use opencode_ctl::clock::TestClock;
use opencode_ctl::error::ErrorCode;
use opencode_ctl::host::Host;
use opencode_ctl::settings::Settings;
use opencode_ctl::testing::fake::{FakeTransport, RecordedCall};
use opencode_ctl::testing::temp_home;
use opencode_ctl::tools::wait::{evaluate, Anchor, AnchorSource, PageState, Snapshot, WaitState};
use opencode_ctl::transport::{TransportError, WaitOutcome};
use serde_json::{json, Value};

const NOW: i64 = 10_000_000;

struct Rig {
    fake: FakeTransport,
    clock: TestClock,
    engine: std::sync::Arc<TestEngine>,
    client: rmcp::service::RunningService<rmcp::RoleClient, ()>,
    _home: opencode_ctl::testing::TempHome,
}

fn data(v: Value) -> Result<Value, TransportError> {
    Ok(json!({ "data": v }))
}

fn paged(v: Value, next: Option<&str>, previous: Option<&str>) -> Result<Value, TransportError> {
    Ok(json!({ "data": v, "cursor": { "next": next, "previous": previous } }))
}

fn tx_err(code: ErrorCode) -> Result<Value, TransportError> {
    Err(TransportError {
        code,
        message: "scripted".into(),
        hint: None,
        details: None,
    })
}

/// Seconds before `NOW`.
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

fn asst(id: &str, t: i64, content: Value) -> Value {
    json!({"id": id, "type": "assistant", "content": content, "time": {"created": t}})
}

fn inbox_item(id: &str, t: i64) -> Value {
    json!({"id": id, "type": "user", "time": {"created": t}})
}

/// Newest first, as `order=desc` returns it.
fn base(fake: &FakeTransport, messages: Vec<Value>) {
    fake.on(
        "session.message.list",
        vec![paged(json!(messages), None, Some("prev0"))],
    );
    fake.on("session.inbox.list", vec![data(json!([]))]);
    fake.on("session.active", vec![data(json!({}))]);
    fake.on("session.permission.list", vec![data(json!([]))]);
    fake.on("session.form.list", vec![data(json!([]))]);
    fake.on(
        "session.get",
        vec![data(json!({"id": SID, "tokens": {"input": 1, "output": 2, "reasoning": 3, "cache_read": 4}}))],
    );
    // Task 11: the slice loop starts a wait child; one that answers at once makes the first evaluation the answer.
    fake.on_wait(vec![(Ok(WaitOutcome::Idle), 0)]);
}

async fn rig_with(settings: Settings, paired: bool, setup: impl FnOnce(&FakeTransport)) -> Rig {
    let home = temp_home();
    let clock = TestClock::new(NOW);
    let fake = FakeTransport::new().with_clock(clock.clone());
    setup(&fake);
    let env = if paired {
        env_paired(&home, SID, Some("0123456789ab"))
    } else {
        env_unpaired(&home)
    };
    let engine = engine(fake.clone(), clock.clone(), Host::Claude, settings, env);
    let client = client_for(engine.clone()).await;
    Rig {
        fake,
        clock,
        engine,
        client,
        _home: home,
    }
}

async fn rig(setup: impl FnOnce(&FakeTransport)) -> Rig {
    rig_with(Settings::default(), false, setup).await
}

async fn wait(r: &Rig, after: Option<&str>) -> Value {
    // Task 11: a one-second slice keeps a `running` answer cheap on virtual time (it adds 1 s to every clock figure).
    let mut args = json!({"session_id": SID, "timeout_s": 1});
    if let Some(a) = after {
        args["after"] = json!(a);
    }
    // Task 11: the slice loop runs on virtual time.
    tokio::select! {
        out = call(&r.client, "session_wait", args) => out,
        () = r.clock.run_until_idle() => unreachable!("run_until_idle never returns"),
    }
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

fn active(fake: &FakeTransport) {
    fake.on(
        "session.active",
        vec![data(json!({ SID: {"type": "running"} }))],
    );
}

// ------------------------------------------------------------------ marker search

#[tokio::test]
async fn wait_idle_after_anchor() {
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "succeeded"), user("msg_a", ago(60))],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "idle", "{out}");
    assert_eq!(out["outcome"], "idle");
    assert_eq!(out["concurrent_turn"], Value::Null);
    assert_eq!(
        out["verify_hint"],
        json!({"tool": "session_changes", "args": {"session_id": SID, "from": "msg_a"}}),
        "Task 11 shape"
    );
    assert_eq!(out["turn_s"], 50);
    assert!(out.get("error").is_none());
}

#[tokio::test]
async fn wait_first_idle_after_anchor_not_newest() {
    let r = rig(|f| {
        base(
            f,
            vec![
                idle("msg_i2", ago(5), "failed"),
                idle("msg_i1", ago(20), "succeeded"),
                user("msg_a", ago(60)),
            ],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "idle", "{out}");
    assert_eq!(out["turn_s"], 40);
}

#[tokio::test]
async fn wait_stale_idle_before_anchor_is_running() {
    let r = rig(|f| {
        base(
            f,
            vec![
                user("msg_a", ago(50)),
                idle("msg_i0", ago(100), "succeeded"),
            ],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "running", "{out}");
    assert!(out.get("outcome").is_none());
    assert!(out.get("concurrent_turn").is_none());
}

#[tokio::test]
async fn wait_explicit_after_not_replaced_by_human_message() {
    let r = rig(|f| {
        base(
            f,
            vec![
                user("msg_h", ago(20)),
                idle("msg_i", ago(40), "succeeded"),
                user("msg_a", ago(60)),
            ],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "idle", "{out}");
    assert_eq!(out["concurrent_turn"], "msg_h");
    let list = calls_of(&r, "session.message.list");
    assert!(
        list.iter().all(|c| param(c, "type").is_none()),
        "an explicit after must not trigger the type=user anchor read"
    );
}

#[tokio::test]
async fn wait_synthetic_never_anchors() {
    let r = rig(|f| {
        base(f, vec![synth("msg_s", ago(10)), user("msg_u", ago(60))]);
        f.on_matching(
            "session.message.list",
            &[("type", "user")],
            vec![paged(json!([user("msg_u", ago(60))]), None, Some("p"))],
        );
    })
    .await;
    let out = wait(&r, None).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(out["turn_s"], 61, "60 s + the 1 s slice");
    let anchor_read = calls_of(&r, "session.message.list")
        .into_iter()
        .find(|c| param(c, "type") == Some("user"))
        .expect("type=user read");
    assert_eq!(param(&anchor_read, "order"), Some("desc"));
    assert_eq!(param(&anchor_read, "limit"), Some("1"));
}

// ------------------------------------------------------------------ inbox and parked anchors

#[tokio::test]
async fn wait_inbox_pending() {
    let r = rig(|f| {
        base(f, vec![user("msg_a", ago(30))]);
        f.on(
            "session.inbox.list",
            vec![data(json!([
                inbox_item("ibx_new", ago(50)),
                inbox_item("ibx_old", ago(120))
            ]))],
        );
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["inbox_pending"], true, "{out}");
    assert_eq!(
        out["parked_s"], 121,
        "parked_s comes from the OLDER item (+1 s slice)"
    );

    let r2 = rig(|f| base(f, vec![user("msg_a", ago(30))])).await;
    let out = wait(&r2, Some("msg_a")).await;
    assert_eq!(out["inbox_pending"], false);
    assert!(out.get("parked_s").is_none());
}

#[tokio::test]
async fn wait_parked_s_from_anchor_parked_since() {
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "succeeded"), user("msg_0", ago(200))],
        );
        f.on(
            "session.inbox.list",
            vec![data(json!([
                inbox_item("ibx_a", ago(30)),
                inbox_item("ibx_0", ago(100))
            ]))],
        );
    })
    .await;
    let out = wait(&r, Some("ibx_a")).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(out["inbox_pending"], true);
    assert_eq!(
        out["parked_s"], 31,
        "(+1 s slice) from the anchor's parked_since, not the older item"
    );
}

#[tokio::test]
async fn wait_parked_anchor_still_surfaces_permission() {
    let r = rig(|f| {
        base(f, vec![user("msg_0", ago(200))]);
        f.on(
            "session.inbox.list",
            vec![data(json!([inbox_item("ibx_a", ago(30))]))],
        );
        f.on(
            "session.permission.list",
            vec![data(
                json!([{"id": "per_1", "action": "bash", "resources": ["ls"]}]),
            )],
        );
    })
    .await;
    let out = wait(&r, Some("ibx_a")).await;
    assert_eq!(out["state"], "awaiting_permission", "{out}");
    assert_eq!(out["inbox_pending"], true);
    assert_eq!(out["parked_s"], 30);
    assert_eq!(out["pending_permissions"][0]["id"], "per_1");
}

#[tokio::test]
async fn wait_parked_budget_exceeded() {
    let r = rig(|f| {
        base(f, vec![user("msg_old", ago(2000))]);
        f.on(
            "session.inbox.list",
            vec![data(json!([inbox_item("ibx_a", ago(30))]))],
        );
    })
    .await;
    let out = wait(&r, Some("ibx_a")).await;
    assert_eq!(out["state"], "budget_exceeded", "{out}");
    assert_eq!(out["inbox_pending"], true);
    assert_eq!(out["turn_s"], 2000);
}

#[tokio::test]
async fn wait_parked_form_is_awaiting_input() {
    let r = rig(|f| {
        base(f, vec![user("msg_0", ago(200))]);
        f.on("session.inbox.list", vec![data(json!([inbox_item("ibx_a", ago(30))]))]);
        f.on(
            "session.form.list",
            vec![data(json!([{"id": "frm_1", "title": "Pick", "fields": [{"key": "k", "type": "string", "title": "K", "secret": "x"}]}]))],
        );
    })
    .await;
    let out = wait(&r, Some("ibx_a")).await;
    assert_eq!(out["state"], "awaiting_input", "{out}");
    assert_eq!(out["pending_forms"][0]["id"], "frm_1");
    assert_eq!(out["pending_forms"][0]["fields"][0]["key"], "k");
    assert!(out["pending_forms"][0]["fields"][0].get("secret").is_none());
    assert_eq!(out["inbox_pending"], true);
    assert!(out["parked_s"].is_number());
}

#[tokio::test]
async fn wait_parked_never_stalled() {
    let r = rig(|f| {
        base(f, vec![asst("msg_x", ago(500), json!([]))]);
        f.on(
            "session.inbox.list",
            vec![data(json!([inbox_item("ibx_a", ago(400))]))],
        );
    })
    .await;
    let out = wait(&r, Some("ibx_a")).await;
    assert_eq!(out["state"], "running", "{out}");
}

#[tokio::test]
async fn wait_queued_prompt_does_not_borrow_previous_idle() {
    let r = rig(|f| {
        base(
            f,
            vec![
                idle("msg_i1", ago(250), "succeeded"),
                user("msg_u1", ago(300)),
            ],
        );
        f.on(
            "session.inbox.list",
            vec![data(json!([inbox_item("ibx_q", ago(100))]))],
        );
    })
    .await;
    let out = wait(&r, Some("ibx_q")).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(
        out["turn_s"], 101,
        "(+1 s slice) the finished turn's user message is not part of this stretch"
    );

    // Promoted: the anchor is now a message created after the previous marker.
    let r = rig(|f| {
        base(
            f,
            vec![
                user("msg_q", ago(100)),
                idle("msg_i1", ago(250), "succeeded"),
                user("msg_u1", ago(300)),
            ],
        );
        active(f);
    })
    .await;
    let out = wait(&r, Some("msg_q")).await;
    assert_eq!(out["state"], "running", "{out}");
}

// ------------------------------------------------------------------ slice-end reads

#[tokio::test]
async fn wait_slice_end_active_read_error_is_error_not_stalled() {
    let r = rig(|f| {
        base(
            f,
            vec![asst("msg_x", ago(500), json!([])), user("msg_a", ago(600))],
        );
        f.on("session.active", vec![tx_err(ErrorCode::ServiceDown)]);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["error"]["code"], "SERVICE_DOWN", "{out}");
    assert!(out.get("state").is_none());
}

#[tokio::test]
async fn wait_slice_end_active_error_beats_poll_permission() {
    let r = rig(|f| {
        base(f, vec![user("msg_a", ago(60))]);
        f.on(
            "session.permission.list",
            vec![data(json!([{"id": "per_1", "action": "bash"}]))],
        );
        f.on("session.active", vec![tx_err(ErrorCode::ServiceDown)]);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["error"]["code"], "SERVICE_DOWN", "{out}");
}

#[tokio::test]
async fn wait_session_get_failure_degrades_tokens() {
    let r = rig(|f| base(f, vec![user("msg_a", ago(60))])).await;
    let with = wait(&r, Some("msg_a")).await;
    assert_eq!(
        with["tokens"],
        json!({"input": 1, "output": 2, "reasoning": 3, "cache_read": 4})
    );

    let r = rig(|f| {
        base(f, vec![user("msg_a", ago(60))]);
        f.on("session.get", vec![tx_err(ErrorCode::ServiceDown)]);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["tokens"], Value::Null, "{out}");
    assert_eq!(out["state"], with["state"]);
}

#[tokio::test]
async fn wait_last_assistant_text_is_last_text_part() {
    let r = rig(|f| {
        base(
            f,
            vec![
                asst(
                    "msg_t2",
                    ago(5),
                    json!([{"type": "tool", "name": "bash", "state": {"status": "completed"}}]),
                ),
                asst(
                    "msg_t1",
                    ago(15),
                    json!([
                        {"type": "text", "text": "first"},
                        {"type": "text", "text": "second"},
                        {"type": "reasoning", "text": "hidden thoughts"},
                        {"type": "tool", "name": "bash", "state": {"status": "completed"}}
                    ]),
                ),
                user("msg_a", ago(60)),
            ],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["last_assistant_text"], "second", "{out}");

    // A pre-anchor assistant row with text is not returned when the post-anchor row is tool-only.
    let r = rig(|f| {
        base(
            f,
            vec![
                asst(
                    "msg_t2",
                    ago(5),
                    json!([{"type": "tool", "name": "bash", "state": {"status": "running"}}]),
                ),
                user("msg_a", ago(60)),
                asst(
                    "msg_old",
                    ago(100),
                    json!([{"type": "text", "text": "stale"}]),
                ),
            ],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["last_assistant_text"], Value::Null, "{out}");
}

// ------------------------------------------------------------------ unknown anchors

#[tokio::test]
async fn wait_after_unknown_then_stalled() {
    let r = rig(|f| {
        base(
            f,
            vec![asst("msg_x", ago(30), json!([])), user("msg_u", ago(60))],
        )
    })
    .await;
    let out = wait(&r, Some("msg_nowhere")).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(
        calls_of(&r, "session.message.list").len(),
        3,
        "the first read, one incremental read for the child return and one at the slice end"
    );

    r.clock.advance(100_000);
    let out = wait(&r, Some("msg_nowhere")).await;
    assert_eq!(out["state"], "stalled", "{out}");
    assert_eq!(calls_of(&r, "session.message.list").len(), 5);
}

#[tokio::test]
async fn wait_anchor_vanished_uses_newest_message_clock() {
    // The newest message is an idle marker, but it is not AFTER the anchor (the anchor takes its time).
    let r = rig(|f| {
        base(
            f,
            vec![
                idle("msg_i", ago(5), "succeeded"),
                user("msg_u", ago(50)),
                idle("msg_i0", ago(80), "failed"),
            ],
        )
    })
    .await;
    let out = wait(&r, Some("msg_vanished")).await;
    assert_eq!(out["state"], "running", "{out}");
    assert!(out.get("outcome").is_none());
}

// ------------------------------------------------------------------ terminal outcomes

#[tokio::test]
async fn wait_failed_outcome() {
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "failed"), user("msg_a", ago(60))],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "failed", "{out}");
    assert_eq!(out["outcome"], "failed");
    assert!(
        out.get("verify_hint").is_none(),
        "verify_hint is for idle only"
    );
}

#[tokio::test]
async fn wait_failed_carries_error() {
    let marker = json!({"id": "msg_i", "type": "idle", "outcome": "failed", "time": {"created": ago(10)},
        "error": {"type": "ProviderError", "message": "boom", "status": 500}});
    let r = rig(|f| base(f, vec![marker, user("msg_a", ago(60))])).await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "failed", "{out}");
    assert_eq!(out["error"]["name"], "ProviderError");
    assert_eq!(out["error"]["message"], "boom");
}

#[tokio::test]
async fn wait_failed_marker_without_assistant() {
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "failed"), user("msg_a", ago(60))],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "failed", "{out}");
    assert!(
        out.get("error").is_none(),
        "a marker without an error reports none: {out}"
    );
}

#[tokio::test]
async fn wait_interrupted_outcome() {
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
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "interrupted", "{out}");
    assert_eq!(out["outcome"], "interrupted");
}

#[tokio::test]
async fn wait_first_marker_after_anchor_terminal_any_delivery() {
    // A queued prompt promoted at the idle boundary: its row exists, the single marker follows, no assistant row at all.
    let r = rig(|f| {
        base(
            f,
            vec![
                idle("msg_i2", ago(10), "succeeded"),
                user("msg_q", ago(50)),
                idle("msg_i1", ago(60), "succeeded"),
            ],
        )
    })
    .await;
    let out = wait(&r, Some("msg_q")).await;
    assert_eq!(out["state"], "idle", "{out}");
}

#[tokio::test]
async fn wait_concurrent_turn_flag() {
    for outcome in ["succeeded", "failed", "interrupted"] {
        let r = rig(|f| {
            base(
                f,
                vec![idle("msg_i", ago(10), outcome), user("msg_a", ago(60))],
            )
        })
        .await;
        let out = wait(&r, Some("msg_a")).await;
        assert!(
            out.as_object().unwrap().contains_key("concurrent_turn"),
            "{outcome}: {out}"
        );
        assert_eq!(out["concurrent_turn"], Value::Null);
    }
    let r = rig(|f| base(f, vec![user("msg_a", ago(60))])).await;
    let out = wait(&r, Some("msg_a")).await;
    assert!(
        !out.as_object().unwrap().contains_key("concurrent_turn"),
        "{out}"
    );
}

#[tokio::test]
async fn wait_concurrent_turn_carries_message_id() {
    let r = rig(|f| {
        base(
            f,
            vec![
                user("msg_h2", ago(5)),
                user("msg_h1", ago(8)),
                idle("msg_i", ago(10), "failed"),
                user("msg_a", ago(60)),
            ],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "failed", "{out}");
    assert_eq!(
        out["concurrent_turn"], "msg_h2",
        "the NEWEST user message after the marker"
    );
}

#[tokio::test]
async fn wait_concurrent_turn_from_messages_only() {
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "succeeded"), user("msg_a", ago(60))],
        );
        active(f);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "idle", "{out}");
    assert_eq!(
        out["concurrent_turn"],
        Value::Null,
        "session.active alone never sets it"
    );
}

#[tokio::test]
async fn wait_turn_s_ends_at_marker() {
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(40), "succeeded"), user("msg_a", ago(100))],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["turn_s"], 60, "{out}");
}

// ------------------------------------------------------------------ rules 3.2 to 3.5

#[tokio::test]
async fn wait_active_session_never_stalled() {
    let r = rig(|f| {
        base(
            f,
            vec![asst("msg_x", ago(500), json!([])), user("msg_a", ago(600))],
        );
        active(f);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "running", "{out}");
}

#[tokio::test]
async fn wait_budget_exceeded() {
    let r = rig(|f| {
        base(f, vec![user("msg_a", ago(2000))]);
        active(f);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "budget_exceeded", "{out}");
    assert_eq!(out["turn_s"], 2000);
    assert!(out.get("outcome").is_none());
}

#[tokio::test]
async fn wait_idle_beats_budget() {
    let r = rig(|f| {
        base(
            f,
            vec![
                idle("msg_i", ago(10), "succeeded"),
                user("msg_a", ago(2000)),
            ],
        );
        active(f);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "idle", "{out}");
}

#[tokio::test]
async fn wait_budget_survives_reprompt() {
    let r = rig(|f| {
        base(
            f,
            vec![
                user("msg_a", ago(50)),
                asst("msg_x", ago(900), json!([])),
                user("msg_u1", ago(1900)),
            ],
        );
        active(f);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "budget_exceeded", "{out}");
    assert_eq!(out["turn_s"], 1900);
}

#[tokio::test]
async fn wait_dead_loop_is_stalled() {
    let r = rig(|f| {
        base(
            f,
            vec![asst("msg_x", ago(200), json!([])), user("msg_a", ago(300))],
        )
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "stalled", "{out}");
}

#[tokio::test]
async fn wait_permission_and_form_precede_budget_and_stall() {
    let r = rig(|f| {
        base(f, vec![user("msg_a", ago(2000))]);
        f.on(
            "session.permission.list",
            vec![data(json!([{"id": "per_1", "action": "bash"}]))],
        );
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "awaiting_permission", "{out}");

    let r = rig(|f| {
        base(f, vec![user("msg_a", ago(2000))]);
        f.on(
            "session.form.list",
            vec![data(json!([{"id": "frm_1", "title": "T", "fields": []}]))],
        );
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "awaiting_input", "{out}");

    // A terminal marker still wins over a pending permission.
    let r = rig(|f| {
        base(
            f,
            vec![idle("msg_i", ago(10), "succeeded"), user("msg_a", ago(60))],
        );
        f.on(
            "session.permission.list",
            vec![data(json!([{"id": "per_1", "action": "bash"}]))],
        );
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "idle", "{out}");
    assert_eq!(
        out["pending_permissions"][0]["id"], "per_1",
        "arrays are present in ANY state"
    );
}

#[tokio::test]
async fn wait_pending_arrays_are_cut_at_twenty_with_more_counts() {
    let perms: Vec<Value> = (0..23)
        .map(|i| json!({"id": format!("per_{i}"), "action": "bash"}))
        .collect();
    let r = rig(|f| {
        base(f, vec![user("msg_a", ago(60))]);
        f.on("session.permission.list", vec![data(json!(perms))]);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(
        out["pending_permissions"].as_array().unwrap().len(),
        20,
        "{out}"
    );
    assert_eq!(out["more_permissions"], 3);
    assert!(out.get("more_forms").is_none());
    assert!(out.get("pending_forms").is_none());
}

#[tokio::test]
async fn wait_pair_present_only_when_paired() {
    let r = rig_with(Settings::default(), true, |f| {
        base(f, vec![user("msg_a", ago(60))])
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert!(out["pair"].is_object(), "{out}");
    assert!(out["pair"].get("tui").is_some() && out["pair"].get("claude").is_some());
    let r = rig(|f| base(f, vec![user("msg_a", ago(60))])).await;
    assert!(wait(&r, Some("msg_a")).await.get("pair").is_none());
}

// ------------------------------------------------------------------ paging

#[tokio::test]
async fn wait_idle_search_pages_to_anchor_timestamp() {
    let r = rig(|f| {
        base(f, vec![]);
        f.on_matching(
            "session.message.list",
            &[("order", "desc")],
            vec![paged(
                json!([
                    idle("msg_i", ago(10), "succeeded"),
                    asst("msg_p1", ago(20), json!([]))
                ]),
                Some("c2"),
                Some("prev1"),
            )],
        );
        f.on_matching(
            "session.message.list",
            &[("cursor", "c2")],
            vec![paged(
                json!([asst("msg_p2", ago(30), json!([]))]),
                Some("c3"),
                Some("x2"),
            )],
        );
        f.on_matching(
            "session.message.list",
            &[("cursor", "c3")],
            vec![paged(
                json!([
                    asst("msg_p3", ago(40), json!([])),
                    user("msg_a", ago(60)),
                    idle("msg_i0", ago(70), "succeeded")
                ]),
                Some("c4"),
                Some("x3"),
            )],
        );
        f.on_matching(
            "session.message.list",
            &[("cursor", "c4")],
            vec![paged(json!([user("msg_old", ago(90))]), None, Some("x4"))],
        );
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "idle", "{out}");
    let list = calls_of(&r, "session.message.list");
    // Three first-read pages (not every page) plus the one incremental read of the child return.
    assert_eq!(
        list.len(),
        4,
        "exactly three page reads + one incremental: {list:?}"
    );
    assert_eq!(param(&list[3], "cursor"), Some("prev1"));
    assert_eq!(param(&list[0], "order"), Some("desc"));
    assert_eq!(param(&list[0], "limit"), Some("200"));
    assert!(param(&list[0], "cursor").is_none());
    for c in &list[1..3] {
        assert!(
            param(c, "order").is_none(),
            "order is never sent with a cursor"
        );
        assert!(param(c, "type").is_none());
        assert!(param(c, "cursor").is_some());
    }
}

#[tokio::test]
async fn wait_search_first_read_limit_200_then_incremental() {
    let r = rig(|f| {
        base(f, vec![]);
        f.on_matching(
            "session.message.list",
            &[("order", "desc")],
            vec![paged(
                json!([asst("msg_p", ago(20), json!([])), user("msg_a", ago(60))]),
                None,
                Some("prev1"),
            )],
        );
        f.on_matching(
            "session.message.list",
            &[("cursor", "prev1")],
            vec![paged(
                json!([idle("msg_i", ago(5), "succeeded")]),
                None,
                Some("prev2"),
            )],
        );
    })
    .await;
    let anchor = Anchor {
        id: "msg_a".into(),
        created_ms: ago(60),
        source: AnchorSource::Message,
    };
    let mut state = PageState::default();
    let first = r
        .engine
        .read_messages_since(SID, &anchor, &mut state)
        .await
        .unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].id, "msg_a", "oldest first");
    assert_eq!(state.newest_cursor_previous.as_deref(), Some("prev1"));

    let later = r
        .engine
        .read_messages_since(SID, &anchor, &mut state)
        .await
        .unwrap();
    assert_eq!(later.len(), 1);
    assert_eq!(later[0].id, "msg_i");
    assert_eq!(state.newest_cursor_previous.as_deref(), Some("prev2"));

    let list = calls_of(&r, "session.message.list");
    assert_eq!(list.len(), 2);
    assert_eq!(param(&list[0], "limit"), Some("200"));
    assert_eq!(param(&list[0], "order"), Some("desc"));
    assert_eq!(param(&list[1], "cursor"), Some("prev1"));
    assert!(param(&list[1], "order").is_none() && param(&list[1], "type").is_none());

    // An empty incremental page keeps the cursor it already holds.
    r.fake.on_matching(
        "session.message.list",
        &[("cursor", "prev2")],
        vec![paged(json!([]), None, None)],
    );
    let none = r
        .engine
        .read_messages_since(SID, &anchor, &mut state)
        .await
        .unwrap();
    assert!(none.is_empty());
    assert_eq!(state.newest_cursor_previous.as_deref(), Some("prev2"));
}

// ------------------------------------------------------------------ no user message and field policy

#[tokio::test]
async fn wait_no_user_message_is_idle() {
    let r = rig(|f| {
        base(f, vec![]);
        f.on_matching(
            "session.message.list",
            &[("type", "user")],
            vec![paged(json!([]), None, None)],
        );
    })
    .await;
    let out = wait(&r, None).await;
    assert_eq!(out["state"], "idle", "{out}");
    assert_eq!(out["turn_s"], Value::Null);
    assert_eq!(out["inbox_pending"], false);
}

#[tokio::test]
async fn wait_always_present_fields_are_null_not_absent() {
    let r = rig(|f| {
        base(f, vec![user("msg_a", ago(60))]);
        f.on("session.get", vec![tx_err(ErrorCode::ServiceDown)]);
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    let o = out.as_object().unwrap();
    for k in [
        "state",
        "elapsed_s",
        "turn_s",
        "inbox_pending",
        "tokens",
        "last_assistant_text",
    ] {
        assert!(o.contains_key(k), "{k} missing: {out}");
    }
    assert_eq!(out["tokens"], Value::Null);
    assert_eq!(out["last_assistant_text"], Value::Null);
    assert_eq!(out["elapsed_s"], 1, "the 1 s slice");
    for k in [
        "outcome",
        "verify_hint",
        "concurrent_turn",
        "error",
        "parked_s",
        "pair",
        "more_permissions",
        "more_forms",
        "pending_permissions",
        "pending_forms",
    ] {
        assert!(!o.contains_key(k), "{k} must be absent: {out}");
    }

    // No user message anywhere in the snapshot of an explicit anchor: turn_s is null, not absent.
    let r = rig(|f| base(f, vec![asst("msg_x", ago(5), json!([]))])).await;
    let out = wait(&r, Some("msg_vanished")).await;
    assert_eq!(out["turn_s"], Value::Null, "{out}");
}

#[tokio::test]
async fn wait_budget_survives_reprompt_across_pages() {
    let r = rig(|f| {
        base(f, vec![]);
        active(f);
        f.on_matching(
            "session.message.list",
            &[("order", "desc")],
            vec![paged(
                json!([asst("msg_p1", ago(20), json!([])), user("msg_a", ago(50))]),
                Some("c2"),
                Some("prev1"),
            )],
        );
        f.on_matching(
            "session.message.list",
            &[("cursor", "c2")],
            vec![paged(
                json!([asst("msg_p2", ago(900), json!([]))]),
                Some("c3"),
                Some("x2"),
            )],
        );
        f.on_matching(
            "session.message.list",
            &[("cursor", "c3")],
            vec![paged(
                json!([user("msg_u1", ago(1900))]),
                Some("c4"),
                Some("x3"),
            )],
        );
        f.on_matching(
            "session.message.list",
            &[("cursor", "c4")],
            vec![paged(
                json!([idle("msg_i0", ago(2500), "succeeded")]),
                Some("c5"),
                Some("x4"),
            )],
        );
    })
    .await;
    let out = wait(&r, Some("msg_a")).await;
    assert_eq!(out["state"], "budget_exceeded", "{out}");
    assert_eq!(
        out["turn_s"], 1900,
        "measured from the original open user message"
    );
    let list = calls_of(&r, "session.message.list");
    assert_eq!(
        list.len(),
        5,
        "four pages (stops at the page holding the idle marker older than the anchor) + one incremental: {list:?}"
    );
    for c in &list[1..] {
        assert!(param(c, "order").is_none() && param(c, "type").is_none());
    }
}

#[tokio::test]
async fn wait_after_unknown_empty_list_stalls_after_stall_s() {
    let settings = Settings::default();
    let anchor = Anchor {
        id: "msg_nowhere".into(),
        created_ms: NOW,
        source: AnchorSource::Neither { newest_ms: NOW },
    };
    let snap = |now_ms| Snapshot {
        messages: vec![],
        inbox: vec![],
        active: false,
        permissions: vec![],
        forms: vec![],
        now_ms,
    };
    let stall_ms = settings.stall_s as i64 * 1000;
    assert_eq!(
        evaluate(&anchor, &snap(NOW + stall_ms - 1), &settings).state,
        WaitState::Running
    );
    assert_eq!(
        evaluate(&anchor, &snap(NOW + stall_ms), &settings).state,
        WaitState::Stalled
    );

    // Through the tool: the stall clock is the call start, so a single call returns running.
    let r = rig(|f| base(f, vec![])).await;
    let out = wait(&r, Some("msg_nowhere")).await;
    assert_eq!(out["state"], "running", "{out}");
    assert_eq!(out["turn_s"], Value::Null);
}

#[test]
fn evaluate_budget_boundary_is_strictly_past_max_turn_s() {
    let settings = Settings::default();
    let anchor = Anchor {
        id: "msg_a".into(),
        created_ms: NOW,
        source: AnchorSource::Message,
    };
    let turn_user = opencode_ctl::compact::Message {
        id: "msg_a".into(),
        r#type: "user".into(),
        outcome: None,
        created: NOW,
        text: "do it".into(),
        last_text: None,
        tools: vec![],
    };
    let snap = |now_ms| Snapshot {
        messages: vec![turn_user.clone()],
        inbox: vec![],
        active: true,
        permissions: vec![],
        forms: vec![],
        now_ms,
    };
    let max_ms = settings.max_turn_s as i64 * 1000;
    let at = evaluate(&anchor, &snap(NOW + max_ms), &settings);
    assert_eq!(
        at.state,
        WaitState::Running,
        "a turn of exactly max_turn_s is within budget"
    );
    assert_eq!(at.turn_s, Some(1800));
    let past = evaluate(&anchor, &snap(NOW + max_ms + 1000), &settings);
    assert_eq!(past.state, WaitState::BudgetExceeded);
    assert_eq!(past.turn_s, Some(1801));
}
