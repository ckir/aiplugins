//! The seam between the engine and the opencode CLI.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

pub mod cli;
pub use cli::CliTransport;

use crate::cli::{blank_is_unset, env_nonempty, Route};
use crate::compact::redact_text;
use crate::error::{ErrorCode, ToolError};
use crate::openapi::Operation;
use crate::settings::Settings;

pub type Params = Vec<(String, String)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lifecycle {
    Start,
    Stop,
    Restart,
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitOutcome {
    Idle,
    DeadlineHit,
}

#[derive(Debug, Clone)]
pub struct TransportError {
    pub code: ErrorCode,
    pub message: String,
    pub hint: Option<String>,
    pub details: Option<Value>,
}

impl From<TransportError> for ToolError {
    fn from(e: TransportError) -> Self {
        ToolError {
            code: e.code,
            message: redact_text(&e.message),
            hint: e.hint.map(|h| redact_text(&h)),
            details: e.details,
        }
    }
}

pub trait Transport: Send + Sync + 'static {
    fn call(
        &self,
        op: &Operation,
        params: &Params,
        body: Option<&Value>,
        deadline: Duration,
    ) -> impl Future<Output = Result<Value, TransportError>> + Send;
    fn lifecycle(
        &self,
        verb: Lifecycle,
    ) -> impl Future<Output = Result<String, TransportError>> + Send;
    fn wait_idle(
        &self,
        session_id: &str,
        deadline: Duration,
    ) -> impl Future<Output = Result<WaitOutcome, TransportError>> + Send;
    fn fetch_openapi(&self) -> impl Future<Output = Result<Value, TransportError>> + Send;
    /// Raw `opencode --version` stdout, trimmed, e.g. `opencode v2.0.32`.
    fn cli_version(&self) -> impl Future<Output = Result<String, TransportError>> + Send;
}

/// `opencode v2.0.32` / `v2.0.32` / `2.0.32` -> `2.0.32`.
pub fn parse_cli_version(raw: &str) -> Option<String> {
    let t = raw.trim();
    let t = t.strip_prefix("opencode ").unwrap_or(t).trim();
    let t = t.strip_prefix('v').unwrap_or(t);
    if t.is_empty() || !t.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    Some(t.to_string())
}

/// The `service status` stdout is the URL when running and `stopped` otherwise.
pub fn service_running(status_stdout: &str) -> bool {
    status_stdout.trim() != "stopped"
}

/// The OS home for `route`, or the missing-home refusal. `Version`, `Usage` and `emit-skill` need no home.
pub fn home_required(
    route: &Route,
    get: &dyn Fn(&str) -> Option<String>,
) -> Result<PathBuf, ToolError> {
    let home = blank_is_unset(get("HOME")).or_else(|| blank_is_unset(get("USERPROFILE")));
    let exempt = match route {
        Route::Version | Route::Usage => true,
        Route::Sub { name, .. } => name == "emit-skill",
        Route::Pair(_) => false,
    };
    match home {
        Some(h) => Ok(PathBuf::from(h)),
        None if exempt => Ok(PathBuf::new()),
        None => Err(ToolError::new(
            ErrorCode::IoError,
            "no home directory: set HOME (USERPROFILE on Windows)",
        )),
    }
}

/// Every call fails `OPENCODE_NOT_FOUND` (kept for the server tests that need a transport with no CLI).
pub struct NotFoundTransport;

fn not_found() -> TransportError {
    TransportError {
        code: ErrorCode::OpencodeNotFound,
        message: "opencode executable not found".to_string(),
        hint: Some("install opencode or set OPENCODE_CTL_BIN".to_string()),
        details: None,
    }
}

impl Transport for NotFoundTransport {
    async fn call(
        &self,
        _op: &Operation,
        _params: &Params,
        _body: Option<&Value>,
        _deadline: Duration,
    ) -> Result<Value, TransportError> {
        Err(not_found())
    }
    async fn lifecycle(&self, _verb: Lifecycle) -> Result<String, TransportError> {
        Err(not_found())
    }
    async fn wait_idle(
        &self,
        _session_id: &str,
        _deadline: Duration,
    ) -> Result<WaitOutcome, TransportError> {
        Err(not_found())
    }
    async fn fetch_openapi(&self) -> Result<Value, TransportError> {
        Err(not_found())
    }
    async fn cli_version(&self) -> Result<String, TransportError> {
        Err(not_found())
    }
}

/// `home` is the ctl home, `user_home` the OS home.
pub fn build_transport(settings: &Settings, home: &Path, user_home: &Path) -> impl Transport {
    CliTransport::new(settings, home, user_home).with_path(env_nonempty("PATH"))
}
