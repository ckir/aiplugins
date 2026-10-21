//! `service_*` tools.

use std::time::Duration;

use serde_json::{json, Value};

use crate::clock::Clock;
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError, ToolResult};
use crate::transport::{service_running, Lifecycle, Transport};

use super::{ConfirmArgs, ServiceStartArgs, ServiceStatusArgs};

/// How long a start or restart that succeeded may still read as stopped, and how often to read again.
const SETTLE_MS: i64 = 5_000;
const SETTLE_POLL: Duration = Duration::from_millis(500);

impl<T: Transport, C: Clock> Engine<T, C> {
    fn refuse_lifecycle(&self, tool: &str) -> Result<(), ToolError> {
        if self.host.refuses_lifecycle() {
            return Err(ToolError::new(
                ErrorCode::RefusedOnHost,
                format!("{tool} is refused on this host: it would stop the daemon the host itself depends on"),
            ));
        }
        if self.settings.server_url.is_some() {
            return Err(ToolError::new(
                ErrorCode::RefusedOnHost,
                format!(
                    "server_url is set: the service is remote and not managed by this plugin, so {tool} is refused; stop or restart it where it runs"
                ),
            ));
        }
        Ok(())
    }

    fn confirm_required(&self, tool: &str, confirm: bool) -> Result<(), ToolError> {
        if confirm {
            Ok(())
        } else {
            Err(ToolError::new(
                ErrorCode::ConfirmRequired,
                format!("{tool} needs confirm: true"),
            ))
        }
    }

    pub async fn service_status(&self, _a: ServiceStatusArgs) -> ToolResult {
        let managed = self.settings.server_url.is_none();
        // Managed mode: ask the CLI first; a server.info call on a stopped service would start it.
        let mut running = if managed {
            service_running(&self.lifecycle(Lifecycle::Status).await?)
        } else {
            true
        };
        let mut reachable = false;
        let (mut urls, mut pid, mut version) = (json!([]), Value::Null, Value::Null);
        if running {
            match self.call_json("server.info", Vec::new(), None).await {
                Ok((info, _, _)) => {
                    reachable = true;
                    urls = info.get("urls").cloned().unwrap_or(urls);
                    pid = info.get("pid").cloned().unwrap_or(Value::Null);
                    version = info.get("version").cloned().unwrap_or(Value::Null);
                    self.invalidate_if_changed(&info).await;
                }
                Err(e) if e.code == ErrorCode::ServiceDown => {
                    if !managed {
                        running = false;
                    }
                }
                Err(e) => return Err(e),
            }
        }
        let paired = match &self.env.session {
            Some(session) => {
                let (tui, claude) = self.pair_liveness();
                json!({
                    "pair_id": self.env.pair,
                    "session_id": session,
                    "directory": self.env.directory.as_ref().map(|d| d.to_string_lossy().to_string()),
                    "server_url": self.env.server,
                    "tui": tui.as_str(),
                    "claude": claude.as_str(),
                })
            }
            None => Value::Null,
        };
        Ok(json!({
            "running": running,
            "urls": urls,
            "pid": pid,
            "version": version,
            "reachable": reachable,
            "paired": paired,
        }))
    }

    /// The status after a start or restart that succeeded. The read right after it can still say stopped
    /// (MEASURED on Windows, Task 25: `service_restart` answered `running: false` and a read 4 s later showed
    /// the new pid), so read again every `SETTLE_POLL` for up to `SETTLE_MS` and answer what the last read
    /// saw. No `server.info` runs while it reads stopped, so a dead service is never started to answer.
    async fn status_after_lifecycle(&self) -> ToolResult {
        let deadline = self.clock().now_ms() + SETTLE_MS;
        loop {
            let status = self.service_status(ServiceStatusArgs {}).await?;
            if status["running"] == json!(true) || self.clock().now_ms() >= deadline {
                return Ok(status);
            }
            self.clock().sleep(SETTLE_POLL).await;
        }
    }

    pub async fn service_start(&self, a: ServiceStartArgs) -> ToolResult {
        let _ = a;
        if self.settings.server_url.is_none() {
            self.lifecycle(Lifecycle::Start).await?;
        }
        self.status_after_lifecycle().await
    }

    pub async fn service_stop(&self, a: ConfirmArgs) -> ToolResult {
        self.refuse_lifecycle("service_stop")?;
        self.confirm_required("service_stop", a.confirm)?;
        self.lifecycle(Lifecycle::Stop).await?;
        Ok(json!({"stopped": true}))
    }

    pub async fn service_restart(&self, a: ConfirmArgs) -> ToolResult {
        self.refuse_lifecycle("service_restart")?;
        self.confirm_required("service_restart", a.confirm)?;
        self.lifecycle(Lifecycle::Restart).await?;
        self.status_after_lifecycle().await
    }
}
