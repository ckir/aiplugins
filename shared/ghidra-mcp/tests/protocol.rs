//! Protocol-layer test (spec §8/§4.2): schema-invalid params are rejected at rmcp's deserialize
//! boundary, BEFORE the handler body and therefore before any Ghidra worker is touched. Runs under
//! plain `just test` — no GHIDRA_MCP_E2E gate, no live worker.
//!
//! HOW that rejection is signalled changed with the rmcp 0.8 -> 3.1 upgrade, and the change was
//! upstream's, not ours:
//!
//!   * 0.8 returned `ErrorData::invalid_params(..)`, which the framework surfaced as a JSON-RPC
//!     protocol error, code -32602.
//!   * 3.1 intercepts exactly that error and converts it into a SUCCESSFUL `CallToolResult` with
//!     `is_error: true` carrying the message as text. See `into_tool_argument_error` in
//!     `rmcp-3.1.4/src/handler/server/router/tool.rs`, and upstream's own test there, whose
//!     expectation reads "argument validation should be a tool result".
//!
//! The property under test is unchanged and is still the point: a bad argument never reaches the
//! handler, and never boots a JVM. Only the shape of the report moved. The newer shape is also the
//! more useful one for an agent — the model receives the reason as tool output it can read and
//! correct, rather than a transport-level error it usually cannot see.

use ghidra_mcp::config::ServerConfig;
use ghidra_mcp::server::GhidraMcpServer;
use ghidra_mcp::state::ServerState;
use rmcp::model::{CallToolRequestParams, ClientInfo};
use rmcp::{ClientHandler, ServiceExt};
use std::sync::Arc;
use std::time::Duration;

/// Minimal client handler: this test never receives a server->client request (sampling/roots/etc.), so
/// the trait's defaults suffice — mirrors rmcp's own `DummyClientHandler` in
/// `tests/test_tool_macros.rs::test_optional_i64_field_with_null_input`.
#[derive(Debug, Clone, Default)]
struct DummyClientHandler {}

impl ClientHandler for DummyClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

/// A `ServerConfig` with placeholder paths. This is safe here ONLY because the argument rejection happens
/// at rmcp's deserialize boundary, before `GhidraMcpServer::inspect_function`'s body — and therefore the
/// worker — is ever reached; the config's paths are never dereferenced. `RawConfig::resolve()` would
/// reject these placeholders (no real Ghidra install / project dir), so we bypass it and build the
/// already-validated `ServerConfig` struct directly (every field is `pub`).
fn dummy_cfg() -> ServerConfig {
    ServerConfig {
        ghidra_install_dir: std::env::temp_dir(),
        project_dir: std::env::temp_dir(),
        project_name: "unused".to_string(),
        bootstrap_program: "unused.exe".to_string(),
        bootstrap_program_path: "/unused.exe".to_string(),
        max_heap: None,
        boot_timeout: Duration::from_secs(1),
        rpc_deadline: Duration::from_secs(1),
        warming_deadline: Duration::from_secs(1),
    }
}

#[tokio::test]
async fn schema_invalid_params_are_rejected_before_the_handler() {
    // 1. Minimal ServerState driven directly (NOT via `server::serve`, which also kicks a background
    //    warmup boot). The worker never boots here: the bad-param call below is rejected pre-dispatch.
    let state = Arc::new(ServerState::new(
        dummy_cfg(),
        ghidra_mcp::paths::versioned_script_dir(),
    ));
    let server = GhidraMcpServer::new(state);

    // 2. In-memory duplex transport pair (EXACT pattern used by rmcp's own
    //    `test_optional_i64_field_with_null_input` in tests/test_tool_macros.rs).
    let (server_io, client_io) = tokio::io::duplex(4096);

    // 3. Serve the server half; serve the client half. `.serve()` performs the initialize handshake on
    //    both sides automatically.
    let server_handle = tokio::spawn(async move {
        let running = server.serve(server_io).await.expect("server handshake");
        let _ = running.waiting().await;
    });
    let client = DummyClientHandler::default()
        .serve(client_io)
        .await
        .expect("client handshake");

    // 4. Call inspect_function with a schema-invalid max_c_lines (InspectArgs::max_c_lines is a u64;
    //    a string value fails schema validation before the handler body runs).
    let result = client
        .call_tool(
            CallToolRequestParams::new("inspect_function").with_arguments(
                serde_json::json!({ "function": "main", "max_c_lines": "not-a-number" })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("under rmcp 3.1 argument validation is reported as a tool result, not an Err");

    // 5. It must be an ERROR result, not a success. `is_error: Some(true)` is what tells the model
    //    its call was rejected; a `None` or `Some(false)` here would mean a malformed argument had
    //    been silently accepted, which is the actual regression worth catching.
    assert_eq!(
        result.is_error,
        Some(true),
        "malformed arguments must produce an error result, got: {result:?}"
    );

    // 6. And it must be the DESERIALIZE-boundary rejection specifically — not some later failure
    //    that happens to also be an error. `inspect_function`'s body cannot produce this message;
    //    reaching the body with no worker would surface a worker error instead. So this string is
    //    what distinguishes "rejected before dispatch" from "dispatched and then failed", which is
    //    the whole claim of this test.
    let text = result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.as_str())
        .expect("the error result carries its reason as text");
    assert!(
        text.starts_with("failed to deserialize parameters:"),
        "expected the argument-deserialization rejection, got: {text}"
    );

    client.cancel().await.expect("client cancel");
    let _ = server_handle.await;
}
