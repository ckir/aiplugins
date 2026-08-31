//! Embed the worker `.java` in the binary and extract it to a per-user script dir at boot, keyed
//! by content hash so the extracted copy always matches the compiled binary (spec §12). Only the
//! thin-slice extraction primitive; the §6 extraction-integrity hardening (0700/0600, ownership,
//! symlink guards) is layered in a later milestone.

use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};

/// The worker script's filename. Ghidra requires a GhidraScript's filename to
/// match its public class name, so this is not a free choice — and it is a
/// const rather than three string literals because `config.rs` hands it to the
/// launcher while `extract_worker` writes the file. Those two disagreeing
/// produces a worker that is on disk under one name and looked for under
/// another, which surfaces as an unexplained boot timeout.
pub const WORKER_SCRIPT_NAME: &str = "GhidraMcpWorker.java";

/// The worker GhidraScript, embedded at compile time (version-locked to this binary).
pub const WORKER_JAVA: &str = include_str!("../worker/GhidraMcpWorker.java");

/// Extract the worker script into `script_dir`, creating the dir if needed and rewriting the file
/// only when its content hash differs from the embedded source. Returns the script path.
pub fn extract_worker(script_dir: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(script_dir)?;
    let path = script_dir.join(WORKER_SCRIPT_NAME);
    let want = hash(WORKER_JAVA.as_bytes());
    let up_to_date = match std::fs::read(&path) {
        Ok(existing) => hash(&existing) == want,
        Err(_) => false,
    };
    if !up_to_date {
        std::fs::write(&path, WORKER_JAVA.as_bytes())?;
    }
    Ok(path)
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_source_is_the_worker_script() {
        assert!(WORKER_JAVA.contains("class GhidraMcpWorker"));
        assert!(WORKER_JAVA.contains("worker/init"));
    }

    #[test]
    fn extract_writes_then_is_idempotent_by_hash() {
        let dir = std::env::temp_dir().join(format!("ghidra-mcp-asset-{}", uuid::Uuid::new_v4()));
        let first = extract_worker(&dir).unwrap();
        assert!(first.exists());
        assert_eq!(first.file_name().unwrap(), WORKER_SCRIPT_NAME);
        let before = std::fs::metadata(&first).unwrap().modified().unwrap();
        // Second call with identical content must NOT rewrite (same hash).
        std::thread::sleep(std::time::Duration::from_millis(20));
        let second = extract_worker(&dir).unwrap();
        let after = std::fs::metadata(&second).unwrap().modified().unwrap();
        assert_eq!(before, after, "unchanged content must not be rewritten");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Ghidra resolves a GhidraScript by filename and then requires the public
    /// class inside to match. A rename that touched only one of the two would
    /// pass every other test here and fail only against a real Ghidra.
    #[test]
    fn script_filename_matches_the_public_class_it_contains() {
        let stem = WORKER_SCRIPT_NAME
            .strip_suffix(".java")
            .expect("worker script is a .java file");
        assert!(
            WORKER_JAVA.contains(&format!("class {stem}")),
            "{WORKER_SCRIPT_NAME} does not declare `class {stem}`"
        );
    }
}
