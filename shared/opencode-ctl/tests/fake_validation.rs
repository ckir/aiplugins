//! `FakeTransport` rejects what the real transport rejects, so a call with an undeclared param cannot pass a test.

mod common;

use std::time::Duration;

use opencode_ctl::error::ErrorCode;
use opencode_ctl::openapi::OperationTable;
use opencode_ctl::testing::fake::FakeTransport;
use opencode_ctl::transport::Transport;
use serde_json::json;

fn param(k: &str, v: &str) -> (String, String) {
    (k.to_string(), v.to_string())
}

#[tokio::test]
async fn fake_rejects_undeclared_param() {
    let fake = FakeTransport::new();
    fake.on("session.active", vec![Ok(json!({"data": {}}))]);
    let table = OperationTable::snapshot();
    let op = table.get("session.active").expect("op").clone();
    let err = fake
        .call(
            &op,
            &vec![param("sessionID", common::SID)],
            None,
            Duration::from_secs(1),
        )
        .await
        .expect_err("session.active declares no params");
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.message.contains("sessionID"), "{}", err.message);
    assert!(fake.calls().is_empty(), "a rejected call is not recorded");
    fake.call(&op, &vec![], None, Duration::from_secs(1))
        .await
        .expect("no params is valid");
}

#[tokio::test]
async fn fake_rejects_malformed_session_id() {
    let fake = FakeTransport::new();
    let table = OperationTable::snapshot();
    let op = table.get("session.get").expect("op").clone();
    let err = fake
        .call(
            &op,
            &vec![param("sessionID", "not-a-session")],
            None,
            Duration::from_secs(1),
        )
        .await
        .expect_err("bad id");
    assert_eq!(err.code, ErrorCode::SessionNotFound);
}
