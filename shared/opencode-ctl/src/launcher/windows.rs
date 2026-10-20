//! Windows rules that are pure text and path logic, compiled on every platform so the Linux and macOS
//! legs test them: the verbatim-prefix strip, the `wt` directory argument, `PATHEXT` resolution, the
//! `cmd.exe /c` wrap and the `.ps1` wrapper.

use std::path::{Path, PathBuf};

use super::WrapperInput;

/// `\\?\C:\x` -> `C:\x` and `\\?\UNC\server\share` -> `\\server\share` (what
/// `std::fs::canonicalize` returns on Windows); any other path is returned unchanged.
pub fn strip_verbatim(p: &str) -> String {
    if let Some(rest) = p.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = p.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    p.to_string()
}

/// The `-d` argument of `wt`, passed through unchanged. MEASURED on Windows 11 with wt: a bare `C:\`
/// opens the drive root, while the `C:` this once produced opens the drive's current directory
/// (`C:\WINDOWS\system32` there). The trailing-backslash worry applies only to a hand-quoted `"C:\"`,
/// and `Command` quotes only arguments that contain whitespace, which a drive root never does.
pub fn wt_directory_arg(dir: &str) -> String {
    dir.to_string()
}

const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

fn is_cmd_ext(ext: &str) -> bool {
    ext.eq_ignore_ascii_case(".cmd") || ext.eq_ignore_ascii_case(".bat")
}

/// The Windows lookup of a bare command name: every directory of `path` (`;`-separated) in order,
/// and in each, the name as is when it already ends in a `pathext` extension, then the name plus
/// each `pathext` extension in order. The flag is true for a `.cmd`/`.bat` script, which
/// `CreateProcess` cannot start by bare name and `cmd_wrap` runs through `cmd.exe /c`.
pub fn resolve_executable(name: &str, path: &str, pathext: &str) -> Option<(PathBuf, bool)> {
    let exts: Vec<&str> = pathext.split(';').filter(|e| !e.is_empty()).collect();
    let own_ext = name
        .rfind('.')
        .map(|i| &name[i..])
        .filter(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)));
    for dir in path.split(';').filter(|d| !d.is_empty()) {
        let dir = Path::new(dir);
        if let Some(ext) = own_ext {
            let cand = dir.join(name);
            if cand.is_file() {
                return Some((cand, is_cmd_ext(ext)));
            }
        }
        for ext in &exts {
            let cand = dir.join(format!("{name}{ext}"));
            if cand.is_file() {
                return Some((cand, is_cmd_ext(ext)));
            }
        }
    }
    None
}

/// `cmd.exe /c <exe> <args>` (spec Launcher step 5: a `.cmd` controller).
pub fn cmd_wrap(exe: &Path, args: &[String]) -> Vec<String> {
    let mut v = vec![
        "cmd.exe".to_string(),
        "/c".to_string(),
        exe.to_string_lossy().into_owned(),
    ];
    v.extend(args.iter().cloned());
    v
}

/// The argv that starts the controller. Off Windows the bare name and the arguments (the OS finds
/// the executable). On Windows the executable is resolved through `PATHEXT` here, and a `.cmd`/`.bat`
/// goes through `cmd.exe /c`; `None` is `CONTROLLER_NOT_FOUND`.
pub fn controller_command(
    name: &str,
    args: &[String],
    windows: bool,
    path: Option<&str>,
    pathext: Option<&str>,
) -> Option<Vec<String>> {
    if !windows {
        let mut v = vec![name.to_string()];
        v.extend(args.iter().cloned());
        return Some(v);
    }
    let (exe, is_cmd) = resolve_executable(name, path?, pathext.unwrap_or(DEFAULT_PATHEXT))?;
    if is_cmd {
        return Some(cmd_wrap(&exe, args));
    }
    let mut v = vec![exe.to_string_lossy().into_owned()];
    v.extend(args.iter().cloned());
    Some(v)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsShell {
    Pwsh,
    PowershellExe,
}

impl WindowsShell {
    pub fn program(self) -> &'static str {
        match self {
            WindowsShell::Pwsh => "pwsh",
            WindowsShell::PowershellExe => "powershell.exe",
        }
    }
}

/// `pwsh` when it is on PATH, else the stock `powershell.exe`.
pub fn select_windows_shell(path_has: &dyn Fn(&str) -> bool) -> WindowsShell {
    if path_has("pwsh") {
        WindowsShell::Pwsh
    } else {
        WindowsShell::PowershellExe
    }
}

/// Single-quote a value for PowerShell, doubling EVERY quote character it recognises: U+0027 and the
/// typographic U+2018, U+2019, U+201A, U+201B (a lone one ends the string, or lets text after it run).
pub fn quote_ps(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('\'');
    for c in v.chars() {
        out.push(c);
        if matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}') {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// The spec's boot value: `LastBootUpTime` as epoch seconds truncated to the minute (the same
/// expression `SystemProbe::boot_id` runs).
pub const BOOT_EXPR_PS1: &str = "[math]::Floor([DateTimeOffset]::new((Get-CimInstance Win32_OperatingSystem).LastBootUpTime).ToUnixTimeSeconds() / 60) * 60";

/// The start time as FILETIME, the number `SystemProbe::starttime` reads from `GetProcessTimes`.
pub const START_EXPR_PS1: &str = "(Get-Process -Id $PID).StartTime.ToFileTime()";

/// The `.ps1` tab wrapper (UTF-8 text, no BOM: `write_wrapper` prepends it). The sentinels are written with
/// `[System.IO.File]::WriteAllText`, which writes UTF-8 without a BOM and without a newline on both
/// PowerShell editions; `Out-File`, `>` and `Set-Content` do not.
pub fn wrapper_script_ps1(input: &WrapperInput) -> String {
    let q = |p: &Path| quote_ps(&p.to_string_lossy());
    let pair_file = |suffix: &str| {
        input.home.join(format!(
            "pair.{}.g{}.{suffix}",
            input.pair_id, input.generation
        ))
    };
    let (boot, start) = if input.boot_id_expr {
        (BOOT_EXPR_PS1, START_EXPR_PS1)
    } else {
        ("'BOOT'", "'START'")
    };
    let mut s = String::new();
    s.push_str(&format!(
        "# opencode-ctl tab wrapper: pair {} generation {}\n",
        input.pair_id, input.generation
    ));
    s.push_str(&format!("$tuiPid = {}\n", q(&pair_file("tui-pid"))));
    s.push_str(&format!("$tuiExited = {}\n", q(&pair_file("tui-exited"))));
    s.push_str(&format!("$boot = {boot}\n"));
    s.push_str(&format!("$start = {start}\n"));
    s.push_str("[System.IO.File]::WriteAllText($tuiPid, ('{0}:{1}:{2}' -f $PID, $boot, $start))\n");
    if let (Some(_), Some(pw)) = (input.server_url, input.server_password) {
        s.push_str(&format!(
            "$env:OPENCODE_SERVER_PASSWORD = {}\n",
            quote_ps(pw)
        ));
        s.push_str("Remove-Item Env:OPENCODE_PASSWORD -ErrorAction SilentlyContinue\n");
    }
    s.push_str("try {\n");
    let mut cmd = format!(
        "& {} {} -s {}",
        q(input.opencode_bin),
        q(input.directory),
        quote_ps(input.session_id)
    );
    if let Some(url) = input.server_url {
        cmd.push_str(&format!(" --server {}", quote_ps(url)));
    }
    cmd.push('\n');
    // The command sits at the start of its line (the tests read it that way).
    s.push_str(&cmd);
    s.push_str("} finally {\n");
    s.push_str(
        "[System.IO.File]::WriteAllText($tuiExited, [string][DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())\n",
    );
    s.push_str("}\n");
    s
}
