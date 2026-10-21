//! Settings: env, user file, workspace file, defaults, with clamps, the
//! protected-key rule and the sticky permission lock.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::host::Host;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPolicy {
    Inherit,
    Allow,
    Ask,
    Deny,
}

impl PermissionPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            PermissionPolicy::Inherit => "inherit",
            PermissionPolicy::Allow => "allow",
            PermissionPolicy::Ask => "ask",
            PermissionPolicy::Deny => "deny",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "inherit" => Some(PermissionPolicy::Inherit),
            "allow" => Some(PermissionPolicy::Allow),
            "ask" => Some(PermissionPolicy::Ask),
            "deny" => Some(PermissionPolicy::Deny),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    UserFile,
    WorkspaceFile,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub wait_timeout_s: u64,
    pub poll_s: u64,
    pub stall_s: u64,
    pub max_turn_s: u64,
    pub call_timeout_s: u64,
    pub output_max_bytes: usize,
    pub body_max_bytes: usize,
    pub wait_max_s: u64,
    pub opencode_bin: Option<PathBuf>,
    pub server_url: Option<String>,
    pub default_model: Option<String>,
    pub default_agent: Option<String>,
    pub permission_policy: PermissionPolicy,
    pub permissions_locked: bool,
    pub lock_source: Option<Source>,
    pub sources: BTreeMap<&'static str, Source>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            wait_timeout_s: 120,
            poll_s: 10,
            stall_s: 90,
            max_turn_s: 1800,
            call_timeout_s: 30,
            output_max_bytes: 16384,
            body_max_bytes: if cfg!(windows) { 24000 } else { 100000 },
            wait_max_s: 300,
            opencode_bin: None,
            server_url: None,
            default_model: None,
            default_agent: None,
            permission_policy: PermissionPolicy::Inherit,
            permissions_locked: false,
            lock_source: None,
            sources: KEYS.iter().map(|k| (*k, Source::Default)).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteReason {
    UnknownKey,
    InvalidValue,
    Clamped { min: i64, max: i64 },
    ProtectedInWorkspace,
    OverriddenByStickyLock,
    UserinfoInUrl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub source: Source,
    pub key: String,
    pub value: String,
    pub reason: NoteReason,
}

pub struct Input<'a> {
    pub host: Host,
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub user_file: Option<&'a str>,
    pub workspace_file: Option<&'a str>,
}

/// Every settings key. `wait_max_s` comes first: `wait_timeout_s` clamps to it.
const KEYS: [&str; 14] = [
    "wait_max_s",
    "wait_timeout_s",
    "poll_s",
    "stall_s",
    "max_turn_s",
    "call_timeout_s",
    "output_max_bytes",
    "body_max_bytes",
    "opencode_bin",
    "server_url",
    "default_model",
    "default_agent",
    "permission_policy",
    "permissions_locked",
];

fn is_protected(key: &str) -> bool {
    matches!(
        key,
        "opencode_bin"
            | "server_url"
            | "default_model"
            | "default_agent"
            | "permission_policy"
            | "permissions_locked"
            | "wait_max_s"
    )
}

pub fn env_name(key: &str) -> String {
    match key {
        "opencode_bin" => "OPENCODE_CTL_BIN".to_string(),
        "server_url" => "OPENCODE_CTL_SERVER".to_string(),
        other => format!("OPENCODE_CTL_{}", other.to_ascii_uppercase()),
    }
}

fn blank(v: &str) -> bool {
    v.trim().is_empty()
}

/// `---` fenced `key: value` lines; `#` comments, CRLF and quotes tolerated.
pub fn parse_frontmatter(source: &str) -> Vec<(String, String)> {
    let mut lines = source.trim_start_matches('\u{feff}').lines();
    let mut out = Vec::new();
    match lines.next() {
        Some(first) if first.trim_end() == "---" => {}
        _ => return out,
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            break;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        let value = ['"', '\'']
            .iter()
            .find_map(|q| value.strip_prefix(*q).and_then(|v| v.strip_suffix(*q)))
            .unwrap_or(value);
        out.push((key.trim().to_string(), value.to_string()));
    }
    out
}

pub fn ctl_home(env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    let nonblank = |k: &str| env(k).filter(|v| !blank(v));
    if let Some(h) = nonblank("OPENCODE_CTL_HOME") {
        return PathBuf::from(h);
    }
    match nonblank("HOME").or_else(|| nonblank("USERPROFILE")) {
        Some(h) => PathBuf::from(h).join(".opencode-ctl"),
        None => PathBuf::from(".opencode-ctl"),
    }
}

pub fn load_from_fs(
    host: Host,
    home: &Path,
    cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> (Settings, Vec<Note>) {
    let user = std::fs::read_to_string(home.join("settings.local.md")).ok();
    let ws = host
        .workspace_settings_rel()
        .and_then(|rel| std::fs::read_to_string(cwd.join(rel)).ok());
    load(Input {
        host,
        env,
        user_file: user.as_deref(),
        workspace_file: ws.as_deref(),
    })
}

fn userinfo_in_url(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    authority.contains('@')
}

fn redact_userinfo(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (format!("{s}://"), r),
        None => (String::new(), url),
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    format!("{scheme}***@{host}{tail}")
}

fn clamp_bounds(key: &str, source: Source, host: Host) -> (u64, u64, bool) {
    match key {
        "poll_s" => (2, 60, false),
        "stall_s" => (30, 600, false),
        "max_turn_s" => (60, 7200, false),
        "call_timeout_s" => (5, 120, false),
        "body_max_bytes" => (1024, 100000, false),
        "output_max_bytes" => (
            1024,
            if source == Source::WorkspaceFile {
                65536
            } else {
                1048576
            },
            false,
        ),
        "wait_max_s" => (30, if host == Host::Opencode { 300 } else { 600 }, false),
        _ => (0, u64::MAX, true), // wait_timeout_s: ceiling is the resolved wait_max_s
    }
}

pub fn load(input: Input) -> (Settings, Vec<Note>) {
    let mut s = Settings::default();
    let mut notes = Vec::new();

    let user: HashMap<String, String> = input
        .user_file
        .map(|f| parse_frontmatter(f).into_iter().collect())
        .unwrap_or_default();
    let ws: HashMap<String, String> = input
        .workspace_file
        .map(|f| parse_frontmatter(f).into_iter().collect())
        .unwrap_or_default();

    for (src, map) in [(Source::UserFile, &user), (Source::WorkspaceFile, &ws)] {
        let mut keys: Vec<&String> = map.keys().filter(|k| !KEYS.contains(&k.as_str())).collect();
        keys.sort();
        for k in keys {
            notes.push(Note {
                source: src,
                key: k.clone(),
                value: map[k].clone(),
                reason: NoteReason::UnknownKey,
            });
        }
    }

    // Candidates for a key in precedence order. Blank env values are unset
    // (except the server key, whose empty value is meaningful); blank file
    // values are unset.
    let cands = |key: &str| -> Vec<(Source, String)> {
        let mut v = Vec::new();
        if let Some(e) = (input.env)(&env_name(key)) {
            if !blank(&e) || key == "server_url" {
                v.push((Source::Env, e));
            }
        }
        for (src, map) in [(Source::UserFile, &user), (Source::WorkspaceFile, &ws)] {
            if let Some(x) = map.get(key).filter(|x| !blank(x)) {
                v.push((src, x.clone()));
            }
        }
        v
    };

    for key in KEYS {
        if key == "permission_policy" || key == "permissions_locked" {
            continue;
        }
        let mut resolved = false;
        for (src, raw) in cands(key) {
            if src == Source::WorkspaceFile && is_protected(key) {
                notes.push(Note {
                    source: src,
                    key: key.to_string(),
                    value: raw,
                    reason: NoteReason::ProtectedInWorkspace,
                });
                continue;
            }
            if resolved {
                continue;
            }
            let mut note = |reason: NoteReason, value: String| {
                notes.push(Note {
                    source: src,
                    key: key.to_string(),
                    value,
                    reason,
                })
            };
            let trimmed = raw.trim();
            match key {
                "opencode_bin" => s.opencode_bin = Some(PathBuf::from(trimmed)),
                "default_model" => s.default_model = Some(trimmed.to_string()),
                "default_agent" => s.default_agent = Some(trimmed.to_string()),
                "server_url" => {
                    if trimmed.is_empty() {
                        s.server_url = None;
                    } else if userinfo_in_url(trimmed) {
                        note(NoteReason::UserinfoInUrl, redact_userinfo(trimmed));
                        continue;
                    } else {
                        s.server_url = Some(trimmed.to_string());
                    }
                }
                _ => {
                    let Ok(n) = trimmed.parse::<u64>() else {
                        note(NoteReason::InvalidValue, raw.clone());
                        continue;
                    };
                    let (min, mut max, ceiling_is_wait_max) = clamp_bounds(key, src, input.host);
                    let min = if ceiling_is_wait_max { 5 } else { min };
                    if ceiling_is_wait_max {
                        max = s.wait_max_s;
                    }
                    let v = n.clamp(min, max);
                    if v != n {
                        note(
                            NoteReason::Clamped {
                                min: min as i64,
                                max: max as i64,
                            },
                            raw.clone(),
                        );
                    }
                    match key {
                        "wait_max_s" => s.wait_max_s = v,
                        "wait_timeout_s" => s.wait_timeout_s = v,
                        "poll_s" => s.poll_s = v,
                        "stall_s" => s.stall_s = v,
                        "max_turn_s" => s.max_turn_s = v,
                        "call_timeout_s" => s.call_timeout_s = v,
                        "output_max_bytes" => s.output_max_bytes = v as usize,
                        _ => s.body_max_bytes = v as usize,
                    }
                }
            }
            s.sources.insert(key, src);
            resolved = true;
        }
    }

    if s.sources["wait_timeout_s"] == Source::Default && s.wait_timeout_s > s.wait_max_s {
        s.wait_timeout_s = s.wait_max_s;
    }

    resolve_permissions(&mut s, &mut notes, &cands);
    (s, notes)
}

fn resolve_permissions(
    s: &mut Settings,
    notes: &mut Vec<Note>,
    cands: &dyn Fn(&str) -> Vec<(Source, String)>,
) {
    let push = |notes: &mut Vec<Note>, src, key: &str, value: String, reason| {
        notes.push(Note {
            source: src,
            key: key.to_string(),
            value,
            reason,
        })
    };
    let protected_only = |notes: &mut Vec<Note>, key: &str| -> Vec<(Source, String)> {
        let mut kept = Vec::new();
        for (src, raw) in cands(key) {
            if src == Source::WorkspaceFile {
                push(notes, src, key, raw, NoteReason::ProtectedInWorkspace);
            } else {
                kept.push((src, raw));
            }
        }
        kept
    };

    // The lock: any protected source saying true wins over every false.
    let mut lock_vals: Vec<(Source, String, bool)> = Vec::new();
    for (src, raw) in protected_only(notes, "permissions_locked") {
        match raw.trim().to_ascii_lowercase().as_str() {
            "true" => lock_vals.push((src, raw, true)),
            "false" => lock_vals.push((src, raw, false)),
            _ => push(
                notes,
                src,
                "permissions_locked",
                raw,
                NoteReason::InvalidValue,
            ),
        }
    }
    if let Some((src, _, _)) = lock_vals.iter().find(|v| v.2) {
        s.permissions_locked = true;
        s.lock_source = Some(*src);
        s.sources.insert("permissions_locked", *src);
        for (osrc, raw, _) in lock_vals.iter().filter(|v| !v.2) {
            push(
                notes,
                *osrc,
                "permissions_locked",
                raw.clone(),
                NoteReason::OverriddenByStickyLock,
            );
        }
    } else if let Some((src, _, _)) = lock_vals.first() {
        s.sources.insert("permissions_locked", *src);
    }

    // The policy: from the lock's source when locked, else first valid.
    let mut resolved = false;
    for (src, raw) in protected_only(notes, "permission_policy") {
        let Some(p) = PermissionPolicy::parse(&raw) else {
            push(
                notes,
                src,
                "permission_policy",
                raw,
                NoteReason::InvalidValue,
            );
            continue;
        };
        let wins = match s.lock_source {
            Some(l) => src == l,
            None => !resolved,
        };
        if wins && !resolved {
            s.permission_policy = p;
            s.sources.insert("permission_policy", src);
            resolved = true;
        } else if s.lock_source.is_some() {
            push(
                notes,
                src,
                "permission_policy",
                raw,
                NoteReason::OverriddenByStickyLock,
            );
        }
    }
}
