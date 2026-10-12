//! Filesystem locations for the server: the ghidra-mcp data dir, the per-version worker-script dir, and
//! the per-instance log path (spec §5). Base dir = $GHIDRA_MCP_HOME, else %USERPROFILE%/$HOME + ".ghidra-mcp",
//! else the system temp dir. No external dir crate (keeps the deny gate small).

use crate::worker_asset::WORKER_JAVA;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// The ghidra-mcp base data dir (created by callers as needed).
pub fn data_dir() -> PathBuf {
    if let Ok(h) = std::env::var("GHIDRA_MCP_HOME") {
        if !h.is_empty() {
            return PathBuf::from(h);
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .filter(|s| !s.is_empty());
    match home {
        Some(h) => PathBuf::from(h).join(".ghidra-mcp"),
        None => std::env::temp_dir().join("ghidra-mcp"),
    }
}

/// The per-version worker-script directory: `<data>/scripts/<hash16>/`. `<hash16>` is the first 16
/// hex chars of the SHA-256 of the embedded `WORKER_JAVA`, so a binary always extracts into a dir
/// unique to its worker version (spec §5 per-version script dir). The file inside MUST stay named
/// `GhidraMcpWorker.java` (Ghidra: filename == class name).
pub fn versioned_script_dir() -> PathBuf {
    let mut h = Sha256::new();
    h.update(WORKER_JAVA.as_bytes());
    let digest = h.finalize();
    let hash16: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    data_dir().join("scripts").join(hash16)
}

/// The per-instance log path: `<data>/logs/worker-<pid>.log` (spec §5 per-instance filename).
pub fn instance_log_path() -> PathBuf {
    data_dir()
        .join("logs")
        .join(format!("worker-{}.log", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Hold while mutating `GHIDRA_MCP_HOME`: the env is process-global and
    /// the test runner executes tests on parallel threads, so an unguarded
    /// `remove_var` in one test can yank the value out from under another.
    /// Restores the prior value (or absence) on drop, so a panicking assert
    /// cannot leak a mutation into the next test either.
    struct HomeGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        prior: Option<String>,
    }

    impl HomeGuard {
        fn set(value: &str) -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let prior = std::env::var("GHIDRA_MCP_HOME").ok();
            std::env::set_var("GHIDRA_MCP_HOME", value);
            Self { _lock: lock, prior }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match self.prior.take() {
                Some(v) => std::env::set_var("GHIDRA_MCP_HOME", v),
                None => std::env::remove_var("GHIDRA_MCP_HOME"),
            }
        }
    }

    #[test]
    fn ghidra_mcp_home_env_overrides_base() {
        let _guard = HomeGuard::set(r"C:\tmp\gh");
        assert_eq!(data_dir(), PathBuf::from(r"C:\tmp\gh"));
    }

    #[test]
    fn versioned_script_dir_is_under_scripts_and_hash_named() {
        let _guard = HomeGuard::set(r"C:\tmp\gh");
        let d = versioned_script_dir();
        assert!(d.starts_with(r"C:\tmp\gh"));
        assert!(d.to_string_lossy().contains("scripts"));
        // leaf is 16 hex chars
        let leaf = d.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(leaf.len(), 16);
        assert!(leaf.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn instance_log_path_names_this_pid() {
        let _guard = HomeGuard::set(r"C:\tmp\gh");
        let p = instance_log_path();
        assert!(p
            .to_string_lossy()
            .contains(&format!("worker-{}", std::process::id())));
    }
}
