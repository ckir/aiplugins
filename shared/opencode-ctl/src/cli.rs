//! Argv routing and the dispatch skeleton.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::ServiceExt;

use crate::clock::{Clock, SystemClock};
use crate::doctor::{default_config_file, default_registration_file, doctor_json, DoctorCtx};
use crate::engine::{Engine, EnvSnapshot};
use crate::error::{ErrorCode, ToolError};
use crate::host::{self, Host};
use crate::liveness::{ProcessProbe, SystemProbe};
use crate::logging::LogScope;
use crate::pairing;
use crate::server::OpencodeCtlServer;
use crate::settings;
use crate::skill_asset::{render, SkillName};
use crate::startup::startup;
use crate::transport::{build_transport, home_required};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Version,
    Usage,
    Sub { name: String, rest: Vec<String> },
    Pair(Vec<String>),
}

const SUBCOMMANDS: [&str; 8] = [
    "serve",
    "pair",
    "resume",
    "attach",
    "gc",
    "install-launcher",
    "emit-skill",
    "doctor-json",
];

/// The ONE empty-is-unset rule: a value empty after trimming is unset; any other value is returned verbatim.
pub fn blank_is_unset(v: Option<String>) -> Option<String> {
    v.filter(|v| !v.trim().is_empty())
}

pub fn env_nonempty(name: &str) -> Option<String> {
    blank_is_unset(std::env::var(name).ok())
}

/// The file stem of `argv0` with the directory and a `.exe` suffix stripped (either separator).
fn argv0_stem(argv0: &str) -> &str {
    let name = argv0.rsplit(['/', '\\']).next().unwrap_or(argv0);
    match name.len().checked_sub(4) {
        Some(i) if name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(".exe") => &name[..i],
        _ => name,
    }
}

/// Under the name `opencode-pair` (the launcher link) a leading `resume`, `attach`, `gc`,
/// `install-launcher`, `doctor-json` or `--version` is that subcommand and anything else, no
/// arguments included, is `pair`. Under the long binary names the subcommand is always explicit.
pub fn route(argv0: Option<&str>, args: &[String]) -> Route {
    if argv0.map(argv0_stem) == Some("opencode-pair") {
        return match args.first().map(String::as_str) {
            Some("--version") => Route::Version,
            Some(w @ ("resume" | "attach" | "gc" | "install-launcher" | "doctor-json")) => {
                Route::Sub {
                    name: w.to_string(),
                    rest: args[1..].to_vec(),
                }
            }
            _ => Route::Pair(args.to_vec()),
        };
    }
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => Route::Version,
        Some(w) if SUBCOMMANDS.contains(&w) => Route::Sub {
            name: w.to_string(),
            rest: args[1..].to_vec(),
        },
        _ => Route::Usage,
    }
}

pub fn version_line(product: &str, version: &str) -> String {
    format!("{product} {version}")
}

pub fn usage() -> String {
    "usage: serve [--host h] | pair | resume | attach | gc | install-launcher | emit-skill <name> --host <h> | doctor-json [verbose] [--host h] | --version"
        .to_string()
}

pub async fn dispatch(
    host: Host,
    product: &str,
    version: &str,
    argv0: Option<&str>,
    args: Vec<String>,
) -> i32 {
    let r = route(argv0, &args);
    let user_home = match home_required(&r, &|k| env_nonempty(k)) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{}", e.message);
            return 1;
        }
    };
    let probe: Arc<dyn ProcessProbe> = Arc::new(SystemProbe);
    match r {
        Route::Version => {
            println!("{}", version_line(product, version));
            0
        }
        Route::Usage => {
            eprintln!("{}", usage());
            2
        }
        Route::Sub { name, rest } if name == "serve" => {
            match serve(host, &user_home, &rest, probe).await {
                Ok(()) => 0,
                Err(msg) => {
                    eprintln!("{msg}");
                    1
                }
            }
        }
        Route::Sub { name, rest } if name == "emit-skill" => match emit_skill(&rest) {
            Ok(text) => {
                use std::io::Write;
                let mut out = std::io::stdout().lock();
                match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("{}", ToolError::from(e).to_json());
                        1
                    }
                }
            }
            Err(e) => {
                eprintln!("{}", e.to_json());
                2
            }
        },
        Route::Sub { name, rest } if name == "doctor-json" => {
            match doctor_main(host, version, &user_home, &rest, probe).await {
                Ok(report) => {
                    println!("{report}");
                    0
                }
                Err(e) => {
                    eprintln!("{}", e.to_json());
                    2
                }
            }
        }
        Route::Sub { name, rest } if name == "gc" => {
            match gc_main(host, &user_home, &rest, probe).await {
                Ok(report) => {
                    println!("{report}");
                    0
                }
                Err(e) => {
                    eprintln!("{}", e.to_json());
                    2
                }
            }
        }
        Route::Pair(_) if launcher_withheld(host) => {
            eprintln!("pair: {}", crate::launcher::NOT_OFFERED);
            2
        }
        Route::Pair(rest) => launcher_main(host, version, &user_home, "pair", &rest, probe).await,
        Route::Sub { name, .. }
            if matches!(
                name.as_str(),
                "pair" | "install-launcher" | "resume" | "attach"
            ) && launcher_withheld(host) =>
        {
            eprintln!("{name}: {}", crate::launcher::NOT_OFFERED);
            2
        }
        Route::Sub { name, rest } if name == "pair" || name == "install-launcher" => {
            launcher_main(host, version, &user_home, &name, &rest, probe).await
        }
        Route::Sub { .. } => {
            eprintln!("not implemented ({host:?}, argv0={argv0:?})");
            2
        }
    }
}

/// What `serve` resolves before it starts logging: settings loaded with the RAW environment (an
/// empty `OPENCODE_CTL_SERVER` must stay visible as the background-service override), the
/// pairing identity resolved and applied (`pairing.rs`), and the warnings for the log.
pub struct ServePrep {
    pub settings: settings::Settings,
    pub notes: Vec<settings::Note>,
    pub env: EnvSnapshot,
    pub identity_warnings: Vec<crate::error::ToolError>,
}

pub fn prepare_serve(
    host: Host,
    ctl_home: &std::path::Path,
    cwd: std::path::PathBuf,
    get: &dyn Fn(&str) -> Option<String>,
) -> ServePrep {
    let mut env = EnvSnapshot::from_lookup(get, cwd, ctl_home.to_path_buf());
    let (mut settings, notes) = settings::load_from_fs(host, ctl_home, &env.cwd, get);
    let (identity, mut identity_warnings) = pairing::resolve_identity(&env, ctl_home);
    identity_warnings.extend(pairing::apply_identity(&mut env, &mut settings, &identity));
    ServePrep {
        settings,
        notes,
        env,
        identity_warnings,
    }
}

async fn serve(
    host: Host,
    user_home: &std::path::Path,
    rest: &[String],
    probe: Arc<dyn ProcessProbe>,
) -> Result<(), String> {
    let host_arg = match rest {
        [] => None,
        [flag, value] if flag == "--host" => Some(value.as_str()),
        _ => return Err(usage()),
    };
    let env_host = env_nonempty("OPENCODE_CTL_HOST");
    let host = host::resolve(host, host_arg, env_host.as_deref()).map_err(|e| e.message)?;
    let ctl_home = settings::ctl_home(&|k| env_nonempty(k));
    let cwd = std::env::current_dir().unwrap_or_default();
    let prep = prepare_serve(host, &ctl_home, cwd, &|k| std::env::var(k).ok());
    let debug = env_nonempty("OPENCODE_CTL_LOG").is_some_and(|v| v.eq_ignore_ascii_case("debug"));
    let report = startup(
        &ctl_home,
        host,
        &prep.env,
        probe.as_ref(),
        SystemClock.now_ms(),
        debug,
        LogScope::Global,
        &prep.identity_warnings,
    );
    if let Some(w) = &report.log_warning {
        eprintln!("{w}");
    }
    for line in &report.identity_stderr {
        eprintln!("opencode-ctl: {line}");
    }
    let transport = build_transport(&prep.settings, &ctl_home, user_home);
    let engine = Arc::new(Engine::new(
        prep.settings,
        prep.notes,
        host,
        prep.env,
        transport,
        SystemClock,
        probe,
    ));
    let running = OpencodeCtlServer::new(engine)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| e.to_string())?;
    running.waiting().await.map_err(|e| e.to_string())?;
    drop(report);
    Ok(())
}

/// `emit-skill <name> --host <cc|qwen|agy|opencode>` (the two words in either order): the rendered skill, byte
/// for byte what goes to stdout. Any other argument shape, name or host is `INVALID_ARGUMENT`.
pub fn emit_skill(rest: &[String]) -> Result<String, ToolError> {
    let bad = |msg: String| ToolError::new(ErrorCode::InvalidArgument, msg);
    let mut name: Option<&str> = None;
    let mut host_arg: Option<&str> = None;
    let mut it = rest.iter().map(String::as_str);
    while let Some(w) = it.next() {
        if w == "--host" {
            let v = it
                .next()
                .ok_or_else(|| bad("--host needs a value (cc|qwen|agy|opencode)".into()))?;
            if host_arg.replace(v).is_some() {
                return Err(bad("--host given twice".into()));
            }
        } else if name.replace(w).is_some() {
            return Err(bad(format!("unexpected argument '{w}'")));
        }
    }
    let name = name.ok_or_else(|| {
        bad(
            "emit-skill <opencode-ctl-driver|opencode-ctl-doctor> --host <cc|qwen|agy|opencode>"
                .into(),
        )
    })?;
    let skill = SkillName::parse(name).ok_or_else(|| {
        bad(format!(
            "unknown skill '{name}' (opencode-ctl-driver or opencode-ctl-doctor)"
        ))
    })?;
    let host = match host_arg {
        Some("cc") => Host::Claude,
        Some("qwen") => Host::Qwen,
        Some("agy") => Host::Agy,
        Some("opencode") => Host::Opencode,
        Some(other) => {
            return Err(bad(format!(
                "unknown --host value '{other}' (cc, qwen, agy or opencode)"
            )))
        }
        None => return Err(bad("--host <cc|qwen|agy|opencode> is required".into())),
    };
    render(skill, host)
}

#[derive(Debug, PartialEq, Eq)]
pub struct DoctorArgs {
    pub verbose: bool,
    pub host_arg: Option<String>,
}

/// `doctor-json [verbose] [--host <h>]`, the two in either order; any other word is `INVALID_ARGUMENT`.
pub fn parse_doctor_args(rest: &[String]) -> Result<DoctorArgs, ToolError> {
    let bad = |msg: String| ToolError::new(ErrorCode::InvalidArgument, msg);
    let mut args = DoctorArgs {
        verbose: false,
        host_arg: None,
    };
    let mut it = rest.iter().map(String::as_str);
    while let Some(w) = it.next() {
        match w {
            "verbose" => args.verbose = true,
            "--host" => {
                let v = it
                    .next()
                    .ok_or_else(|| bad("--host needs a value (opencode)".into()))?;
                if args.host_arg.replace(v.to_string()).is_some() {
                    return Err(bad("--host given twice".into()));
                }
            }
            other => {
                return Err(bad(format!(
                    "unexpected argument '{other}' (doctor-json [verbose] [--host h])"
                )))
            }
        }
    }
    Ok(args)
}

/// The launcher install dir (spec Launcher, install-launcher): `~/.local/bin` on POSIX,
/// `%LOCALAPPDATA%\opencode-ctl` on Windows. `get` is the raw environment lookup.
pub fn launcher_bin_dir(user_home: &Path, get: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    if cfg!(windows) {
        blank_is_unset(get("LOCALAPPDATA"))
            .map(PathBuf::from)
            .unwrap_or_else(|| user_home.join("AppData").join("Local"))
            .join("opencode-ctl")
    } else {
        user_home.join(".local").join("bin")
    }
}

/// `doctor-json`: the report for the real environment, as the value to print on stdout.
async fn doctor_main(
    host: Host,
    version: &str,
    user_home: &Path,
    rest: &[String],
    probe: Arc<dyn ProcessProbe>,
) -> Result<serde_json::Value, ToolError> {
    let args = parse_doctor_args(rest)?;
    let env_host = env_nonempty("OPENCODE_CTL_HOST");
    let host = host::resolve(host, args.host_arg.as_deref(), env_host.as_deref())?;
    let raw = |k: &str| std::env::var(k).ok();
    let ctl_home = settings::ctl_home(&|k| env_nonempty(k));
    let cwd = std::env::current_dir().unwrap_or_default();
    let prep = prepare_serve(host, &ctl_home, cwd, &raw);
    let transport = build_transport(&prep.settings, &ctl_home, user_home);
    let engine = Engine::new(
        prep.settings,
        prep.notes,
        host,
        prep.env,
        transport,
        SystemClock,
        probe,
    );
    let bin_dir = launcher_bin_dir(user_home, &raw);
    let running_exe = std::env::current_exe().unwrap_or_default();
    let registration = default_registration_file(&raw, user_home);
    let config = default_config_file(&raw, user_home);
    let ctx = DoctorCtx {
        bin_dir: &bin_dir,
        running_exe: &running_exe,
        version,
        user_home,
        path: env_nonempty("PATH"),
        registration_file: Some(&registration),
        config_file: Some(&config),
        env: &raw,
    };
    Ok(doctor_json(&engine, &ctx, args.verbose).await)
}

/// One line per pair `gc` touched, appended to that pair's `pair-<id>.log` (the `gc` subcommand
/// opens each log `Scoped`; `pair` writes to its own already-open log instead and never calls this).
fn log_gc_report(home: &Path, report: &crate::gc::GcReport, debug: bool) {
    let touched = report
        .pairs_deleted
        .iter()
        .map(|id| (id, "deleted"))
        .chain(
            report
                .dead_seen_written
                .iter()
                .map(|id| (id, "dead-seen written")),
        )
        .chain(
            report
                .dead_seen_cleared
                .iter()
                .map(|id| (id, "dead-seen cleared")),
        );
    for (id, what) in touched {
        let path = home.join("logs").join(format!("pair-{id}.log"));
        if let Ok(handle) = crate::logging::init_file_logging(&path, debug, LogScope::Scoped) {
            tracing::info!("gc: pair {id} {what}");
            drop(handle);
        }
    }
}

/// `gc` (no arguments): the report for the real environment as the JSON to print on stdout.
async fn gc_main(
    host: Host,
    user_home: &Path,
    rest: &[String],
    probe: Arc<dyn ProcessProbe>,
) -> Result<serde_json::Value, ToolError> {
    if !rest.is_empty() {
        return Err(ToolError::new(
            ErrorCode::InvalidArgument,
            format!("unexpected argument '{}' (gc takes none)", rest[0]),
        ));
    }
    let raw = |k: &str| std::env::var(k).ok();
    let ctl_home = settings::ctl_home(&|k| env_nonempty(k));
    let cwd = std::env::current_dir().unwrap_or_default();
    let prep = prepare_serve(host, &ctl_home, cwd, &raw);
    let transport = build_transport(&prep.settings, &ctl_home, user_home);
    let engine = Engine::new(
        prep.settings,
        prep.notes,
        host,
        prep.env,
        transport,
        SystemClock,
        probe.clone(),
    );
    let now = SystemClock.now_ms();
    let report = crate::gc::gc(&engine, probe.as_ref(), now, None).await;
    let debug = env_nonempty("OPENCODE_CTL_LOG").is_some_and(|v| v.eq_ignore_ascii_case("debug"));
    log_gc_report(&ctl_home, &report, debug);
    serde_json::to_value(&report).map_err(|e| ToolError::new(ErrorCode::IoError, e.to_string()))
}

/// The OpenCode host (the compiled constant or `OPENCODE_CTL_HOST=opencode`) is never offered the launcher.
fn launcher_withheld(host: Host) -> bool {
    let env_host = env_nonempty("OPENCODE_CTL_HOST");
    host::resolve(host, None, env_host.as_deref()).is_ok_and(|h| !h.offers_launcher())
}

/// `pair` and `install-launcher` for the real environment; returns the exit code.
async fn launcher_main(
    host: Host,
    version: &str,
    user_home: &Path,
    name: &str,
    rest: &[String],
    probe: Arc<dyn ProcessProbe>,
) -> i32 {
    match launcher_run(host, version, user_home, name, rest, probe).await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{}", e.to_json());
            2
        }
    }
}

async fn launcher_run(
    host: Host,
    version: &str,
    user_home: &Path,
    name: &str,
    rest: &[String],
    probe: Arc<dyn ProcessProbe>,
) -> Result<i32, ToolError> {
    use crate::launcher::{self, LauncherCtx};
    let raw = |k: &str| std::env::var(k).ok();
    let ctl_home = settings::ctl_home(&|k| env_nonempty(k));
    let cwd = std::env::current_dir().unwrap_or_default();
    let prep = prepare_serve(host, &ctl_home, cwd, &raw);
    let opencode_bin = crate::transport::cli::resolve_binary(
        prep.settings.opencode_bin.as_deref(),
        env_nonempty("PATH").as_deref(),
        user_home,
    )
    .ok();
    let transport = build_transport(&prep.settings, &ctl_home, user_home);
    let engine = Engine::new(
        prep.settings,
        prep.notes,
        host,
        prep.env,
        transport,
        SystemClock,
        probe.clone(),
    );
    let bin_dir = launcher_bin_dir(user_home, &raw);
    let running_exe = std::env::current_exe().unwrap_or_default();
    let password = env_nonempty("OPENCODE_SERVER_PASSWORD");
    let now = || SystemClock.now_ms();
    let mut stdout = std::io::stdout();
    let mut ctx = LauncherCtx {
        home: &ctl_home,
        bin_dir: &bin_dir,
        running_exe: &running_exe,
        host,
        probe: probe.as_ref(),
        now_ms: &now,
        path_has: &launcher::system_path_has,
        opencode_bin: opencode_bin.as_deref(),
        version,
        parent_chain: &launcher::system_parent_chain,
        server_password: password.as_deref(),
        spawn_detached: &launcher::system_spawn_detached,
        spawn_and_wait: &launcher::system_spawn_and_wait,
        before_claim: None,
        out: &mut stdout,
    };
    match name {
        "install-launcher" => {
            let force = match rest {
                [] => false,
                [f] if f == "--force" => true,
                [other, ..] => {
                    return Err(ToolError::new(
                        ErrorCode::InvalidArgument,
                        format!("unexpected argument '{other}' (install-launcher [--force])"),
                    ))
                }
            };
            launcher::install_launcher(&mut ctx, force).map(|()| 0)
        }
        _ => {
            let args = launcher::parse_pair_args(rest)?;
            launcher::pair(args, &mut ctx, &engine).await
        }
    }
}
