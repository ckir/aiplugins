//! Everything `serve` does before it builds the server, callable from tests without stdio.
//! (`resolve_identity` / `apply_identity` live in `pairing.rs` and run before this; `startup`
//! takes the already-applied environment.)

use std::path::Path;

use crate::engine::EnvSnapshot;
use crate::error::ToolError;
use crate::host::Host;
use crate::liveness::{ProcessProbe, RebindOutcome};
use crate::logging::{init_file_logging, LogHandle, LogScope};
use crate::sweep::{startup_sweep, SweepReport};

pub struct StartupReport {
    pub log: Option<LogHandle>,
    pub log_warning: Option<String>,
    pub identity_stderr: Vec<String>,
    pub sweep: SweepReport,
    pub rebind: Option<RebindOutcome>,
}

/// One warning, one line: `CODE: message (hint)`.
pub fn render_warning(e: &ToolError) -> String {
    match &e.hint {
        Some(h) => format!("{e} ({h})"),
        None => e.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn startup(
    home: &Path,
    _host: Host,
    env: &EnvSnapshot,
    probe: &dyn ProcessProbe,
    now_ms: i64,
    debug: bool,
    scope: LogScope,
    identity_warnings: &[ToolError],
) -> StartupReport {
    let log_path = home
        .join("logs")
        .join(format!("serve-{}.log", std::process::id()));
    let (log, log_warning) = match init_file_logging(&log_path, debug, scope) {
        Ok(h) => (Some(h), None),
        Err(e) => (
            None,
            Some(format!(
                "opencode-ctl: cannot open the log {}: {e}; continuing without a file log",
                log_path.display()
            )),
        ),
    };

    let mut identity_stderr = Vec::new();
    for w in identity_warnings {
        let line = render_warning(w);
        if log.is_some() {
            tracing::warn!("{line}");
        } else {
            identity_stderr.push(line);
        }
    }

    let sweep = startup_sweep(home, now_ms, probe);
    for e in &sweep.errors {
        tracing::warn!("startup sweep: {e}");
    }
    if !sweep.removed.is_empty() {
        tracing::info!("startup sweep removed {} entries", sweep.removed.len());
    }

    let rebind = env.pair.as_deref().and_then(|pair| {
        let Some(parent) = probe.parent_pid(std::process::id()) else {
            tracing::warn!("pair {pair}: parent process unknown, no re-bind");
            return None;
        };
        match crate::liveness::rebind_on_spawn(home, pair, parent, probe) {
            Ok(outcome) => Some(outcome),
            Err(e) => {
                tracing::warn!("pair {pair}: re-bind failed: {e}");
                None
            }
        }
    });

    StartupReport {
        log,
        log_warning,
        identity_stderr,
        sweep,
        rebind,
    }
}
