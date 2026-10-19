//! The POSIX tab wrapper, the shell expressions it embeds, and the terminal walk and tab commands.

use std::path::Path;

use super::windows::{wt_directory_arg, WindowsShell};
use super::WrapperInput;

/// The boot id as the wrapper writes it (spec Pairing, lines 322-331). POSIX `sh` text valid under
/// dash; embedded as `$(<EXPR>)` by `wrapper_script_posix`.
#[cfg(target_os = "linux")]
pub const BOOT_ID_EXPR_POSIX: &str = "cat /proc/sys/kernel/random/boot_id";
/// Field 22 of `/proc/$$/stat`: the 20th token after the last `)` (`$$` inside `$(...)` is the
/// shell's pid; measured under dash).
#[cfg(target_os = "linux")]
pub const STARTTIME_EXPR_POSIX: &str = "s=$(cat /proc/$$/stat); s=${s##*) }; set -- $s; echo ${20}";

/// `kern.boottime`'s `sec = N` field.
#[cfg(target_os = "macos")]
pub const BOOT_ID_EXPR_POSIX: &str =
    r#"sysctl -n kern.boottime | sed 's/.*sec = \([0-9]*\).*/\1/'"#;
/// `ps -o lstart=` as epoch seconds, both sides forced to UTC. UNVERIFIED here: the `date -j`
/// format string is measured on a Mac in Task 25 Step 4.
#[cfg(target_os = "macos")]
pub const STARTTIME_EXPR_POSIX: &str =
    r#"TZ=UTC ps -o lstart= -p $$ | { read l; TZ=UTC date -j -f '%a %b %e %T %Y' "$l" +%s; }"#;

/// Every other target (Windows included, because the CI matrix builds it and
/// `wrapper_script_posix` is pure text) carries the Linux text so the text tests compile there.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub const BOOT_ID_EXPR_POSIX: &str = "cat /proc/sys/kernel/random/boot_id";
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub const STARTTIME_EXPR_POSIX: &str = "s=$(cat /proc/$$/stat); s=${s##*) }; set -- $s; echo ${20}";

/// Single-quote a value for POSIX `sh`: every `'` becomes `'\''`.
pub fn quote_sh(v: &str) -> String {
    format!("'{}'", v.replace('\'', r"'\''"))
}

/// The bash tab wrapper. It writes `tui-pid` (`$$:<boot_id>:<starttime>`, no newline) at its start and
/// `tui-exited` (epoch ms) from its EXIT trap, both under a `umask 077` subshell so the sentinels
/// are 0600 without changing the umask `opencode` runs under.
pub fn wrapper_script_posix(input: &WrapperInput) -> String {
    let q = |p: &Path| quote_sh(&p.to_string_lossy());
    let pair_file = |suffix: &str| {
        input.home.join(format!(
            "pair.{}.g{}.{suffix}",
            input.pair_id, input.generation
        ))
    };
    let (boot, start) = if input.boot_id_expr {
        (
            format!("$({BOOT_ID_EXPR_POSIX})"),
            format!("$({STARTTIME_EXPR_POSIX})"),
        )
    } else {
        ("BOOT".to_string(), "START".to_string())
    };
    let mut s = String::new();
    s.push_str("#!/usr/bin/env bash\n");
    s.push_str(&format!(
        "# opencode-ctl tab wrapper: pair {} generation {}\n",
        input.pair_id, input.generation
    ));
    s.push_str(&format!(
        "on_exit() {{ ( umask 077; printf '%s' \"$(date +%s)000\" > {} ); }}\n",
        q(&pair_file("tui-exited"))
    ));
    s.push_str("trap on_exit EXIT\n");
    s.push_str(&format!("boot={boot}\n"));
    s.push_str(&format!("start={start}\n"));
    s.push_str(&format!(
        "( umask 077; printf '%s' \"$$:$boot:$start\" > {} )\n",
        q(&pair_file("tui-pid"))
    ));
    if let (Some(_), Some(pw)) = (input.server_url, input.server_password) {
        s.push_str(&format!(
            "export OPENCODE_SERVER_PASSWORD={}\n",
            quote_sh(pw)
        ));
        s.push_str("unset OPENCODE_PASSWORD\n");
    }
    s.push_str(&format!(
        "{} {} -s {}",
        q(input.opencode_bin),
        q(input.directory),
        quote_sh(input.session_id)
    ));
    if let Some(url) = input.server_url {
        s.push_str(&format!(" --server {}", quote_sh(url)));
    }
    s.push('\n');
    s
}

// ------------------------------------------------------------------ terminals

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminal {
    Mate,
    Gnome,
    Xfce,
    Konsole,
    Wt,
}

impl Terminal {
    pub fn program(self) -> &'static str {
        match self {
            Terminal::Mate => "mate-terminal",
            Terminal::Gnome => "gnome-terminal",
            Terminal::Xfce => "xfce4-terminal",
            Terminal::Konsole => "konsole",
            Terminal::Wt => "wt",
        }
    }
}

/// The four POSIX terminals in the spec's PATH order.
const PATH_ORDER: [Terminal; 4] = [
    Terminal::Mate,
    Terminal::Gnome,
    Terminal::Xfce,
    Terminal::Konsole,
];

fn terminal_for_process(name: &str) -> Option<Terminal> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    match base {
        "mate-terminal" => Some(Terminal::Mate),
        // The gnome client execs; the shell's ancestor is the server process.
        "gnome-terminal" | "gnome-terminal-server" => Some(Terminal::Gnome),
        "xfce4-terminal" => Some(Terminal::Xfce),
        "konsole" => Some(Terminal::Konsole),
        _ => None,
    }
}

/// FIRST the terminal found by walking the parent process names (nearest first), and ONLY when no
/// parent matches the first of the four on PATH, in the spec's order. Windows: `wt` when on PATH.
pub fn select_terminal(
    parent_chain: &[String],
    path_has: &dyn Fn(&str) -> bool,
    windows: bool,
) -> Option<Terminal> {
    if windows {
        return path_has("wt").then_some(Terminal::Wt);
    }
    parent_chain
        .iter()
        .find_map(|n| terminal_for_process(n))
        .or_else(|| PATH_ORDER.into_iter().find(|t| path_has(t.program())))
}

/// The tab command, program first (spec Launcher step 4). `mate-terminal` and `xfce4-terminal` split
/// the ONE `-e` string with `g_shell_parse_argv`, so that element is the script path single-quoted
/// with every `'` written as `'\''`; `gnome-terminal` and `konsole` take argv elements and get the
/// raw path.
pub fn tab_command(
    t: Terminal,
    script: &Path,
    directory: &Path,
    shell: WindowsShell,
) -> Vec<String> {
    let script = script.to_string_lossy().into_owned();
    let argv = |parts: &[&str]| parts.iter().map(|p| p.to_string()).collect::<Vec<_>>();
    match t {
        Terminal::Mate | Terminal::Xfce => {
            let mut v = argv(&[t.program(), "--tab", "-e"]);
            v.push(quote_sh(&script));
            v
        }
        Terminal::Gnome => {
            let mut v = argv(&["gnome-terminal", "--tab", "--"]);
            v.push(script);
            v
        }
        Terminal::Konsole => {
            let mut v = argv(&["konsole", "--new-tab", "-e"]);
            v.push(script);
            v
        }
        Terminal::Wt => {
            let mut v = argv(&["wt", "-w", "0", "nt", "-d"]);
            v.push(wt_directory_arg(&directory.to_string_lossy()));
            v.extend(argv(&[
                shell.program(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ]));
            v.push(script);
            v
        }
    }
}

/// The names of this process's ancestors, nearest first (the parent's basename of
/// `/proc/<pid>/cmdline`'s first element on Linux, `ps -o comm=` on macOS; empty elsewhere).
pub fn system_parent_chain() -> Vec<String> {
    use crate::liveness::{ProcessProbe, SystemProbe};
    let mut chain = Vec::new();
    let mut pid = SystemProbe.parent_pid(std::process::id());
    while let Some(p) = pid.filter(|p| *p > 1 && chain.len() < 32) {
        if let Some(name) = process_name(p) {
            chain.push(name);
        }
        pid = SystemProbe.parent_pid(p);
    }
    chain
}

#[cfg(target_os = "linux")]
fn process_name(pid: u32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let first = raw.split(|b| *b == 0).next().filter(|f| !f.is_empty())?;
    let first = String::from_utf8_lossy(first).into_owned();
    Some(first.rsplit('/').next().unwrap_or(&first).to_string())
}

#[cfg(target_os = "macos")]
fn process_name(pid: u32) -> Option<String> {
    let out = std::process::Command::new("ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!name.is_empty()).then(|| name.rsplit('/').next().unwrap_or(&name).to_string())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn process_name(_pid: u32) -> Option<String> {
    None
}
