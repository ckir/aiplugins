//! Server logging: a per-instance, size-bounded, owner-only file log — never stdout (spec §5). stdout
//! is the MCP JSON-RPC channel; any byte there corrupts the protocol. The 128-bit connection token is
//! NEVER passed to a log macro anywhere in the server (enforced by review, not code).

use ghidra_ipc::owner_only::restrict_to_owner;
use std::io;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;

/// Initialize file-only tracing. Returns a guard that MUST be held for the process lifetime (dropping
/// it flushes+stops the writer). `log_path` is the per-instance path from `paths::instance_log_path()`.
pub fn init_file_logging(log_path: &Path) -> io::Result<WorkerGuard> {
    let dir = log_path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "log path has no parent"))?;
    std::fs::create_dir_all(dir)?;
    restrict_to_owner(dir)?;

    let file_name = log_path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "log path has no file name"))?;
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(file_name.to_string_lossy().as_ref())
        .max_log_files(5)
        .build(dir)
        .map_err(|e| io::Error::other(format!("log appender: {e}")))?;
    let (nb, guard) = tracing_appender::non_blocking(appender);

    // File-only writer: stdout is the MCP JSON-RPC channel and must never see a stray byte.
    tracing_subscriber::fmt()
        .with_writer(nb)
        .with_ansi(false)
        .init();
    // Re-assert ownership after the appender created today's log file (belt-and-suspenders: the
    // directory ACL already restricts new files, but this covers any window before that ACL applied).
    let _ = restrict_to_owner(dir);
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_owner_only_log_dir_and_never_panics() {
        let base = std::env::temp_dir().join(format!("ghidra-mcp-log-{}", std::process::id()));
        let log = base.join("logs").join("worker-test.log");
        let guard = init_file_logging(&log).expect("logging inits");
        tracing::info!("hello file log");
        drop(guard);
        assert!(log.parent().unwrap().exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(base.join("logs"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "log dir must be owner-only");
        }
        std::fs::remove_dir_all(&base).ok();
    }
}
