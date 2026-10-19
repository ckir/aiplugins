//! Process identity probing. Trait and system implementation; the pairing
//! logic that uses it lands in a later task.

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
        static BOOT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        BOOT.get_or_init(|| {
            let expr = "[math]::Floor([DateTimeOffset]::new((Get-CimInstance Win32_OperatingSystem).LastBootUpTime).ToUnixTimeSeconds() / 60) * 60";
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", expr])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default()
        })
        .clone()
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
