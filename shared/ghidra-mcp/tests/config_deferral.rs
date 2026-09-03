//! Deferred configuration resolution.
//!
//! `serve` must start, handshake and list its tools even when Ghidra configuration is absent or
//! invalid, so that a tool-schema probe works on a machine with no Ghidra install. The configuration
//! failure surfaces on the first tool CALL instead of killing the process at startup, where it used
//! to exit 2 before `server::serve` was ever reached.
//!
//! These tests run WITHOUT a Ghidra install and are deliberately NOT gated on `GHIDRA_MCP_E2E` —
//! running in exactly the environment that used to be impossible is the whole point.

use ghidra_ipc::error::ErrorCode;
use ghidra_mcp::config::RawConfig;
use ghidra_mcp::execute::call_worker;
use ghidra_mcp::paths::versioned_script_dir;
use ghidra_mcp::state::ServerState;
use std::sync::Arc;
use std::time::Duration;

/// A configuration that cannot resolve: every required field is absent.
fn unresolvable() -> RawConfig {
    RawConfig::default()
}

/// Give a wrongly-spawned background boot a chance to register itself before asserting it did not
/// happen. `spawn_boot` bumps `boot_count` as its first action, so this is long enough to catch one.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[test]
fn server_state_builds_from_a_config_that_cannot_resolve() {
    // Previously impossible: `ServerState::new` demanded an already-resolved `ServerConfig`, and
    // resolution is what fails with no Ghidra present.
    let state = ServerState::from_raw(unresolvable(), versioned_script_dir());
    assert_eq!(state.boot_count(), 0);
}

#[tokio::test]
async fn warmup_does_not_boot_a_worker_when_config_is_unresolved() {
    let state = Arc::new(ServerState::from_raw(
        unresolvable(),
        versioned_script_dir(),
    ));

    // `server::serve` calls this unconditionally today.
    state.start_warmup().await;
    settle().await;

    assert_eq!(
        state.boot_count(),
        0,
        "warmup must not launch a worker when there is no configuration to launch it with"
    );
}

#[tokio::test]
async fn tool_call_reports_the_config_error_without_attempting_a_boot() {
    let state = Arc::new(ServerState::from_raw(
        unresolvable(),
        versioned_script_dir(),
    ));

    let err = call_worker(
        &state,
        "list_programs",
        "",
        "list_programs",
        serde_json::json!({}),
    )
    .await
    .map(|_| ())
    .expect_err("a tool call with no Ghidra configuration must fail");

    assert_eq!(
        err.error.code,
        ErrorCode::GhidraNotFound,
        "a configuration failure is not a transport failure; got {:?}",
        err.error.code
    );
    assert!(
        err.error.message.contains("ghidra_install_dir"),
        "the error must name the missing field so the user can act on it; got: {}",
        err.error.message
    );

    // The assertion that matters. `wait_until_ready` kicks a single-flight boot whenever the slot is
    // `Empty` and then waits out `warming_deadline` (8 s) in 250 ms slices, resetting to `Empty` on
    // each transient failure. Without a short-circuit ahead of it, an unconfigured server spins that
    // loop and finally answers WORKER_WARMING — a timeout that says nothing about the real problem.
    assert_eq!(
        state.boot_count(),
        0,
        "the config error must short-circuit ahead of wait_until_ready, not spin its boot loop"
    );
}

#[tokio::test]
async fn the_config_error_is_identical_on_every_call() {
    let state = Arc::new(ServerState::from_raw(
        unresolvable(),
        versioned_script_dir(),
    ));

    let first = call_worker(
        &state,
        "list_programs",
        "",
        "list_programs",
        serde_json::json!({}),
    )
    .await
    .map(|_| ())
    .expect_err("first call must fail");
    let second = call_worker(
        &state,
        "list_programs",
        "",
        "list_programs",
        serde_json::json!({}),
    )
    .await
    .map(|_| ())
    .expect_err("second call must fail");

    assert_eq!(first.error.code, second.error.code);
    assert_eq!(
        first.error.message, second.error.message,
        "a deterministic misconfiguration must not produce a drifting error"
    );
    assert_eq!(state.boot_count(), 0);
}

// ---- the contract as a client actually sees it ----

/// Minimal client handler; this test never receives a server->client request, so the trait defaults
/// suffice. Mirrors the one in `protocol.rs`.
#[derive(Debug, Clone, Default)]
struct DummyClientHandler {}

impl rmcp::ClientHandler for DummyClientHandler {
    fn get_info(&self) -> rmcp::model::ClientInfo {
        rmcp::model::ClientInfo::default()
    }
}

/// The reason this whole change exists: a host can complete the MCP handshake and read every tool
/// schema from a machine that has no Ghidra install at all. Before deferral, `serve` exited 2 and the
/// host got nothing.
#[tokio::test]
async fn tools_list_answers_over_mcp_with_no_ghidra_configuration() {
    use rmcp::ServiceExt;

    let state = Arc::new(ServerState::from_raw(
        unresolvable(),
        versioned_script_dir(),
    ));
    let server = ghidra_mcp::server::GhidraMcpServer::new(Arc::clone(&state));

    let (server_io, client_io) = tokio::io::duplex(8192);
    let server_handle = tokio::spawn(async move {
        let running = server.serve(server_io).await.expect("server handshake");
        let _ = running.waiting().await;
    });
    let client = DummyClientHandler::default()
        .serve(client_io)
        .await
        .expect("client handshake must succeed without any Ghidra configuration");

    let tools = client
        .list_all_tools()
        .await
        .expect("tools/list must answer");

    assert!(
        !tools.is_empty(),
        "an unconfigured server must still advertise its tool schemas"
    );
    for expected in ["inspect_function", "list_project_programs"] {
        assert!(
            tools.iter().any(|t| t.name == expected),
            "tool {expected} missing from an unconfigured server's tools/list"
        );
    }
    assert_eq!(
        state.boot_count(),
        0,
        "listing schemas must never launch a worker"
    );

    let _ = client.cancel().await;
    server_handle.abort();
}
