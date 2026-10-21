//! Process identity probing, the pid sentinels and the per-side liveness rules.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::ToolError;
use crate::pairing::{self, Record};

pub trait ProcessProbe: Send + Sync {
    fn is_alive(&self, pid: u32) -> bool;
    fn starttime(&self, pid: u32) -> Option<String>;
    fn boot_id(&self) -> String;
    fn parent_pid(&self, pid: u32) -> Option<u32>;
}

pub struct SystemProbe;

/// `sysctl -n kern.boottime` -> epoch seconds (the `sec = N` field).
pub fn parse_kern_boottime(s: &str) -> Option<i64> {
    let rest = &s[s.find("sec")? + 3..];
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    rest[..end].parse().ok()
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `ps -o lstart=` output (`Sun Oct 18 12:34:56 2026`, no zone) parsed as UTC
/// to epoch seconds. Callers run `ps` with `TZ=UTC`.
pub fn parse_lstart(s: &str) -> Option<i64> {
    let t: Vec<&str> = s.split_whitespace().collect();
    if t.len() != 5 {
        return None;
    }
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = MONTHS.iter().position(|m| *m == t[1])? as i64 + 1;
    let day: i64 = t[2].parse().ok()?;
    let year: i64 = t[4].parse().ok()?;
    let hms: Vec<i64> = t[3]
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    if hms.len() != 3 || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hms[0] * 3600 + hms[1] * 60 + hms[2])
}

/// Field 22 of a `/proc/<pid>/stat` line: the 20th whitespace-separated token
/// after the last `)` (the comm field may itself contain `)` and spaces).
pub fn parse_proc_stat_starttime(s: &str) -> Option<String> {
    let after = &s[s.rfind(')')? + 1..];
    after.split_whitespace().nth(19).map(str::to_string)
}

#[cfg(target_os = "linux")]
impl ProcessProbe for SystemProbe {
    fn is_alive(&self, pid: u32) -> bool {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(s) => {
                let state = s
                    .rfind(')')
                    .and_then(|i| s[i + 1..].split_whitespace().next().map(str::to_string));
                state.as_deref() != Some("Z")
            }
            Err(_) => false,
        }
    }

    fn starttime(&self, pid: u32) -> Option<String> {
        let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        parse_proc_stat_starttime(&s)
    }

    fn boot_id(&self) -> String {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    fn parent_pid(&self, pid: u32) -> Option<u32> {
        let s = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        s.lines()
            .find_map(|l| l.strip_prefix("PPid:"))
            .and_then(|v| v.trim().parse().ok())
    }
}

#[cfg(target_os = "macos")]
fn run_capture(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd)
        .args(args)
        .env("TZ", "UTC")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
impl ProcessProbe for SystemProbe {
    fn is_alive(&self, pid: u32) -> bool {
        self.starttime(pid).is_some()
    }

    fn starttime(&self, pid: u32) -> Option<String> {
        let out = run_capture("ps", &["-o", "lstart=", "-p", &pid.to_string()])?;
        parse_lstart(&out).map(|v| v.to_string())
    }

    fn boot_id(&self) -> String {
        run_capture("sysctl", &["-n", "kern.boottime"])
            .and_then(|s| parse_kern_boottime(&s))
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    fn parent_pid(&self, pid: u32) -> Option<u32> {
        run_capture("ps", &["-o", "ppid=", "-p", &pid.to_string()])?
            .parse()
            .ok()
    }
}

#[cfg(windows)]
impl ProcessProbe for SystemProbe {
    fn is_alive(&self, pid: u32) -> bool {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        const STILL_ACTIVE: u32 = 259;
        unsafe {
            let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
                return false;
            };
            let mut code = 0u32;
            let ok = GetExitCodeProcess(h, &mut code).is_ok();
            let _ = CloseHandle(h);
            ok && code == STILL_ACTIVE
        }
    }

    fn starttime(&self, pid: u32) -> Option<String> {
        use windows::Win32::Foundation::{CloseHandle, FILETIME};
        use windows::Win32::System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let (mut c, mut e, mut k, mut u) = (
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
            );
            let r = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
            let _ = CloseHandle(h);
            r.ok()?;
            Some((((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64).to_string())
        }
    }

    fn boot_id(&self) -> String {
        // Only a good value is cached: a failed or empty PowerShell call is retried next time.
        static BOOT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        if let Some(v) = BOOT.get() {
            return v.clone();
        }
        let expr = "[math]::Floor([DateTimeOffset]::new((Get-CimInstance Win32_OperatingSystem).LastBootUpTime).ToUnixTimeSeconds() / 60) * 60";
        let value = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", expr])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| parse_boot_output(&String::from_utf8_lossy(&o.stdout)));
        match value {
            Some(v) => BOOT.get_or_init(|| v).clone(),
            None => String::new(),
        }
    }

    fn parent_pid(&self, pid: u32) -> Option<u32> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut found = None;
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    if entry.th32ProcessID == pid {
                        found = Some(entry.th32ParentProcessID);
                        break;
                    }
                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snap);
            found
        }
    }
}

/// Delete `<home>/stdout/<pid>-<starttime>` directories whose process is dead or whose start time
/// differs from the live process's (a reused pid). Names of any other shape are left alone.
/// Returns the directories removed.
pub fn sweep_stdout_dirs(
    home: &std::path::Path,
    probe: &dyn ProcessProbe,
) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(home.join("stdout")) else {
        return Vec::new();
    };
    let mut removed = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let Some((pid, start)) = name.to_str().and_then(|n| n.split_once('-')) else {
            continue;
        };
        let Ok(pid) = pid.parse::<u32>() else {
            continue;
        };
        // The transport names an unreadable start time "0".
        let live =
            probe.is_alive(pid) && probe.starttime(pid).unwrap_or_else(|| "0".to_string()) == start;
        if !live && path.is_dir() && std::fs::remove_dir_all(&path).is_ok() {
            removed.push(path);
        }
    }
    removed
}

/// The PowerShell boot expression's stdout as a boot value: digits only, else `None`.
pub fn parse_boot_output(stdout: &str) -> Option<String> {
    let t = stdout.trim();
    (!t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())).then(|| t.to_string())
}

// ------------------------------------------------------------------ sentinels

/// `<pid>:<boot_id>:<starttime>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentinel {
    pub pid: u32,
    pub boot_id: String,
    pub starttime: String,
}

impl Sentinel {
    /// A leading U+FEFF and surrounding ASCII whitespace (CRLF included) are dropped first.
    pub fn parse(raw: &str) -> Option<Sentinel> {
        let t = raw
            .strip_prefix('\u{feff}')
            .unwrap_or(raw)
            .trim_matches(|c: char| c.is_ascii_whitespace());
        let mut it = t.splitn(3, ':');
        let pid = it.next()?.parse().ok()?;
        let boot_id = it.next()?;
        let starttime = it.next()?;
        if boot_id.is_empty() || starttime.is_empty() {
            return None;
        }
        Some(Sentinel {
            pid,
            boot_id: boot_id.to_string(),
            starttime: starttime.to_string(),
        })
    }
}

impl std::fmt::Display for Sentinel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.pid, self.boot_id, self.starttime)
    }
}

/// Windows boot values are epoch seconds truncated to the minute; clock adjustments drift them,
/// a reboot moves them by far more: equal within 120 s.
pub fn boot_matches_windows(a: &str, b: &str) -> bool {
    match (a.parse::<i64>(), b.parse::<i64>()) {
        (Ok(x), Ok(y)) => (x - y).abs() <= 120,
        _ => false,
    }
}

/// Exact on POSIX; within 120 s on Windows.
pub fn boot_matches(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        boot_matches_windows(a, b)
    } else {
        a == b
    }
}

/// The pid exists, its boot value matches and its start time is exactly the sentinel's.
pub fn sentinel_alive(s: &Sentinel, probe: &dyn ProcessProbe) -> bool {
    probe.is_alive(s.pid)
        && boot_matches(&s.boot_id, &probe.boot_id())
        && probe.starttime(s.pid).as_deref() == Some(s.starttime.as_str())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Liveness {
    Alive,
    Dead,
    Starting,
    Unknown,
    None,
}

impl Liveness {
    pub fn as_str(self) -> &'static str {
        match self {
            Liveness::Alive => "alive",
            Liveness::Dead => "dead",
            Liveness::Starting => "starting",
            Liveness::Unknown => "unknown",
            Liveness::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Tui,
    Controller,
}

/// A side with no sentinel yet is `starting` for this long after its clock origin.
const STARTING_MS: i64 = 30_000;

fn mtime_ms(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    let d = t.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_millis() as i64)
}

fn pair_file(home: &Path, id: &str, suffix: &str) -> PathBuf {
    home.join(format!("pair.{id}.{suffix}"))
}

/// `starting` while the clock origin is younger than 30 s, else `unknown` (also without an origin).
fn starting_or_unknown(origin: Option<i64>, now_ms: i64) -> Liveness {
    match origin {
        Some(m) if now_ms - m < STARTING_MS => Liveness::Starting,
        _ => Liveness::Unknown,
    }
}

/// The highest generation that has a `tui-pid` file.
fn highest_tui_generation(home: &Path, id: &str) -> Option<u32> {
    let prefix = format!("pair.{id}.g");
    std::fs::read_dir(home)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.strip_prefix(&prefix)?
                .strip_suffix(".tui-pid")?
                .parse::<u32>()
                .ok()
        })
        .max()
}

fn sentinel_state(path: &Path, probe: &dyn ProcessProbe) -> Option<Liveness> {
    let raw = std::fs::read_to_string(path).ok()?;
    Some(match Sentinel::parse(&raw) {
        None => Liveness::Unknown,
        Some(s) if sentinel_alive(&s, probe) => Liveness::Alive,
        Some(_) => Liveness::Dead,
    })
}

/// One side's liveness, re-derived from the files on every call (spec Pairing, liveness values).
pub fn side_liveness(
    side: Side,
    record: &Record,
    home: &Path,
    probe: &dyn ProcessProbe,
    now_ms: i64,
) -> Liveness {
    let id = record.pair_id.as_str();
    match side {
        Side::Tui => {
            if !record.tab {
                return Liveness::None;
            }
            let wanted = record.generation;
            let generation = if pair_file(home, id, &format!("g{wanted}.tui-pid")).exists() {
                wanted
            } else {
                highest_tui_generation(home, id).unwrap_or(wanted)
            };
            let wrapper_origin = |g: u32| {
                ["sh", "ps1"]
                    .iter()
                    .find_map(|ext| mtime_ms(&pair_file(home, id, &format!("g{g}.wrapper.{ext}"))))
            };
            let state = if pair_file(home, id, &format!("g{generation}.tui-exited")).exists() {
                Liveness::Dead
            } else {
                match sentinel_state(
                    &pair_file(home, id, &format!("g{generation}.tui-pid")),
                    probe,
                ) {
                    Some(state) => state,
                    None => starting_or_unknown(wrapper_origin(generation), now_ms),
                }
            };
            // The fallback keeps a live older tab visible past a failed `attach`; a dead older tab must
            // not hide the tab `attach` has just re-opened, whose fresh wrapper reads `starting`.
            if generation != wanted
                && state != Liveness::Alive
                && starting_or_unknown(wrapper_origin(wanted), now_ms) == Liveness::Starting
            {
                return Liveness::Starting;
            }
            state
        }
        Side::Controller => {
            // An attach record carries no controller. Once `resume` (or a re-binding MCP server) has put
            // controller files beside it, the side is real; a genuine attach record has none of them.
            if record.controller.is_none()
                && !["claude-pid", "claude-exited", "resume-lock"]
                    .iter()
                    .any(|f| pair_file(home, id, f).exists())
            {
                return Liveness::None;
            }
            if pair_file(home, id, "claude-exited").exists() {
                return Liveness::Dead;
            }
            match sentinel_state(&pair_file(home, id, "claude-pid"), probe) {
                Some(state) => state,
                None => {
                    let origin = mtime_ms(&pair_file(home, id, "resume-lock"))
                        .or_else(|| mtime_ms(&pairing::record_path(home, id)));
                    starting_or_unknown(origin, now_ms)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebindOutcome {
    Rebound,
    ControllerAlive,
    NoRecord,
}

fn remove_if_present(path: &Path) -> Result<(), ToolError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// An MCP server that starts under `OPENCODE_CTL_PAIR`: unless `claude-pid` names an alive
/// process, drop `claude-exited` and `dead-seen` and write `claude-pid` from the parent process.
pub fn rebind_on_spawn(
    home: &Path,
    pair_id: &str,
    parent_pid: u32,
    probe: &dyn ProcessProbe,
) -> Result<RebindOutcome, ToolError> {
    if pairing::read_record(home, pair_id)?.is_none() {
        return Ok(RebindOutcome::NoRecord);
    }
    // A failed boot read is no value: a sentinel written with it could never verify, and a live
    // controller could not be told from a dead one.
    let boot_id = probe.boot_id();
    if boot_id.is_empty() {
        return Err(ToolError::new(
            crate::error::ErrorCode::IoError,
            "cannot read the boot id of this machine",
        ));
    }
    let pid_file = pair_file(home, pair_id, "claude-pid");
    if pairing::read_sentinel(&pid_file).is_some_and(|s| sentinel_alive(&s, probe)) {
        tracing::info!("pair {pair_id}: controller alive, no re-bind");
        return Ok(RebindOutcome::ControllerAlive);
    }
    let starttime = probe.starttime(parent_pid).ok_or_else(|| {
        ToolError::new(
            crate::error::ErrorCode::IoError,
            format!("cannot read the start time of parent process {parent_pid}"),
        )
    })?;
    let sentinel = Sentinel {
        pid: parent_pid,
        boot_id,
        starttime,
    };
    remove_if_present(&pair_file(home, pair_id, "claude-exited"))?;
    remove_if_present(&pair_file(home, pair_id, "dead-seen"))?;
    pairing::write_sentinel(&pid_file, &sentinel)?;
    tracing::info!("pair {pair_id}: re-bound the controller side to pid {parent_pid}");
    Ok(RebindOutcome::Rebound)
}
