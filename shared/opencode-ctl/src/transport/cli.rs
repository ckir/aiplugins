//! The real transport: shells out to the `opencode` CLI.
//!
//! Stdout goes to a temp file, never a pipe: the CLI writes the body with one un-awaited
//! `process.stdout.write` and then calls `process.exit`, so a pipe loses whatever the reader has
//! not drained (spec Transport). The file is deleted by a drop guard on every exit path.

use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use proc_guard::JobObject;
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::{service_running, Lifecycle, Params, Transport, TransportError, WaitOutcome};
use crate::error::{ErrorCode, ToolError};
use crate::liveness::{ProcessProbe, SystemProbe};
use crate::openapi::{validate_params, validate_session_id, Method, Operation};
use crate::settings::Settings;

/// Runaway-output ceiling for a child's stdout file (not a budget: legitimate reads are far below).
pub const STDOUT_READ_CEILING: u64 = 32 * 1024 * 1024;

const SIZE_POLL: Duration = Duration::from_millis(100);
const STDERR_CAP: usize = 256 * 1024;
const STDERR_DRAIN: Duration = Duration::from_secs(2);

pub struct CliTransport {
    bin: OnceLock<Result<PathBuf, TransportError>>,
    bin_hint: Option<PathBuf>,
    server_url: Option<String>,
    stdout_dir: PathBuf,
    call_timeout: Duration,
    body_max_bytes: usize,
    extra_env: Vec<(String, String)>,
    removed_env: Vec<String>,
    path: Option<String>,
    /// The OS home: the third input of `resolve_binary`, kept because resolution is lazy.
    user_home: PathBuf,
}

impl CliTransport {
    /// `home` is the ctl home (for the stdout directory), `user_home` the OS home.
    pub fn new(settings: &Settings, home: &Path, user_home: &Path) -> Self {
        let pid = std::process::id();
        let start = SystemProbe
            .starttime(pid)
            .unwrap_or_else(|| "0".to_string());
        Self {
            bin: OnceLock::new(),
            bin_hint: settings.opencode_bin.clone(),
            server_url: settings.server_url.clone(),
            stdout_dir: home.join("stdout").join(format!("{pid}-{start}")),
            call_timeout: Duration::from_secs(settings.call_timeout_s),
            body_max_bytes: settings.body_max_bytes,
            extra_env: Vec::new(),
            removed_env: Vec::new(),
            path: None,
            user_home: user_home.to_path_buf(),
        }
    }

    /// Variables added to every child's environment.
    pub fn with_extra_env(mut self, env: Vec<(String, String)>) -> Self {
        self.extra_env = env;
        self
    }

    /// Variables removed from every child's environment (applied after `with_extra_env`).
    pub fn with_env_removed(mut self, names: Vec<String>) -> Self {
        self.removed_env = names;
        self
    }

    /// The PATH value `resolve_binary` searches; `None` = no PATH search.
    pub fn with_path(mut self, path: Option<String>) -> Self {
        self.path = path;
        self
    }

    /// This process's stdout directory `<home>/stdout/<pid>-<starttime>` (created on first use).
    pub fn stdout_dir(&self) -> &Path {
        &self.stdout_dir
    }

    fn bin(&self) -> Result<&Path, TransportError> {
        let r = self.bin.get_or_init(|| {
            resolve_binary(
                self.bin_hint.as_deref(),
                self.path.as_deref(),
                &self.user_home,
            )
            .map_err(from_tool)
        });
        match r {
            Ok(p) => Ok(p.as_path()),
            Err(e) => Err(e.clone()),
        }
    }
}

fn from_tool(e: ToolError) -> TransportError {
    TransportError {
        code: e.code,
        message: e.message,
        hint: e.hint,
        details: e.details,
    }
}

/// The executable an npm install of opencode keeps beside its `opencode.cmd` shim in `dir`
/// (`node_modules\@opencode\cli\bin\opencode.exe`, the very file the shim runs), when both exist. The
/// shim itself is never started: `cmd.exe` would expand `%VAR%` inside prompt text passed as arguments.
pub fn npm_shim_exe(dir: &Path) -> Option<PathBuf> {
    if !dir.join("opencode.cmd").is_file() {
        return None;
    }
    let exe = dir.join("node_modules/@opencode/cli/bin/opencode.exe");
    exe.is_file().then_some(exe)
}

/// Explicit path; else the first `opencode`/`opencode.exe` on `path` (on Windows also the executable behind
/// an npm `opencode.cmd` shim, see `npm_shim_exe`); else `<home>/.opencode/bin/opencode[.exe]`; else
/// `OPENCODE_NOT_FOUND`. Pure: nothing is spawned.
pub fn resolve_binary(
    explicit: Option<&Path>,
    path: Option<&str>,
    home_dir: &Path,
) -> Result<PathBuf, ToolError> {
    let name = if cfg!(windows) {
        "opencode.exe"
    } else {
        "opencode"
    };
    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }
    if let Some(path) = path {
        for dir in std::env::split_paths(OsStr::new(path)) {
            if dir.as_os_str().is_empty() {
                continue;
            }
            let cand = dir.join(name);
            if cand.is_file() {
                return Ok(cand);
            }
            if cfg!(windows) {
                if let Some(exe) = npm_shim_exe(&dir) {
                    return Ok(exe);
                }
            }
        }
    }
    let fallback = home_dir.join(".opencode").join("bin").join(name);
    if fallback.is_file() {
        return Ok(fallback);
    }
    Err(
        ToolError::new(ErrorCode::OpencodeNotFound, "opencode executable not found")
            .hint("install opencode or set OPENCODE_CTL_BIN"),
    )
}

/// The one mapping of a failed `Command::spawn`.
fn spawn_error(e: &io::Error) -> TransportError {
    match e.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => TransportError {
            code: ErrorCode::OpencodeNotFound,
            message: format!("opencode executable could not be started: {e}"),
            hint: Some("install opencode or set OPENCODE_CTL_BIN".to_string()),
            details: None,
        },
        _ => TransportError {
            code: ErrorCode::IoError,
            message: format!("could not start opencode: {e}"),
            hint: None,
            details: None,
        },
    }
}

fn io_error(what: &str, e: &io::Error) -> TransportError {
    TransportError {
        code: ErrorCode::IoError,
        message: format!("{what}: {e}"),
        hint: None,
        details: None,
    }
}

/// What a failed (non-zero exit) CLI run maps to.
pub enum Mapped {
    Error(TransportError),
    /// The `CLI_ERROR` to return if the reachability probe says the service is up.
    ProbeNeeded(TransportError),
}

fn err(code: ErrorCode, message: impl Into<String>) -> TransportError {
    TransportError {
        code,
        message: message.into(),
        hint: None,
        details: None,
    }
}

const AUTH_HINT: &str =
    "compare `opencode service get password` (local service) or the remote's password; \
the CLI reads OPENCODE_PASSWORD, then OPENCODE_SERVER_PASSWORD, from the environment";

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|l| !l.is_empty())
}

fn exit_text(code: Option<i32>) -> String {
    code.map_or_else(|| "by a signal".to_string(), |c| c.to_string())
}

/// The spec's mapping table, in order. Reached only after a successful spawn.
pub fn map_cli_failure(status: Option<i32>, stdout: &str, stderr: &str) -> Mapped {
    // 1. A `_tag` on stdout.
    if let Ok(Value::Object(o)) = serde_json::from_str::<Value>(stdout.trim()) {
        if let Some(tag) = o.get("_tag").and_then(Value::as_str) {
            let message = o.get("message").and_then(Value::as_str).unwrap_or(tag);
            let code = match tag {
                "SessionNotFoundError" => ErrorCode::SessionNotFound,
                "UnauthorizedError" => ErrorCode::AuthFailed,
                "InvalidRequestError"
                | "InvalidCursorError"
                | "FormInvalidAnswerError"
                | "MessageNotFoundError"
                | "LocationNotFoundError" => ErrorCode::InvalidRequest,
                "ServiceUnavailableError" => ErrorCode::ServiceDown,
                "ConflictError"
                | "SessionBusyError"
                | "FormAlreadySettledError"
                | "PermissionNotFoundError"
                | "FormNotFoundError" => ErrorCode::Conflict,
                _ => ErrorCode::CliError,
            };
            let mut e = err(code, message);
            e.details = Some(json!({ "tag": tag }));
            if code == ErrorCode::AuthFailed {
                e.hint = Some(AUTH_HINT.to_string());
            }
            return Mapped::Error(e);
        }
    }
    // 2. A password complaint.
    if stderr.contains("rejected the password") || stderr.contains("requires a password") {
        let mut e = err(
            ErrorCode::AuthFailed,
            first_line(stderr).unwrap_or("the server rejected the password"),
        );
        e.hint = Some(AUTH_HINT.to_string());
        return Mapped::Error(e);
    }
    // 3. Unreachable (before the `[cause]:` rule: that stderr carries a `[cause]: ClientError` line).
    if stderr.contains("Could not reach server at") {
        let line = stderr
            .lines()
            .map(str::trim)
            .find(|l| l.contains("Could not reach server at"))
            .unwrap_or("Could not reach server");
        let mut e = err(ErrorCode::ServiceDown, line);
        e.hint = Some("start the service with service_start, or check server_url".to_string());
        return Mapped::Error(e);
    }
    // 4. Empty stdout and a `[cause]:` line: the FIRST one is the message.
    if stdout.trim().is_empty() {
        let cause = stderr
            .lines()
            .map(str::trim)
            .find_map(|l| l.strip_prefix("[cause]:"))
            .map(str::trim);
        if let Some(cause) = cause {
            let msg = cause.strip_prefix("Error: ").unwrap_or(cause);
            let code = if cause.contains("Operation not found") {
                ErrorCode::UnknownOperation
            } else {
                ErrorCode::CliError
            };
            return Mapped::Error(err(code, msg));
        }
    }
    // 5. Anything else: decided by the reachability probe.
    let message = first_line(stderr).map_or_else(
        || format!("opencode exited {} without output", exit_text(status)),
        str::to_string,
    );
    Mapped::ProbeNeeded(err(ErrorCode::CliError, message))
}

fn deadline_error(what: &str, deadline: Duration, mutating: bool) -> TransportError {
    let mut e = err(
        ErrorCode::Deadline,
        format!("{what} did not finish within {} s", deadline.as_secs()),
    );
    if mutating {
        e.details = Some(json!({ "outcome": "unknown" }));
        e.hint = Some(
            "the call may have been applied: check the inbox and messages before repeating it"
                .to_string(),
        );
    }
    e
}

fn ceiling_error() -> TransportError {
    let mut e = err(
        ErrorCode::CliError,
        "opencode output passed the 32 MiB read ceiling and was cut",
    );
    e.details = Some(json!({ "reason": "stdout_ceiling" }));
    e
}

fn unparseable(mutating: bool) -> TransportError {
    let mut e = err(
        ErrorCode::CliError,
        "opencode returned no parseable JSON on stdout",
    );
    let mut d = json!({ "reason": "unparseable stdout" });
    if mutating {
        d["outcome"] = json!("unknown");
        e.hint = Some(
            "the call may have been applied: check the inbox and messages before repeating it"
                .to_string(),
        );
    }
    e.details = Some(d);
    e
}

/// Deletes the stdout temp file on every path, including a failed spawn.
struct StdoutFile {
    path: PathBuf,
}

impl Drop for StdoutFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum How {
    Exited,
    Deadline,
    Ceiling,
}

struct Finished {
    how: How,
    code: Option<i32>,
    success: bool,
    stdout: String,
    stderr: String,
}

impl CliTransport {
    fn ensure_stdout_dir(&self) -> Result<(), TransportError> {
        let mut b = std::fs::DirBuilder::new();
        b.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        b.create(&self.stdout_dir)
            .map_err(|e| io_error("cannot create the stdout directory", &e))
    }

    fn server_args(&self, args: &mut Vec<String>) {
        if let Some(url) = &self.server_url {
            args.push("--server".to_string());
            args.push(url.clone());
        }
    }

    /// Spawn `opencode <args>` with stdout in a temp file, bounded by `deadline`.
    async fn run(&self, args: &[String], deadline: Duration) -> Result<Finished, TransportError> {
        let bin = self.bin()?;
        self.ensure_stdout_dir()?;
        let path = self
            .stdout_dir
            .join(format!("{}.json", uuid::Uuid::new_v4()));
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts
            .open(&path)
            .map_err(|e| io_error("cannot create the stdout file", &e))?;
        let guard = StdoutFile { path };

        let mut cmd = Command::new(bin);
        cmd.args(args)
            .envs(self.extra_env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::from(file))
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for k in &self.removed_env {
            cmd.env_remove(k);
        }
        let mut job =
            JobObject::new().map_err(|e| io_error("cannot create the process guard", &e))?;
        #[cfg(unix)]
        job.configure_command(cmd.as_std_mut());
        let spawned = cmd.spawn();
        drop(cmd); // closes the parent's copy of the stdout file
        let mut child = spawned.map_err(|e| spawn_error(&e))?;
        #[cfg(unix)]
        if let Some(pid) = child.id() {
            job.assign_pid(pid);
        }
        #[cfg(windows)]
        if let Some(h) = child.raw_handle() {
            if let Err(e) = job.assign_raw_handle(h) {
                let _ = child.start_kill();
                return Err(io_error("cannot assign the process guard", &e));
            }
        }

        let mut stderr_pipe = child.stderr.take();
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            if let Some(p) = stderr_pipe.as_mut() {
                let mut chunk = [0u8; 8192];
                while let Ok(n) = p.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }
                    let room = STDERR_CAP.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..n.min(room)]);
                }
            }
            buf
        });

        let sleep = tokio::time::sleep(deadline);
        tokio::pin!(sleep);
        let mut tick = tokio::time::interval(SIZE_POLL);
        let mut how = How::Exited;
        let mut status = None;
        loop {
            tokio::select! {
                st = child.wait() => {
                    status = st.ok();
                    break;
                }
                _ = &mut sleep => {
                    how = How::Deadline;
                    break;
                }
                _ = tick.tick() => {
                    if std::fs::metadata(&guard.path).map_or(0, |m| m.len()) > STDOUT_READ_CEILING {
                        how = How::Ceiling;
                        break;
                    }
                }
            }
        }
        if how != How::Exited {
            let _ = job.kill_group();
            let _ = child.start_kill();
            status = child.wait().await.ok();
        }
        // Stragglers of the process group die with it; the leader is already reaped. On Windows a call
        // that exited normally keeps what it started: a managed-mode call auto-starts the service, and
        // unlike POSIX (`detached: true` puts it in its own session) the service is inside this job, so
        // killing the job here killed the service the call had just started (MEASURED, Task 25). A
        // deadline or the ceiling still kills the tree above.
        #[cfg(windows)]
        if how == How::Exited && job.release().is_ok() {
            // disarmed: nothing is killed
        } else {
            let _ = job.kill_group();
        }
        #[cfg(not(windows))]
        let _ = job.kill_group();
        let stderr_bytes = match tokio::time::timeout(STDERR_DRAIN, stderr_task).await {
            Ok(Ok(b)) => b,
            _ => Vec::new(),
        };
        // A fast writer cannot bypass the ceiling: check again before reading the file.
        if how == How::Exited
            && std::fs::metadata(&guard.path).map_or(0, |m| m.len()) > STDOUT_READ_CEILING
        {
            how = How::Ceiling;
        }
        let stdout = if how == How::Exited {
            let bytes = tokio::fs::read(&guard.path)
                .await
                .map_err(|e| io_error("cannot read the stdout file", &e))?;
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            String::new()
        };
        Ok(Finished {
            how,
            code: status.and_then(|s| s.code()),
            success: status.is_some_and(|s| s.success()),
            stdout,
            stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        })
    }

    /// `opencode service status` once (deadline `call_timeout`): non-zero exit or `stopped` is
    /// `SERVICE_DOWN`, otherwise the carried error.
    async fn probe(&self, carried: TransportError) -> TransportError {
        let args = ["service".to_string(), "status".to_string()];
        match self.run(&args, self.call_timeout).await {
            Err(e) => e,
            Ok(f) if f.how == How::Exited && (!f.success || !service_running(&f.stdout)) => {
                let mut e = err(
                    ErrorCode::ServiceDown,
                    "the opencode service is not running",
                );
                e.hint = Some("start it with service_start (`opencode service start`)".to_string());
                e
            }
            Ok(_) => carried,
        }
    }

    /// A failed run mapped, probed when the mapping asks for it and `server_url` is unset.
    async fn fail(&self, mapped: Mapped) -> TransportError {
        match mapped {
            Mapped::Error(mut e) => {
                if let (ErrorCode::AuthFailed, Some(url)) = (e.code, &self.server_url) {
                    e.hint = Some(format!(
                        "wrong password for {url}: compare `opencode service get password` (local) or \
                         the remote's password with OPENCODE_PASSWORD / OPENCODE_SERVER_PASSWORD"
                    ));
                }
                e
            }
            Mapped::ProbeNeeded(e) if self.server_url.is_some() => e,
            Mapped::ProbeNeeded(e) => self.probe(e).await,
        }
    }

    /// Run and settle everything but a successful exit: deadline, ceiling and a mapped failure.
    async fn run_ok(
        &self,
        what: &str,
        args: &[String],
        deadline: Duration,
        mutating: bool,
    ) -> Result<Finished, TransportError> {
        let f = self.run(args, deadline).await?;
        match f.how {
            How::Deadline => Err(deadline_error(what, deadline, mutating)),
            How::Ceiling => Err(ceiling_error()),
            How::Exited if f.success => Ok(f),
            How::Exited => Err(self
                .fail(map_cli_failure(f.code, &f.stdout, &f.stderr))
                .await),
        }
    }
}

fn plain_failure(f: &Finished, what: &str) -> TransportError {
    let message = first_line(&f.stderr).map_or_else(
        || format!("{what} exited {} without output", exit_text(f.code)),
        str::to_string,
    );
    err(ErrorCode::CliError, message)
}

fn verb(v: Lifecycle) -> &'static str {
    match v {
        Lifecycle::Start => "start",
        Lifecycle::Stop => "stop",
        Lifecycle::Restart => "restart",
        Lifecycle::Status => "status",
    }
}

impl Transport for CliTransport {
    async fn call(
        &self,
        op: &Operation,
        params: &Params,
        body: Option<&Value>,
        deadline: Duration,
    ) -> Result<Value, TransportError> {
        validate_params(op, params).map_err(from_tool)?;
        for (name, value) in params {
            if name == "sessionID" {
                validate_session_id(value).map_err(from_tool)?;
            }
        }
        let body_json = body.map(Value::to_string);
        if body_json
            .as_ref()
            .is_some_and(|b| b.len() > self.body_max_bytes)
        {
            let mut e = err(
                ErrorCode::BodyTooLarge,
                format!(
                    "the request body is larger than {} bytes",
                    self.body_max_bytes
                ),
            );
            e.hint = Some(
                "shorten the prompt, or write it into the workspace and reference the path"
                    .to_string(),
            );
            return Err(e);
        }
        let mut args = vec!["api".to_string(), op.id.clone()];
        for (k, v) in params {
            args.push("--param".to_string());
            args.push(format!("{k}={v}"));
        }
        if let Some(b) = body_json {
            args.push("-d".to_string());
            args.push(b);
        }
        self.server_args(&mut args);

        let mutating = op.mutating();
        let attempts = if op.method == Method::Get { 2 } else { 1 };
        let what = format!("opencode api {}", op.id);
        let mut last = unparseable(mutating);
        for _ in 0..attempts {
            let f = self.run_ok(&what, &args, deadline, mutating).await?;
            let text = f.stdout.trim();
            if text.is_empty() {
                if op.success_status == 204 {
                    return Ok(json!({ "ok": true }));
                }
            } else if let Ok(v) = serde_json::from_str::<Value>(text) {
                return Ok(v);
            }
            last = unparseable(mutating);
        }
        Err(last)
    }

    async fn lifecycle(&self, v: Lifecycle) -> Result<String, TransportError> {
        let name = verb(v);
        let args = ["service".to_string(), name.to_string()];
        let what = format!("opencode service {name}");
        let f = self.run(&args, self.call_timeout).await?;
        match f.how {
            How::Deadline => Err(deadline_error(
                &what,
                self.call_timeout,
                v != Lifecycle::Status,
            )),
            How::Ceiling => Err(ceiling_error()),
            How::Exited if f.success => Ok(f.stdout.trim().to_string()),
            // The service verbs print no JSON: no `_tag` parsing and no probe.
            How::Exited => Err(plain_failure(&f, &what)),
        }
    }

    async fn wait_idle(
        &self,
        session_id: &str,
        deadline: Duration,
    ) -> Result<WaitOutcome, TransportError> {
        validate_session_id(session_id).map_err(from_tool)?;
        let mut args = vec![
            "api".to_string(),
            "experimental.session.wait".to_string(),
            "--param".to_string(),
            format!("sessionID={session_id}"),
        ];
        self.server_args(&mut args);
        let f = self.run(&args, deadline).await?;
        match f.how {
            How::Deadline => Ok(WaitOutcome::DeadlineHit),
            How::Ceiling => Err(ceiling_error()),
            How::Exited if f.success => Ok(WaitOutcome::Idle),
            How::Exited => Err(self
                .fail(map_cli_failure(f.code, &f.stdout, &f.stderr))
                .await),
        }
    }

    async fn fetch_openapi(&self) -> Result<Value, TransportError> {
        let mut args = vec![
            "api".to_string(),
            "GET".to_string(),
            "/openapi.json".to_string(),
        ];
        self.server_args(&mut args);
        let f = self
            .run_ok(
                "opencode api GET /openapi.json",
                &args,
                self.call_timeout,
                false,
            )
            .await?;
        serde_json::from_str(f.stdout.trim()).map_err(|_| unparseable(false))
    }

    async fn cli_version(&self) -> Result<String, TransportError> {
        let args = ["--version".to_string()];
        let f = self.run(&args, self.call_timeout).await?;
        match f.how {
            How::Deadline => Err(deadline_error(
                "opencode --version",
                self.call_timeout,
                false,
            )),
            How::Ceiling => Err(ceiling_error()),
            How::Exited if f.success => Ok(f.stdout.trim().to_string()),
            How::Exited => Err(plain_failure(&f, "opencode --version")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_error_kinds() {
        for (kind, code) in [
            (io::ErrorKind::NotFound, ErrorCode::OpencodeNotFound),
            (io::ErrorKind::PermissionDenied, ErrorCode::OpencodeNotFound),
            (io::ErrorKind::Other, ErrorCode::IoError),
        ] {
            assert_eq!(spawn_error(&io::Error::from(kind)).code, code, "{kind:?}");
        }
    }

    /// The sixth spawn site runs only after a spawn succeeded, so it is driven directly.
    #[tokio::test]
    async fn probe_spawn_failure_is_opencode_not_found() {
        let dir = crate::testing::temp_home();
        let settings = Settings {
            opencode_bin: Some(dir.path().join("missing")),
            ..Settings::default()
        };
        let t = CliTransport::new(&settings, dir.path(), dir.path());
        let carried = err(ErrorCode::CliError, "carried");
        let e = t.probe(carried).await;
        assert_eq!(e.code, ErrorCode::OpencodeNotFound);
        assert_eq!(
            std::fs::read_dir(t.stdout_dir()).map_or(0, |d| d.count()),
            0
        );
    }
}
