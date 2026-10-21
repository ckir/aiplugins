//! Shared helpers for the in-process protocol tests.
#![allow(dead_code)]

use std::sync::Arc;

use opencode_ctl::clock::TestClock;
use opencode_ctl::engine::{Engine, EnvSnapshot};
use opencode_ctl::host::Host;
use opencode_ctl::settings::Settings;
use opencode_ctl::testing::fake::FakeTransport;
use opencode_ctl::testing::{FakeProbe, TempHome};
use rmcp::model::CallToolRequestParams;
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::Value;

pub type TestEngine = Engine<FakeTransport, TestClock>;

pub const SID: &str = "ses_abcdefghijklmnopqrstuvwxyz";
pub const SID2: &str = "ses_ABCDEFGHIJKLMNOPQRSTUVWXYZ";

pub async fn client_for(engine: Arc<TestEngine>) -> RunningService<RoleClient, ()> {
    let (server_io, client_io) = tokio::io::duplex(1 << 20);
    let server = opencode_ctl::server::OpencodeCtlServer::new(engine);
    tokio::spawn(async move {
        let running = server.serve(server_io).await.expect("server handshake");
        let _ = running.waiting().await;
    });
    ().serve(client_io).await.expect("client handshake")
}

pub fn engine_with_probe(
    fake: FakeTransport,
    clock: TestClock,
    host: Host,
    settings: Settings,
    env: EnvSnapshot,
    probe: FakeProbe,
) -> Arc<TestEngine> {
    Arc::new(Engine::new(
        settings,
        Vec::new(),
        host,
        env,
        fake,
        clock,
        Arc::new(probe),
    ))
}

pub fn engine(
    fake: FakeTransport,
    clock: TestClock,
    host: Host,
    settings: Settings,
    env: EnvSnapshot,
) -> Arc<TestEngine> {
    engine_with_probe(fake, clock, host, settings, env, FakeProbe::new("boot"))
}

pub fn env_unpaired(home: &TempHome) -> EnvSnapshot {
    EnvSnapshot::for_tests(home.path().join("cwd"), home.path().to_path_buf())
}

pub fn env_paired(home: &TempHome, session_id: &str, pair_id: Option<&str>) -> EnvSnapshot {
    let mut env = env_unpaired(home);
    env.session = Some(session_id.to_string());
    env.pair = pair_id.map(str::to_string);
    env
}

/// Call a tool and parse its text content; an `is_error` result that is not JSON becomes
/// `{"error":{"code":"RMCP","message":<text>}}`.
pub async fn call(client: &RunningService<RoleClient, ()>, name: &str, args: Value) -> Value {
    let params = CallToolRequestParams::new(name.to_string())
        .with_arguments(args.as_object().cloned().unwrap_or_default());
    let result = client.call_tool(params).await.expect("call_tool");
    let text = result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default();
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => v,
        Err(_) => serde_json::json!({"error": {"code": "RMCP", "message": text}}),
    }
}

pub fn ms(now: i64, delta: i64) -> i64 {
    now + delta
}
