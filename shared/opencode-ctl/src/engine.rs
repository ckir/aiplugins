//! The engine: settings, host, environment and the transport behind every tool.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::cli::env_nonempty;
use crate::clock::Clock;
use crate::compact::{cap_json, redact, redact_text, unwrap_envelope, Cap};
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::host::Host;
use crate::liveness::ProcessProbe;
use crate::openapi::{Operation, OperationTable, TableSource, SNAPSHOT_VERSION};
use crate::settings::{Note, Settings};
use crate::transport::{Lifecycle, Params, Transport, WaitOutcome};

/// What the pairing environment says, read once at startup. A blank value reads as `None`.
#[derive(Debug, Clone)]
pub struct EnvSnapshot {
    pub session: Option<String>,
    pub directory: Option<PathBuf>,
    pub server: Option<String>,
    pub pair: Option<String>,
    pub host_env: Option<String>,
    pub cwd: PathBuf,
    pub home: PathBuf,
    pub server_password_present: bool,
}

impl EnvSnapshot {
    pub fn from_env(ctl_home: PathBuf) -> Self {
        Self::from_lookup(
            &|k| env_nonempty(k),
            std::env::current_dir().unwrap_or_default(),
            ctl_home,
        )
    }

    pub fn from_lookup(get: &dyn Fn(&str) -> Option<String>, cwd: PathBuf, home: PathBuf) -> Self {
        let nonblank = |k: &str| crate::cli::blank_is_unset(get(k));
        Self {
            session: nonblank("OPENCODE_CTL_SESSION"),
            directory: nonblank("OPENCODE_CTL_DIR").map(PathBuf::from),
            server: nonblank("OPENCODE_CTL_SERVER"),
            pair: nonblank("OPENCODE_CTL_PAIR"),
            host_env: nonblank("OPENCODE_CTL_HOST"),
            cwd,
            home,
            server_password_present: nonblank("OPENCODE_SERVER_PASSWORD").is_some(),
        }
    }

    pub fn for_tests(cwd: PathBuf, home: PathBuf) -> Self {
        Self::from_lookup(&|_| None, cwd, home)
    }
}

#[derive(Debug, Default)]
pub struct OpenapiCache {
    pub live: Option<OperationTable>,
    pub pid: Option<u64>,
    pub version: Option<String>,
    pub last_error_was_service_down: bool,
}

pub struct Engine<T: Transport, C: Clock> {
    pub settings: Settings,
    pub notes: Vec<Note>,
    pub host: Host,
    pub env: EnvSnapshot,
    pub probe: Arc<dyn ProcessProbe>,
    transport: T,
    clock: C,
    openapi: tokio::sync::Mutex<OpenapiCache>,
}

const EXEMPT_ARRAYS: &[&str] = &["pending_permissions", "pending_forms"];

impl<T: Transport, C: Clock> Engine<T, C> {
    pub fn new(
        settings: Settings,
        notes: Vec<Note>,
        host: Host,
        env: EnvSnapshot,
        transport: T,
        clock: C,
        probe: Arc<dyn ProcessProbe>,
    ) -> Self {
        Self {
            settings,
            notes,
            host,
            env,
            probe,
            transport,
            clock,
            openapi: tokio::sync::Mutex::new(OpenapiCache::default()),
        }
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }

    /// The live table (fetched once) or, when the fetch fails, the bundled snapshot with a warning.
    pub async fn operation_table(&self) -> (OperationTable, TableSource, Option<String>) {
        let mut cache = self.openapi.lock().await;
        if let Some(live) = &cache.live {
            return (live.clone(), TableSource::Live, None);
        }
        let failure = match self.transport.fetch_openapi().await {
            Ok(doc) => match OperationTable::from_openapi(&doc, TableSource::Live) {
                Ok(table) => {
                    cache.live = Some(table.clone());
                    cache.last_error_was_service_down = false;
                    return (table, TableSource::Live, None);
                }
                Err(e) => format!("live OpenAPI document unusable: {e}"),
            },
            Err(e) => {
                cache.last_error_was_service_down = e.code == ErrorCode::ServiceDown;
                format!(
                    "live OpenAPI document unavailable: {}",
                    redact_text(&e.message)
                )
            }
        };
        (
            OperationTable::snapshot(),
            TableSource::Snapshot,
            Some(format!(
                "{failure}; using the bundled snapshot v{SNAPSHOT_VERSION}"
            )),
        )
    }

    /// Drop the live table when `info` (a `server.info` result) shows another pid or version, or
    /// when the service has just recovered from `SERVICE_DOWN`.
    pub async fn invalidate_if_changed(&self, info: &Value) {
        let pid = info.get("pid").and_then(Value::as_u64);
        let version = info
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut cache = self.openapi.lock().await;
        let changed = (cache.pid.is_some() && cache.pid != pid)
            || (cache.version.is_some() && cache.version != version);
        if changed || cache.last_error_was_service_down {
            cache.live = None;
            cache.last_error_was_service_down = false;
        }
        cache.pid = pid;
        cache.version = version;
    }

    pub async fn op(&self, id: &str) -> Result<Operation, ToolError> {
        let (table, _, _) = self.operation_table().await;
        table.get(id).cloned().ok_or_else(|| {
            ToolError::new(
                ErrorCode::UnknownOperation,
                format!("unknown operation {id}"),
            )
            .hint(&format!("nearest: {}", table.nearest(id, 3).join(", ")))
        })
    }

    pub async fn cli_version(&self) -> Result<String, ToolError> {
        Ok(self.transport.cli_version().await?)
    }

    pub async fn lifecycle(&self, verb: Lifecycle) -> Result<String, ToolError> {
        Ok(self.transport.lifecycle(verb).await?)
    }

    pub async fn wait_idle(
        &self,
        session_id: &str,
        deadline: Duration,
    ) -> Result<WaitOutcome, ToolError> {
        Ok(self.transport.wait_idle(session_id, deadline).await?)
    }

    /// One operation call: `(data, cursor, meta)` with the envelope unwrapped and secrets redacted.
    pub async fn call_json(
        &self,
        id: &str,
        params: Params,
        body: Option<Value>,
    ) -> Result<(Value, Option<Value>, Option<Value>), ToolError> {
        let op = self.op(id).await?;
        let deadline = Duration::from_secs(self.settings.call_timeout_s);
        let raw = self
            .transport
            .call(&op, &params, body.as_ref(), deadline)
            .await?;
        let (mut data, mut cursor, mut meta) = unwrap_envelope(raw)?;
        redact(&mut data);
        cursor.iter_mut().for_each(redact);
        meta.iter_mut().for_each(redact);
        Ok((data, cursor, meta))
    }

    /// The tool argument, else the paired session; either must look like a session id.
    pub fn resolve_session(&self, arg: Option<&str>) -> Result<String, ToolError> {
        let id = match arg {
            Some(a) => a,
            None => self.env.session.as_deref().ok_or_else(|| {
                ToolError::new(
                    ErrorCode::NoSession,
                    "no session_id given and this server is not paired",
                )
                .hint("pass session_id, or start the controller through opencode-pair")
            })?,
        };
        if valid_session_id(id) {
            Ok(id.to_string())
        } else {
            Err(ToolError::new(
                ErrorCode::SessionNotFound,
                format!("'{}' is not a session id", redact_text(id)),
            )
            .hint("session ids match ^ses_[A-Za-z0-9]{26}$; list them with session_list"))
        }
    }

    pub fn resolve_directory(&self, arg: Option<&std::path::Path>) -> PathBuf {
        match arg {
            Some(p) => p.to_path_buf(),
            None => self
                .env
                .directory
                .clone()
                .unwrap_or_else(|| self.env.cwd.clone()),
        }
    }

    pub fn finish(&self, value: Value) -> Value {
        cap_json(
            value,
            &Cap {
                max_bytes: self.settings.output_max_bytes,
                exempt_arrays: EXEMPT_ARRAYS,
                exempt_entry_max: 1024,
                exempt_len_max: 20,
            },
        )
    }

    /// Every tool result passes here: redaction, then the byte cap; errors keep the closed shape.
    pub fn render(&self, result: ToolResult) -> rmcp::model::CallToolResult {
        use rmcp::model::{CallToolResult, ContentBlock};
        match result {
            Ok(mut v) => {
                redact(&mut v);
                CallToolResult::success(vec![ContentBlock::text(self.finish(v).to_string())])
            }
            Err(mut e) => {
                e.message = redact_text(&e.message);
                e.hint = e.hint.map(|h| redact_text(&h));
                if let Some(d) = e.details.as_mut() {
                    redact(d);
                }
                CallToolResult::error(vec![ContentBlock::text(e.to_json().to_string())])
            }
        }
    }
}

pub fn valid_session_id(id: &str) -> bool {
    id.strip_prefix("ses_")
        .is_some_and(|r| r.len() == 26 && r.chars().all(|c| c.is_ascii_alphanumeric()))
}
