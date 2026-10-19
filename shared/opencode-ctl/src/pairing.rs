//! The pairing contract on disk: identity resolution, the pair record, the `claim_file` primitive
//! and the liveness sentinels (spec "Pairing contract"; files are `pair.<pair-id>.*` in the ctl home).

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::engine::{valid_session_id, EnvSnapshot};
use crate::error::{ErrorCode, ToolError};
use crate::liveness::Sentinel;
use crate::settings::{Settings, Source};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    Env,
    File,
    None,
}

/// Who this server is paired with. `source` is `File` only when a record was read and parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub session_id: Option<String>,
    pub directory: Option<PathBuf>,
    pub server_url: Option<String>,
    pub pair_id: Option<String>,
    pub source: IdentitySource,
}

impl Identity {
    fn unpaired() -> Self {
        Identity {
            session_id: None,
            directory: None,
            server_url: None,
            pair_id: None,
            source: IdentitySource::None,
        }
    }
}

/// `pair.<id>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub pair_id: String,
    pub session_id: String,
    pub directory: PathBuf,
    #[serde(default)]
    pub server_url: Option<String>,
    pub created: i64,
    #[serde(default)]
    pub controller: Option<String>,
    pub generation: u32,
    pub tab: bool,
}

/// `^[0-9a-f]{12}$`.
pub fn valid_pair_id(id: &str) -> bool {
    id.len() == 12 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn invalid_pair(id: &str) -> ToolError {
    ToolError::new(
        ErrorCode::InvalidArgument,
        format!("'{}' is not a pair id", crate::compact::redact_text(id)),
    )
    .hint("pair ids match ^[0-9a-f]{12}$")
}

/// Env vars first; only `OPENCODE_CTL_PAIR` set means the record supplies the rest. Returns the
/// identity and the startup warnings (logged by the caller). An invalid id leaves the server unpaired.
pub fn resolve_identity(env: &EnvSnapshot, home: &Path) -> (Identity, Vec<ToolError>) {
    let mut warnings = Vec::new();
    if let Some(p) = env.pair.as_deref() {
        if !valid_pair_id(p) {
            warnings.push(
                invalid_pair(p).hint("OPENCODE_CTL_PAIR is ignored; this server runs unpaired"),
            );
        }
    }
    if let Some(s) = env.session.as_deref() {
        if !valid_session_id(s) {
            warnings.push(
                ToolError::new(
                    ErrorCode::InvalidArgument,
                    format!(
                        "OPENCODE_CTL_SESSION '{}' is not a session id",
                        crate::compact::redact_text(s)
                    ),
                )
                .hint("session ids match ^ses_[A-Za-z0-9]{26}$; this server runs unpaired"),
            );
        }
    }
    if !warnings.is_empty() {
        return (Identity::unpaired(), warnings);
    }
    if env.session.is_some() {
        return (
            Identity {
                session_id: env.session.clone(),
                directory: env.directory.clone(),
                server_url: env.server.clone(),
                pair_id: env.pair.clone(),
                source: IdentitySource::Env,
            },
            warnings,
        );
    }
    let Some(pair) = env.pair.clone() else {
        let any = env.directory.is_some() || env.server.is_some();
        return (
            Identity {
                session_id: None,
                directory: env.directory.clone(),
                server_url: env.server.clone(),
                pair_id: None,
                source: if any {
                    IdentitySource::Env
                } else {
                    IdentitySource::None
                },
            },
            warnings,
        );
    };
    let fallback = Identity {
        pair_id: Some(pair.clone()),
        ..Identity::unpaired()
    };
    match read_record(home, &pair) {
        Ok(Some(r)) if valid_session_id(&r.session_id) => (
            Identity {
                session_id: Some(r.session_id),
                directory: env.directory.clone().or(Some(r.directory)),
                server_url: env.server.clone().or(r.server_url),
                pair_id: Some(pair),
                source: IdentitySource::File,
            },
            warnings,
        ),
        Ok(_) => {
            warnings.push(
                ToolError::new(
                    ErrorCode::PairNotFound,
                    format!("no usable pair record for {pair}: running with the pair id only"),
                )
                .hint("restart the controller with `opencode-pair resume <pair-id>`"),
            );
            (fallback, warnings)
        }
        Err(e) => {
            warnings.push(e);
            (fallback, warnings)
        }
    }
}

fn has_userinfo(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split('/').next().is_some_and(|a| a.contains('@'))
}

/// Write the resolved identity into the environment snapshot and, for a `File` identity, the
/// record's `server_url` into the settings: `settings.server_url = identity.server_url` (a null
/// record means the background service, exactly as an empty `OPENCODE_CTL_SERVER` does) with the
/// source `Env`, overriding a user-file value. `Env` and `None` identities (a missing or
/// unparsable record yields `None`) leave the settings untouched. A record URL carrying userinfo
/// is ignored: the settings keep their current value and source, and `env.server` is cleared.
pub fn apply_identity(env: &mut EnvSnapshot, settings: &mut Settings, id: &Identity) {
    env.session = id.session_id.clone();
    env.directory = id.directory.clone();
    env.server = id.server_url.clone();
    env.pair = id.pair_id.clone();
    if id.source != IdentitySource::File {
        return;
    }
    if id.server_url.as_deref().is_some_and(has_userinfo) {
        tracing::warn!("the pair record's server_url carries userinfo and is ignored");
        env.server = None;
        return;
    }
    settings.server_url = id.server_url.clone();
    settings.sources.insert("server_url", Source::Env);
}

// ------------------------------------------------------------------ files

pub fn record_path(home: &Path, id: &str) -> PathBuf {
    home.join(format!("pair.{id}.json"))
}

fn lock_path(home: &Path, id: &str) -> PathBuf {
    home.join(format!("pair.{id}.lock"))
}

fn ensure_dir(dir: &Path) -> Result<(), ToolError> {
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

fn open_new(path: &Path, mode: u32) -> std::io::Result<File> {
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    o.open(path)
}

fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".tmp-{}", uuid::Uuid::new_v4()));
    path.with_file_name(name)
}

fn write_temp(path: &Path, content: &[u8], mode: u32) -> Result<PathBuf, ToolError> {
    let tmp = temp_sibling(path);
    let mut f = open_new(&tmp, mode)?;
    if let Err(e) = f.write_all(content).and_then(|()| f.flush()) {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(tmp)
}

/// Create `path` with `content` only if it does not exist, complete from the first moment it is
/// visible (temp + `hard_link`). `Ok(false)` when `path` already exists.
pub fn claim_file(path: &Path, content: &[u8], mode: u32) -> Result<bool, ToolError> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        ensure_dir(dir)?;
    }
    let tmp = write_temp(path, content, mode)?;
    let linked = std::fs::hard_link(&tmp, path);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(false),
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::Unsupported | ErrorKind::PermissionDenied | ErrorKind::CrossesDevices
            ) =>
        {
            static LOGGED: OnceLock<()> = OnceLock::new();
            LOGGED.get_or_init(|| tracing::warn!("claim fallback: {:?}", e.kind()));
            match open_new(path, mode) {
                Ok(mut f) => {
                    f.write_all(content).and_then(|()| f.flush())?;
                    Ok(true)
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(false),
                Err(e) => Err(e.into()),
            }
        }
        Err(e) => Err(e.into()),
    }
}

/// Temp + rename over `path`.
fn replace_file(path: &Path, content: &[u8], mode: u32) -> Result<(), ToolError> {
    let tmp = write_temp(path, content, mode)?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        ToolError::from(e)
    })
}

pub fn write_record_new(home: &Path, record: &Record) -> Result<(), ToolError> {
    if !valid_pair_id(&record.pair_id) {
        return Err(invalid_pair(&record.pair_id));
    }
    let body = serde_json::to_vec_pretty(record)
        .map_err(|e| ToolError::new(ErrorCode::IoError, e.to_string()))?;
    if claim_file(&record_path(home, &record.pair_id), &body, 0o600)? {
        Ok(())
    } else {
        Err(ToolError::new(
            ErrorCode::InvalidArgument,
            format!("pair record {} already exists", record.pair_id),
        ))
    }
}

fn parse_record(path: &Path, raw: &str) -> Option<Record> {
    match serde_json::from_str::<Record>(raw) {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::warn!("unparsable pair record {}: {e}", path.display());
            None
        }
    }
}

/// `Ok(None)` for a missing or unparsable record (the latter logged); `IO_ERROR` for other read errors.
pub fn read_record(home: &Path, id: &str) -> Result<Option<Record>, ToolError> {
    if !valid_pair_id(id) {
        return Err(invalid_pair(id));
    }
    let path = record_path(home, id);
    match std::fs::read_to_string(&path) {
        Ok(raw) => Ok(parse_record(&path, &raw)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ToolError::new(
            ErrorCode::IoError,
            format!("pair record unreadable: {e}"),
        )),
    }
}

/// Every parsable `pair.<id>.json`; an unparsable one is logged and skipped.
pub fn list_records(home: &Path) -> Vec<Record> {
    let Ok(entries) = std::fs::read_dir(home) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(id) = name
            .to_str()
            .and_then(|n| n.strip_prefix("pair."))
            .and_then(|n| n.strip_suffix(".json"))
        else {
            continue;
        };
        if !valid_pair_id(id) {
            continue;
        }
        let path = entry.path();
        if let Ok(raw) = std::fs::read_to_string(&path) {
            out.extend(parse_record(&path, &raw));
        }
    }
    out
}

/// Rewrite only `generation` and `tab`, under the pair's lock, never lowering the generation.
pub fn rewrite_record(
    home: &Path,
    id: &str,
    generation: u32,
    tab: bool,
    after_lock: Option<&dyn Fn()>,
) -> Result<(), ToolError> {
    if !valid_pair_id(id) {
        return Err(invalid_pair(id));
    }
    ensure_dir(home)?;
    let mut o = OpenOptions::new();
    o.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let lock = o.open(lock_path(home, id))?;
    lock.lock()?;
    if let Some(f) = after_lock {
        f();
    }
    let mut record = read_record(home, id)?.ok_or_else(|| {
        ToolError::new(ErrorCode::PairNotFound, format!("no pair record for {id}"))
    })?;
    if generation < record.generation {
        tracing::warn!(
            "rewrite_record {id}: generation {generation} is below the stored {}; ignored",
            record.generation
        );
        return Ok(());
    }
    record.generation = generation;
    record.tab = tab;
    let body = serde_json::to_vec_pretty(&record)
        .map_err(|e| ToolError::new(ErrorCode::IoError, e.to_string()))?;
    replace_file(&record_path(home, id), &body, 0o600)
    // `lock` is released on drop
}

/// Remove every file of one pair (`pair.<id>.*`, including temp files and the lock).
pub fn delete_pair_files(home: &Path, id: &str) -> Result<(), ToolError> {
    if !valid_pair_id(id) {
        return Err(invalid_pair(id));
    }
    let prefix = format!("pair.{id}.");
    let Ok(entries) = std::fs::read_dir(home) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            match std::fs::remove_file(entry.path()) {
                Ok(()) => {}
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
    Ok(())
}

pub fn write_sentinel(path: &Path, s: &Sentinel) -> Result<(), ToolError> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        ensure_dir(dir)?;
    }
    replace_file(path, s.to_string().as_bytes(), 0o600)
}

/// An absent, empty or unparsable file reads as `None`.
pub fn read_sentinel(path: &Path) -> Option<Sentinel> {
    Sentinel::parse(&std::fs::read_to_string(path).ok()?)
}
