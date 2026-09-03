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

// ---- which configuration is wrong, and what the user is told to do about it ----

/// `suggested_action` is serialized into the tool result the user reads, so a wrong one is a wrong
/// instruction. A project-level misconfiguration must not send someone off to reinstall Ghidra.
///
/// `RawConfig::resolve` checks every required field for presence BEFORE it touches the filesystem, so
/// setting `ghidra_install_dir` to any non-empty string is enough to get past it and reach the
/// project_dir check — no Ghidra fixture needed.
#[tokio::test]
async fn a_project_misconfiguration_is_not_reported_as_a_missing_ghidra_install() {
    let raw = RawConfig {
        ghidra_install_dir: Some("anything-non-empty".to_string()),
        ..RawConfig::default()
    };
    let state = Arc::new(ServerState::from_raw(raw, versioned_script_dir()));

    let err = call_worker(
        &state,
        "list_programs",
        "",
        "list_programs",
        serde_json::json!({}),
    )
    .await
    .map(|_| ())
    .expect_err("a missing project_dir must still fail the call");

    assert!(
        err.error.message.contains("project_dir"),
        "the failing field should be project_dir, got: {}",
        err.error.message
    );
    assert_ne!(
        err.error.code,
        ErrorCode::GhidraNotFound,
        "the Ghidra install is not what is missing here"
    );
    assert!(
        !err.error.suggested_action.contains("GHIDRA_INSTALL_DIR"),
        "must not tell the user to fix an install directory that is not the problem; got: {}",
        err.error.suggested_action
    );
    assert!(
        err.error.suggested_action.contains("settings"),
        "the action should point at the configuration, got: {}",
        err.error.suggested_action
    );
}

/// Regression guard for an ordering that is load-bearing and easy to reverse.
///
/// `call_worker` runs the readiness wait — which is where the deferred configuration error is
/// raised — BEFORE it acquires the bounded permit (`execute.rs`, steps 2 and 3). Reversed, a burst of
/// concurrent calls larger than `SERVER_PERMITS` would report SERVER_BUSY to the overflow callers,
/// telling them to retry a server that can never succeed, instead of naming the misconfiguration.
///
/// A capstone review asserted the code already had that defect; it does not, and this pins the reason.
#[tokio::test]
async fn concurrent_calls_all_report_the_config_error_not_server_busy() {
    let state = Arc::new(ServerState::from_raw(
        unresolvable(),
        versioned_script_dir(),
    ));

    // Well above SERVER_PERMITS (4), dispatched together.
    let calls = (0..16).map(|_| {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            call_worker(
                &state,
                "list_programs",
                "",
                "list_programs",
                serde_json::json!({}),
            )
            .await
            .map(|_| ())
            .expect_err("every call must fail")
            .error
            .code
        })
    });

    for call in calls {
        assert_eq!(
            call.await.expect("task joins"),
            ErrorCode::GhidraNotFound,
            "a saturated permit pool must not mask the configuration error as SERVER_BUSY"
        );
    }
    assert_eq!(state.boot_count(), 0);
}

// ---- the complete ConfigError surface, enumerated ----

/// A scratch directory that removes itself.
struct Scratch {
    dir: std::path::PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ghidra-mcp-cfg-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("create scratch");
        Self { dir }
    }

    /// Make this look like a Ghidra install to `RawConfig::resolve`, which checks only that
    /// `support/analyzeHeadless[.bat]` exists.
    fn into_ghidra_install(self) -> Self {
        let support = self.dir.join("support");
        std::fs::create_dir_all(&support).expect("create support/");
        let launcher = if cfg!(windows) {
            "analyzeHeadless.bat"
        } else {
            "analyzeHeadless"
        };
        std::fs::write(support.join(launcher), "").expect("create launcher");
        self
    }

    fn s(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn code_for(raw: RawConfig) -> ErrorCode {
    match ServerState::from_raw(raw, versioned_script_dir()).cfg() {
        Ok(_) => panic!("this configuration must not resolve"),
        Err(e) => e.error.code,
    }
}

/// Every `ConfigError` this resolver can produce, and the code each one reports.
///
/// `config_unusable` matches `ConfigError::Missing` on the `field` STRING, so renaming a field in
/// `config.rs` would silently drop that case into the catch-all and report a Ghidra-install problem
/// for something that is not one. Nothing else in the build would notice. This enumerates the whole
/// surface — all four `Missing` fields plus both filesystem variants — so that rename fails loudly.
#[test]
fn every_config_error_reports_the_subsystem_that_is_actually_wrong() {
    let install = Scratch::new("install").into_ghidra_install();

    // 1. ghidra_install_dir absent.
    assert_eq!(code_for(RawConfig::default()), ErrorCode::GhidraNotFound);

    // 2. project_dir absent.
    assert_eq!(
        code_for(RawConfig {
            ghidra_install_dir: Some(install.s()),
            ..RawConfig::default()
        }),
        ErrorCode::ProjectNotFound
    );

    // 3. project_name absent.
    assert_eq!(
        code_for(RawConfig {
            ghidra_install_dir: Some(install.s()),
            project_dir: Some(install.s()),
            ..RawConfig::default()
        }),
        ErrorCode::ProjectNotFound
    );

    // 4. bootstrap_program absent. Deliberately NOT ProgramNotFound: that code means "the
    //    program_path you passed is not in the project VFS" (crucible.rs), and giving it a second
    //    producer would make a bad argument indistinguishable from an unconfigured server.
    assert_eq!(
        code_for(RawConfig {
            ghidra_install_dir: Some(install.s()),
            project_dir: Some(install.s()),
            project_name: Some("proj".into()),
            ..RawConfig::default()
        }),
        ErrorCode::ProjectNotFound
    );

    // 5. NotGhidra: every field present, but the install dir has no launcher.
    let bare = Scratch::new("bare");
    assert_eq!(
        code_for(RawConfig {
            ghidra_install_dir: Some(bare.s()),
            project_dir: Some(install.s()),
            project_name: Some("proj".into()),
            bootstrap_program: Some("x.exe".into()),
            ..RawConfig::default()
        }),
        ErrorCode::GhidraNotFound
    );

    // 6. NoProjectDir: install is valid, project dir does not exist.
    assert_eq!(
        code_for(RawConfig {
            ghidra_install_dir: Some(install.s()),
            project_dir: Some(install.dir.join("nope").to_string_lossy().into_owned()),
            project_name: Some("proj".into()),
            bootstrap_program: Some("x.exe".into()),
            ..RawConfig::default()
        }),
        ErrorCode::ProjectNotFound
    );
}
