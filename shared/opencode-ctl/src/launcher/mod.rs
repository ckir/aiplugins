//! The launcher: `pair`, the tab wrappers, the terminal walk, `install-launcher` and the pure
//! Windows resolvers (spec "Launcher"), and `resume` and `attach` (Task 19), which reuse
//! `claim_file`, `write_wrapper`, `controller_env` and `LauncherCtx::before_claim`.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tracing::instrument::WithSubscriber;

use crate::clock::{Clock, SystemClock};
use crate::doctor::{
    launcher_link_status, parse_source_file, windows_link_status, LinkStatus, SourceFile,
};
use crate::engine::{valid_session_id, Engine};
use crate::error::{ErrorCode, ToolError};
use crate::host::Host;
use crate::liveness::{sentinel_alive, ProcessProbe, Sentinel};
use crate::logging::{init_file_logging, LogHandle, LogScope};
use crate::openapi::Operation;
use crate::pairing::{self, claim_file, valid_pair_id, write_record_new, Record};
use crate::tools::SessionCreateArgs;
use crate::transport::{Lifecycle, Params, Transport, TransportError, WaitOutcome};

pub mod posix;
pub mod windows;

pub use posix::{
    quote_sh, select_terminal, system_parent_chain, tab_command, wrapper_script_posix, Terminal,
    BOOT_ID_EXPR_POSIX, STARTTIME_EXPR_POSIX,
};
pub use windows::{
    cmd_line, cmd_wrap, controller_command, controller_process, quote_ps, resolve_executable,
    select_windows_shell, strip_verbatim, wrapper_script_ps1, wt_directory_arg, WindowsShell,
};

/// What `pair`, `resume`, `attach` and `install-launcher` print on the OpenCode host.
pub const NOT_OFFERED: &str = "not offered on the OpenCode host";

/// The notice posted into the worker session when the controller exits (spec Launcher step 5).
pub const EXIT_NOTICE: &str = "The controller process that drove this session has exited. A human may be typing here now; treat further instructions as theirs. Do not resume unfinished controller work on your own.";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PairArgs {
    pub directory: Option<PathBuf>,
    pub model: Option<String>,
    pub title: Option<String>,
    pub controller: Option<String>,
    pub no_tab: bool,
    pub dry_run: bool,
    pub controller_args: Vec<String>,
}

/// `pair [directory] [--model m] [--title t] [--controller c] [--no-tab] [--dry-run] [-- <controller args>]`.
pub fn parse_pair_args(rest: &[String]) -> Result<PairArgs, ToolError> {
    let bad = |msg: String| ToolError::new(ErrorCode::InvalidArgument, msg);
    let mut args = PairArgs::default();
    let mut it = rest.iter();
    while let Some(w) = it.next() {
        let (flag, inline) = match w.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (w.as_str(), None),
        };
        let mut value = |name: &str| {
            inline
                .clone()
                .or_else(|| it.next().cloned())
                .ok_or_else(|| bad(format!("{name} needs a value")))
        };
        match flag {
            "--" => {
                args.controller_args = it.by_ref().cloned().collect();
                break;
            }
            "--model" => args.model = Some(value("--model")?),
            "--title" => args.title = Some(value("--title")?),
            "--controller" => args.controller = Some(value("--controller")?),
            "--no-tab" => args.no_tab = true,
            "--dry-run" => args.dry_run = true,
            f if f.starts_with('-') && f.len() > 1 => {
                return Err(bad(format!("unknown option '{f}'")).hint(
                    "pair [directory] [--model m] [--title t] [--controller claude|qwen|agy] [--no-tab] [--dry-run] [-- <controller args>]",
                ));
            }
            dir => {
                if args.directory.replace(PathBuf::from(dir)).is_some() {
                    return Err(bad(format!("unexpected argument '{dir}'")));
                }
            }
        }
    }
    Ok(args)
}

/// `dyn Fn(&mut Command, &mut dyn FnMut(u32)) -> io::Result<ExitStatus>`.
pub type SpawnAndWaitFn<'a> =
    dyn Fn(&mut Command, &mut dyn FnMut(u32)) -> io::Result<std::process::ExitStatus> + 'a;

pub struct LauncherCtx<'a> {
    /// `= engine.env.home`; the launcher asserts equality.
    pub home: &'a Path,
    /// `~/.local/bin` or `%LOCALAPPDATA%\opencode-ctl`; tests pass a temp dir.
    pub bin_dir: &'a Path,
    pub running_exe: &'a Path,
    pub host: Host,
    pub probe: &'a dyn ProcessProbe,
    pub now_ms: &'a dyn Fn() -> i64,
    pub path_has: &'a dyn Fn(&str) -> bool,
    /// The RESOLVED opencode binary for the wrapper; `None` = `OPENCODE_NOT_FOUND` for `pair` and `attach`.
    pub opencode_bin: Option<&'a Path>,
    /// The front's `CARGO_PKG_VERSION`: the `version=` line of `opencode-pair.exe.source`.
    pub version: &'a str,
    pub parent_chain: &'a dyn Fn() -> Vec<String>,
    /// `OPENCODE_SERVER_PASSWORD` at launch (empty = absent).
    pub server_password: Option<&'a str>,
    /// The terminal tab: spawn and return the pid.
    pub spawn_detached: &'a dyn Fn(&mut Command) -> io::Result<u32>,
    /// The controller: calls the callback with the pid right after spawn, then waits.
    pub spawn_and_wait: &'a SpawnAndWaitFn<'a>,
    /// Test failpoint for `attach`: called after the record is read, before the first claim.
    pub before_claim: Option<&'a dyn Fn()>,
    pub out: &'a mut dyn Write,
}

// ------------------------------------------------------------------ wrapper files

pub struct WrapperInput<'a> {
    pub home: &'a Path,
    /// The launcher's resolved binary.
    pub opencode_bin: &'a Path,
    pub pair_id: &'a str,
    pub generation: u32,
    pub directory: &'a Path,
    pub session_id: &'a str,
    pub server_url: Option<&'a str>,
    pub server_password: Option<&'a str>,
    /// True: embed the runtime expressions for `boot_id` and `starttime`; false (tests only): the
    /// literals `BOOT` and `START`.
    pub boot_id_expr: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapperKind {
    Sh,
    Ps1,
}

/// The ONE wrapper writer: `claim_file(path, bytes, 0o700)`, with U+FEFF prepended for a `.ps1`
/// (`powershell.exe` 5.1 decodes a BOM-less script as the ANSI code page). `Ok(false)`: it exists.
pub fn write_wrapper(path: &Path, script: &str, kind: WrapperKind) -> Result<bool, ToolError> {
    let mut bytes = Vec::with_capacity(script.len() + 3);
    if kind == WrapperKind::Ps1 {
        bytes.extend_from_slice("\u{feff}".as_bytes());
    }
    bytes.extend_from_slice(script.as_bytes());
    claim_file(path, &bytes, 0o700)
}

pub fn wrapper_path(home: &Path, pair_id: &str, generation: u32, windows: bool) -> PathBuf {
    let ext = if windows { "ps1" } else { "sh" };
    home.join(format!("pair.{pair_id}.g{generation}.wrapper.{ext}"))
}

fn pair_file(home: &Path, pair_id: &str, suffix: &str) -> PathBuf {
    home.join(format!("pair.{pair_id}.{suffix}"))
}

// ------------------------------------------------------------------ values

fn invalid(msg: String) -> ToolError {
    ToolError::new(ErrorCode::InvalidArgument, msg)
}

/// "Values that reach a shell" (spec Launcher): ids match their patterns, a newline is refused
/// everywhere, `;` in the directory on Windows (`wt` splits its command line on it), and on Windows
/// a controller argument with a `cmd.exe` metacharacter. Nothing is written before this passes.
pub fn validate_launch_values(
    directory: &Path,
    session_id: Option<&str>,
    pair_id: &str,
    controller_args: &[String],
    windows: bool,
) -> Result<(), ToolError> {
    if !valid_pair_id(pair_id) {
        return Err(
            invalid(format!("'{pair_id}' is not a pair id")).hint("pair ids match ^[0-9a-f]{12}$")
        );
    }
    if let Some(s) = session_id {
        if !valid_session_id(s) {
            return Err(invalid("not a session id".to_string())
                .hint("session ids match ^ses_[A-Za-z0-9]{26}$"));
        }
    }
    let dir = directory.to_string_lossy();
    if dir.contains(['\n', '\r', '\0']) {
        return Err(invalid("the directory contains a newline".into()));
    }
    if windows && dir.contains(';') {
        return Err(invalid(format!(
            "the directory '{dir}' contains ';' (wt splits its command line on it)"
        )));
    }
    if windows {
        for a in controller_args {
            if a.contains(['&', '|', '<', '>', '^', '"', '%', '\n', '\r']) {
                return Err(invalid(format!(
                    "controller argument '{a}' contains a cmd.exe metacharacter (& | < > ^ \" %) or a newline"
                ))
                .hint("controller arguments travel through cmd.exe /c on Windows"));
            }
        }
    }
    Ok(())
}

/// Spec step 5's environment, shared with `resume`. `OPENCODE_CTL_SERVER` is the EMPTY string when
/// there is no `server_url` (never omitted: `Command` inherits the parent environment, and a stale
/// value would point the controller at another server; empty = the background service).
pub fn controller_env(
    session_id: &str,
    directory: &Path,
    server_url: Option<&str>,
    pair_id: &str,
) -> Vec<(String, String)> {
    vec![
        ("OPENCODE_CTL_SESSION".into(), session_id.into()),
        (
            "OPENCODE_CTL_DIR".into(),
            directory.to_string_lossy().into_owned(),
        ),
        (
            "OPENCODE_CTL_SERVER".into(),
            server_url.unwrap_or_default().into(),
        ),
        ("OPENCODE_CTL_PAIR".into(), pair_id.into()),
    ]
}

fn mint_pair_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

fn utc_hhmm(ms: i64) -> String {
    let minutes = ms.div_euclid(60_000).rem_euclid(1440);
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn resolve_directory(arg: Option<&Path>, cwd: &Path) -> Result<PathBuf, ToolError> {
    let base = match arg {
        Some(d) if d.is_absolute() => d.to_path_buf(),
        Some(d) => cwd.join(d),
        None => cwd.to_path_buf(),
    };
    let canon = std::fs::canonicalize(&base)
        .map_err(|e| invalid(format!("directory {} is not usable: {e}", base.display())))?;
    if !canon.is_dir() {
        return Err(invalid(format!("{} is not a directory", base.display())));
    }
    if cfg!(windows) {
        return Ok(PathBuf::from(strip_verbatim(&canon.to_string_lossy())));
    }
    Ok(canon)
}

fn resolve_controller(args: &PairArgs, host: Host) -> Result<String, ToolError> {
    match args.controller.as_deref() {
        Some(c @ ("claude" | "qwen" | "agy")) => Ok(c.to_string()),
        Some(other) => Err(invalid(format!(
            "unknown controller '{other}' (claude, qwen or agy)"
        ))),
        None => host
            .controller()
            .map(str::to_string)
            .ok_or_else(|| invalid("no controller: pass --controller claude|qwen|agy".to_string())),
    }
}

fn opencode_missing() -> ToolError {
    ToolError::new(ErrorCode::OpencodeNotFound, "opencode executable not found")
        .hint("install opencode or set OPENCODE_CTL_BIN")
}

fn say(ctx: &mut LauncherCtx, line: &str) {
    let _ = writeln!(ctx.out, "{line}");
}

// ------------------------------------------------------------------ dry-run body capture

/// A transport that records the body of the one `session.create` call, so the dry run shows exactly
/// what `Engine::session_create` (policy, model, lock rules and all) would send.
struct CaptureTransport {
    body: Arc<Mutex<Option<Value>>>,
}

impl Transport for CaptureTransport {
    async fn call(
        &self,
        _op: &Operation,
        _params: &Params,
        body: Option<&Value>,
        _deadline: Duration,
    ) -> Result<Value, TransportError> {
        *self.body.lock().unwrap_or_else(|e| e.into_inner()) = body.cloned();
        Ok(json!({"data": {"id": "ses_dryrundryrundryrundryrun00"}}))
    }

    async fn lifecycle(&self, _verb: Lifecycle) -> Result<String, TransportError> {
        Ok(String::new())
    }

    async fn wait_idle(&self, _id: &str, _d: Duration) -> Result<WaitOutcome, TransportError> {
        Ok(WaitOutcome::Idle)
    }

    async fn fetch_openapi(&self) -> Result<Value, TransportError> {
        Err(TransportError {
            code: ErrorCode::ServiceDown,
            message: "dry run".into(),
            hint: None,
            details: None,
        })
    }

    async fn cli_version(&self) -> Result<String, TransportError> {
        Ok(String::new())
    }
}

fn create_args(args: &PairArgs, directory: &Path, title: &str) -> SessionCreateArgs {
    SessionCreateArgs {
        title: Some(title.to_string()),
        directory: Some(directory.to_string_lossy().into_owned()),
        agent: None,
        model: args.model.clone(),
        permissions: None,
        parent_id: None,
    }
}

async fn dry_create_body<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    args: &PairArgs,
    directory: &Path,
    title: &str,
) -> Result<Value, ToolError> {
    let body = Arc::new(Mutex::new(None));
    let capture = Engine::new(
        engine.settings.clone(),
        Vec::new(),
        engine.host,
        engine.env.clone(),
        CaptureTransport { body: body.clone() },
        SystemClock,
        engine.probe.clone(),
    );
    capture
        .session_create(create_args(args, directory, title))
        .await?;
    let sent = body.lock().unwrap_or_else(|e| e.into_inner()).clone();
    sent.ok_or_else(|| ToolError::new(ErrorCode::CliError, "session.create sent no body"))
}

// ------------------------------------------------------------------ pair

fn tab_argv_command(argv: &[String]) -> Command {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

fn is_missing_program(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    )
}

/// `pair`: spec Launcher steps 1-5. The order at the top is: mint the id, validate, `OPENCODE_NOT_FOUND`
/// when there is no binary, open the log, then step 1; `--dry-run` writes, spawns and calls nothing.
pub async fn pair<T: Transport, C: Clock>(
    args: PairArgs,
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
) -> Result<i32, ToolError> {
    if !ctx.host.offers_launcher() {
        return Err(ToolError::new(ErrorCode::RefusedOnHost, NOT_OFFERED));
    }
    if ctx.home != engine.env.home.as_path() {
        return Err(ToolError::new(
            ErrorCode::IoError,
            "launcher home differs from the engine's home",
        ));
    }
    let windows = cfg!(windows);
    let pair_id = mint_pair_id();
    let controller = resolve_controller(&args, ctx.host)?;
    let directory = resolve_directory(args.directory.as_deref(), &engine.env.cwd)?;
    validate_launch_values(&directory, None, &pair_id, &args.controller_args, windows)?;
    let opencode_bin = ctx.opencode_bin.ok_or_else(opencode_missing)?.to_path_buf();
    let basename = directory
        .file_name()
        .map_or_else(|| "root".to_string(), |n| n.to_string_lossy().into_owned());
    let title = args
        .title
        .clone()
        .unwrap_or_else(|| format!("pair {basename} {}Z", utc_hhmm((ctx.now_ms)())));
    if args.dry_run {
        return dry_run(&args, ctx, engine, &directory, &controller, &title).await;
    }
    let _log = open_pair_log(ctx.home, &pair_id);
    run(
        &args,
        ctx,
        engine,
        &pair_id,
        &directory,
        &controller,
        &title,
        &opencode_bin,
    )
    .with_current_subscriber()
    .await
}

/// `<home>/logs/pair-<id>.log`, scoped to this thread; a failed open is a stderr warning (plan
/// decision 10) and the run continues.
fn open_pair_log(home: &Path, pair_id: &str) -> Option<LogHandle> {
    let debug = crate::cli::env_nonempty("OPENCODE_CTL_LOG")
        .is_some_and(|v| v.eq_ignore_ascii_case("debug"));
    let log_path = home.join("logs").join(format!("pair-{pair_id}.log"));
    match init_file_logging(&log_path, debug, LogScope::Scoped) {
        Ok(h) => Some(h),
        Err(e) => {
            eprintln!("opencode-ctl: cannot open {}: {e}", log_path.display());
            None
        }
    }
}

fn tab_plan(ctx: &LauncherCtx, args: &PairArgs) -> Option<Terminal> {
    if args.no_tab {
        return None;
    }
    let chain = (ctx.parent_chain)();
    select_terminal(&chain, ctx.path_has, cfg!(windows))
}

async fn dry_run<T: Transport, C: Clock>(
    args: &PairArgs,
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
    directory: &Path,
    controller: &str,
    title: &str,
) -> Result<i32, ToolError> {
    let windows = cfg!(windows);
    let body = dry_create_body(engine, args, directory, title).await?;
    let mut lines = Vec::new();
    if engine.settings.server_url.is_none() {
        lines.push("would run: opencode service start".to_string());
    }
    lines.push("would run: gc".to_string());
    lines.push(format!("would call: session.create {body}"));
    if let Some(t) = tab_plan(ctx, args) {
        let script = wrapper_path(ctx.home, "<pair-id>", 1, windows);
        let shell = select_windows_shell(ctx.path_has);
        let argv = tab_command(t, &script, directory, shell);
        lines.push(format!("would run: {}", argv.join(" ")));
    }
    let argv = controller_command(
        controller,
        &args.controller_args,
        windows,
        crate::cli::env_nonempty("PATH").as_deref(),
        crate::cli::env_nonempty("PATHEXT").as_deref(),
    )
    .ok_or_else(|| controller_missing(controller))?;
    lines.push(format!("would run: {}", argv.join(" ")));
    for l in lines {
        say(ctx, &l);
    }
    Ok(0)
}

fn controller_missing(controller: &str) -> ToolError {
    ToolError::new(
        ErrorCode::ControllerNotFound,
        format!("controller executable '{controller}' not found"),
    )
    .hint("install it, or pass --controller claude|qwen|agy")
}

/// Remove what a failed step 3 or 4 left: the session, then every `pair.<id>.*` file.
async fn rollback<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    home: &Path,
    pair_id: &str,
    session_id: &str,
) {
    tracing::warn!("pair {pair_id}: rolling back session {session_id}");
    let params = vec![("sessionID".to_string(), session_id.to_string())];
    if let Err(e) = engine.call_json("session.remove", params, None).await {
        tracing::warn!("pair {pair_id}: session.remove failed: {e}");
    }
    if let Err(e) = pairing::delete_pair_files(home, pair_id) {
        tracing::warn!("pair {pair_id}: removing the pair files failed: {e}");
    }
}

struct Staged {
    terminal: Option<Terminal>,
}

#[allow(clippy::too_many_arguments)]
async fn run<T: Transport, C: Clock>(
    args: &PairArgs,
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
    pair_id: &str,
    directory: &Path,
    controller: &str,
    title: &str,
    opencode_bin: &Path,
) -> Result<i32, ToolError> {
    let home = ctx.home;
    tracing::info!("pair {pair_id}: directory {}", directory.display());
    if engine.settings.server_url.is_none() {
        let url = engine.lifecycle(Lifecycle::Start).await?;
        tracing::info!("pair {pair_id}: service started at {url}");
    } else {
        tracing::info!("pair {pair_id}: service start skipped (server_url is set)");
    }
    let report = crate::gc::gc(engine, ctx.probe, (ctx.now_ms)(), None).await;
    tracing::info!(
        "gc: swept {}, pairs deleted {}, dead-seen written {}, cleared {}, skipped {}",
        report.swept.len(),
        report.pairs_deleted.len(),
        report.dead_seen_written.len(),
        report.dead_seen_cleared.len(),
        report.skipped_service_down
    );

    let created = engine
        .session_create(create_args(args, directory, title))
        .await?;
    let session_id = created
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    tracing::info!("pair {pair_id}: session created {session_id}");

    let staged = stage(
        args,
        ctx,
        pair_id,
        directory,
        controller,
        &session_id,
        opencode_bin,
        engine.settings.server_url.as_deref(),
    );
    let staged = match staged {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("pair {pair_id}: {e}");
            rollback(engine, home, pair_id, &session_id).await;
            return Err(e);
        }
    };
    let server_url = engine.settings.server_url.as_deref();
    match staged.terminal {
        Some(t) => {
            say(ctx, &format!("tui: {}", t.program()));
        }
        None => {
            let why = if args.no_tab {
                "--no-tab"
            } else {
                "no terminal found"
            };
            say(
                ctx,
                &format!("tui: none ({why}); open one later with: opencode-pair attach {pair_id}"),
            );
        }
    }

    step5(
        ctx,
        engine,
        pair_id,
        &session_id,
        directory,
        controller,
        &args.controller_args,
        server_url,
        None,
    )
    .await
}

/// Step 5: the controller, as a child with inherited stdio. Shared with `resume`, which passes the
/// `resume-lock` it holds: deleted once `claude-pid` is written.
#[allow(clippy::too_many_arguments)]
async fn step5<T: Transport, C: Clock>(
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
    pair_id: &str,
    session_id: &str,
    directory: &Path,
    controller: &str,
    controller_args: &[String],
    server_url: Option<&str>,
    lock: Option<&Path>,
) -> Result<i32, ToolError> {
    let windows = cfg!(windows);
    let home = ctx.home;
    let argv = controller_command(
        controller,
        controller_args,
        windows,
        crate::cli::env_nonempty("PATH").as_deref(),
        crate::cli::env_nonempty("PATHEXT").as_deref(),
    );
    let Some(argv) = argv else {
        return Err(controller_unavailable(ctx, pair_id, controller, None));
    };
    let mut cmd = controller_process(&argv);
    cmd.envs(controller_env(session_id, directory, server_url, pair_id));
    tracing::info!("pair {pair_id}: starting controller {controller}");
    let probe = ctx.probe;
    let mut bind = |pid: u32| {
        if bind_controller(home, pair_id, probe, pid) {
            if let Some(l) = lock {
                let _ = std::fs::remove_file(l);
            }
        }
    };
    let status = match (ctx.spawn_and_wait)(&mut cmd, &mut bind) {
        Ok(s) => s,
        Err(e) => return Err(controller_unavailable(ctx, pair_id, controller, Some(e))),
    };
    tracing::info!("pair {pair_id}: controller exited ({status})");
    let exited = (ctx.now_ms)().to_string();
    match claim_file(
        &pair_file(home, pair_id, "claude-exited"),
        exited.as_bytes(),
        0o600,
    ) {
        Ok(_) => {}
        Err(e) => tracing::warn!("pair {pair_id}: claude-exited not written: {e}"),
    }
    post_exit_notice(engine, pair_id, session_id).await;
    print_closing(ctx, pair_id, session_id, directory, server_url);
    Ok(status.code().unwrap_or(1))
}

fn controller_unavailable(
    ctx: &mut LauncherCtx,
    pair_id: &str,
    controller: &str,
    cause: Option<io::Error>,
) -> ToolError {
    let attach = format!("opencode-pair attach {pair_id}");
    let resume = format!("opencode-pair resume {pair_id}");
    say(
        ctx,
        &format!(
            "worker session is ready; attach with `{attach}`, start a controller with `{resume}`"
        ),
    );
    match cause {
        Some(e) if !is_missing_program(&e) => {
            tracing::warn!("pair {pair_id}: controller spawn failed: {e}");
            ToolError::new(
                ErrorCode::IoError,
                format!("controller '{controller}' could not be started: {e}"),
            )
        }
        _ => {
            tracing::warn!("pair {pair_id}: controller {controller} not found");
            controller_missing(controller)
        }
    }
}

/// Steps 3 and 4: the terminal, the record (once), the wrapper and the tab. A failure is returned
/// for the caller to roll back.
#[allow(clippy::too_many_arguments)]
fn stage(
    args: &PairArgs,
    ctx: &mut LauncherCtx<'_>,
    pair_id: &str,
    directory: &Path,
    controller: &str,
    session_id: &str,
    opencode_bin: &Path,
    server_url: Option<&str>,
) -> Result<Staged, ToolError> {
    let windows = cfg!(windows);
    let home = ctx.home;
    validate_launch_values(
        directory,
        Some(session_id),
        pair_id,
        &args.controller_args,
        windows,
    )?;
    let terminal = tab_plan(ctx, args);
    let record = Record {
        pair_id: pair_id.to_string(),
        session_id: session_id.to_string(),
        directory: directory.to_path_buf(),
        server_url: server_url.map(str::to_string),
        created: (ctx.now_ms)(),
        controller: Some(controller.to_string()),
        generation: 1,
        tab: terminal.is_some(),
    };
    write_record_new(home, &record)?;
    tracing::info!("pair {pair_id}: record written (tab {})", record.tab);
    let Some(t) = terminal else {
        return Ok(Staged { terminal });
    };
    let input = WrapperInput {
        home,
        opencode_bin,
        pair_id,
        generation: 1,
        directory,
        session_id,
        server_url,
        server_password: ctx.server_password,
        boot_id_expr: true,
    };
    let (script, kind) = wrapper_text(&input, windows);
    let path = wrapper_path(home, pair_id, 1, windows);
    if !write_wrapper(&path, &script, kind)? {
        return Err(ToolError::new(
            ErrorCode::IoError,
            format!("{} already exists", path.display()),
        ));
    }
    tracing::info!("pair {pair_id}: wrapper written {}", path.display());
    let pid = open_tab(ctx, t, &path, directory)?;
    tracing::info!("pair {pair_id}: tab spawned ({}, pid {pid})", t.program());
    Ok(Staged { terminal })
}

fn wrapper_text(input: &WrapperInput, windows: bool) -> (String, WrapperKind) {
    if windows {
        (wrapper_script_ps1(input), WrapperKind::Ps1)
    } else {
        (wrapper_script_posix(input), WrapperKind::Sh)
    }
}

/// Spec step 4's last action: the wrapper in a new tab of `t`; returns the tab process's pid.
fn open_tab(
    ctx: &LauncherCtx,
    t: Terminal,
    script: &Path,
    directory: &Path,
) -> Result<u32, ToolError> {
    let shell = select_windows_shell(ctx.path_has);
    let argv = tab_command(t, script, directory, shell);
    let mut cmd = tab_argv_command(&argv);
    (ctx.spawn_detached)(&mut cmd).map_err(|e| {
        ToolError::new(
            ErrorCode::IoError,
            format!("cannot open a {} tab: {e}", t.program()),
        )
    })
}

/// `claude-pid` from the child's pid, boot id and start time.
fn bind_controller(home: &Path, pair_id: &str, probe: &dyn ProcessProbe, pid: u32) -> bool {
    let Some(starttime) = probe.starttime(pid) else {
        tracing::warn!("pair {pair_id}: cannot read the start time of controller pid {pid}");
        return false;
    };
    let sentinel = Sentinel {
        pid,
        boot_id: probe.boot_id(),
        starttime,
    };
    match pairing::write_sentinel(&pair_file(home, pair_id, "claude-pid"), &sentinel) {
        Ok(()) => {
            tracing::info!("pair {pair_id}: controller started, claude-pid {sentinel}");
            true
        }
        Err(e) => {
            tracing::warn!("pair {pair_id}: claude-pid not written: {e}");
            false
        }
    }
}

/// `session.synthetic` `{text, resume: false}` with no client id; one retry with the same body on
/// the id-collision conflict. Failure is logged, never fatal.
async fn post_exit_notice<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    pair_id: &str,
    session_id: &str,
) {
    let body = json!({"text": EXIT_NOTICE, "resume": false});
    for attempt in 0..2 {
        let params = vec![("sessionID".to_string(), session_id.to_string())];
        match engine
            .call_json("session.synthetic", params, Some(body.clone()))
            .await
        {
            Ok(_) => {
                tracing::info!("pair {pair_id}: exit notice posted");
                return;
            }
            Err(e)
                if attempt == 0
                    && e.code == ErrorCode::Conflict
                    && e.message.starts_with("Synthetic input ID conflicts") =>
            {
                tracing::warn!("pair {pair_id}: exit notice id conflict, retrying once");
            }
            Err(e) => {
                tracing::warn!("pair {pair_id}: exit notice not posted: {e}");
                return;
            }
        }
    }
}

fn print_closing(
    ctx: &mut LauncherCtx,
    pair_id: &str,
    session_id: &str,
    directory: &Path,
    server_url: Option<&str>,
) {
    say(
        ctx,
        &format!(
            "worker session {session_id} is still running; attach with `opencode-pair attach {pair_id}`, restart the controller with `opencode-pair resume {pair_id}`"
        ),
    );
    for (name, value) in controller_env(session_id, directory, server_url, pair_id) {
        let line = if cfg!(windows) {
            format!("$env:{name} = {}", quote_ps(&value))
        } else {
            format!("export {name}={}", quote_sh(&value))
        };
        say(ctx, &line);
    }
}

// ------------------------------------------------------------------ resume and attach

fn launcher_guard<T: Transport, C: Clock>(
    ctx: &LauncherCtx,
    engine: &Engine<T, C>,
) -> Result<(), ToolError> {
    if !ctx.host.offers_launcher() {
        return Err(ToolError::new(ErrorCode::RefusedOnHost, NOT_OFFERED));
    }
    if ctx.home != engine.env.home.as_path() {
        return Err(ToolError::new(
            ErrorCode::IoError,
            "launcher home differs from the engine's home",
        ));
    }
    Ok(())
}

fn check_pair_id(id: &str) -> Result<(), ToolError> {
    if valid_pair_id(id) {
        Ok(())
    } else {
        Err(invalid(format!(
            "'{}' is not a pair id",
            crate::compact::redact_text(id)
        ))
        .hint("pair ids match ^[0-9a-f]{12}$"))
    }
}

/// `resume <pair-id> [--controller c] [-- <controller args>]` -> (pair id, controller, args).
pub fn parse_resume_args(
    rest: &[String],
) -> Result<(String, Option<String>, Vec<String>), ToolError> {
    let usage = "resume <pair-id> [--controller claude|qwen|agy] [-- <controller args>]";
    let (mut id, mut controller) = (None::<String>, None::<String>);
    let mut it = rest.iter();
    let mut args = Vec::new();
    while let Some(w) = it.next() {
        match w.as_str() {
            "--" => {
                args = it.by_ref().cloned().collect();
                break;
            }
            "--controller" => {
                controller = Some(
                    it.next()
                        .cloned()
                        .ok_or_else(|| invalid("--controller needs a value".into()).hint(usage))?,
                );
            }
            f if f.starts_with("--controller=") => {
                controller = Some(f["--controller=".len()..].to_string());
            }
            f if f.starts_with('-') && f.len() > 1 => {
                return Err(invalid(format!("unknown option '{f}'")).hint(usage));
            }
            other => {
                if id.replace(other.to_string()).is_some() {
                    return Err(invalid(format!("unexpected argument '{other}'")).hint(usage));
                }
            }
        }
    }
    let id = id.ok_or_else(|| invalid("resume needs a pair id".into()).hint(usage))?;
    Ok((id, controller, args))
}

/// `attach <session-id | pair-id>` -> the target.
pub fn parse_attach_args(rest: &[String]) -> Result<String, ToolError> {
    match rest {
        [t] => Ok(t.clone()),
        _ => Err(invalid("attach takes one session id or pair id".into())
            .hint("attach <ses_... | pair-id>")),
    }
}

/// Deletes the `resume-lock` it was handed on drop, whatever way `resume` ends.
struct LockGuard(PathBuf);

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn remove_if_present(path: &Path) -> Result<(), ToolError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

fn live_controller(pair_id: &str) -> ToolError {
    ToolError::new(
        ErrorCode::LauncherConflict,
        format!("pair {pair_id} already has a live controller"),
    )
    .hint("two controllers on one worker is not supported; end the running one first")
}

/// `--controller`, else the record's, else the host constant (an `attach` record has `None`).
fn pick_controller(
    flag: Option<&str>,
    record: Option<&str>,
    host: Host,
) -> Result<String, ToolError> {
    let args = PairArgs {
        controller: flag.or(record).map(str::to_string),
        ..PairArgs::default()
    };
    resolve_controller(&args, host)
}

/// `resume <pair-id>` with the record's (else the host's) controller.
pub async fn resume<T: Transport, C: Clock>(
    pair_id: &str,
    controller_args: &[String],
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
) -> Result<i32, ToolError> {
    resume_with(pair_id, None, controller_args, ctx, engine).await
}

/// `resume` with an explicit `--controller` (which wins over the record and the host constant).
pub async fn resume_with<T: Transport, C: Clock>(
    pair_id: &str,
    controller_flag: Option<&str>,
    controller_args: &[String],
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
) -> Result<i32, ToolError> {
    launcher_guard(ctx, engine)?;
    check_pair_id(pair_id)?;
    let record = pairing::read_record(ctx.home, pair_id)?.ok_or_else(|| {
        ToolError::new(
            ErrorCode::PairNotFound,
            format!("no pair record for {pair_id}"),
        )
    })?;
    let controller = pick_controller(controller_flag, record.controller.as_deref(), ctx.host)?;
    validate_launch_values(
        &record.directory,
        Some(&record.session_id),
        pair_id,
        controller_args,
        cfg!(windows),
    )?;
    let _log = open_pair_log(ctx.home, pair_id);
    resume_run(pair_id, &record, &controller, controller_args, ctx, engine)
        .with_current_subscriber()
        .await
}

async fn resume_run<T: Transport, C: Clock>(
    pair_id: &str,
    record: &Record,
    controller: &str,
    controller_args: &[String],
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
) -> Result<i32, ToolError> {
    let home = ctx.home;
    tracing::info!("resume {pair_id}: controller {controller}");
    let params = vec![("sessionID".to_string(), record.session_id.clone())];
    engine.call_json("session.get", params, None).await?;

    let pid_file = pair_file(home, pair_id, "claude-pid");
    if pairing::read_sentinel(&pid_file).is_some_and(|s| sentinel_alive(&s, ctx.probe)) {
        return Err(live_controller(pair_id));
    }
    // Nothing changes when the controller cannot be started at all.
    if controller_command(
        controller,
        controller_args,
        cfg!(windows),
        crate::cli::env_nonempty("PATH").as_deref(),
        crate::cli::env_nonempty("PATHEXT").as_deref(),
    )
    .is_none()
    {
        return Err(controller_unavailable(ctx, pair_id, controller, None));
    }

    let lock_path = pair_file(home, pair_id, "resume-lock");
    // A lock whose owner is dead (a crash between its create and the claude-pid write) is swept
    // here too, so it never wedges the pair. An empty or unparsable lock may be mid-creation.
    if let Some(owner) = pairing::read_sentinel(&lock_path) {
        if !sentinel_alive(&owner, ctx.probe) {
            if let Some(f) = ctx.before_claim {
                f(); // test failpoint: between the dead-lock read and the unlink
            }
            // Re-read right before the unlink: delete only the exact owner judged dead. A lock that
            // now names someone else is a fresh winner; it stays and the create below loses. The
            // residual window between this re-read and the unlink is inherent to the spec's
            // delete-then-create and is covered by the claude-pid re-check after the claim.
            if pairing::read_sentinel(&lock_path).as_ref() == Some(&owner) {
                tracing::info!("resume {pair_id}: removing the dead lock of {owner}");
                remove_if_present(&lock_path)?;
            }
        }
    }
    let me = std::process::id();
    let own = Sentinel {
        pid: me,
        boot_id: ctx.probe.boot_id(),
        starttime: ctx.probe.starttime(me).ok_or_else(|| {
            ToolError::new(
                ErrorCode::IoError,
                format!("cannot read the start time of process {me}"),
            )
        })?,
    };
    if let Some(f) = ctx.before_claim {
        f(); // test failpoint: after the first claude-pid check, before the lock claim
    }
    if !claim_file(&lock_path, own.to_string().as_bytes(), 0o600)? {
        return Err(ToolError::new(
            ErrorCode::LauncherConflict,
            format!("another resume of pair {pair_id} is in progress"),
        )
        .hint("wait for it, or remove pair.<id>.resume-lock when its owner is gone"));
    }
    let _lock = LockGuard(lock_path.clone());
    // Re-check under the lock: another resume may have started a controller and dropped its lock
    // between the first check and our claim. Nothing is touched before this.
    if pairing::read_sentinel(&pid_file).is_some_and(|s| sentinel_alive(&s, ctx.probe)) {
        return Err(live_controller(pair_id));
    }
    remove_if_present(&pair_file(home, pair_id, "claude-exited"))?;
    remove_if_present(&pair_file(home, pair_id, "dead-seen"))?;
    step5(
        ctx,
        engine,
        pair_id,
        &record.session_id,
        &record.directory,
        controller,
        controller_args,
        record.server_url.as_deref(),
        Some(&lock_path),
    )
    .await
}

/// `pair.<id>.g<k>.*` for every `k < n`: the tabs of older generations.
fn delete_older_generations(home: &Path, pair_id: &str, n: u32) {
    let prefix = format!("pair.{pair_id}.g");
    let Ok(entries) = std::fs::read_dir(home) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(rest) = name.to_str().and_then(|n| n.strip_prefix(&prefix)) else {
            continue;
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if rest[digits.len()..].starts_with('.') && digits.parse::<u32>().is_ok_and(|k| k < n) {
            if let Err(e) = remove_if_present(&entry.path()) {
                tracing::warn!(
                    "attach {pair_id}: cannot remove {}: {e}",
                    entry.path().display()
                );
            }
        }
    }
}

fn announce_tab(ctx: &mut LauncherCtx, terminal: Option<Terminal>, pair_id: &str, wrapper: &Path) {
    match terminal {
        Some(t) => say(ctx, &format!("tui: {}", t.program())),
        None => say(
            ctx,
            &format!(
                "tui: none (no terminal found); run {} by hand to open the worker for pair {pair_id}",
                wrapper.display()
            ),
        ),
    }
}

/// `attach <session-id | pair-id>`.
pub async fn attach<T: Transport, C: Clock>(
    target: &str,
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
) -> Result<i32, ToolError> {
    launcher_guard(ctx, engine)?;
    if valid_pair_id(target) {
        attach_pair(target, ctx).await
    } else if target.starts_with("ses_") {
        if !valid_session_id(target) {
            return Err(
                invalid("not a session id".into()).hint("session ids match ^ses_[A-Za-z0-9]{26}$")
            );
        }
        attach_session(target, ctx, engine).await
    } else {
        Err(invalid(format!(
            "'{}' is neither a pair id nor a session id",
            crate::compact::redact_text(target)
        ))
        .hint("attach <ses_... | 12-hex pair id>"))
    }
}

async fn attach_pair(pair_id: &str, ctx: &mut LauncherCtx<'_>) -> Result<i32, ToolError> {
    let record = pairing::read_record(ctx.home, pair_id)?.ok_or_else(|| {
        ToolError::new(
            ErrorCode::PairNotFound,
            format!("no pair record for {pair_id}"),
        )
    })?;
    let windows = cfg!(windows);
    validate_launch_values(
        &record.directory,
        Some(&record.session_id),
        pair_id,
        &[],
        windows,
    )?;
    let opencode_bin = ctx.opencode_bin.ok_or_else(opencode_missing)?.to_path_buf();
    let _log = open_pair_log(ctx.home, pair_id);
    attach_pair_run(pair_id, &record, &opencode_bin, ctx)
        .with_current_subscriber()
        .await
}

async fn attach_pair_run(
    pair_id: &str,
    record: &Record,
    opencode_bin: &Path,
    ctx: &mut LauncherCtx<'_>,
) -> Result<i32, ToolError> {
    let windows = cfg!(windows);
    let home = ctx.home;
    let n = record.generation;
    let terminal = tab_plan(ctx, &PairArgs::default());
    tracing::info!("attach {pair_id}: generation {n}, terminal {terminal:?}");
    if let Some(f) = ctx.before_claim {
        f();
    }
    // The exclusive create of the wrapper is the claim token; orphans of a crashed attach are
    // skipped.
    let mut claimed = None;
    for g in (1..=5).map(|k| n.saturating_add(k)) {
        let input = WrapperInput {
            home,
            opencode_bin,
            pair_id,
            generation: g,
            directory: &record.directory,
            session_id: &record.session_id,
            server_url: record.server_url.as_deref(),
            server_password: ctx.server_password,
            boot_id_expr: true,
        };
        let (script, kind) = wrapper_text(&input, windows);
        let path = wrapper_path(home, pair_id, g, windows);
        if write_wrapper(&path, &script, kind)? {
            claimed = Some((g, path));
            break;
        }
        tracing::info!("attach {pair_id}: generation {g} is taken");
    }
    let Some((g, wrapper)) = claimed else {
        return Err(ToolError::new(
            ErrorCode::LauncherConflict,
            format!(
                "generations {}..{} of pair {pair_id} are all taken",
                n + 1,
                n + 5
            ),
        )
        .hint("another attach is running; retry in a moment"));
    };
    tracing::info!("attach {pair_id}: claimed generation {g}");
    if let Err(e) = pairing::rewrite_record(home, pair_id, g, terminal.is_some(), None) {
        tracing::warn!("attach {pair_id}: record not rewritten: {e}");
        let _ = std::fs::remove_file(&wrapper);
        return Err(e);
    }
    if let Err(e) = remove_if_present(&pair_file(home, pair_id, "dead-seen")) {
        tracing::warn!("attach {pair_id}: dead-seen not removed: {e}");
    }
    delete_older_generations(home, pair_id, n);
    if let Some(t) = terminal {
        let pid = open_tab(ctx, t, &wrapper, &record.directory)?;
        tracing::info!("attach {pair_id}: tab spawned ({}, pid {pid})", t.program());
    }
    announce_tab(ctx, terminal, pair_id, &wrapper);
    say(ctx, &format!("pair {pair_id}: generation {g}"));
    Ok(0)
}

async fn attach_session<T: Transport, C: Clock>(
    session_id: &str,
    ctx: &mut LauncherCtx<'_>,
    engine: &Engine<T, C>,
) -> Result<i32, ToolError> {
    let windows = cfg!(windows);
    let params = vec![("sessionID".to_string(), session_id.to_string())];
    let (data, _, _) = engine.call_json("session.get", params, None).await?;
    let directory = data
        .pointer("/location/directory")
        .and_then(Value::as_str)
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            ToolError::new(
                ErrorCode::IoError,
                format!("session {session_id} reports no directory"),
            )
        })?;
    let pair_id = mint_pair_id();
    validate_launch_values(&directory, Some(session_id), &pair_id, &[], windows)?;
    let opencode_bin = ctx.opencode_bin.ok_or_else(opencode_missing)?.to_path_buf();
    let _log = open_pair_log(ctx.home, &pair_id);
    let _scope = tracing::info_span!("attach").entered();
    tracing::info!("attach {pair_id}: new record for session {session_id}");
    let staged = stage_attached(
        ctx,
        &pair_id,
        session_id,
        &directory,
        &opencode_bin,
        engine.settings.server_url.as_deref(),
    );
    match staged {
        Ok((terminal, wrapper)) => {
            announce_tab(ctx, terminal, &pair_id, &wrapper);
            say(
                ctx,
                &format!(
                    "pair {pair_id}: new record for session {session_id}; start a controller with `opencode-pair resume {pair_id}`"
                ),
            );
            Ok(0)
        }
        Err(e) => {
            tracing::warn!("attach {pair_id}: {e}");
            // Neither a session (attach created none) nor anything else to remove.
            if let Err(d) = pairing::delete_pair_files(ctx.home, &pair_id) {
                tracing::warn!("attach {pair_id}: removing the pair files failed: {d}");
            }
            Err(e)
        }
    }
}

/// The new record (`controller: null`, generation 1, `tab` from the terminal walk), then the
/// wrapper, then the tab.
fn stage_attached(
    ctx: &mut LauncherCtx<'_>,
    pair_id: &str,
    session_id: &str,
    directory: &Path,
    opencode_bin: &Path,
    server_url: Option<&str>,
) -> Result<(Option<Terminal>, PathBuf), ToolError> {
    let windows = cfg!(windows);
    let home = ctx.home;
    let terminal = tab_plan(ctx, &PairArgs::default());
    write_record_new(
        home,
        &Record {
            pair_id: pair_id.to_string(),
            session_id: session_id.to_string(),
            directory: directory.to_path_buf(),
            server_url: server_url.map(str::to_string),
            created: (ctx.now_ms)(),
            controller: None,
            generation: 1,
            tab: terminal.is_some(),
        },
    )?;
    let input = WrapperInput {
        home,
        opencode_bin,
        pair_id,
        generation: 1,
        directory,
        session_id,
        server_url,
        server_password: ctx.server_password,
        boot_id_expr: true,
    };
    let (script, kind) = wrapper_text(&input, windows);
    let path = wrapper_path(home, pair_id, 1, windows);
    if !write_wrapper(&path, &script, kind)? {
        return Err(ToolError::new(
            ErrorCode::IoError,
            format!("{} already exists", path.display()),
        ));
    }
    if let Some(t) = terminal {
        let pid = open_tab(ctx, t, &path, directory)?;
        tracing::info!("attach {pair_id}: tab spawned ({}, pid {pid})", t.program());
    }
    Ok((terminal, path))
}

// ------------------------------------------------------------------ install-launcher

fn conflict(msg: String, hint: &str) -> ToolError {
    ToolError::new(ErrorCode::LauncherConflict, msg).hint(hint)
}

fn ensure_bin_dir(dir: &Path) -> Result<(), ToolError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    Ok(())
}

#[cfg(unix)]
fn replace_with_symlink(target: &Path, link: &Path) -> Result<(), ToolError> {
    let tmp = link.with_file_name(format!(".opencode-pair.tmp-{}", uuid::Uuid::new_v4()));
    std::os::unix::fs::symlink(target, &tmp)?;
    std::fs::rename(&tmp, link).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        ToolError::from(e)
    })
}

#[cfg(not(unix))]
fn replace_with_symlink(_target: &Path, _link: &Path) -> Result<(), ToolError> {
    Err(ToolError::new(
        ErrorCode::IoError,
        "symlinks are not used on this platform",
    ))
}

/// Windows: an in-use copy is first renamed to `.old` (which `gc` deletes), then the running exe is
/// copied over and the `.source` file written.
fn install_windows_copy(dir: &Path, exe: &Path, version: &str) -> Result<(), ToolError> {
    let copy = dir.join("opencode-pair.exe");
    let old = dir.join("opencode-pair.exe.old");
    let moved = copy.exists();
    if moved {
        let _ = std::fs::remove_file(&old);
        std::fs::rename(&copy, &old)?;
    }
    if let Err(e) = std::fs::copy(exe, &copy) {
        if moved {
            let _ = std::fs::rename(&old, &copy);
        }
        return Err(e.into());
    }
    let source = format!("path={}\nversion={version}\n", exe.display());
    std::fs::write(dir.join("opencode-pair.exe.source"), source)?;
    Ok(())
}

fn same_bytes(a: &Path, b: &Path) -> bool {
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

pub fn install_launcher(ctx: &mut LauncherCtx<'_>, force: bool) -> Result<(), ToolError> {
    install_launcher_as(ctx, force, cfg!(windows))
}

/// `install-launcher [--force]` for a platform flag (so the Windows copy rules run on every leg).
pub fn install_launcher_as(
    ctx: &mut LauncherCtx<'_>,
    force: bool,
    windows: bool,
) -> Result<(), ToolError> {
    if !ctx.host.offers_launcher() {
        return Err(ToolError::new(ErrorCode::RefusedOnHost, NOT_OFFERED));
    }
    let (dir, exe) = (ctx.bin_dir, ctx.running_exe);
    ensure_bin_dir(dir)?;
    let link = dir.join(if windows {
        "opencode-pair.exe"
    } else {
        "opencode-pair"
    });
    let status = if windows {
        let source: Option<SourceFile> =
            std::fs::read_to_string(dir.join("opencode-pair.exe.source"))
                .ok()
                .and_then(|raw| parse_source_file(&raw));
        match windows_link_status(link.is_file(), source.as_ref(), exe, ctx.host, ctx.version) {
            // Same path and version, but a rebuilt binary: replace the copy.
            LinkStatus::Fresh if !same_bytes(&link, exe) => LinkStatus::Stale,
            other => other,
        }
    } else {
        launcher_link_status(dir, exe, ctx.host, ctx.version)
    };
    let l = link.display().to_string();
    let install = |ctx: &mut LauncherCtx| -> Result<(), ToolError> {
        if windows {
            install_windows_copy(dir, exe, ctx.version)?;
        } else {
            replace_with_symlink(exe, &link)?;
        }
        say(ctx, &format!("installed: {l} -> {}", exe.display()));
        Ok(())
    };
    match status {
        LinkStatus::Missing | LinkStatus::Stale => install(ctx)?,
        LinkStatus::Fresh => say(ctx, &format!("already current: {l}")),
        LinkStatus::OtherHost if force => install(ctx)?,
        LinkStatus::OtherHost => {
            return Err(conflict(
                format!("{l} belongs to another host's binary"),
                "run `install-launcher --force` to take it over",
            ))
        }
        LinkStatus::NotALink if windows && force => install(ctx)?,
        LinkStatus::NotALink => {
            return Err(conflict(
                format!("{l} is not a link install-launcher made; it is left alone"),
                if windows {
                    "run `install-launcher --force` to replace it"
                } else {
                    "remove it by hand, or install into another directory"
                },
            ))
        }
    }
    if !(ctx.path_has)("opencode-pair") {
        say(
            ctx,
            &format!(
                "note: {} is not on PATH; add it to PATH to run `opencode-pair`",
                dir.display()
            ),
        );
    }
    Ok(())
}

// ------------------------------------------------------------------ real-process seams

/// `name` (or `name.exe`, `.cmd`, `.bat` on Windows) found as a file in a directory of `PATH`.
pub fn system_path_has(name: &str) -> bool {
    let Some(path) = crate::cli::env_nonempty("PATH") else {
        return false;
    };
    let exts: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    std::env::split_paths(&path).any(|dir| {
        !dir.as_os_str().is_empty()
            && exts
                .iter()
                .any(|e| dir.join(format!("{name}{e}")).is_file())
    })
}

/// The terminal tab: spawned and left to run; the child is reaped on a thread so it never lingers
/// as a zombie of this long-lived process.
pub fn system_spawn_detached(cmd: &mut Command) -> io::Result<u32> {
    let mut child = cmd.spawn()?;
    let pid = child.id();
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(pid)
}

/// The controller: spawn with inherited stdio, hand the pid to `on_pid`, wait.
pub fn system_spawn_and_wait(
    cmd: &mut Command,
    on_pid: &mut dyn FnMut(u32),
) -> io::Result<std::process::ExitStatus> {
    let mut child = cmd.spawn()?;
    on_pid(child.id());
    child.wait()
}
