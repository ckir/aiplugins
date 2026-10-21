//! `doctor-json`: the eighteen checks behind the `opencode-ctl-doctor` skill (spec Skills).
//!
//! The doctor COMPUTES the checks in a fixed order (`ctl_home_override`, `binary`, `registration`,
//! `server_info`, then the rest in id order) and EMITS them in id order. `registration` is read
//! before anything can start the service, so an `ok_started` cannot rewrite the file it reports on.
//! `server_info` is the ONE place that starts a stopped service, explicitly; every other check
//! leaves the service alone: `wait_route` never fetches the OpenAPI document itself (it reads the
//! table `server.info` already cached) and `registration` never calls `server.info`.
//!
//! Every pair decision keys on the FILE NAME's id (`pair.<id>.json`), never on the id inside the
//! record: `records_by_name` enumerates the files itself and overwrites the record's inner id.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::cli::blank_is_unset;
use crate::clock::Clock;
use crate::compact::redact_text;
use crate::engine::Engine;
use crate::error::{ErrorCode, ToolError};
use crate::host::Host;
use crate::liveness::{side_liveness, Liveness, Side};
use crate::openapi::TableSource;
use crate::pairing::{list_records, read_record, Record};
use crate::settings::{self, Note, NoteReason, Source};
use crate::transport::cli::resolve_binary;
use crate::transport::{parse_cli_version, service_running, Lifecycle, Transport};

/// The check ids, in emit order.
pub const CHECK_IDS: [&str; 18] = [
    "ctl_home_override",
    "binary",
    "version_range",
    "registration",
    "server_info",
    "wait_route",
    "pairing",
    "lock_source",
    "env_vs_user_file",
    "windows_source_old",
    "launcher_link",
    "server_url_password",
    "opencode_password_set",
    "log",
    "orphaned_controllers",
    "orphaned_workers",
    "settings_notes",
    "settings_file_hash",
];

pub struct DoctorCtx<'a> {
    /// The launcher install dir (`~/.local/bin`, `%LOCALAPPDATA%\opencode-ctl`).
    pub bin_dir: &'a Path,
    /// `std::env::current_exe()`.
    pub running_exe: &'a Path,
    /// The front's `CARGO_PKG_VERSION`, for `launcher_link_status`.
    pub version: &'a str,
    /// The OS home: third input of `resolve_binary`.
    pub user_home: &'a Path,
    /// `env_nonempty("PATH")`: second input of `resolve_binary`.
    pub path: Option<String>,
    /// The CLI's `Service.Info` registration file (packages/client/src/effect/service.ts:146).
    pub registration_file: Option<&'a Path>,
    /// The CLI's `ServiceConfig.Info` file (packages/cli/src/services/service-config.ts:16).
    pub config_file: Option<&'a Path>,
    /// The RAW environment lookup. Every read except `OPENCODE_CTL_SERVER` goes through `blank_is_unset`.
    pub env: &'a dyn Fn(&str) -> Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
    Skip,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Fail => "fail",
            Status::Skip => "skip",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub id: &'static str,
    pub status: Status,
    pub message: String,
    pub details: Option<Value>,
}

impl Check {
    fn new(id: &'static str, status: Status, variant: &str, subs: &[(&str, &str)]) -> Check {
        Check {
            id,
            status,
            message: message(id, status, variant, subs),
            details: None,
        }
    }

    fn details(mut self, d: Value) -> Check {
        self.details = Some(d);
        self
    }

    pub fn to_json(&self) -> Value {
        let mut v = json!({"id": self.id, "status": self.status.as_str(), "message": self.message});
        if let Some(d) = &self.details {
            v["details"] = d.clone();
        }
        v
    }
}

const SIDES: &str = "pair <pair>: tui <tui>; controller <controller>; generation <generation> sentinel <sentinel>; session <session> <session_state>";

/// Every message the doctor can print, keyed by `(id, status, variant)`; `<...>` placeholders are
/// filled by `message`. A check with several texts per status has one variant tag per text.
const MESSAGES: &[(&str, &str, &str, &str)] = &[
    ("ctl_home_override", "ok", "", "OPENCODE_CTL_HOME is not set"),
    ("ctl_home_override", "warn", "", "OPENCODE_CTL_HOME is set to <path>: the settings file and the pair files are read from there"),
    ("binary", "ok", "", "opencode found at <path>"),
    ("binary", "fail", "", "opencode executable not found: install opencode or set OPENCODE_CTL_BIN"),
    ("binary", "fail", "missing_explicit", "the configured opencode_bin <path> is not a file"),
    ("version_range", "skip", "", "binary failed"),
    ("version_range", "fail", "unparsable", "unparsable version output"),
    ("version_range", "fail", "below", "<cli> is below 2.0"),
    ("version_range", "fail", "cli_error", "cannot run opencode --version: <message>"),
    ("version_range", "warn", "server_outside_range", "server <server> is outside the tested 2.0.x range"),
    ("version_range", "ok", "cli_only", "server version unknown; CLI <cli>"),
    ("version_range", "ok", "", "CLI <cli> and server <server> are within the tested 2.0.x range"),
    ("registration", "skip", "", "no registration file at <path>"),
    ("registration", "skip", "no_path", "no registration file location is known"),
    ("registration", "warn", "unreadable", "registration file <path> is not a valid service registration"),
    ("registration", "fail", "", "registered pid <pid> is dead"),
    ("registration", "warn", "disabled", "service disabled in <config_file>: every CLI call runs a private standalone server"),
    ("registration", "ok", "", "registered pid <pid> is alive"),
    ("server_info", "ok", "", "server.info answered: version <version>, pid <pid>"),
    ("server_info", "ok", "ok_started", "service was stopped; opencode service start brought it up"),
    ("server_info", "fail", "fail_start", "service stopped and opencode service start failed: <stderr>"),
    ("server_info", "fail", "fail_unreachable", "cannot reach <url>: <message>"),
    ("server_info", "fail", "fail_auth", "server at <url> rejected the credentials: compare `opencode service get password` with OPENCODE_SERVER_PASSWORD"),
    ("server_info", "fail", "fail_other", "server.info failed (<code>): <message>"),
    ("wait_route", "skip", "", "server_info failed"),
    ("wait_route", "ok", "", "experimental.session.wait is in the operation table"),
    ("wait_route", "warn", "missing", "experimental.session.wait is not in the operation table: session_wait falls back to polling"),
    ("wait_route", "warn", "unconfirmed", "the live operation table is unavailable, so experimental.session.wait could not be confirmed"),
    ("pairing", "skip", "", "not paired: OPENCODE_CTL_PAIR is not set"),
    ("pairing", "fail", "record", "no usable pair record for <pair>"),
    ("pairing", "fail", "unreadable", "pair record for <pair> is unreadable: <error>"),
    ("pairing", "fail", "session", SIDES),
    ("pairing", "warn", "", SIDES),
    ("pairing", "ok", "", SIDES),
    ("lock_source", "ok", "", "permissions_locked=<locked> (<locked_source>); permission_policy=<policy> (<policy_source>)"),
    ("env_vs_user_file", "ok", "", "no protected key's environment value differs from the user-level file"),
    ("env_vs_user_file", "warn", "", "environment overrides the user-level file for: <keys>"),
    ("windows_source_old", "skip", "off_windows", "not applicable off Windows"),
    ("windows_source_old", "skip", "", "no launcher copy at <path>"),
    ("windows_source_old", "ok", "", "opencode-pair.exe.source is valid and no .old leftover exists"),
    ("windows_source_old", "warn", "", "launcher copy problems: <problems>"),
    ("launcher_link", "skip", "", "not offered on this host"),
    ("launcher_link", "ok", "", "<link> resolves to the running binary"),
    ("launcher_link", "warn", "missing", "missing: <link> does not exist; run install-launcher"),
    ("launcher_link", "warn", "stale", "stale: <link> does not match the running binary (version <version>); re-run install-launcher"),
    ("launcher_link", "warn", "other_host", "other_host: <link> belongs to another host's binary; run install-launcher --force to take it over"),
    ("launcher_link", "warn", "not_a_link", "not_a_link: <link> is not a link install-launcher made; it is left alone and install-launcher reports LAUNCHER_CONFLICT"),
    ("server_url_password", "skip", "", "server_url is not set"),
    ("server_url_password", "ok", "", "server_url is set and OPENCODE_SERVER_PASSWORD is present"),
    ("server_url_password", "warn", "", "server_url is set (<url>) but OPENCODE_SERVER_PASSWORD is not: the CLI cannot authenticate"),
    ("opencode_password_set", "ok", "", "OPENCODE_PASSWORD is not set"),
    ("opencode_password_set", "warn", "", "OPENCODE_PASSWORD is set: the CLI reads it before OPENCODE_SERVER_PASSWORD"),
    ("log", "skip", "", "no log file in <dir>"),
    ("log", "ok", "", "newest log <path>: no error line"),
    ("log", "warn", "", "newest log <path>: last error: <line>"),
    ("orphaned_controllers", "ok", "", "no pair has a dead controller beside a live TUI"),
    ("orphaned_controllers", "warn", "", "controller dead while the TUI is alive for pair(s): <ids>"),
    ("orphaned_workers", "skip", "", "server_info failed"),
    ("orphaned_workers", "ok", "", "no active session belongs to a pair without a living side"),
    ("orphaned_workers", "warn", "", "active session(s) whose pair has no living side: <ids>"),
    ("orphaned_workers", "warn", "api", "cannot list active sessions: <message>"),
    ("settings_notes", "ok", "", "no settings entry was ignored or clamped"),
    ("settings_notes", "warn", "", "<count> settings entries ignored or clamped: <list>"),
    ("settings_file_hash", "skip", "", "no user-level settings file at <path>"),
    ("settings_file_hash", "ok", "", "<path>: sha256 <hash>, modified <mtime_ms> ms since the epoch"),
    ("settings_file_hash", "warn", "", "cannot read <path>: <message>"),
];

fn message(id: &str, status: Status, variant: &str, subs: &[(&str, &str)]) -> String {
    let status = status.as_str();
    let template = MESSAGES
        .iter()
        .find(|(i, s, v, _)| *i == id && *s == status && *v == variant)
        .unwrap_or_else(|| panic!("no doctor message for ({id}, {status}, {variant})"))
        .3;
    let mut out = template.to_string();
    for (name, value) in subs {
        out = out.replace(&format!("<{name}>"), value);
    }
    out
}

// ------------------------------------------------------------------ small helpers

fn source_name(s: Source) -> &'static str {
    match s {
        Source::Env => "env",
        Source::UserFile => "user file",
        Source::WorkspaceFile => "workspace file",
        Source::Default => "default",
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

fn mtime_ms(meta: &std::fs::Metadata) -> Option<i64> {
    let d = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some(d.as_millis() as i64)
}

/// The registration file's default location: `$XDG_STATE_HOME/opencode/service.json`, else
/// `~/.local/state/opencode/service.json` (packages/util/src/global.ts:79 roots).
pub fn default_registration_file(
    get: &dyn Fn(&str) -> Option<String>,
    user_home: &Path,
) -> PathBuf {
    let base = blank_is_unset(get("XDG_STATE_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| user_home.join(".local").join("state"));
    base.join("opencode").join("service.json")
}

/// The service config's default location: `$OPENCODE_CONFIG_DIR/service.json`
/// (packages/util/src/global.ts:79), else `$XDG_CONFIG_HOME/opencode/`, else `~/.config/opencode/`.
pub fn default_config_file(get: &dyn Fn(&str) -> Option<String>, user_home: &Path) -> PathBuf {
    if let Some(dir) = blank_is_unset(get("OPENCODE_CONFIG_DIR")) {
        return PathBuf::from(dir).join("service.json");
    }
    let base = blank_is_unset(get("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| user_home.join(".config"));
    base.join("opencode").join("service.json")
}

// ------------------------------------------------------------------ launcher link

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    Fresh,
    Stale,
    OtherHost,
    Missing,
    NotALink,
}

impl LinkStatus {
    fn word(self) -> &'static str {
        match self {
            LinkStatus::Fresh => "fresh",
            LinkStatus::Stale => "stale",
            LinkStatus::OtherHost => "other_host",
            LinkStatus::Missing => "missing",
            LinkStatus::NotALink => "not_a_link",
        }
    }
}

const BINARY_NAMES: [&str; 3] = [
    "opencode-ctl-cc-mcp",
    "opencode-ctl-qwen-mcp",
    "opencode-ctl-agy-mcp",
];

fn link_name() -> &'static str {
    if cfg!(windows) {
        "opencode-pair.exe"
    } else {
        "opencode-pair"
    }
}

/// The two lines of `opencode-pair.exe.source`: `path=<staged path>` and `version=<plugin semver>`
/// (spec install-launcher; split on the first `=`, any parse failure counts as absent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub path: String,
    pub version: String,
}

pub fn parse_source_file(raw: &str) -> Option<SourceFile> {
    let (mut path, mut version) = (None, None);
    for line in raw.trim_start_matches('\u{feff}').lines() {
        match line.split_once('=') {
            Some(("path", v)) if !v.is_empty() => path = Some(v.to_string()),
            Some(("version", v)) if !v.is_empty() => version = Some(v.to_string()),
            _ => {}
        }
    }
    Some(SourceFile {
        path: path?,
        version: version?,
    })
}

/// A file name with the `.exe` suffix dropped, from either kind of separator.
fn stem_of(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or("");
    let lower = name.to_ascii_lowercase();
    match lower.strip_suffix(".exe") {
        Some(_) => name[..name.len() - 4].to_string(),
        None => name.to_string(),
    }
}

fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// Which kind of binary a staged or linked file name belongs to, relative to `host`.
fn name_status(stem: &str, host: Host) -> Option<LinkStatus> {
    if stem == host.binary_name() {
        Some(LinkStatus::Stale)
    } else if BINARY_NAMES.contains(&stem) {
        Some(LinkStatus::OtherHost)
    } else {
        None
    }
}

/// The Windows rule on already-read inputs (pure, so it is tested on every platform): `Stale` when
/// the `.source` `path=` differs from the running exe OR its `version=` differs from `version`.
pub fn windows_link_status(
    copy_exists: bool,
    source: Option<&SourceFile>,
    running_exe: &Path,
    host: Host,
    version: &str,
) -> LinkStatus {
    if !copy_exists {
        return LinkStatus::Missing;
    }
    let Some(src) = source else {
        return LinkStatus::NotALink;
    };
    if same_path(&src.path, &running_exe.to_string_lossy()) {
        return if src.version == version {
            LinkStatus::Fresh
        } else {
            LinkStatus::Stale
        };
    }
    name_status(&stem_of(&src.path), host).unwrap_or(LinkStatus::NotALink)
}

/// POSIX: the `opencode-pair` symlink's target; Windows: the copy and its `.source` file.
pub fn launcher_link_status(
    bin_dir: &Path,
    running_exe: &Path,
    host: Host,
    version: &str,
) -> LinkStatus {
    let link = bin_dir.join(link_name());
    #[cfg(windows)]
    {
        let source = std::fs::read_to_string(bin_dir.join("opencode-pair.exe.source"))
            .ok()
            .and_then(|raw| parse_source_file(&raw));
        windows_link_status(link.is_file(), source.as_ref(), running_exe, host, version)
    }
    #[cfg(not(windows))]
    {
        let _ = version;
        let meta = match std::fs::symlink_metadata(&link) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LinkStatus::Missing,
            Err(_) => return LinkStatus::NotALink,
        };
        if !meta.file_type().is_symlink() {
            return LinkStatus::NotALink;
        }
        let Ok(target) = std::fs::read_link(&link) else {
            return LinkStatus::NotALink;
        };
        let target = bin_dir.join(target);
        let resolves = target == running_exe
            || matches!(
                (std::fs::canonicalize(&target), std::fs::canonicalize(running_exe)),
                (Ok(a), Ok(b)) if a == b
            );
        if resolves {
            return LinkStatus::Fresh;
        }
        let stem = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        name_status(&stem, host).unwrap_or(LinkStatus::NotALink)
    }
}

/// `windows_source_old` on a given platform flag (so the Windows rule is tested everywhere):
/// the copy's `.source` file must parse, and no `.old` leftover may remain.
pub fn windows_source_old_check(bin_dir: &Path, windows: bool) -> Check {
    const ID: &str = "windows_source_old";
    if !windows {
        return Check::new(ID, Status::Skip, "off_windows", &[]);
    }
    let copy = bin_dir.join("opencode-pair.exe");
    if !copy.is_file() {
        return Check::new(
            ID,
            Status::Skip,
            "",
            &[("path", &copy.display().to_string())],
        );
    }
    let source = bin_dir.join("opencode-pair.exe.source");
    let old = bin_dir.join("opencode-pair.exe.old");
    let mut problems = Vec::new();
    let parsed = std::fs::read_to_string(&source)
        .ok()
        .and_then(|raw| parse_source_file(&raw));
    if parsed.is_none() {
        problems.push(format!(
            "{} is missing or unparsable (re-run install-launcher)",
            source.display()
        ));
    }
    if old.exists() {
        problems.push(format!(
            "{} is left over from an in-use replacement (gc deletes it)",
            old.display()
        ));
    }
    if problems.is_empty() {
        Check::new(ID, Status::Ok, "", &[])
    } else {
        Check::new(ID, Status::Warn, "", &[("problems", &problems.join("; "))])
    }
}

fn launcher_link_check<T: Transport, C: Clock>(engine: &Engine<T, C>, ctx: &DoctorCtx) -> Check {
    const ID: &str = "launcher_link";
    if !engine.host.offers_launcher() {
        return Check::new(ID, Status::Skip, "", &[]);
    }
    let link = ctx.bin_dir.join(link_name());
    let status = launcher_link_status(ctx.bin_dir, ctx.running_exe, engine.host, ctx.version);
    let l = link.display().to_string();
    let subs = [("link", l.as_str()), ("version", ctx.version)];
    let (st, variant) = match status {
        LinkStatus::Fresh => (Status::Ok, ""),
        LinkStatus::Missing => (Status::Warn, "missing"),
        LinkStatus::Stale => (Status::Warn, "stale"),
        LinkStatus::OtherHost => (Status::Warn, "other_host"),
        LinkStatus::NotALink => (Status::Warn, "not_a_link"),
    };
    Check::new(ID, st, variant, &subs).details(json!({"link": status.word(), "path": l}))
}

// ------------------------------------------------------------------ pair files (file-name ids)

fn side_words(side: Side, l: Liveness) -> String {
    let why = match (l, side) {
        (Liveness::Alive, Side::Tui) => "the tab is running",
        (Liveness::Alive, Side::Controller) => "the controller is running",
        (Liveness::Dead, _) => "the process is gone",
        (Liveness::Starting, _) => "launched less than 30 s ago, no sentinel yet",
        (Liveness::Unknown, _) => "the sentinel cannot be verified",
        (Liveness::None, Side::Tui) => "no tab was requested",
        (Liveness::None, Side::Controller) => "no controller was recorded",
    };
    format!("{} ({why})", l.as_str())
}

// ------------------------------------------------------------------ the checks

struct ServerState {
    ok: bool,
    version: Option<String>,
}

fn ctl_home_override(ctx: &DoctorCtx) -> Check {
    match blank_is_unset((ctx.env)("OPENCODE_CTL_HOME")) {
        Some(p) => Check::new("ctl_home_override", Status::Warn, "", &[("path", &p)]),
        None => Check::new("ctl_home_override", Status::Ok, "", &[]),
    }
}

fn binary_check<T: Transport, C: Clock>(engine: &Engine<T, C>, ctx: &DoctorCtx) -> Check {
    let explicit = engine.settings.opencode_bin.as_deref();
    match resolve_binary(explicit, ctx.path.as_deref(), ctx.user_home) {
        Ok(p) if explicit.is_some() && !p.is_file() => Check::new(
            "binary",
            Status::Fail,
            "missing_explicit",
            &[("path", &p.display().to_string())],
        ),
        Ok(p) => Check::new(
            "binary",
            Status::Ok,
            "",
            &[("path", &p.display().to_string())],
        ),
        Err(_) => Check::new("binary", Status::Fail, "", &[]),
    }
}

fn registration_check<T: Transport, C: Clock>(engine: &Engine<T, C>, ctx: &DoctorCtx) -> Check {
    const ID: &str = "registration";
    // A disabled service config selects a private standalone server for every call (server-connection.ts:41),
    // so it outranks an absent, dead or live registration.
    let disabled = ctx
        .config_file
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .is_some_and(|v| v.get("disabled") == Some(&Value::Bool(true)));
    if disabled {
        let cfg = ctx.config_file.map(|f| f.display().to_string());
        return Check::new(
            ID,
            Status::Warn,
            "disabled",
            &[("config_file", cfg.as_deref().unwrap_or(""))],
        );
    }
    let Some(file) = ctx.registration_file else {
        return Check::new(ID, Status::Skip, "no_path", &[]);
    };
    let shown = file.display().to_string();
    let Ok(raw) = std::fs::read_to_string(file) else {
        return if file.exists() {
            Check::new(ID, Status::Warn, "unreadable", &[("path", &shown)])
        } else {
            Check::new(ID, Status::Skip, "", &[("path", &shown)])
        };
    };
    let parsed: Option<(String, u64)> = serde_json::from_str::<Value>(&raw).ok().and_then(|v| {
        let url = v.get("url")?.as_str()?.to_string();
        let pid = v.get("pid")?.as_u64().filter(|p| *p > 0)?;
        Some((url, pid))
    });
    let Some((url, pid)) = parsed else {
        return Check::new(ID, Status::Warn, "unreadable", &[("path", &shown)]);
    };
    let pid_s = pid.to_string();
    let alive = u32::try_from(pid).is_ok_and(|p| engine.probe.is_alive(p));
    if !alive {
        return Check::new(ID, Status::Fail, "", &[("pid", &pid_s)])
            .details(json!({"url": redact_text(&url)}));
    }
    Check::new(ID, Status::Ok, "", &[("pid", &pid_s)])
}

async fn server_info_check<T: Transport, C: Clock>(engine: &Engine<T, C>) -> (Check, ServerState) {
    const ID: &str = "server_info";
    let down = |c: Check| {
        (
            c,
            ServerState {
                ok: false,
                version: None,
            },
        )
    };
    let other = |e: &ToolError| {
        Check::new(
            ID,
            Status::Fail,
            "fail_other",
            &[("code", e.code.as_str()), ("message", &e.message)],
        )
    };
    let managed = engine.settings.server_url.is_none();
    let mut url = engine.settings.server_url.clone().unwrap_or_default();
    let mut started = false;
    if managed {
        match engine.lifecycle(Lifecycle::Status).await {
            Err(e) => return down(other(&e)),
            Ok(out) if service_running(&out) => url = out.trim().to_string(),
            Ok(_) => match engine.lifecycle(Lifecycle::Start).await {
                Err(e) => {
                    return down(Check::new(
                        ID,
                        Status::Fail,
                        "fail_start",
                        &[("stderr", first_line(&e.message))],
                    ))
                }
                Ok(out) => {
                    started = true;
                    url = out.trim().to_string();
                }
            },
        }
    }
    match engine.call_json("server.info", Vec::new(), None).await {
        Ok((info, _, _)) => {
            engine.invalidate_if_changed(&info).await;
            let version = info
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string);
            let pid = info.get("pid").and_then(Value::as_u64);
            let pid_s = pid.map_or("unknown".to_string(), |p| p.to_string());
            let details = json!({"url": url, "version": version, "pid": pid});
            let check = if started {
                Check::new(ID, Status::Ok, "ok_started", &[]).details(
                    json!({"url": url, "version": version, "pid": pid, "started_by_doctor": true}),
                )
            } else {
                Check::new(
                    ID,
                    Status::Ok,
                    "",
                    &[
                        ("version", version.as_deref().unwrap_or("unknown")),
                        ("pid", &pid_s),
                    ],
                )
                .details(details)
            };
            (check, ServerState { ok: true, version })
        }
        Err(e) => down(match e.code {
            ErrorCode::ServiceDown => Check::new(
                ID,
                Status::Fail,
                "fail_unreachable",
                &[("url", &url), ("message", &e.message)],
            ),
            ErrorCode::AuthFailed => Check::new(ID, Status::Fail, "fail_auth", &[("url", &url)]),
            _ => other(&e),
        }),
    }
}

async fn version_range_check<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    binary: &Check,
    server: &ServerState,
) -> Check {
    const ID: &str = "version_range";
    if binary.status != Status::Ok {
        return Check::new(ID, Status::Skip, "", &[]);
    }
    let raw = match engine.cli_version().await {
        Ok(raw) => raw,
        Err(e) => return Check::new(ID, Status::Fail, "cli_error", &[("message", &e.message)]),
    };
    let major = parse_cli_version(&raw).and_then(|v| {
        let digits: String = v.chars().take_while(char::is_ascii_digit).collect();
        digits.parse::<u64>().ok().map(|m| (v, m))
    });
    let Some((cli, major)) = major else {
        return Check::new(ID, Status::Fail, "unparsable", &[]);
    };
    if major < 2 {
        return Check::new(ID, Status::Fail, "below", &[("cli", &cli)]);
    }
    match server.version.as_deref().filter(|_| server.ok) {
        None => Check::new(ID, Status::Ok, "cli_only", &[("cli", &cli)]),
        Some(sv) => {
            let bare = sv.strip_prefix('v').unwrap_or(sv);
            if bare == "2.0" || bare.starts_with("2.0.") {
                Check::new(ID, Status::Ok, "", &[("cli", &cli), ("server", sv)])
            } else {
                Check::new(ID, Status::Warn, "server_outside_range", &[("server", sv)])
            }
        }
    }
}

async fn wait_route_check<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    server: &ServerState,
) -> Check {
    const ID: &str = "wait_route";
    if !server.ok {
        return Check::new(ID, Status::Skip, "", &[]);
    }
    // After a successful `server.info` the live table is cached, so this does not fetch again.
    let (table, source, _) = engine.operation_table().await;
    if source != TableSource::Live {
        Check::new(ID, Status::Warn, "unconfirmed", &[])
    } else if table.get("experimental.session.wait").is_some() {
        Check::new(ID, Status::Ok, "", &[])
    } else {
        Check::new(ID, Status::Warn, "missing", &[])
    }
}

async fn pairing_check<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    server: &ServerState,
) -> Check {
    const ID: &str = "pairing";
    let Some(pair) = engine.env.pair.as_deref() else {
        return Check::new(ID, Status::Skip, "", &[]);
    };
    let home = &engine.env.home;
    let record = match read_record(home, pair) {
        Ok(Some(r)) => r,
        Ok(None) => return Check::new(ID, Status::Fail, "record", &[("pair", pair)]),
        Err(e) => {
            return Check::new(
                ID,
                Status::Fail,
                "unreadable",
                &[("pair", pair), ("error", &e.to_string())],
            )
        }
    };
    let now = engine.clock().now_ms();
    let tui = side_liveness(Side::Tui, &record, home, &*engine.probe, now);
    let controller = side_liveness(Side::Controller, &record, home, &*engine.probe, now);
    let sentinel = if !record.tab {
        "not applicable (no tab)"
    } else if home
        .join(format!("pair.{pair}.g{}.tui-pid", record.generation))
        .is_file()
    {
        "present"
    } else {
        "missing"
    };
    let session_id = engine
        .env
        .session
        .clone()
        .unwrap_or_else(|| record.session_id.clone());
    let (state, state_key) = if !server.ok {
        ("skipped".to_string(), "skipped")
    } else {
        match engine
            .call_json(
                "session.get",
                vec![("sessionID".to_string(), session_id.clone())],
                None,
            )
            .await
        {
            Ok(_) => ("found".to_string(), "found"),
            Err(e) if e.code == ErrorCode::SessionNotFound => {
                ("not found".to_string(), "not_found")
            }
            Err(e) => (format!("lookup failed ({})", e.code.as_str()), "error"),
        }
    };
    let faulty = |l: Liveness| matches!(l, Liveness::Dead | Liveness::Unknown);
    let (status, variant) = if state_key == "not_found" {
        (Status::Fail, "session")
    } else if faulty(tui) || faulty(controller) || sentinel == "missing" || state_key == "error" {
        (Status::Warn, "")
    } else {
        (Status::Ok, "")
    };
    let (tui_s, ctl_s) = (
        side_words(Side::Tui, tui),
        side_words(Side::Controller, controller),
    );
    let generation = record.generation.to_string();
    Check::new(
        ID,
        status,
        variant,
        &[
            ("pair", pair),
            ("tui", &tui_s),
            ("controller", &ctl_s),
            ("generation", &generation),
            ("sentinel", sentinel),
            ("session", &session_id),
            ("session_state", &state),
        ],
    )
    .details(json!({
        "pair_id": pair,
        "session_id": session_id,
        "tui": tui.as_str(),
        "controller": controller.as_str(),
        "generation": record.generation,
        "sentinel": sentinel,
        "session": state_key,
    }))
}

fn lock_source_check<T: Transport, C: Clock>(engine: &Engine<T, C>) -> Check {
    let s = &engine.settings;
    let src = |k: &str| source_name(s.sources.get(k).copied().unwrap_or(Source::Default));
    Check::new(
        "lock_source",
        Status::Ok,
        "",
        &[
            ("locked", &s.permissions_locked.to_string()),
            ("locked_source", src("permissions_locked")),
            ("policy", s.permission_policy.as_str()),
            ("policy_source", src("permission_policy")),
        ],
    )
}

/// The protected keys (spec Settings), in the order the doctor lists them.
const PROTECTED: [&str; 7] = [
    "opencode_bin",
    "server_url",
    "default_model",
    "default_agent",
    "permission_policy",
    "permissions_locked",
    "wait_max_s",
];

fn env_vs_user_file<T: Transport, C: Clock>(engine: &Engine<T, C>, ctx: &DoctorCtx) -> Check {
    const ID: &str = "env_vs_user_file";
    let file = std::fs::read_to_string(engine.env.home.join("settings.local.md"))
        .map(|raw| settings::parse_frontmatter(&raw))
        .unwrap_or_default();
    let mut found = Vec::new();
    for key in PROTECTED {
        let name = settings::env_name(key);
        // `OPENCODE_CTL_SERVER` is read raw: an empty value is the background-service override.
        let env = if key == "server_url" {
            (ctx.env)(&name)
        } else {
            blank_is_unset((ctx.env)(&name))
        };
        let from_file = file
            .iter()
            .find(|(k, v)| k == key && !v.trim().is_empty())
            .map(|(_, v)| v.as_str());
        if let (Some(e), Some(f)) = (env, from_file) {
            if e.trim() != f.trim() {
                found.push((key, redact_text(&e), redact_text(f)));
            }
        }
    }
    if found.is_empty() {
        return Check::new(ID, Status::Ok, "", &[]);
    }
    let list: Vec<String> = found
        .iter()
        .map(|(k, e, f)| format!("{k} (env \"{e}\", file \"{f}\")"))
        .collect();
    Check::new(ID, Status::Warn, "", &[("keys", &list.join(", "))]).details(json!({
        "keys": found.iter().map(|(k, e, f)| json!({"key": k, "env": e, "file": f})).collect::<Vec<_>>(),
    }))
}

fn server_url_password<T: Transport, C: Clock>(engine: &Engine<T, C>, ctx: &DoctorCtx) -> Check {
    const ID: &str = "server_url_password";
    let Some(url) = engine.settings.server_url.as_deref() else {
        return Check::new(ID, Status::Skip, "", &[]);
    };
    if blank_is_unset((ctx.env)("OPENCODE_SERVER_PASSWORD")).is_some() {
        Check::new(ID, Status::Ok, "", &[])
    } else {
        Check::new(ID, Status::Warn, "", &[("url", &redact_text(url))])
    }
}

fn opencode_password_set(ctx: &DoctorCtx) -> Check {
    if blank_is_unset((ctx.env)("OPENCODE_PASSWORD")).is_some() {
        Check::new("opencode_password_set", Status::Warn, "", &[])
    } else {
        Check::new("opencode_password_set", Status::Ok, "", &[])
    }
}

fn log_check<T: Transport, C: Clock>(engine: &Engine<T, C>) -> Check {
    const ID: &str = "log";
    let dir = engine.env.home.join("logs");
    let newest = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.ends_with(".log") && (n.starts_with("serve-") || n.starts_with("pair-"))
        })
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((mtime_ms(&m)?, e.file_name(), e.path()))
        })
        .max_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let Some((_, _, path)) = newest else {
        return Check::new(ID, Status::Skip, "", &[("dir", &dir.display().to_string())]);
    };
    let shown = path.display().to_string();
    let text = std::fs::read(&path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let last = text
        .lines()
        .rev()
        .find(|l| l.split_whitespace().any(|w| w == "ERROR"));
    match last {
        None => Check::new(ID, Status::Ok, "", &[("path", &shown)]),
        Some(line) => {
            let line: String = redact_text(line.trim()).chars().take(300).collect();
            Check::new(ID, Status::Warn, "", &[("path", &shown), ("line", &line)])
                .details(json!({"path": shown, "last_error": line}))
        }
    }
}

fn orphaned_controllers<T: Transport, C: Clock>(engine: &Engine<T, C>) -> Check {
    let home = &engine.env.home;
    let now = engine.clock().now_ms();
    let ids: Vec<String> = list_records(home)
        .into_iter()
        .filter(|r| {
            side_liveness(Side::Controller, r, home, &*engine.probe, now) == Liveness::Dead
                && side_liveness(Side::Tui, r, home, &*engine.probe, now) == Liveness::Alive
        })
        .map(|r| r.pair_id)
        .collect();
    if ids.is_empty() {
        Check::new("orphaned_controllers", Status::Ok, "", &[])
    } else {
        Check::new(
            "orphaned_controllers",
            Status::Warn,
            "",
            &[("ids", &ids.join(", "))],
        )
        .details(json!({"pairs": ids}))
    }
}

async fn orphaned_workers<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    server: &ServerState,
) -> Check {
    const ID: &str = "orphaned_workers";
    if !server.ok {
        return Check::new(ID, Status::Skip, "", &[]);
    }
    let active = match engine.call_json("session.active", Vec::new(), None).await {
        Ok((data, _, _)) => data,
        Err(e) => return Check::new(ID, Status::Warn, "api", &[("message", &e.message)]),
    };
    let home = &engine.env.home;
    let now = engine.clock().now_ms();
    let records = list_records(home);
    let living = |l: Liveness| matches!(l, Liveness::Alive | Liveness::Starting);
    let mut ids: Vec<String> = active
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    ids.sort();
    ids.retain(|id| {
        let mine: Vec<&Record> = records.iter().filter(|r| &r.session_id == id).collect();
        !mine.is_empty()
            && !mine.iter().any(|r| {
                living(side_liveness(Side::Tui, r, home, &*engine.probe, now))
                    || living(side_liveness(
                        Side::Controller,
                        r,
                        home,
                        &*engine.probe,
                        now,
                    ))
            })
    });
    if ids.is_empty() {
        Check::new(ID, Status::Ok, "", &[])
    } else {
        Check::new(ID, Status::Warn, "", &[("ids", &ids.join(", "))])
            .details(json!({"sessions": ids}))
    }
}

fn reason_words(r: &NoteReason) -> String {
    match r {
        NoteReason::UnknownKey => "unknown key".to_string(),
        NoteReason::InvalidValue => "invalid value".to_string(),
        NoteReason::Clamped { min, max } => format!("clamped to {min}..{max}"),
        NoteReason::ProtectedInWorkspace => "protected key in a workspace file".to_string(),
        NoteReason::OverriddenByStickyLock => "overridden by the permission lock".to_string(),
        NoteReason::UserinfoInUrl => "userinfo in the URL".to_string(),
    }
}

fn note_json(n: &Note) -> Value {
    json!({
        "source": source_name(n.source),
        "key": n.key,
        "value": redact_text(&n.value),
        "reason": reason_words(&n.reason),
    })
}

fn settings_notes<T: Transport, C: Clock>(engine: &Engine<T, C>) -> Check {
    if engine.notes.is_empty() {
        return Check::new("settings_notes", Status::Ok, "", &[]);
    }
    let list: Vec<String> = engine
        .notes
        .iter()
        .map(|n| {
            format!(
                "{} {}={} ({})",
                source_name(n.source),
                n.key,
                redact_text(&n.value),
                reason_words(&n.reason)
            )
        })
        .collect();
    Check::new(
        "settings_notes",
        Status::Warn,
        "",
        &[
            ("count", &engine.notes.len().to_string()),
            ("list", &list.join("; ")),
        ],
    )
}

fn settings_file_hash<T: Transport, C: Clock>(engine: &Engine<T, C>) -> Check {
    const ID: &str = "settings_file_hash";
    let file = engine.env.home.join("settings.local.md");
    let shown = file.display().to_string();
    match std::fs::read(&file) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Check::new(ID, Status::Skip, "", &[("path", &shown)])
        }
        Err(e) => Check::new(
            ID,
            Status::Warn,
            "",
            &[("path", &shown), ("message", &e.to_string())],
        ),
        Ok(bytes) => {
            let hash = hex(&Sha256::digest(&bytes));
            let mtime = std::fs::metadata(&file).ok().and_then(|m| mtime_ms(&m));
            let mtime_s = mtime.map_or("unknown".to_string(), |m| m.to_string());
            Check::new(
                ID,
                Status::Ok,
                "",
                &[("path", &shown), ("hash", &hash), ("mtime_ms", &mtime_s)],
            )
            .details(json!({"path": shown, "sha256": hash, "mtime_ms": mtime}))
        }
    }
}

fn resolved_settings<T: Transport, C: Clock>(engine: &Engine<T, C>) -> Value {
    let s = &engine.settings;
    let mut sources = Map::new();
    for (k, v) in &s.sources {
        sources.insert((*k).to_string(), json!(source_name(*v)));
    }
    json!({
        "wait_timeout_s": s.wait_timeout_s,
        "poll_s": s.poll_s,
        "stall_s": s.stall_s,
        "max_turn_s": s.max_turn_s,
        "call_timeout_s": s.call_timeout_s,
        "output_max_bytes": s.output_max_bytes,
        "body_max_bytes": s.body_max_bytes,
        "wait_max_s": s.wait_max_s,
        "opencode_bin": s.opencode_bin.as_ref().map(|p| p.display().to_string()),
        "server_url": s.server_url.as_deref().map(redact_text),
        "default_model": s.default_model,
        "default_agent": s.default_agent,
        "permission_policy": s.permission_policy.as_str(),
        "permissions_locked": s.permissions_locked,
        "lock_source": s.lock_source.map(source_name),
        "sources": sources,
    })
}

/// The doctor report: `{"checks":[...], "notes":[...], "settings"?}`.
pub async fn doctor_json<T: Transport, C: Clock>(
    engine: &Engine<T, C>,
    ctx: &DoctorCtx<'_>,
    verbose: bool,
) -> Value {
    // COMPUTE order: ctl_home_override, binary, registration (before anything can start the
    // service), server_info, then the rest in id order.
    let mut checks = vec![ctl_home_override(ctx)];
    let binary = binary_check(engine, ctx);
    let registration = registration_check(engine, ctx);
    let (server_check, server) = server_info_check(engine).await;
    checks.push(binary.clone());
    checks.push(registration);
    checks.push(server_check);
    checks.push(version_range_check(engine, &binary, &server).await);
    checks.push(wait_route_check(engine, &server).await);
    checks.push(pairing_check(engine, &server).await);
    checks.push(lock_source_check(engine));
    checks.push(env_vs_user_file(engine, ctx));
    checks.push(windows_source_old_check(ctx.bin_dir, cfg!(windows)));
    checks.push(launcher_link_check(engine, ctx));
    checks.push(server_url_password(engine, ctx));
    checks.push(opencode_password_set(ctx));
    checks.push(log_check(engine));
    checks.push(orphaned_controllers(engine));
    checks.push(orphaned_workers(engine, &server).await);
    checks.push(settings_notes(engine));
    checks.push(settings_file_hash(engine));
    // EMIT order: id order, so version_range could read server_info's response.
    checks.sort_by_key(|c| CHECK_IDS.iter().position(|i| *i == c.id));
    let mut out = json!({
        "checks": checks.iter().map(Check::to_json).collect::<Vec<_>>(),
        "notes": engine.notes.iter().map(note_json).collect::<Vec<_>>(),
    });
    if verbose {
        out["settings"] = resolved_settings(engine);
    }
    out
}
