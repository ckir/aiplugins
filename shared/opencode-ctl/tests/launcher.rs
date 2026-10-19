//! The launcher, part 1: `pair`, the tab wrappers, the terminal walk, `install-launcher`, name
//! dispatch and the pure Windows resolvers (Task 18).

mod common;

use std::cell::{Cell, RefCell};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use common::{env_unpaired, SID};
use opencode_ctl::cli::{dispatch, route, Route};
use opencode_ctl::clock::TestClock;
use opencode_ctl::engine::Engine;
use opencode_ctl::error::{ErrorCode, ToolError};
use opencode_ctl::host::Host;
use opencode_ctl::launcher::{
    attach, cmd_wrap, controller_command, controller_env, install_launcher_as, pair,
    resolve_executable, resume, resume_with, select_terminal, select_windows_shell, strip_verbatim,
    tab_command, validate_launch_values, wrapper_path, wrapper_script_posix, wrapper_script_ps1,
    write_wrapper, wt_directory_arg, LauncherCtx, PairArgs, Terminal, WindowsShell, WrapperInput,
    WrapperKind, BOOT_ID_EXPR_POSIX, STARTTIME_EXPR_POSIX,
};
use opencode_ctl::liveness::ProcessProbe;
use opencode_ctl::settings::{PermissionPolicy, Settings};
use opencode_ctl::testing::fake::FakeTransport;
use opencode_ctl::testing::{exit_status, temp_home, FakeProbe, TempHome};
use opencode_ctl::transport::{Lifecycle, TransportError};
use serde_json::{json, Value};

const NOW: i64 = 1_800_000_000_000; // 08:00 UTC
const URL: &str = "http://127.0.0.1:4096";

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

// ------------------------------------------------------------------ the rig

/// A spawned controller: its argv and the environment it was given.
type Spawned = (Vec<String>, Vec<(String, Option<String>)>);

struct Rig {
    home: TempHome,
    ws: TempHome,
    bin: TempHome,
    fake: FakeTransport,
    probe: Arc<FakeProbe>,
    engine: Engine<FakeTransport, TestClock>,
    host: Host,
    opencode: Option<PathBuf>,
    chain: Vec<String>,
    on_path: Vec<String>,
    password: Option<String>,
    tabs: RefCell<Vec<Vec<String>>>,
    tab_fails: Cell<bool>,
    ctrl: RefCell<Vec<Spawned>>,
    ctrl_err: Cell<Option<io::ErrorKind>>,
    record_at_spawn: RefCell<Option<Vec<u8>>>,
    /// (name, contents) of every file in the home at the moment a controller is spawned.
    at_spawn: RefCell<Vec<(String, String)>>,
}

/// What `Rig::drive` runs.
enum Op {
    Pair(PairArgs),
    Attach(String),
    Resume(String, Option<String>, Vec<String>),
}

impl Rig {
    fn new() -> Rig {
        Rig::with(Host::Claude, Settings::default())
    }

    fn with(host: Host, settings: Settings) -> Rig {
        let home = temp_home();
        let ws = temp_home();
        let bin = temp_home();
        let probe = Arc::new(FakeProbe::new("boot"));
        probe.set_alive(4242, "777");
        // `resume` writes its own pid into the lock, so the probe must know it.
        probe.set_alive(std::process::id(), "own-start");
        let fake = FakeTransport::new();
        fake.on("session.active", vec![Ok(json!({"data": {}}))]);
        fake.on(
            "session.create",
            vec![Ok(
                json!({"data": {"id": SID, "location": {"directory": "/w"}}}),
            )],
        );
        fake.on("session.remove", vec![Ok(json!({"data": {}}))]);
        fake.on("session.synthetic", vec![Ok(json!({"data": {}}))]);
        let engine = Engine::new(
            settings,
            Vec::new(),
            host,
            env_unpaired(&home),
            fake.clone(),
            TestClock::new(NOW),
            probe.clone() as Arc<dyn ProcessProbe>,
        );
        let opencode = Some(bin.path().join("opencode"));
        Rig {
            home,
            ws,
            bin,
            fake,
            probe,
            engine,
            host,
            opencode,
            chain: vec![
                "bash".into(),
                "gnome-terminal-server".into(),
                "systemd".into(),
            ],
            on_path: Vec::new(),
            password: None,
            tabs: RefCell::default(),
            tab_fails: Cell::new(false),
            ctrl: RefCell::default(),
            ctrl_err: Cell::new(None),
            record_at_spawn: RefCell::default(),
            at_spawn: RefCell::default(),
        }
    }

    fn dir_arg(&self) -> PathBuf {
        self.ws.path().to_path_buf()
    }

    fn args(&self) -> PairArgs {
        PairArgs {
            directory: Some(self.dir_arg()),
            ..PairArgs::default()
        }
    }

    async fn pair(&self, args: PairArgs) -> (Result<i32, ToolError>, String) {
        self.drive(Op::Pair(args), None).await
    }

    async fn attach(&self, target: &str) -> (Result<i32, ToolError>, String) {
        self.drive(Op::Attach(target.to_string()), None).await
    }

    async fn resume(&self, id: &str, args: &[&str]) -> (Result<i32, ToolError>, String) {
        self.drive(Op::Resume(id.to_string(), None, s(args)), None)
            .await
    }

    async fn drive(
        &self,
        op: Op,
        before_claim: Option<&dyn Fn()>,
    ) -> (Result<i32, ToolError>, String) {
        let mut out: Vec<u8> = Vec::new();
        let now = || NOW;
        let path_has = |n: &str| self.on_path.iter().any(|p| p == n);
        let chain = || self.chain.clone();
        let spawn_detached = |c: &mut Command| -> io::Result<u32> {
            if self.tab_fails.get() {
                return Err(io::Error::other("tab spawn refused"));
            }
            let mut argv = vec![c.get_program().to_string_lossy().into_owned()];
            argv.extend(c.get_args().map(|a| a.to_string_lossy().into_owned()));
            self.tabs.borrow_mut().push(argv);
            Ok(9001)
        };
        let home_path = self.home.path().to_path_buf();
        let spawn_and_wait =
            |c: &mut Command, cb: &mut dyn FnMut(u32)| -> io::Result<std::process::ExitStatus> {
                if let Some(kind) = self.ctrl_err.get() {
                    return Err(io::Error::new(kind, "no such file"));
                }
                let mut argv = vec![c.get_program().to_string_lossy().into_owned()];
                argv.extend(c.get_args().map(|a| a.to_string_lossy().into_owned()));
                let envs = c
                    .get_envs()
                    .map(|(k, v)| {
                        (
                            k.to_string_lossy().into_owned(),
                            v.map(|v| v.to_string_lossy().into_owned()),
                        )
                    })
                    .collect();
                self.ctrl.borrow_mut().push((argv, envs));
                *self.at_spawn.borrow_mut() = fs::read_dir(&home_path)
                    .map(|rd| {
                        rd.flatten()
                            .filter(|e| e.path().is_file())
                            .map(|e| {
                                (
                                    e.file_name().to_string_lossy().into_owned(),
                                    fs::read_to_string(e.path()).unwrap_or_default(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(id) = pair_ids(&home_path).first() {
                    *self.record_at_spawn.borrow_mut() =
                        fs::read(home_path.join(format!("pair.{id}.json"))).ok();
                }
                cb(4242);
                Ok(exit_status(0))
            };
        let exe = PathBuf::from("/usr/bin/opencode-ctl-cc-mcp");
        let result = {
            let mut ctx = LauncherCtx {
                home: self.home.path(),
                bin_dir: self.bin.path(),
                running_exe: &exe,
                host: self.host,
                probe: &*self.probe,
                now_ms: &now,
                path_has: &path_has,
                opencode_bin: self.opencode.as_deref(),
                version: "0.1.0",
                parent_chain: &chain,
                server_password: self.password.as_deref(),
                spawn_detached: &spawn_detached,
                spawn_and_wait: &spawn_and_wait,
                before_claim,
                out: &mut out,
            };
            match op {
                Op::Pair(args) => pair(args, &mut ctx, &self.engine).await,
                Op::Attach(t) => attach(&t, &mut ctx, &self.engine).await,
                Op::Resume(id, None, a) => resume(&id, &a, &mut ctx, &self.engine).await,
                Op::Resume(id, c, a) => {
                    resume_with(&id, c.as_deref(), &a, &mut ctx, &self.engine).await
                }
            }
        };
        (result, String::from_utf8(out).unwrap())
    }

    fn fake_calls(&self, op: &str) -> Vec<Value> {
        self.fake
            .calls()
            .into_iter()
            .filter(|c| c.op == op)
            .map(|c| c.body.unwrap_or(Value::Null))
            .collect()
    }
}

fn pair_ids(home: &Path) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(home)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.strip_prefix("pair.")
                        .and_then(|r| r.strip_suffix(".json"))
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

fn entries(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn lines(out: &str) -> Vec<String> {
    out.lines().map(str::to_string).collect()
}

fn dry(rig: &Rig) -> PairArgs {
    PairArgs {
        dry_run: true,
        ..rig.args()
    }
}

fn wrapper_input<'a>(home: &'a Path, bin: &'a Path, dir: &'a Path) -> WrapperInput<'a> {
    WrapperInput {
        home,
        opencode_bin: bin,
        pair_id: "0123456789ab",
        generation: 1,
        directory: dir,
        session_id: SID,
        server_url: None,
        server_password: None,
        boot_id_expr: false,
    }
}

// ------------------------------------------------------------------ pure: wrappers

#[test]
fn wrapper_sentinel_writers() {
    let (home, bin, dir) = (Path::new("/h"), Path::new("/b/opencode"), Path::new("/w"));
    let sh = wrapper_script_posix(&wrapper_input(home, bin, dir));
    assert!(sh.contains("printf '%s'"), "{sh}");
    assert!(sh.contains("pair.0123456789ab.g1.tui-pid"), "{sh}");
    assert!(sh.contains("pair.0123456789ab.g1.tui-exited"), "{sh}");
    let writers: Vec<&str> = sh.lines().filter(|l| l.contains("printf '%s'")).collect();
    assert_eq!(writers.len(), 2, "{sh}");
    for w in &writers {
        assert!(w.contains(" > '"), "{w}");
    }
    assert!(sh.contains("trap "), "{sh}");
    assert!(sh.starts_with("#!/usr/bin/env bash\n"), "{sh}");

    let ps = wrapper_script_ps1(&wrapper_input(home, bin, dir));
    assert_eq!(
        ps.matches("[System.IO.File]::WriteAllText(").count(),
        2,
        "{ps}"
    );
    for bad in ["Out-File", "Set-Content", "Add-Content", ">"] {
        assert!(!ps.contains(bad), "{bad} in {ps}");
    }
    assert!(
        !ps.starts_with('\u{feff}'),
        "wrapper_script_ps1 returns no BOM"
    );
}

#[test]
fn pair_wrapper_exports_server_password_only_with_server_url() {
    let (home, bin, dir) = (Path::new("/h"), Path::new("/b/opencode"), Path::new("/w"));
    let render = |url: Option<&str>, pw: Option<&str>| {
        let mut i = wrapper_input(home, bin, dir);
        i.server_url = url;
        i.server_password = pw;
        (wrapper_script_posix(&i), wrapper_script_ps1(&i))
    };
    let (sh, ps) = render(Some(URL), Some("pw1"));
    assert!(sh.contains("export OPENCODE_SERVER_PASSWORD='pw1'"), "{sh}");
    assert!(sh.contains("unset OPENCODE_PASSWORD"), "{sh}");
    assert!(ps.contains("$env:OPENCODE_SERVER_PASSWORD = 'pw1'"), "{ps}");
    assert!(
        ps.contains("Remove-Item Env:OPENCODE_PASSWORD -ErrorAction SilentlyContinue"),
        "{ps}"
    );
    for (url, pw) in [(None, Some("pw1")), (Some(URL), None), (None, None)] {
        let (sh, ps) = render(url, pw);
        for t in [&sh, &ps] {
            assert!(
                !t.contains("OPENCODE_SERVER_PASSWORD"),
                "{url:?} {pw:?}: {t}"
            );
            assert!(!t.contains("OPENCODE_PASSWORD"), "{url:?} {pw:?}: {t}");
        }
    }
}

#[test]
fn wrapper_paths_absolute() {
    let (home, bin, dir) = (
        Path::new("/abs/home"),
        Path::new("/abs/bin/opencode"),
        Path::new("/abs/work"),
    );
    let sh = wrapper_script_posix(&wrapper_input(home, bin, dir));
    for p in [
        "'/abs/home/pair.0123456789ab.g1.tui-pid'",
        "'/abs/home/pair.0123456789ab.g1.tui-exited'",
        "'/abs/bin/opencode'",
        "'/abs/work'",
    ] {
        assert!(sh.contains(p), "{p} not in {sh}");
    }
    let ps = wrapper_script_ps1(&wrapper_input(home, bin, dir));
    assert!(ps.contains("pair.0123456789ab.g1.tui-pid"), "{ps}");
}

#[test]
fn wrapper_bakes_resolved_opencode_bin() {
    let dir = Path::new("/w");
    let bin = Path::new("/opt/o c/opencode");
    let sh = wrapper_script_posix(&wrapper_input(Path::new("/h"), bin, dir));
    let line = sh
        .lines()
        .find(|l| l.contains("'/opt/o c/opencode'"))
        .expect("command line");
    assert!(
        line.starts_with(&format!("'/opt/o c/opencode' '/w' -s '{SID}'")),
        "{line}"
    );
    let ps = wrapper_script_ps1(&wrapper_input(Path::new("/h"), bin, dir));
    assert!(
        ps.lines()
            .any(|l| l.starts_with("& '/opt/o c/opencode' '/w' -s '")),
        "{ps}"
    );
    let mut with_url = wrapper_input(Path::new("/h"), bin, dir);
    with_url.server_url = Some(URL);
    let sh = wrapper_script_posix(&with_url);
    assert!(sh.contains(&format!("-s '{SID}' --server '{URL}'")), "{sh}");
}

#[test]
fn wrapper_embeds_shell_constants() {
    let (home, bin, dir) = (Path::new("/h"), Path::new("/b/o"), Path::new("/w"));
    let mut i = wrapper_input(home, bin, dir);
    i.boot_id_expr = true;
    let sh = wrapper_script_posix(&i);
    assert!(sh.contains(&format!("$({BOOT_ID_EXPR_POSIX})")), "{sh}");
    assert!(sh.contains(&format!("$({STARTTIME_EXPR_POSIX})")), "{sh}");
    i.boot_id_expr = false;
    let sh = wrapper_script_posix(&i);
    assert!(!sh.contains(BOOT_ID_EXPR_POSIX), "{sh}");
    assert!(sh.contains("BOOT") && sh.contains("START"), "{sh}");
    let ps = wrapper_script_ps1(&i);
    assert!(ps.contains("'BOOT'") && ps.contains("'START'"), "{ps}");
    i.boot_id_expr = true;
    let ps = wrapper_script_ps1(&i);
    assert!(ps.contains("LastBootUpTime"), "{ps}");
    assert!(ps.contains("(Get-Process -Id $PID).StartTime"), "{ps}");
    assert!(ps.contains("ToFileTime()"), "{ps}");
}

#[cfg(unix)]
#[test]
fn wrapper_starttime_matches_probe() {
    use opencode_ctl::liveness::SystemProbe;
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let script = format!(
        "echo $$; echo \"$({BOOT_ID_EXPR_POSIX})\"; echo \"$({STARTTIME_EXPR_POSIX})\"; read _"
    );
    let mut child = Command::new("sh")
        .args(["-c", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sh");
    let mut rd = BufReader::new(child.stdout.take().unwrap());
    let mut got = Vec::new();
    for _ in 0..3 {
        let mut l = String::new();
        rd.read_line(&mut l).unwrap();
        got.push(l.trim_end_matches('\n').to_string());
    }
    let pid: u32 = got[0].parse().expect("pid line");
    let outcome = (
        SystemProbe.boot_id(),
        SystemProbe.starttime(pid).unwrap_or_default(),
    );
    drop(child.stdin.take());
    let _ = child.wait();
    assert_eq!(got[1], outcome.0, "boot id");
    assert_eq!(got[2], outcome.1, "starttime");
}

#[cfg(unix)]
mod shell_roundtrip {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const NASTY: &str = "-d'x y $a %b ^c &(d) !e \u{e9}\u{4e2d}";
    const PW: &str = "p'w $x `y` \\ %z";

    /// Run the bash wrapper for real with a fake `opencode` that records its argv and password.
    fn run_wrapper(
        home: &Path,
        bin: &Path,
        dir: &Path,
        url: Option<&str>,
        pw: Option<&str>,
    ) -> (Vec<String>, String, bool) {
        let out = home.join("argv-out");
        let mut i = wrapper_input(home, bin, dir);
        i.server_url = url;
        i.server_password = pw;
        i.boot_id_expr = true;
        let script = wrapper_script_posix(&i);
        let path = home.join("pair.0123456789ab.g1.wrapper.sh");
        assert!(write_wrapper(&path, &script, WrapperKind::Sh).unwrap());
        let status = Command::new("bash")
            .arg(&path)
            .env("OUT", &out)
            .env("OPENCODE_PASSWORD", "stray")
            .status()
            .expect("bash");
        assert!(status.success());
        let raw = fs::read(&out).unwrap();
        let argv = raw
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8(p.to_vec()).unwrap())
            .collect();
        let pw_seen = fs::read_to_string(home.join("argv-out.pw")).unwrap();
        let stray_unset = home.join("argv-out.unset").exists();
        (argv, pw_seen, stray_unset)
    }

    fn fake_opencode(dir: &Path) -> PathBuf {
        let d = dir.join("bin dir's");
        fs::create_dir_all(&d).unwrap();
        let p = d.join("opencode");
        fs::write(
            &p,
            "#!/bin/sh\nprintf '%s\\0' \"$@\" > \"$OUT\"\nprintf '%s' \"$OPENCODE_SERVER_PASSWORD\" > \"$OUT.pw\"\n[ -z \"${OPENCODE_PASSWORD+x}\" ] && echo unset > \"$OUT.unset\"\nexit 0\n",
        )
        .unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn wrapper_quoting_round_trips_every_value() {
        let t = temp_home();
        let home = t.path().join(NASTY);
        let dir = t.path().join(NASTY).join("work-\u{2019}-dir");
        fs::create_dir_all(&dir).unwrap();
        let bin = fake_opencode(t.path());
        let (argv, pw, stray_unset) = run_wrapper(&home, &bin, &dir, Some(URL), Some(PW));
        assert_eq!(
            argv,
            vec![
                dir.to_string_lossy().into_owned(),
                "-s".to_string(),
                SID.to_string(),
                "--server".to_string(),
                URL.to_string()
            ]
        );
        assert_eq!(pw, PW);
        assert!(
            stray_unset,
            "OPENCODE_PASSWORD must be unset by the wrapper"
        );
        // The sentinels were written by the script: `<pid>:<boot>:<start>` and `<epoch ms>`.
        let pid_line = fs::read_to_string(home.join("pair.0123456789ab.g1.tui-pid")).unwrap();
        let sentinel = opencode_ctl::liveness::Sentinel::parse(&pid_line).expect(&pid_line);
        assert!(!sentinel.boot_id.is_empty() && !sentinel.starttime.is_empty());
        assert!(!pid_line.ends_with('\n'));
        let exited = fs::read_to_string(home.join("pair.0123456789ab.g1.tui-exited")).unwrap();
        assert!(
            exited.bytes().all(|b| b.is_ascii_digit()) && exited.len() >= 13,
            "{exited}"
        );
        // Modes: wrapper 0700, sentinels 0600.
        let mode = |n: &str| fs::metadata(home.join(n)).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode("pair.0123456789ab.g1.wrapper.sh"), 0o700);
        assert_eq!(mode("pair.0123456789ab.g1.tui-pid"), 0o600);
        assert_eq!(mode("pair.0123456789ab.g1.tui-exited"), 0o600);
    }

    #[test]
    fn wrapper_without_server_url_passes_no_server_and_no_password() {
        let t = temp_home();
        let home = t.path().join("h");
        let dir = t.path().join("d");
        fs::create_dir_all(&dir).unwrap();
        let bin = fake_opencode(t.path());
        let (argv, pw, stray_unset) = run_wrapper(&home, &bin, &dir, None, Some(PW));
        assert_eq!(argv.len(), 3, "{argv:?}");
        assert_eq!(pw, "");
        assert!(!stray_unset, "no URL: the stray variable is left alone");
    }

    #[test]
    fn wrapper_quotes_home_with_space() {
        let t = temp_home();
        let home = t.path().join("my home");
        let dir = t.path().join("d");
        fs::create_dir_all(&dir).unwrap();
        let bin = fake_opencode(t.path());
        let (argv, ..) = run_wrapper(&home, &bin, &dir, None, None);
        assert_eq!(argv[1], "-s");
        assert!(home.join("pair.0123456789ab.g1.tui-pid").is_file());
    }
}

#[test]
fn wrapper_quotes_home_with_space_text() {
    let sh = wrapper_script_posix(&wrapper_input(
        Path::new("/my home/it's"),
        Path::new("/b/o"),
        Path::new("/w"),
    ));
    assert!(
        sh.contains(r#"'/my home/it'\''s/pair.0123456789ab.g1.tui-pid'"#),
        "{sh}"
    );
}

#[test]
fn ps1_quoting_doubles_every_quote_character_and_carries_the_bom() {
    let t = temp_home();
    let dir = Path::new("C:\\Users\\Bob\u{2019}s \u{2018}p\u{201a}r\u{201b}o'j");
    let mut i = wrapper_input(
        Path::new("C:\\h\u{2019}"),
        Path::new("C:\\o\\opencode.exe"),
        dir,
    );
    i.server_url = Some(URL);
    i.server_password = Some("pw\u{2019}; Write-Output INJECTED; \u{2018}b");
    let ps = wrapper_script_ps1(&i);
    for (q, doubled) in [
        ('\'', "''"),
        ('\u{2018}', "\u{2018}\u{2018}"),
        ('\u{2019}', "\u{2019}\u{2019}"),
        ('\u{201a}', "\u{201a}\u{201a}"),
        ('\u{201b}', "\u{201b}\u{201b}"),
    ] {
        let _ = q;
        assert!(ps.contains(doubled), "{doubled:?} missing in {ps}");
    }
    assert!(
        ps.contains("'C:\\Users\\Bob\u{2019}\u{2019}s \u{2018}\u{2018}p\u{201a}\u{201a}r\u{201b}\u{201b}o''j'"),
        "{ps}"
    );
    assert!(
        ps.contains("$env:OPENCODE_SERVER_PASSWORD = 'pw\u{2019}\u{2019}; Write-Output INJECTED; \u{2018}\u{2018}b'"),
        "{ps}"
    );
    let path = t.path().join("w.ps1");
    assert!(write_wrapper(&path, &ps, WrapperKind::Ps1).unwrap());
    let bytes = fs::read(&path).unwrap();
    assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF], "UTF-8 BOM");
    assert_eq!(&bytes[3..], ps.as_bytes());
    // Claimed once: a second write is refused without touching the file.
    assert!(!write_wrapper(&path, "other", WrapperKind::Ps1).unwrap());
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn quoting_refusals() {
    let ok = Path::new("/w");
    let id = "0123456789ab";
    for bad_dir in ["/w\nx", "/w\r", "/w\0x"] {
        for windows in [false, true] {
            let e = validate_launch_values(Path::new(bad_dir), None, id, &[], windows).unwrap_err();
            assert_eq!(e.code, ErrorCode::InvalidArgument, "{bad_dir:?}");
        }
    }
    // `;` is refused on Windows only (wt splits its command line on it).
    assert!(validate_launch_values(Path::new("/a;b"), None, id, &[], false).is_ok());
    // The nasty value is fine as a directory.
    let nasty = "/-d'x y $a %b ^c &(d) !e \u{e9}";
    assert!(validate_launch_values(Path::new(nasty), None, id, &[], false).is_ok());
    // ids
    let e = validate_launch_values(ok, None, "NOTAPAIR", &[], false).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument);
    let e = validate_launch_values(ok, Some("ses_short"), id, &[], false).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument);
    assert!(validate_launch_values(ok, Some(SID), id, &[], false).is_ok());
}

#[test]
fn launcher_rejects_semicolon_dir_windows() {
    let e = validate_launch_values(Path::new("C:\\proj;evil"), None, "0123456789ab", &[], true)
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgument);
    assert!(e.message.contains(';'), "{}", e.message);
}

#[test]
fn launcher_rejects_cmd_metachars_in_controller_args_windows() {
    for bad in ["a&b", "a|b", "a<b", "a>b", "a^b", "a\"b", "%PATH%", "a\nb"] {
        let e = validate_launch_values(
            Path::new("C:\\p"),
            None,
            "0123456789ab",
            &[bad.to_string()],
            true,
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidArgument, "{bad:?}");
        // The same argument is plain argv off Windows.
        let off = validate_launch_values(
            Path::new("/p"),
            None,
            "0123456789ab",
            &[bad.to_string()],
            false,
        );
        assert!(off.is_ok(), "{bad:?} is plain argv off Windows");
    }
    assert!(validate_launch_values(
        Path::new("C:\\p"),
        None,
        "0123456789ab",
        &s(&["--resume", "x y", "-p"]),
        true
    )
    .is_ok());
}

#[test]
fn launcher_strips_verbatim_prefix_windows() {
    assert_eq!(strip_verbatim(r"\\?\C:\x\y"), r"C:\x\y");
    assert_eq!(
        strip_verbatim(r"\\?\UNC\server\share\d"),
        r"\\server\share\d"
    );
    assert_eq!(strip_verbatim(r"C:\plain"), r"C:\plain");
    assert_eq!(strip_verbatim(r"\\server\share"), r"\\server\share");
}

#[test]
fn transport_unc_prefix() {
    assert_eq!(strip_verbatim(r"\\?\UNC\server\share"), r"\\server\share");
}

#[test]
fn wt_directory_arg_drops_drive_root_backslash() {
    assert_eq!(wt_directory_arg(r"C:\"), "C:");
    assert_eq!(wt_directory_arg(r"C:\x"), r"C:\x");
    assert_eq!(wt_directory_arg(r"\\server\share"), r"\\server\share");
    assert_eq!(wt_directory_arg(r"C:\x\"), r"C:\x\");
}

#[test]
fn launcher_resolves_cmd_controller_windows() {
    let t = temp_home();
    let d = t.path();
    fs::write(d.join("claude.cmd"), "@echo off").unwrap();
    fs::write(d.join("qwen.exe"), "x").unwrap();
    let path = format!("{};{}", d.join("missing").display(), d.display());
    let (p, is_cmd) = resolve_executable("claude", &path, ".com;.exe;.bat;.cmd").unwrap();
    assert_eq!(p, d.join("claude.cmd"));
    assert!(is_cmd);
    let (p, is_cmd) = resolve_executable("qwen", &path, ".com;.exe;.bat;.cmd").unwrap();
    assert_eq!(p, d.join("qwen.exe"));
    assert!(!is_cmd);
    assert!(resolve_executable("agy", &path, ".EXE;.cmd").is_none());
    // PATHEXT order decides between two matches.
    fs::write(d.join("claude.exe"), "x").unwrap();
    let (p, is_cmd) = resolve_executable("claude", &path, ".exe;.cmd").unwrap();
    assert_eq!((p, is_cmd), (d.join("claude.exe"), false));
    // A name that already carries a PATHEXT extension resolves as is.
    let (p, is_cmd) = resolve_executable("claude.cmd", &path, ".exe;.cmd").unwrap();
    assert_eq!((p, is_cmd), (d.join("claude.cmd"), true));

    let wrapped = cmd_wrap(Path::new("C:\\npm\\claude.cmd"), &s(&["--resume", "x y"]));
    assert_eq!(
        wrapped,
        s(&["cmd.exe", "/c", "C:\\npm\\claude.cmd", "--resume", "x y"])
    );

    let argv = controller_command("claude", &s(&["-p"]), true, Some(&path), Some(".cmd;.exe"));
    assert_eq!(argv.unwrap()[..2], s(&["cmd.exe", "/c"]));
    let argv = controller_command("qwen", &[], true, Some(&path), Some(".exe;.cmd")).unwrap();
    assert_eq!(
        argv,
        vec![d.join("qwen.exe").to_string_lossy().into_owned()]
    );
    assert!(controller_command("agy", &[], true, Some(&path), Some(".exe")).is_none());
    assert_eq!(
        controller_command("claude", &s(&["-p"]), false, None, None).unwrap(),
        s(&["claude", "-p"])
    );
}

// ------------------------------------------------------------------ terminals

#[test]
fn pair_commands_per_terminal() {
    let script = Path::new("/home/u/.opencode-ctl/pair.0123456789ab.g1.wrapper.sh");
    let dir = Path::new("/w");
    let t = |t| tab_command(t, script, dir, WindowsShell::Pwsh);
    let sp = script.to_string_lossy().into_owned();
    assert_eq!(
        t(Terminal::Mate),
        vec!["mate-terminal", "--tab", "-e", &format!("'{sp}'")]
    );
    assert_eq!(
        t(Terminal::Gnome),
        vec!["gnome-terminal", "--tab", "--", &sp]
    );
    assert_eq!(
        t(Terminal::Xfce),
        vec!["xfce4-terminal", "--tab", "-e", &format!("'{sp}'")]
    );
    assert_eq!(
        t(Terminal::Konsole),
        vec!["konsole", "--new-tab", "-e", &sp]
    );
    let wt = tab_command(
        Terminal::Wt,
        Path::new(r"C:\h\pair.0123456789ab.g1.wrapper.ps1"),
        Path::new(r"C:\"),
        WindowsShell::PowershellExe,
    );
    assert_eq!(
        wt,
        s(&[
            "wt",
            "-w",
            "0",
            "nt",
            "-d",
            "C:",
            "powershell.exe",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            r"C:\h\pair.0123456789ab.g1.wrapper.ps1"
        ])
    );
    let wt = tab_command(
        Terminal::Wt,
        Path::new("s.ps1"),
        Path::new(r"C:\x"),
        WindowsShell::Pwsh,
    );
    assert_eq!(wt[5], r"C:\x");
    assert_eq!(wt[6], "pwsh");

    // A HOME with a space and a quote: the `-e` element is the escaped single-quoted path.
    let odd = Path::new("/home/o'brien x/.opencode-ctl/pair.0123456789ab.g1.wrapper.sh");
    for term in [Terminal::Mate, Terminal::Xfce] {
        let argv = tab_command(term, odd, dir, WindowsShell::Pwsh);
        assert_eq!(
            argv[3],
            "'/home/o'\\''brien x/.opencode-ctl/pair.0123456789ab.g1.wrapper.sh'"
        );
    }
    let argv = tab_command(Terminal::Gnome, odd, dir, WindowsShell::Pwsh);
    assert_eq!(argv[3], odd.to_string_lossy());
}

#[test]
fn terminal_walk_prefers_the_parent_then_path_order() {
    let none = |_: &str| false;
    let all = |_: &str| true;
    let pick =
        |chain: &[&str], path: &dyn Fn(&str) -> bool| select_terminal(&s(chain), path, false);
    assert_eq!(
        pick(&["bash", "mate-terminal", "x"], &none),
        Some(Terminal::Mate)
    );
    assert_eq!(
        pick(&["bash", "gnome-terminal-server"], &none),
        Some(Terminal::Gnome)
    );
    assert_eq!(
        pick(&["bash", "gnome-terminal"], &none),
        Some(Terminal::Gnome)
    );
    assert_eq!(pick(&["xfce4-terminal"], &none), Some(Terminal::Xfce));
    assert_eq!(
        pick(&["fish", "/usr/bin/konsole"], &none),
        Some(Terminal::Konsole)
    );
    // The parent wins over the PATH order.
    assert_eq!(pick(&["konsole"], &all), Some(Terminal::Konsole));
    // No parent matches: the first of the four on PATH, in the spec's order.
    assert_eq!(pick(&["bash", "sshd"], &all), Some(Terminal::Mate));
    let only = |n: &str| n == "xfce4-terminal" || n == "konsole";
    assert_eq!(pick(&["bash"], &only), Some(Terminal::Xfce));
    assert_eq!(pick(&["bash"], &none), None);
    // Windows: wt on PATH, nothing else.
    assert_eq!(
        select_terminal(&s(&["mate-terminal"]), &|n| n == "wt", true),
        Some(Terminal::Wt)
    );
    assert_eq!(
        select_terminal(&s(&["mate-terminal"]), &all, true),
        Some(Terminal::Wt)
    );
    assert_eq!(select_terminal(&[], &|n| n == "konsole", true), None);
    assert_eq!(select_windows_shell(&|n| n == "pwsh"), WindowsShell::Pwsh);
    assert_eq!(select_windows_shell(&none), WindowsShell::PowershellExe);
}

// ------------------------------------------------------------------ pair (dry run)

#[tokio::test]
async fn pair_dry_run_writes_nothing() {
    let mut rig = Rig::new();
    rig.on_path = vec!["konsole".into()];
    rig.chain = vec!["bash".into()];
    let before = entries(rig.home.path());
    let (r, out) = rig.pair(dry(&rig)).await;
    assert_eq!(r.unwrap(), 0, "{out}");
    assert_eq!(entries(rig.home.path()), before);
    assert!(rig.tabs.borrow().is_empty() && rig.ctrl.borrow().is_empty());
    assert!(rig.fake.calls().is_empty(), "no API call");
    assert!(rig.fake.lifecycle_calls().is_empty(), "no service start");
    assert!(!rig.home.path().join("logs").exists(), "no log file");
    assert!(out.lines().all(|l| l.starts_with("would ")), "{out}");
}

#[tokio::test]
async fn pair_dry_run_lines_and_order() {
    let rig = Rig::new();
    let (r, out) = rig.pair(dry(&rig)).await;
    assert_eq!(r.unwrap(), 0);
    let l = lines(&out);
    let wrapper = rig.home.path().join("pair.<pair-id>.g1.wrapper.sh");
    assert_eq!(l[0], "would run: opencode service start");
    assert_eq!(l[1], "would run: gc");
    assert!(l[2].starts_with("would call: session.create {"), "{out}");
    let body: Value =
        serde_json::from_str(l[2].strip_prefix("would call: session.create ").unwrap()).unwrap();
    assert_eq!(
        body["title"],
        "pair work 08:00Z".replace("work", &basename(&rig.dir_arg()))
    );
    assert_eq!(
        body["location"]["directory"],
        fs::canonicalize(rig.dir_arg())
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(
        l[3],
        format!("would run: gnome-terminal --tab -- {}", wrapper.display())
    );
    assert_eq!(l[4], "would run: claude");
    assert_eq!(l.len(), 5, "{out}");
}

fn basename(p: &Path) -> String {
    fs::canonicalize(p)
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[tokio::test]
async fn pair_runs_gc_after_service_up() {
    let rig = Rig::new();
    let (_, out) = rig.pair(dry(&rig)).await;
    let l = lines(&out);
    let at = |needle: &str| l.iter().position(|x| x.contains(needle)).unwrap();
    assert!(at("service start") < at("would run: gc"));
    assert!(at("would run: gc") < at("session.create"));
    // A real run does the same in the same order.
    let (r, _) = rig.pair(rig.args()).await;
    r.unwrap();
    let life = rig.fake.lifecycle_calls();
    assert_eq!(life.len(), 1);
    assert_eq!(life[0].0, Lifecycle::Start);
    let ops: Vec<String> = rig.fake.calls().into_iter().map(|c| c.op).collect();
    assert_eq!(
        ops[0], "session.active",
        "gc runs before session.create: {ops:?}"
    );
    assert_eq!(ops[1], "session.create");
}

#[tokio::test]
async fn pair_skips_service_start_with_server_url() {
    let settings = Settings {
        server_url: Some(URL.into()),
        ..Settings::default()
    };
    let rig = Rig::with(Host::Claude, settings);
    let (r, out) = rig.pair(dry(&rig)).await;
    assert_eq!(r.unwrap(), 0);
    assert!(!out.contains("would run: opencode service start"), "{out}");
    assert!(out.contains("would run: gc"), "{out}");
    let (r, _) = rig.pair(rig.args()).await;
    r.unwrap();
    assert!(
        rig.fake.lifecycle_calls().is_empty(),
        "no service start with a server_url"
    );
    assert!(
        rig.fake.calls().iter().any(|c| c.op == "session.active"),
        "gc ran"
    );
}

#[tokio::test]
async fn pair_passes_server_url_to_wrapper() {
    let settings = Settings {
        server_url: Some(URL.into()),
        ..Settings::default()
    };
    let mut rig = Rig::with(Host::Claude, settings);
    rig.password = Some("secret".into());
    let (r, _) = rig.pair(rig.args()).await;
    r.unwrap();
    let id = &pair_ids(rig.home.path())[0];
    let sh = fs::read_to_string(rig.home.path().join(format!("pair.{id}.g1.wrapper.sh"))).unwrap();
    assert!(sh.contains(&format!("-s '{SID}' --server '{URL}'")), "{sh}");
    assert!(
        sh.contains("export OPENCODE_SERVER_PASSWORD='secret'"),
        "{sh}"
    );
    let rec: Value =
        serde_json::from_slice(&fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap())
            .unwrap();
    assert_eq!(rec["server_url"], URL);
    // The password is in no argv and not in the record or the log.
    assert!(!rec.to_string().contains("secret"));
    for argv in rig
        .tabs
        .borrow()
        .iter()
        .chain(rig.ctrl.borrow().iter().map(|c| &c.0))
    {
        assert!(!argv.join(" ").contains("secret"));
    }
    let log = fs::read_to_string(rig.home.path().join(format!("logs/pair-{id}.log"))).unwrap();
    assert!(!log.contains("secret"), "{log}");
}

#[tokio::test]
async fn pair_applies_permission_policy() {
    let settings = Settings {
        permission_policy: PermissionPolicy::Allow,
        default_model: Some("openai/gpt-5#high".into()),
        ..Settings::default()
    };
    let rig = Rig::with(Host::Claude, settings);
    let (r, out) = rig.pair(dry(&rig)).await;
    assert_eq!(r.unwrap(), 0);
    let line = out
        .lines()
        .find(|l| l.starts_with("would call: session.create"))
        .unwrap();
    let body: Value =
        serde_json::from_str(line.strip_prefix("would call: session.create ").unwrap()).unwrap();
    assert_eq!(
        body["permissions"],
        json!([{"action": "*", "resource": "*", "effect": "allow"}])
    );
    assert_eq!(
        body["model"],
        json!({"providerID": "openai", "id": "gpt-5", "variant": "high"})
    );
    // --model overrides default_model.
    let args = PairArgs {
        model: Some("anthropic/claude-x".into()),
        title: Some("T".into()),
        ..dry(&rig)
    };
    let (_, out) = rig.pair(args).await;
    let line = out
        .lines()
        .find(|l| l.starts_with("would call: session.create"))
        .unwrap();
    let body: Value =
        serde_json::from_str(line.strip_prefix("would call: session.create ").unwrap()).unwrap();
    assert_eq!(body["model"]["providerID"], "anthropic");
    assert_eq!(body["title"], "T");
    // A bad model is refused even in a dry run, with nothing written.
    let args = PairArgs {
        model: Some("nonsense".into()),
        ..dry(&rig)
    };
    let (r, _) = rig.pair(args).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::InvalidArgument);
}

#[tokio::test]
async fn pair_controller_flag() {
    for (host, flag, want) in [
        (Host::Claude, None, "claude"),
        (Host::Qwen, None, "qwen"),
        (Host::Agy, None, "agy"),
        (Host::Claude, Some("qwen"), "qwen"),
        (Host::Qwen, Some("agy"), "agy"),
    ] {
        let rig = Rig::with(host, Settings::default());
        let args = PairArgs {
            controller: flag.map(str::to_string),
            controller_args: s(&["--resume", "x y"]),
            ..dry(&rig)
        };
        let (r, out) = rig.pair(args).await;
        r.unwrap();
        assert!(
            out.lines()
                .any(|l| l == format!("would run: {want} --resume x y")),
            "{host:?} {flag:?}: {out}"
        );
        // The record carries the controller of a real run.
        let (r, _) = rig
            .pair(PairArgs {
                controller: flag.map(str::to_string),
                ..rig.args()
            })
            .await;
        r.unwrap();
        let id = &pair_ids(rig.home.path())[0];
        let rec: Value = serde_json::from_slice(
            &fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(rec["controller"], want);
        assert_eq!(rig.ctrl.borrow()[0].0[0], want);
    }
    let rig = Rig::new();
    let args = PairArgs {
        controller: Some("emacs".into()),
        ..dry(&rig)
    };
    assert_eq!(
        rig.pair(args).await.0.unwrap_err().code,
        ErrorCode::InvalidArgument
    );
}

#[tokio::test]
async fn pair_no_terminal_tab_false() {
    // No parent matches and nothing on PATH: no tab, the attach command is printed.
    let mut rig = Rig::new();
    rig.chain = vec!["bash".into()];
    let (r, out) = rig.pair(rig.args()).await;
    r.unwrap();
    let id = pair_ids(rig.home.path())[0].clone();
    let rec: Value =
        serde_json::from_slice(&fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap())
            .unwrap();
    assert_eq!(rec["tab"], false);
    assert!(rig.tabs.borrow().is_empty());
    assert!(!rig
        .home
        .path()
        .join(format!("pair.{id}.g1.wrapper.sh"))
        .exists());
    assert!(out.contains("tui: none"), "{out}");
    assert!(out.contains(&format!("opencode-pair attach {id}")), "{out}");

    // --no-tab with a terminal available behaves the same.
    let rig = Rig::new();
    let (r, _) = rig
        .pair(PairArgs {
            no_tab: true,
            ..rig.args()
        })
        .await;
    r.unwrap();
    let id = pair_ids(rig.home.path())[0].clone();
    let rec: Value =
        serde_json::from_slice(&fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap())
            .unwrap();
    assert_eq!(rec["tab"], false);
    assert!(rig.tabs.borrow().is_empty());
    // And a dry run with --no-tab lists no tab command.
    let (_, out) = rig
        .pair(PairArgs {
            no_tab: true,
            ..dry(&rig)
        })
        .await;
    assert!(!out.contains("gnome-terminal"), "{out}");

    // With a terminal the record says tab: true.
    let rig = Rig::new();
    let (r, _) = rig.pair(rig.args()).await;
    r.unwrap();
    let id = pair_ids(rig.home.path())[0].clone();
    let rec: Value =
        serde_json::from_slice(&fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap())
            .unwrap();
    assert_eq!(rec["tab"], true);
    assert_eq!(rec["generation"], 1);
    assert_eq!(rig.tabs.borrow().len(), 1);
}

#[test]
fn pair_controller_env_server_empty_for_background() {
    let dir = Path::new("/w");
    let bg = controller_env(SID, dir, None, "0123456789ab");
    let get =
        |v: &[(String, String)], k: &str| v.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    assert_eq!(get(&bg, "OPENCODE_CTL_SERVER"), Some(String::new()));
    assert_eq!(get(&bg, "OPENCODE_CTL_SESSION"), Some(SID.into()));
    assert_eq!(get(&bg, "OPENCODE_CTL_DIR"), Some("/w".into()));
    assert_eq!(get(&bg, "OPENCODE_CTL_PAIR"), Some("0123456789ab".into()));
    assert_eq!(bg.len(), 4);
    let url = controller_env(SID, dir, Some(URL), "0123456789ab");
    assert_eq!(get(&url, "OPENCODE_CTL_SERVER"), Some(URL.into()));
}

#[tokio::test]
async fn pair_controller_env_reaches_the_spawned_command() {
    let rig = Rig::new();
    let (r, _) = rig.pair(rig.args()).await;
    r.unwrap();
    let envs = rig.ctrl.borrow()[0].1.clone();
    let get = |k: &str| envs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    assert_eq!(get("OPENCODE_CTL_SERVER"), Some(Some(String::new())));
    assert_eq!(get("OPENCODE_CTL_SESSION"), Some(Some(SID.into())));
    let settings = Settings {
        server_url: Some(URL.into()),
        ..Settings::default()
    };
    let rig = Rig::with(Host::Claude, settings);
    rig.pair(rig.args()).await.0.unwrap();
    let envs = rig.ctrl.borrow()[0].1.clone();
    assert_eq!(
        envs.iter()
            .find(|(n, _)| n == "OPENCODE_CTL_SERVER")
            .unwrap()
            .1,
        Some(URL.to_string())
    );
}

// ------------------------------------------------------------------ pair (real run, fakes)

#[tokio::test]
async fn pair_record_written_once() {
    let rig = Rig::new();
    let (r, out) = rig.pair(rig.args()).await;
    assert_eq!(r.unwrap(), 0, "{out}");
    let id = pair_ids(rig.home.path())[0].clone();
    let after = fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap();
    // Byte-identical when the controller started and when the launcher finished.
    assert_eq!(
        rig.record_at_spawn.borrow().as_deref(),
        Some(after.as_slice())
    );
    let rec: Value = serde_json::from_slice(&after).unwrap();
    assert_eq!(rec["session_id"], SID);
    assert_eq!(rec["pair_id"], id.as_str());
    assert_eq!(rec["created"], NOW);
    assert_eq!(rec["generation"], 1);
    assert!(rec["server_url"].is_null());
    assert_eq!(id.len(), 12);
    // claude-pid / claude-exited were written around the controller.
    let pid = fs::read_to_string(rig.home.path().join(format!("pair.{id}.claude-pid"))).unwrap();
    assert_eq!(pid, "4242:boot:777");
    assert!(rig
        .home
        .path()
        .join(format!("pair.{id}.claude-exited"))
        .is_file());
    // The exit notice and the closing instructions.
    let bodies = rig.fake_calls("session.synthetic");
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0]["resume"], false);
    assert!(bodies[0].get("id").is_none());
    assert!(bodies[0]["text"]
        .as_str()
        .unwrap()
        .contains("treat further instructions as theirs"));
    assert!(
        out.contains(&format!("worker session {SID} is still running")),
        "{out}"
    );
    assert!(out.contains(&format!("opencode-pair attach {id}")), "{out}");
    assert!(out.contains(&format!("opencode-pair resume {id}")), "{out}");
    for name in ["SESSION", "DIR", "SERVER", "PAIR"] {
        assert!(
            out.lines()
                .any(|l| l.starts_with(&format!("export OPENCODE_CTL_{name}="))),
            "{name}: {out}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn wrapper_and_record_modes() {
    use std::os::unix::fs::PermissionsExt;
    let rig = Rig::new();
    rig.pair(rig.args()).await.0.unwrap();
    let id = pair_ids(rig.home.path())[0].clone();
    let mode = |n: &str| {
        fs::metadata(rig.home.path().join(n))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode(&format!("pair.{id}.g1.wrapper.sh")), 0o700);
    assert_eq!(mode(&format!("pair.{id}.json")), 0o600);
    assert_eq!(mode(&format!("pair.{id}.claude-pid")), 0o600);
    assert_eq!(mode(&format!("pair.{id}.claude-exited")), 0o600);
}

#[tokio::test]
async fn pair_rolls_back_wrapper_on_tab_failure() {
    let rig = Rig::new();
    rig.tab_fails.set(true);
    let (r, _) = rig.pair(rig.args()).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::IoError);
    let removed = rig
        .fake
        .calls()
        .into_iter()
        .filter(|c| c.op == "session.remove")
        .collect::<Vec<_>>();
    assert_eq!(removed.len(), 1);
    assert_eq!(
        removed[0].params,
        vec![("sessionID".to_string(), SID.to_string())]
    );
    let left: Vec<String> = entries(rig.home.path())
        .into_iter()
        .filter(|n| n.starts_with("pair."))
        .collect();
    assert!(left.is_empty(), "{left:?}");
    assert!(rig.ctrl.borrow().is_empty(), "the controller never started");
}

#[cfg(unix)]
#[tokio::test]
async fn pair_removes_session_on_record_failure() {
    use std::os::unix::fs::PermissionsExt;
    let rig = Rig::new();
    fs::set_permissions(rig.home.path(), fs::Permissions::from_mode(0o500)).unwrap();
    if fs::File::create(rig.home.path().join("probe")).is_ok() {
        fs::set_permissions(rig.home.path(), fs::Permissions::from_mode(0o700)).unwrap();
        eprintln!("skipped: running as root");
        return;
    }
    let (r, _) = rig.pair(rig.args()).await;
    fs::set_permissions(rig.home.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(r.unwrap_err().code, ErrorCode::IoError);
    assert_eq!(rig.fake_calls("session.remove").len(), 1);
    assert!(rig.tabs.borrow().is_empty() && rig.ctrl.borrow().is_empty());
    assert!(pair_ids(rig.home.path()).is_empty());
}

#[tokio::test]
async fn launcher_opencode_missing_writes_nothing() {
    let mut rig = Rig::new();
    rig.opencode = None;
    let before = entries(rig.home.path());
    let (r, out) = rig.pair(rig.args()).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::OpencodeNotFound);
    assert!(out.is_empty());
    assert_eq!(entries(rig.home.path()), before, "home untouched");
    assert!(!rig.home.path().join("logs").exists(), "no logs/ entry");
    assert!(
        rig.fake.calls().is_empty() && rig.fake.lifecycle_calls().is_empty(),
        "no gc, no service"
    );
    // Also for a dry run.
    let (r, _) = rig.pair(dry(&rig)).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::OpencodeNotFound);
    // `attach <pair-id>` on an existing record (Task 19): the record holds the values it validates.
    let mut rig = Rig::new();
    let id = seed(&rig, 1, false, Some("claude"), None);
    rig.opencode = None;
    let record = fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap();
    let before = entries(rig.home.path());
    let (r, out) = rig.attach(&id).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::OpencodeNotFound);
    assert!(out.is_empty());
    assert_eq!(
        fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap(),
        record,
        "record bytes unchanged"
    );
    assert_eq!(entries(rig.home.path()), before, "no new wrapper, no logs/");
    assert!(rig.tabs.borrow().is_empty());
}

#[tokio::test]
async fn launcher_validation_precedes_everything() {
    let rig = Rig::new();
    let args = PairArgs {
        directory: Some(rig.ws.path().join("does-not-exist")),
        ..PairArgs::default()
    };
    let (r, _) = rig.pair(args).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::InvalidArgument);
    assert!(rig.fake.calls().is_empty() && entries(rig.home.path()).is_empty());
    let bad = rig.ws.path().join("a\nb");
    if fs::create_dir(&bad).is_ok() {
        let args = PairArgs {
            directory: Some(bad),
            ..PairArgs::default()
        };
        let (r, _) = rig.pair(args).await;
        assert_eq!(r.unwrap_err().code, ErrorCode::InvalidArgument);
        assert!(rig.fake.calls().is_empty() && entries(rig.home.path()).is_empty());
    }
}

#[tokio::test]
async fn launcher_writes_pair_log() {
    let rig = Rig::new();
    rig.pair(rig.args()).await.0.unwrap();
    let id = pair_ids(rig.home.path())[0].clone();
    let log = fs::read_to_string(rig.home.path().join(format!("logs/pair-{id}.log"))).unwrap();
    for step in [
        "service started",
        "gc:",
        "session created",
        "record written",
        "wrapper written",
        "tab spawned",
        "controller started",
        "controller exited",
        "exit notice posted",
    ] {
        assert!(log.contains(step), "{step:?} missing from:\n{log}");
    }
    // A dry run leaves no log.
    let rig = Rig::new();
    rig.pair(dry(&rig)).await.0.unwrap();
    assert!(!rig.home.path().join("logs").exists());
}

#[tokio::test]
async fn controller_not_found_keeps_record() {
    for kind in [io::ErrorKind::NotFound, io::ErrorKind::PermissionDenied] {
        let rig = Rig::new();
        rig.ctrl_err.set(Some(kind));
        let (r, out) = rig.pair(rig.args()).await;
        let e = r.unwrap_err();
        assert_eq!(e.code, ErrorCode::ControllerNotFound, "{kind:?}");
        let id = pair_ids(rig.home.path())[0].clone();
        assert!(rig.home.path().join(format!("pair.{id}.json")).is_file());
        assert!(rig
            .home
            .path()
            .join(format!("pair.{id}.g1.wrapper.sh"))
            .is_file());
        assert_eq!(rig.tabs.borrow().len(), 1);
        assert!(
            rig.fake_calls("session.remove").is_empty(),
            "the session stays"
        );
        assert!(rig.fake_calls("session.synthetic").is_empty());
        assert!(out.contains(&format!("opencode-pair attach {id}")), "{out}");
        assert!(out.contains(&format!("opencode-pair resume {id}")), "{out}");
    }
}

fn conflict(msg: &str) -> TransportError {
    TransportError {
        code: ErrorCode::Conflict,
        message: msg.into(),
        hint: None,
        details: None,
    }
}

/// A rig whose transport has the given `session.synthetic` script.
fn rig_with_synthetic(script: Vec<Result<Value, TransportError>>) -> Rig {
    let f = FakeTransport::new();
    f.on("session.active", vec![Ok(json!({"data": {}}))]);
    f.on("session.create", vec![Ok(json!({"data": {"id": SID}}))]);
    f.on("session.synthetic", script);
    let mut rig = Rig::new();
    rig.fake = f.clone();
    rig.engine = Engine::new(
        Settings::default(),
        Vec::new(),
        Host::Claude,
        env_unpaired(&rig.home),
        f,
        TestClock::new(NOW),
        rig.probe.clone() as Arc<dyn ProcessProbe>,
    );
    rig
}

#[tokio::test]
async fn exit_notice_409_retry() {
    let rig = rig_with_synthetic(vec![
        Err(conflict(
            "Synthetic input ID conflicts with an existing message",
        )),
        Ok(json!({"data": {}})),
    ]);
    let (r, _) = rig.pair(rig.args()).await;
    assert_eq!(r.unwrap(), 0);
    let bodies = rig.fake_calls("session.synthetic");
    assert_eq!(bodies.len(), 2, "one retry");
    assert_eq!(bodies[0], bodies[1], "same body");
    assert!(bodies[0].get("id").is_none());

    // Any other conflict, and any other failure, is logged and not retried; never fatal.
    for err in [
        conflict("something else entirely"),
        TransportError {
            code: ErrorCode::CliError,
            message: "boom".into(),
            hint: None,
            details: None,
        },
    ] {
        let rig = rig_with_synthetic(vec![Err(err)]);
        let (r, _) = rig.pair(rig.args()).await;
        assert_eq!(r.unwrap(), 0);
        assert_eq!(rig.fake_calls("session.synthetic").len(), 1);
    }
    // A conflict on the retry too: still one retry only.
    let rig = rig_with_synthetic(vec![Err(conflict("Synthetic input ID conflicts"))]);
    assert_eq!(rig.pair(rig.args()).await.0.unwrap(), 0);
    assert_eq!(rig.fake_calls("session.synthetic").len(), 2);
}

// ------------------------------------------------------------------ name dispatch

#[test]
fn pair_argv0_dispatch_table() {
    let sub = |n: &str, rest: &[&str]| Route::Sub {
        name: n.into(),
        rest: s(rest),
    };
    for argv0 in [
        "opencode-pair",
        "/home/u/.local/bin/opencode-pair",
        "opencode-pair.exe",
        r"C:\Users\u\AppData\Local\opencode-ctl\opencode-pair.exe",
    ] {
        let r = |a: &[&str]| route(Some(argv0), &s(a));
        assert_eq!(r(&[]), Route::Pair(vec![]), "{argv0}");
        assert_eq!(r(&["/some/dir"]), Route::Pair(s(&["/some/dir"])));
        assert_eq!(r(&["--dry-run"]), Route::Pair(s(&["--dry-run"])));
        assert_eq!(r(&["./attach"]), Route::Pair(s(&["./attach"])));
        assert_eq!(r(&["serve"]), Route::Pair(s(&["serve"])));
        assert_eq!(r(&["resume", "x"]), sub("resume", &["x"]));
        assert_eq!(r(&["attach", "x"]), sub("attach", &["x"]));
        assert_eq!(r(&["gc"]), sub("gc", &[]));
        assert_eq!(
            r(&["install-launcher", "--force"]),
            sub("install-launcher", &["--force"])
        );
        assert_eq!(
            r(&["doctor-json", "verbose"]),
            sub("doctor-json", &["verbose"])
        );
        assert_eq!(r(&["--version"]), Route::Version);
    }
    // The long binary names always take an explicit subcommand.
    for argv0 in [
        Some("opencode-ctl-cc-mcp"),
        Some("/x/opencode-ctl-qwen-mcp"),
        None,
    ] {
        assert_eq!(route(argv0, &[]), Route::Usage);
        assert_eq!(route(argv0, &s(&["/some/dir"])), Route::Usage);
        assert_eq!(route(argv0, &s(&["pair", "/d"])), sub("pair", &["/d"]));
    }
    // A look-alike is not the launcher name.
    assert_eq!(route(Some("my-opencode-pair"), &s(&["/d"])), Route::Usage);
    assert_eq!(route(Some("opencode-pair-old"), &[]), Route::Usage);
}

#[tokio::test]
async fn opencode_host_withholds_launcher() {
    for name in ["pair", "resume", "attach", "install-launcher"] {
        let code = dispatch(
            Host::Opencode,
            "opencode-ctl-cc-mcp",
            "0.1.0",
            None,
            s(&[name]),
        )
        .await;
        assert_eq!(code, 2, "{name}");
    }
    assert_eq!(
        dispatch(
            Host::Opencode,
            "opencode-ctl-cc-mcp",
            "0.1.0",
            Some("opencode-pair"),
            s(&["/d"])
        )
        .await,
        2
    );
    assert_eq!(
        opencode_ctl::launcher::NOT_OFFERED,
        "not offered on the OpenCode host"
    );
    assert!(!Host::Opencode.offers_launcher());
    // `pair` itself refuses too.
    let rig = Rig::with(Host::Opencode, Settings::default());
    let (r, _) = rig.pair(rig.args()).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::RefusedOnHost);
    assert!(rig.fake.calls().is_empty());
}

// ------------------------------------------------------------------ install-launcher

struct Installer {
    bin: TempHome,
    exe_dir: TempHome,
}

impl Installer {
    fn new() -> Installer {
        Installer {
            bin: temp_home(),
            exe_dir: temp_home(),
        }
    }

    fn exe(&self, name: &str) -> PathBuf {
        let p = self.exe_dir.path().join(name);
        fs::write(&p, format!("binary {name}")).unwrap();
        p
    }

    fn run(
        &self,
        exe: &Path,
        host: Host,
        force: bool,
        windows: bool,
        on_path: bool,
    ) -> (Result<(), ToolError>, String) {
        let probe = FakeProbe::new("boot");
        let mut out: Vec<u8> = Vec::new();
        let home = temp_home();
        let now = || NOW;
        let path_has = move |n: &str| on_path && n.starts_with("opencode-pair");
        let chain = Vec::new;
        let sd = |_: &mut Command| -> io::Result<u32> { Ok(1) };
        let sw = |_: &mut Command,
                  _: &mut dyn FnMut(u32)|
         -> io::Result<std::process::ExitStatus> { Ok(exit_status(0)) };
        let mut ctx = LauncherCtx {
            home: home.path(),
            bin_dir: self.bin.path(),
            running_exe: exe,
            host,
            probe: &probe,
            now_ms: &now,
            path_has: &path_has,
            opencode_bin: None,
            version: "0.1.0",
            parent_chain: &chain,
            server_password: None,
            spawn_detached: &sd,
            spawn_and_wait: &sw,
            before_claim: None,
            out: &mut out,
        };
        let r = install_launcher_as(&mut ctx, force, windows);
        (r, String::from_utf8(out).unwrap())
    }
}

#[cfg(unix)]
#[test]
fn install_launcher_conflict_rules() {
    use std::os::unix::fs::symlink;
    let i = Installer::new();
    let mine = i.exe("opencode-ctl-cc-mcp");
    let link = i.bin.path().join("opencode-pair");
    // Missing: created, with a PATH hint when the directory is not on PATH.
    let (r, out) = i.run(&mine, Host::Claude, false, false, false);
    r.unwrap();
    assert_eq!(fs::read_link(&link).unwrap(), mine);
    assert!(out.contains("PATH"), "{out}");
    // Fresh: left as is, no hint when it resolves on PATH.
    let (r, out) = i.run(&mine, Host::Claude, false, false, true);
    r.unwrap();
    assert!(!out.contains("not on PATH"), "{out}");
    // Stale: a link to THIS host's binary name elsewhere is replaced.
    let old_dir = temp_home();
    let old = old_dir.path().join("opencode-ctl-cc-mcp");
    fs::write(&old, "old").unwrap();
    fs::remove_file(&link).unwrap();
    symlink(&old, &link).unwrap();
    i.run(&mine, Host::Claude, false, false, true).0.unwrap();
    assert_eq!(fs::read_link(&link).unwrap(), mine);
    // Another host's link: LAUNCHER_CONFLICT with the --force hint, untouched; --force takes it.
    let qwen = i.exe("opencode-ctl-qwen-mcp");
    fs::remove_file(&link).unwrap();
    symlink(&qwen, &link).unwrap();
    let e = i
        .run(&mine, Host::Claude, false, false, true)
        .0
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::LauncherConflict);
    assert!(
        e.hint
            .as_deref()
            .unwrap()
            .contains("install-launcher --force"),
        "{e:?}"
    );
    assert_eq!(fs::read_link(&link).unwrap(), qwen);
    i.run(&mine, Host::Claude, true, false, true).0.unwrap();
    assert_eq!(fs::read_link(&link).unwrap(), mine);
    // A symlink elsewhere (not one of ours) and a regular file are never touched, --force or not.
    let other = i.exe("some-other-tool");
    fs::remove_file(&link).unwrap();
    symlink(&other, &link).unwrap();
    for force in [false, true] {
        let e = i
            .run(&mine, Host::Claude, force, false, true)
            .0
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::LauncherConflict);
        assert_eq!(fs::read_link(&link).unwrap(), other);
    }
    fs::remove_file(&link).unwrap();
    fs::write(&link, "regular").unwrap();
    for force in [false, true] {
        let e = i
            .run(&mine, Host::Claude, force, false, true)
            .0
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::LauncherConflict);
        assert_eq!(fs::read_to_string(&link).unwrap(), "regular");
    }
}

#[cfg(unix)]
#[test]
fn install_launcher_other_host_conflict() {
    use std::os::unix::fs::symlink;
    let i = Installer::new();
    let link = i.bin.path().join("opencode-pair");
    let agy = i.exe("opencode-ctl-agy-mcp");
    symlink(&agy, &link).unwrap();
    for (host, exe_name) in [
        (Host::Qwen, "opencode-ctl-qwen-mcp"),
        (Host::Claude, "opencode-ctl-cc-mcp"),
    ] {
        let exe = i.exe(exe_name);
        let e = i.run(&exe, host, false, false, true).0.unwrap_err();
        assert_eq!(e.code, ErrorCode::LauncherConflict, "{host:?}");
        assert_eq!(fs::read_link(&link).unwrap(), agy);
    }
}

#[cfg(unix)]
#[test]
fn install_launcher_creates_bin_dir() {
    use std::os::unix::fs::PermissionsExt;
    let i = Installer::new();
    let nested = i.bin.path().join("a").join("bin");
    let probe = FakeProbe::new("boot");
    let home = temp_home();
    let exe = i.exe("opencode-ctl-cc-mcp");
    let mut out: Vec<u8> = Vec::new();
    let now = || NOW;
    let ph = |_: &str| false;
    let chain = Vec::new;
    let sd = |_: &mut Command| -> io::Result<u32> { Ok(1) };
    let sw = |_: &mut Command, _: &mut dyn FnMut(u32)| -> io::Result<std::process::ExitStatus> {
        Ok(exit_status(0))
    };
    let mut ctx = LauncherCtx {
        home: home.path(),
        bin_dir: &nested,
        running_exe: &exe,
        host: Host::Claude,
        probe: &probe,
        now_ms: &now,
        path_has: &ph,
        opencode_bin: None,
        version: "0.1.0",
        parent_chain: &chain,
        server_password: None,
        spawn_detached: &sd,
        spawn_and_wait: &sw,
        before_claim: None,
        out: &mut out,
    };
    install_launcher_as(&mut ctx, false, false).unwrap();
    assert!(nested.join("opencode-pair").is_symlink());
    assert_eq!(
        fs::metadata(&nested).unwrap().permissions().mode() & 0o777,
        0o700
    );
}

#[test]
fn install_launcher_source_file() {
    let i = Installer::new();
    let exe = i.exe("opencode-ctl-cc-mcp");
    let copy = i.bin.path().join("opencode-pair.exe");
    let source = i.bin.path().join("opencode-pair.exe.source");
    i.run(&exe, Host::Claude, false, true, true).0.unwrap();
    assert_eq!(fs::read(&copy).unwrap(), fs::read(&exe).unwrap());
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        format!("path={}\nversion=0.1.0\n", exe.display())
    );
    // Re-running with an updated binary replaces the copy; the old one is renamed to `.old` first.
    fs::write(&exe, "binary v2").unwrap();
    i.run(&exe, Host::Claude, false, true, true).0.unwrap();
    assert_eq!(fs::read_to_string(&copy).unwrap(), "binary v2");
    assert_eq!(
        fs::read_to_string(i.bin.path().join("opencode-pair.exe.old")).unwrap(),
        "binary opencode-ctl-cc-mcp"
    );
    // Another host's copy: LAUNCHER_CONFLICT (hint --force), replaced with --force.
    let qwen = i.exe("opencode-ctl-qwen-mcp");
    let e = i.run(&qwen, Host::Qwen, false, true, true).0.unwrap_err();
    assert_eq!(e.code, ErrorCode::LauncherConflict);
    assert!(e.hint.as_deref().unwrap().contains("--force"));
    i.run(&qwen, Host::Qwen, true, true, true).0.unwrap();
    assert!(fs::read_to_string(&source)
        .unwrap()
        .starts_with(&format!("path={}", qwen.display())));
    // A copy with no parsable `.source` is not ours: conflict unless forced.
    fs::write(&source, "garbage").unwrap();
    let e = i.run(&qwen, Host::Qwen, false, true, true).0.unwrap_err();
    assert_eq!(e.code, ErrorCode::LauncherConflict);
    i.run(&qwen, Host::Qwen, true, true, true).0.unwrap();
}

#[test]
fn doctor_conflict_hint_names_force() {
    // The doctor's other_host wording and install-launcher's refusal name the same flag.
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let i = Installer::new();
        let qwen = i.exe("opencode-ctl-qwen-mcp");
        symlink(&qwen, i.bin.path().join("opencode-pair")).unwrap();
        let mine = i.exe("opencode-ctl-cc-mcp");
        let e = i
            .run(&mine, Host::Claude, false, false, true)
            .0
            .unwrap_err();
        let hint = e.hint.unwrap();
        assert!(hint.contains("install-launcher --force"), "{hint}");
        assert!(e.message.contains("opencode-pair"), "{}", e.message);
    }
}

// ------------------------------------------------------------------ Task 19: resume and attach

use opencode_ctl::pairing::{write_record_new, Record};
use std::sync::Barrier;

const SID_OTHER: &str = "ses_ABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// A record in the rig's home for the rig's workspace; returns the pair id.
fn seed(
    rig: &Rig,
    generation: u32,
    tab: bool,
    controller: Option<&str>,
    server_url: Option<&str>,
) -> String {
    let id = "0123456789ab".to_string();
    write_record_new(
        rig.home.path(),
        &Record {
            pair_id: id.clone(),
            session_id: SID.to_string(),
            directory: rig.ws.path().to_path_buf(),
            server_url: server_url.map(str::to_string),
            created: 1,
            controller: controller.map(str::to_string),
            generation,
            tab,
        },
    )
    .unwrap();
    id
}

fn put(rig: &Rig, name: &str, content: &str) {
    fs::write(rig.home.path().join(name), content).unwrap();
}

fn has(rig: &Rig, name: &str) -> bool {
    rig.home.path().join(name).exists()
}

fn read_json(rig: &Rig, id: &str) -> Value {
    serde_json::from_slice(&fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap())
        .unwrap()
}

fn not_found() -> TransportError {
    TransportError {
        code: ErrorCode::SessionNotFound,
        message: "session not found".into(),
        hint: None,
        details: None,
    }
}

/// `session.get` answers with the rig's workspace as the session's directory.
fn script_session_get(rig: &Rig) {
    rig.fake.on(
        "session.get",
        vec![Ok(json!({"data": {
            "id": SID,
            "location": {"directory": rig.ws.path().to_string_lossy()}
        }}))],
    );
}

fn pair_files(home: &Path) -> Vec<String> {
    entries(home)
        .into_iter()
        .filter(|n| n.starts_with("pair."))
        .collect()
}

#[tokio::test]
async fn attach_bumps_generation() {
    let rig = Rig::new();
    let id = seed(&rig, 1, false, Some("qwen"), Some(URL));
    let before = read_json(&rig, &id);
    let (r, out) = rig.attach(&id).await;
    assert_eq!(r.unwrap(), 0, "{out}");
    let after = read_json(&rig, &id);
    assert_eq!(after["generation"], 2);
    assert_eq!(after["tab"], true);
    for k in [
        "pair_id",
        "session_id",
        "directory",
        "server_url",
        "created",
        "controller",
    ] {
        assert_eq!(after[k], before[k], "{k} is not rewritten");
    }
    let w = wrapper_path(rig.home.path(), &id, 2, false);
    assert!(w.is_file(), "the g2 wrapper is the claim token");
    assert!(!has(&rig, &format!("pair.{id}.g1.wrapper.sh")));
    let script = fs::read_to_string(&w).unwrap();
    assert!(
        script.contains(&format!("pair.{id}.g2.tui-pid")),
        "{script}"
    );
    assert!(
        script.contains("--server"),
        "the record's server_url: {script}"
    );
    let tabs = rig.tabs.borrow();
    assert_eq!(tabs.len(), 1);
    assert!(tabs[0].iter().any(|a| a == w.to_string_lossy().as_ref()));
    assert!(rig.fake.calls().is_empty(), "a pair id needs no API call");
    assert!(rig
        .home
        .path()
        .join("logs")
        .join(format!("pair-{id}.log"))
        .is_file());
}

#[cfg(unix)]
#[tokio::test]
async fn attach_wrapper_mode_is_0700() {
    use std::os::unix::fs::PermissionsExt;
    let rig = Rig::new();
    let id = seed(&rig, 1, true, None, None);
    rig.attach(&id).await.0.unwrap();
    let mode = fs::metadata(wrapper_path(rig.home.path(), &id, 2, false))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700);
}

#[tokio::test]
async fn attach_deletes_generations_older_than_n() {
    let rig = Rig::new();
    let id = seed(&rig, 3, true, None, None);
    for g in 1..=3 {
        for f in ["wrapper.sh", "tui-pid", "tui-exited"] {
            put(&rig, &format!("pair.{id}.g{g}.{f}"), "x");
        }
    }
    rig.attach(&id).await.0.unwrap();
    for g in 1..=2 {
        for f in ["wrapper.sh", "tui-pid", "tui-exited"] {
            assert!(!has(&rig, &format!("pair.{id}.g{g}.{f}")), "g{g} {f}");
        }
    }
    assert!(has(&rig, &format!("pair.{id}.g3.tui-pid")), "g<n> is kept");
    assert!(has(&rig, &format!("pair.{id}.g4.wrapper.sh")));
    assert_eq!(read_json(&rig, &id)["generation"], 4);
}

/// The standalone driver for the threaded tests: its own fakes, the shared home.
fn attach_alone(
    home: &Path,
    ws: &Path,
    bin: &Path,
    id: &str,
    before: Option<&dyn Fn()>,
) -> (Result<i32, ToolError>, Vec<Vec<String>>) {
    let probe = FakeProbe::new("boot");
    let fake = FakeTransport::new();
    let engine = Engine::new(
        Settings::default(),
        Vec::new(),
        Host::Claude,
        opencode_ctl::engine::EnvSnapshot::for_tests(ws.to_path_buf(), home.to_path_buf()),
        fake,
        TestClock::new(NOW),
        Arc::new(FakeProbe::new("boot")) as Arc<dyn ProcessProbe>,
    );
    let tabs = RefCell::new(Vec::new());
    let now = || NOW;
    let path_has = |_: &str| false;
    let chain = || vec!["gnome-terminal-server".to_string()];
    let spawn_detached = |c: &mut Command| -> io::Result<u32> {
        let mut argv = vec![c.get_program().to_string_lossy().into_owned()];
        argv.extend(c.get_args().map(|a| a.to_string_lossy().into_owned()));
        tabs.borrow_mut().push(argv);
        Ok(1)
    };
    let spawn_and_wait =
        |_: &mut Command, _: &mut dyn FnMut(u32)| -> io::Result<_> { Ok(exit_status(0)) };
    let exe = PathBuf::from("/usr/bin/opencode-ctl-cc-mcp");
    let opencode = bin.join("opencode");
    let mut out: Vec<u8> = Vec::new();
    let mut ctx = LauncherCtx {
        home,
        bin_dir: bin,
        running_exe: &exe,
        host: Host::Claude,
        probe: &probe,
        now_ms: &now,
        path_has: &path_has,
        opencode_bin: Some(&opencode),
        version: "0.1.0",
        parent_chain: &chain,
        server_password: None,
        spawn_detached: &spawn_detached,
        spawn_and_wait: &spawn_and_wait,
        before_claim: before,
        out: &mut out,
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let r = rt.block_on(attach(id, &mut ctx, &engine));
    (r, tabs.into_inner())
}

#[test]
fn attach_exclusive_create() {
    let rig = Rig::new();
    let id = seed(&rig, 1, false, None, None);
    let barrier = Barrier::new(2);
    let (home, ws, bin) = (
        rig.home.path().to_path_buf(),
        rig.ws.path().to_path_buf(),
        rig.bin.path().to_path_buf(),
    );
    let run = || {
        // Both threads have read generation 1 before either claims.
        let before = || {
            barrier.wait();
        };
        attach_alone(&home, &ws, &bin, &id, Some(&before))
    };
    let (a, b) = std::thread::scope(|sc| {
        let ta = sc.spawn(run);
        let tb = sc.spawn(run);
        (ta.join().unwrap(), tb.join().unwrap())
    });
    assert_eq!(a.0.unwrap(), 0);
    assert_eq!(b.0.unwrap(), 0);
    let claimed: Vec<String> = entries(&home)
        .into_iter()
        .filter(|n| n.ends_with(".wrapper.sh"))
        .collect();
    assert_eq!(
        claimed,
        vec![
            format!("pair.{id}.g2.wrapper.sh"),
            format!("pair.{id}.g3.wrapper.sh")
        ],
        "winner n+1 and loser n+2"
    );
    assert_eq!(
        read_json(&rig, &id)["generation"],
        3,
        "final generation n+2"
    );
    let mut tabs = a.1;
    tabs.extend(b.1);
    assert_eq!(tabs.len(), 2);
}

#[tokio::test]
async fn attach_loser_retries_then_conflict() {
    // A racing winner takes n+1 between the read and the claim: the loser takes n+2.
    let rig = Rig::new();
    let id = seed(&rig, 1, false, None, None);
    let home = rig.home.path().to_path_buf();
    let wid = id.clone();
    let winner = move || {
        let w = wrapper_path(&home, &wid, 2, false);
        assert!(write_wrapper(&w, "#!/bin/sh\n", WrapperKind::Sh).unwrap());
    };
    let (r, _) = rig.drive(Op::Attach(id.clone()), Some(&winner)).await;
    r.unwrap();
    assert_eq!(read_json(&rig, &id)["generation"], 3);

    // n+1 .. n+5 all taken: LAUNCHER_CONFLICT, nothing rewritten, no tab.
    let rig = Rig::new();
    let id = seed(&rig, 1, false, None, None);
    for g in 2..=6 {
        put(&rig, &format!("pair.{id}.g{g}.wrapper.sh"), "claimed");
    }
    let record = fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap();
    let (r, _) = rig.attach(&id).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::LauncherConflict);
    assert_eq!(
        fs::read(rig.home.path().join(format!("pair.{id}.json"))).unwrap(),
        record
    );
    assert!(rig.tabs.borrow().is_empty());
    assert!(
        !has(&rig, &format!("pair.{id}.g7.wrapper.sh")),
        "five tries only"
    );

    // The fifth try still wins.
    let rig = Rig::new();
    let id = seed(&rig, 1, false, None, None);
    for g in 2..=5 {
        put(&rig, &format!("pair.{id}.g{g}.wrapper.sh"), "claimed");
    }
    rig.attach(&id).await.0.unwrap();
    assert_eq!(read_json(&rig, &id)["generation"], 6);
}

#[tokio::test]
async fn attach_skips_orphaned_claims() {
    let rig = Rig::new();
    let id = seed(&rig, 1, false, None, None);
    // A crashed attach claimed g2 and never rewrote the record.
    put(&rig, &format!("pair.{id}.g2.wrapper.sh"), "orphan");
    rig.attach(&id).await.0.unwrap();
    assert_eq!(read_json(&rig, &id)["generation"], 3);
    assert_eq!(
        fs::read_to_string(wrapper_path(rig.home.path(), &id, 2, false)).unwrap(),
        "orphan",
        "the orphan is not overwritten"
    );
}

#[tokio::test]
async fn attach_clears_dead_seen() {
    let rig = Rig::new();
    let id = seed(&rig, 1, true, None, None);
    put(&rig, &format!("pair.{id}.dead-seen"), "1700000000000");
    rig.attach(&id).await.0.unwrap();
    assert!(!has(&rig, &format!("pair.{id}.dead-seen")));
}

#[tokio::test]
async fn attach_session_id_new_record() {
    let rig = Rig::new();
    script_session_get(&rig);
    let (r, out) = rig.attach(SID).await;
    assert_eq!(r.unwrap(), 0, "{out}");
    let ids = pair_ids(rig.home.path());
    assert_eq!(ids.len(), 1);
    let id = &ids[0];
    assert!(opencode_ctl::pairing::valid_pair_id(id));
    let rec = read_json(&rig, id);
    assert_eq!(rec["session_id"], SID);
    assert_eq!(rec["pair_id"], id.as_str());
    assert_eq!(rec["controller"], Value::Null);
    assert_eq!(rec["generation"], 1);
    assert_eq!(rec["tab"], true);
    assert_eq!(rec["directory"], rig.ws.path().to_string_lossy().as_ref());
    assert!(wrapper_path(rig.home.path(), id, 1, false).is_file());
    assert_eq!(rig.tabs.borrow().len(), 1);
    assert_eq!(rig.fake_calls("session.get").len(), 1);
    assert!(rig.fake_calls("session.remove").is_empty());
    assert!(rig
        .home
        .path()
        .join("logs")
        .join(format!("pair-{id}.log"))
        .is_file());
}

#[tokio::test]
async fn attach_sets_tab_true() {
    let rig = Rig::new();
    let id = seed(&rig, 1, false, None, None);
    rig.attach(&id).await.0.unwrap();
    assert_eq!(read_json(&rig, &id)["tab"], true);

    // Derived from the rule text ("true only when a terminal was found"): no terminal -> false,
    // and the generation is still claimed and bumped.
    let mut rig = Rig::new();
    rig.chain = Vec::new();
    let id = seed(&rig, 1, true, None, None);
    let (r, out) = rig.attach(&id).await;
    r.unwrap();
    let rec = read_json(&rig, &id);
    assert_eq!(rec["tab"], false);
    assert_eq!(rec["generation"], 2);
    assert!(rig.tabs.borrow().is_empty());
    assert!(out.contains("tui: none"), "{out}");
}

#[tokio::test]
async fn attach_rejects_bad_target() {
    for bad in [
        "",
        "zzz",
        "ses_short",
        "0123456789AB",
        "0123456789a",
        "0123456789abc",
        "ses_abcdefghijklmnopqrstuvwxy!",
        "../x",
    ] {
        let rig = Rig::new();
        let before = entries(rig.home.path());
        let (r, _) = rig.attach(bad).await;
        assert_eq!(r.unwrap_err().code, ErrorCode::InvalidArgument, "{bad:?}");
        assert_eq!(entries(rig.home.path()), before, "{bad:?}");
        assert!(rig.fake.calls().is_empty(), "{bad:?}");
    }
    // A pair-id target that has no record.
    let rig = Rig::new();
    let (r, _) = rig.attach("0123456789ab").await;
    assert_eq!(r.unwrap_err().code, ErrorCode::PairNotFound);
}

#[tokio::test]
async fn attach_session_id_checks_binary_before_record() {
    let mut rig = Rig::new();
    rig.opencode = None;
    script_session_get(&rig);
    let before = entries(rig.home.path());
    let (r, out) = rig.attach(SID).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::OpencodeNotFound);
    assert!(out.is_empty());
    assert_eq!(entries(rig.home.path()), before, "home untouched");
    assert!(!rig.home.path().join("logs").exists(), "no logs/ entry");
    assert!(rig.tabs.borrow().is_empty());
}

#[tokio::test]
async fn attach_session_id_rolls_back_on_tab_failure() {
    let rig = Rig::new();
    script_session_get(&rig);
    rig.tab_fails.set(true);
    let (r, _) = rig.attach(SID).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::IoError);
    assert_eq!(pair_files(rig.home.path()), Vec::<String>::new());
    assert!(
        rig.fake_calls("session.remove").is_empty(),
        "attach created no session"
    );
}

#[tokio::test]
async fn attach_session_id_missing_session_writes_nothing() {
    let rig = Rig::new();
    rig.fake.on("session.get", vec![Err(not_found())]);
    let before = entries(rig.home.path());
    let (r, _) = rig.attach(SID_OTHER).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::SessionNotFound);
    assert_eq!(entries(rig.home.path()), before);
}

#[tokio::test]
async fn resume_execs_controller_from_record() {
    for (url, want) in [(None, ""), (Some(URL), URL)] {
        let rig = Rig::new();
        script_session_get(&rig);
        let id = seed(&rig, 1, true, Some("qwen"), url);
        let (r, out) = rig.resume(&id, &["--continue", "x y"]).await;
        assert_eq!(r.unwrap(), 0, "{out}");
        let ctrl = rig.ctrl.borrow();
        assert_eq!(ctrl.len(), 1);
        assert_eq!(ctrl[0].0, s(&["qwen", "--continue", "x y"]));
        let env = |k: &str| {
            ctrl[0]
                .1
                .iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(env("OPENCODE_CTL_SERVER"), Some(Some(want.to_string())));
        assert_eq!(env("OPENCODE_CTL_SESSION"), Some(Some(SID.to_string())));
        assert_eq!(
            env("OPENCODE_CTL_DIR"),
            Some(Some(rig.ws.path().to_string_lossy().into_owned()))
        );
        assert_eq!(env("OPENCODE_CTL_PAIR"), Some(Some(id.clone())));
        // claude-pid is bound, the lock is gone, claude-exited is written on return, the tab is
        // untouched and the notice is posted.
        assert!(has(&rig, &format!("pair.{id}.claude-pid")));
        assert!(!has(&rig, &format!("pair.{id}.resume-lock")));
        assert!(has(&rig, &format!("pair.{id}.claude-exited")));
        assert!(rig.tabs.borrow().is_empty());
        assert_eq!(rig.fake_calls("session.synthetic").len(), 1);
        assert_eq!(rig.fake_calls("session.get").len(), 1);
        let rec = read_json(&rig, &id);
        assert_eq!(rec["generation"], 1, "the record is not rewritten");
        assert!(out.contains(&format!("opencode-pair resume {id}")), "{out}");
    }
}

#[tokio::test]
async fn resume_controller_precedence() {
    // --controller, else the record, else the host constant (an attach record has null).
    for (host, record, flag, want) in [
        (Host::Claude, Some("qwen"), Some("agy"), "agy"),
        (Host::Claude, Some("qwen"), None, "qwen"),
        (Host::Qwen, None, None, "qwen"),
        (Host::Agy, None, Some("claude"), "claude"),
    ] {
        let rig = Rig::with(host, Settings::default());
        script_session_get(&rig);
        let id = seed(&rig, 1, true, record, None);
        let (r, _) = rig
            .drive(Op::Resume(id, flag.map(str::to_string), Vec::new()), None)
            .await;
        r.unwrap();
        assert_eq!(
            rig.ctrl.borrow()[0].0,
            s(&[want]),
            "{host:?} {record:?} {flag:?}"
        );
    }
    // An unknown --controller is refused before any state changes.
    let rig = Rig::new();
    script_session_get(&rig);
    let id = seed(&rig, 1, true, None, None);
    put(&rig, &format!("pair.{id}.claude-exited"), "1");
    let (r, _) = rig
        .drive(
            Op::Resume(id.clone(), Some("bash".into()), Vec::new()),
            None,
        )
        .await;
    assert_eq!(r.unwrap_err().code, ErrorCode::InvalidArgument);
    assert!(has(&rig, &format!("pair.{id}.claude-exited")));
    assert!(rig.ctrl.borrow().is_empty());
}

#[tokio::test]
async fn resume_refuses_alive_controller() {
    let rig = Rig::new();
    script_session_get(&rig);
    let id = seed(&rig, 1, true, None, None);
    // 4242 is alive in the rig with start time 777.
    put(&rig, &format!("pair.{id}.claude-pid"), "4242:boot:777");
    put(&rig, &format!("pair.{id}.claude-exited"), "1");
    let (r, _) = rig.resume(&id, &[]).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::LauncherConflict);
    assert!(rig.ctrl.borrow().is_empty());
    assert!(!has(&rig, &format!("pair.{id}.resume-lock")));
    assert!(
        has(&rig, &format!("pair.{id}.claude-exited")),
        "nothing was changed"
    );
    // A claude-pid naming a dead process does not refuse.
    put(&rig, &format!("pair.{id}.claude-pid"), "9999:boot:1");
    rig.resume(&id, &[]).await.0.unwrap();
    assert_eq!(rig.ctrl.borrow().len(), 1);
}

#[tokio::test]
async fn resume_lock_exclusive() {
    let rig = Rig::new();
    script_session_get(&rig);
    let id = seed(&rig, 1, true, None, None);
    // Another resume holds the lock and its owner is alive.
    put(&rig, &format!("pair.{id}.resume-lock"), "4242:boot:777");
    let (r, _) = rig.resume(&id, &[]).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::LauncherConflict);
    assert!(rig.ctrl.borrow().is_empty());
    assert_eq!(
        fs::read_to_string(rig.home.path().join(format!("pair.{id}.resume-lock"))).unwrap(),
        "4242:boot:777",
        "the winner's lock is untouched"
    );
    // An empty lock may be mid-creation: left alone, so the resume loses.
    put(&rig, &format!("pair.{id}.resume-lock"), "");
    let (r, _) = rig.resume(&id, &[]).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::LauncherConflict);
    assert!(rig.ctrl.borrow().is_empty());
}

#[tokio::test]
async fn resume_holds_its_own_lock_until_claude_pid_is_written() {
    let rig = Rig::new();
    script_session_get(&rig);
    rig.probe.set_alive(std::process::id(), "own-start");
    let id = seed(&rig, 1, true, None, None);
    rig.resume(&id, &[]).await.0.unwrap();
    let at = rig.at_spawn.borrow();
    let lock = at
        .iter()
        .find(|(n, _)| n == &format!("pair.{id}.resume-lock"))
        .expect("the lock exists while the controller is being spawned");
    assert_eq!(lock.1, format!("{}:boot:own-start", std::process::id()));
    assert!(!has(&rig, &format!("pair.{id}.resume-lock")));
}

#[tokio::test]
async fn resume_clears_exited_and_dead_seen() {
    let rig = Rig::new();
    script_session_get(&rig);
    let id = seed(&rig, 1, true, None, None);
    put(&rig, &format!("pair.{id}.claude-exited"), "1");
    put(&rig, &format!("pair.{id}.dead-seen"), "1");
    rig.resume(&id, &[]).await.0.unwrap();
    let at = rig.at_spawn.borrow();
    assert!(
        !at.iter().any(|(n, _)| n.ends_with(".dead-seen")),
        "dead-seen is gone before the spawn"
    );
    assert!(
        !at.iter().any(|(n, _)| n.ends_with(".claude-exited")),
        "claude-exited is gone before the spawn"
    );
    assert!(!has(&rig, &format!("pair.{id}.dead-seen")));
}

#[tokio::test]
async fn resume_sweeps_dead_lock_first() {
    let rig = Rig::new();
    script_session_get(&rig);
    rig.probe.set_alive(std::process::id(), "own-start");
    let id = seed(&rig, 1, true, None, None);
    // A crash between the lock's create and the claude-pid write left this (pid 9999 is dead).
    put(&rig, &format!("pair.{id}.resume-lock"), "9999:boot:1");
    let (r, out) = rig.resume(&id, &[]).await;
    assert_eq!(r.unwrap(), 0, "{out}");
    assert_eq!(rig.ctrl.borrow().len(), 1);
    let at = rig.at_spawn.borrow();
    let lock = at
        .iter()
        .find(|(n, _)| n.ends_with(".resume-lock"))
        .unwrap();
    assert!(
        lock.1.starts_with(&format!("{}:", std::process::id())),
        "our own line replaced it"
    );
}

#[tokio::test]
async fn resume_missing_session_not_found() {
    let rig = Rig::new();
    rig.fake.on("session.get", vec![Err(not_found())]);
    let id = seed(&rig, 1, true, None, None);
    put(&rig, &format!("pair.{id}.claude-exited"), "1");
    let (r, _) = rig.resume(&id, &[]).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::SessionNotFound);
    assert!(rig.ctrl.borrow().is_empty());
    assert!(!has(&rig, &format!("pair.{id}.resume-lock")));
    assert!(has(&rig, &format!("pair.{id}.claude-exited")));
}

#[tokio::test]
async fn resume_missing_record_is_pair_not_found() {
    let rig = Rig::new();
    let (r, _) = rig.resume("0123456789ab", &[]).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::PairNotFound);
    assert!(rig.fake.calls().is_empty());
}

#[tokio::test]
async fn resume_rejects_bad_pair_id_before_fs() {
    for bad in [
        "",
        "../etc/passwd",
        "0123456789AB",
        "0123456789a",
        "zzzzzzzzzzzz",
    ] {
        let rig = Rig::new();
        let before = entries(rig.home.path());
        let (r, _) = rig.resume(bad, &[]).await;
        assert_eq!(r.unwrap_err().code, ErrorCode::InvalidArgument, "{bad:?}");
        assert_eq!(
            entries(rig.home.path()),
            before,
            "{bad:?}: no file, no logs/"
        );
        assert!(rig.fake.calls().is_empty(), "{bad:?}");
    }
}

#[tokio::test]
async fn resume_controller_not_found_keeps_state_and_drops_the_lock() {
    for kind in [io::ErrorKind::NotFound, io::ErrorKind::PermissionDenied] {
        let rig = Rig::new();
        script_session_get(&rig);
        rig.ctrl_err.set(Some(kind));
        let id = seed(&rig, 1, true, None, None);
        let (r, out) = rig.resume(&id, &[]).await;
        assert_eq!(
            r.unwrap_err().code,
            ErrorCode::ControllerNotFound,
            "{kind:?}"
        );
        assert!(!has(&rig, &format!("pair.{id}.resume-lock")), "{kind:?}");
        assert!(has(&rig, &format!("pair.{id}.json")));
        assert!(out.contains(&format!("opencode-pair resume {id}")), "{out}");
    }
}

#[tokio::test]
async fn resume_and_attach_refused_on_the_opencode_host() {
    let rig = Rig::with(Host::Opencode, Settings::default());
    let (r, _) = rig.attach(SID).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::RefusedOnHost);
    let (r, _) = rig.resume("0123456789ab", &[]).await;
    assert_eq!(r.unwrap_err().code, ErrorCode::RefusedOnHost);
    assert!(rig.fake.calls().is_empty());
}
