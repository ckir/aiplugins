//! Test support shared with the front crates.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::liveness::ProcessProbe;

pub mod mock;

pub struct TempHome {
    path: PathBuf,
}

impl TempHome {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

pub fn temp_home() -> TempHome {
    let path = std::env::temp_dir().join(format!("opencode-ctl-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).expect("create temp home");
    TempHome { path }
}

pub fn env_map(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |k| map.get(k).cloned()
}

#[cfg(unix)]
pub fn exit_status(code: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code << 8)
}

#[cfg(windows)]
pub fn exit_status(code: i32) -> std::process::ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code as u32)
}

/// In-memory `ProcessProbe`. Liveness can change between reads through a
/// shared reference.
pub struct FakeProbe {
    pub alive: Mutex<HashMap<u32, String>>,
    pub boot: String,
    pub parents: Mutex<HashMap<u32, u32>>,
}

impl FakeProbe {
    pub fn new(boot: &str) -> Self {
        Self {
            alive: Mutex::new(HashMap::new()),
            boot: boot.to_string(),
            parents: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_alive(&self, pid: u32, starttime: &str) {
        self.alive
            .lock()
            .unwrap()
            .insert(pid, starttime.to_string());
    }

    pub fn set_dead(&self, pid: u32) {
        self.alive.lock().unwrap().remove(&pid);
    }
}

impl ProcessProbe for FakeProbe {
    fn is_alive(&self, pid: u32) -> bool {
        self.alive.lock().unwrap().contains_key(&pid)
    }

    fn starttime(&self, pid: u32) -> Option<String> {
        self.alive.lock().unwrap().get(&pid).cloned()
    }

    fn boot_id(&self) -> String {
        self.boot.clone()
    }

    fn parent_pid(&self, pid: u32) -> Option<u32> {
        self.parents.lock().unwrap().get(&pid).copied()
    }
}
