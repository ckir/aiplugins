// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Where the list lives: starting directory (7.1), git common directory (7.2),
//! store folder (7.3), list name (7.4) and the outside-git key (7.5).
//!
//! No `git` process is run here: the walk reads `.git`, `HEAD` and `commondir` itself.

use crate::error::{Error, Result};
use crate::host::Host;
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The values read from the environment, so resolution is testable without it.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub repo_tasks_dir: Option<String>,
    pub repo_tasks_list: Option<String>,
    pub home: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub local_app_data: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
}

impl Env {
    pub fn from_process() -> Env {
        let var = |k: &str| std::env::var_os(k);
        Env {
            repo_tasks_dir: var("REPO_TASKS_DIR").map(|v| v.to_string_lossy().into_owned()),
            repo_tasks_list: var("REPO_TASKS_LIST").map(|v| v.to_string_lossy().into_owned()),
            home: var("HOME").map(PathBuf::from),
            xdg_data_home: var("XDG_DATA_HOME").map(PathBuf::from),
            local_app_data: var("LOCALAPPDATA").map(PathBuf::from),
            cwd: std::env::current_dir().ok(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    ServerCall,
    Reminder,
    Subcommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartSource {
    Variable,
    Argument,
    Root,
    Workspace,
    ProcessDir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartDir {
    pub path: PathBuf,
    pub source: StartSource,
}

/// `REPO_TASKS_DIR` when set and not blank; it must then be an absolute existing directory.
fn variable_dir(env: &Env) -> Result<Option<PathBuf>> {
    match env.repo_tasks_dir.as_deref() {
        Some(v) if !v.trim().is_empty() => {
            let p = Path::new(v);
            if p.is_absolute() && p.is_dir() {
                Ok(Some(p.to_path_buf()))
            } else {
                Err(Error::EnvDir {
                    value: v.to_string(),
                })
            }
        }
        _ => Ok(None),
    }
}

fn process_dir(env: &Env) -> Result<StartDir> {
    match &env.cwd {
        Some(p) => Ok(StartDir {
            path: p.clone(),
            source: StartSource::ProcessDir,
        }),
        None => Err(Error::NoWorkingDir),
    }
}

/// The 7.1 table. `root` is the client's first usable root (server calls only);
/// `workspace` is the host event's path (reminder only); `arg` is `--dir`
/// (subcommands only). Antigravity never falls back to the process directory
/// for the server or the reminder.
pub fn start_dir(
    entry: Entry,
    env: &Env,
    arg: Option<&Path>,
    root: Option<&Path>,
    workspace: Option<&Path>,
    host: Host,
) -> Result<StartDir> {
    let found = |path: &Path, source| StartDir {
        path: path.to_path_buf(),
        source,
    };
    match entry {
        Entry::Subcommand => {
            if let Some(a) = arg {
                return Ok(found(a, StartSource::Argument));
            }
            if let Some(v) = variable_dir(env)? {
                return Ok(found(&v, StartSource::Variable));
            }
            process_dir(env)
        }
        Entry::ServerCall | Entry::Reminder => {
            if let Some(v) = variable_dir(env)? {
                return Ok(found(&v, StartSource::Variable));
            }
            let (given, source) = if entry == Entry::ServerCall {
                (root, StartSource::Root)
            } else {
                (workspace, StartSource::Workspace)
            };
            if let Some(p) = given {
                return Ok(found(p, source));
            }
            if host == Host::Agy {
                return Err(Error::NoWorkspace);
            }
            process_dir(env)
        }
    }
}

/// Where one list lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    pub dir: PathBuf,
    pub list: String,
    pub common_dir: Option<PathBuf>,
    pub start: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    List,
    Lock,
    Tmp,
    Archive,
}

impl Store {
    pub fn file(&self, kind: FileKind) -> PathBuf {
        let n = &self.list;
        self.dir.join(match kind {
            FileKind::List => format!("{n}.json"),
            FileKind::Lock => format!("{n}.lock"),
            FileKind::Tmp => format!("{n}.json.tmp"),
            FileKind::Archive => format!("{n}.archive.jsonl"),
        })
    }
}

/// `[A-Za-z0-9_-]{1,64}` (7.4).
pub fn valid_list_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Remove the `\\?\` prefix canonicalization adds on Windows; a no-op elsewhere.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    if cfg!(windows) {
        let s = p.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    p
}

/// The canonical form of `p`, without the Windows `\\?\` prefix. Public so tests build
/// their expectations from the same spelling the engine uses.
pub fn canonical(p: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(p).map(strip_verbatim)
}

#[cfg(unix)]
fn path_bytes(p: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(p: &Path) -> Vec<u8> {
    p.to_string_lossy().into_owned().into_bytes()
}

/// First 16 hexadecimal characters of the SHA-256 of the path's UTF-8 bytes (7.5).
pub fn store_key(canonical_dir: &Path) -> String {
    let digest = Sha256::digest(path_bytes(canonical_dir));
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn git_error(entry: &Path, reason: impl std::fmt::Display) -> Error {
    Error::GitEntry {
        entry: entry.to_path_buf(),
        reason: reason.to_string(),
    }
}

/// Follow one `.git` entry to the git common directory (7.2).
fn common_dir_of(entry: &Path, holder: &Path) -> Result<PathBuf> {
    let meta = fs::metadata(entry).map_err(|e| git_error(entry, e))?;
    let g = if meta.is_dir() {
        entry.to_path_buf()
    } else {
        let text = fs::read_to_string(entry).map_err(|e| git_error(entry, e))?;
        let rest = text
            .strip_prefix("gitdir: ")
            .ok_or_else(|| git_error(entry, "the file does not start with 'gitdir: '"))?;
        holder.join(rest.trim_end())
    };
    if !g.is_dir() {
        return Err(git_error(
            entry,
            format!("{} is not a directory", g.display()),
        ));
    }
    if !g.join("HEAD").is_file() {
        return Err(git_error(
            entry,
            format!("{} has no HEAD file", g.display()),
        ));
    }
    let common = g.join("commondir");
    let c = match fs::read_to_string(&common) {
        Ok(text) => g.join(text.trim_end()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => g,
        Err(e) => return Err(git_error(entry, format!("{}: {e}", common.display()))),
    };
    canonical(&c).map_err(|e| git_error(entry, format!("{}: {e}", c.display())))
}

/// Walk up from `start` (already canonical) to the first directory with a `.git` entry.
fn find_common_dir(start: &Path) -> Result<Option<PathBuf>> {
    for dir in start.ancestors() {
        let entry = dir.join(".git");
        match fs::symlink_metadata(&entry) {
            Ok(_) => return common_dir_of(&entry, dir).map(Some),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::Io {
                    path: entry,
                    source,
                })
            }
        }
    }
    Ok(None)
}

/// The per-user data directory B (7.5).
fn data_dir(env: &Env) -> Result<PathBuf> {
    let need = |v: &Option<PathBuf>, name: &str| {
        v.clone()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| Error::NoDataDir {
                reason: format!("{name} is not set"),
            })
    };
    if cfg!(windows) {
        need(&env.local_app_data, "LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        Ok(need(&env.home, "HOME")?.join("Library/Application Support"))
    } else if let Some(x) = env
        .xdg_data_home
        .clone()
        .filter(|p| !p.as_os_str().is_empty())
    {
        Ok(x)
    } else {
        Ok(need(&env.home, "HOME")?.join(".local/share"))
    }
}

/// Resolve the store for the directory `start` (7.2 to 7.5).
pub fn locate(start: &Path, env: &Env) -> Result<Store> {
    let list = match env.repo_tasks_list.as_deref() {
        Some(v) if !v.trim().is_empty() => {
            if !valid_list_name(v) {
                return Err(Error::ListName {
                    value: v.to_string(),
                });
            }
            v.to_string()
        }
        _ => "default".to_string(),
    };
    let start = canonical(start).map_err(|source| Error::Io {
        path: start.to_path_buf(),
        source,
    })?;
    match find_common_dir(&start)? {
        Some(c) => Ok(Store {
            dir: c.join("repo-tasks"),
            list,
            common_dir: Some(c),
            start,
        }),
        None => Ok(Store {
            dir: data_dir(env)?.join("repo-tasks").join(store_key(&start)),
            list,
            common_dir: None,
            start,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::host::Host;
    use std::path::{Path, PathBuf};

    fn env_with(dir: Option<&str>) -> Env {
        Env {
            repo_tasks_dir: dir.map(str::to_string),
            cwd: Some(PathBuf::from("/the/cwd")),
            ..Env::default()
        }
    }

    fn existing() -> PathBuf {
        std::env::temp_dir()
    }

    fn sd(r: Result<StartDir>) -> (PathBuf, StartSource) {
        let s = r.expect("start_dir should succeed");
        (s.path, s.source)
    }

    #[test]
    fn start_dir_table() {
        let ex = existing();
        let exs = ex.to_str().unwrap().to_string();
        let root = PathBuf::from("/the/root");
        let ws = PathBuf::from("/the/workspace");
        let arg = PathBuf::from("/the/arg");
        let cwd = PathBuf::from("/the/cwd");

        // Server call: variable, then root, then process directory.
        let e = env_with(Some(&exs));
        assert_eq!(
            sd(start_dir(
                Entry::ServerCall,
                &e,
                None,
                Some(&root),
                None,
                Host::Claude
            )),
            (ex.clone(), StartSource::Variable)
        );
        let e = env_with(None);
        assert_eq!(
            sd(start_dir(
                Entry::ServerCall,
                &e,
                None,
                Some(&root),
                None,
                Host::Claude
            )),
            (root.clone(), StartSource::Root)
        );
        assert_eq!(
            sd(start_dir(
                Entry::ServerCall,
                &e,
                None,
                None,
                None,
                Host::Claude
            )),
            (cwd.clone(), StartSource::ProcessDir)
        );
        // A blank variable is absent.
        let e = env_with(Some("   "));
        assert_eq!(
            sd(start_dir(
                Entry::ServerCall,
                &e,
                None,
                Some(&root),
                None,
                Host::Claude
            )),
            (root.clone(), StartSource::Root)
        );
        let e = env_with(Some(""));
        assert_eq!(
            sd(start_dir(
                Entry::ServerCall,
                &e,
                None,
                None,
                None,
                Host::Qwen
            )),
            (cwd.clone(), StartSource::ProcessDir)
        );
        // A relative or non-existent variable is an error naming variable and value.
        for bad in ["relative/dir", "/definitely/not/there/xyz"] {
            let e = env_with(Some(bad));
            for entry in [Entry::ServerCall, Entry::Reminder, Entry::Subcommand] {
                let err = start_dir(entry, &e, None, Some(&root), Some(&ws), Host::Claude)
                    .expect_err("bad variable must fail");
                let text = err.to_string();
                assert!(text.contains("REPO_TASKS_DIR"), "{text}");
                assert!(text.contains(bad), "{text}");
                assert!(!text.contains('\n'));
            }
        }
        // Antigravity server call: no process-directory step.
        let e = env_with(None);
        let err = start_dir(Entry::ServerCall, &e, None, None, None, Host::Agy)
            .expect_err("agy without variable or root must fail");
        assert!(err.to_string().contains("REPO_TASKS_DIR"), "{err}");
        assert_eq!(
            sd(start_dir(
                Entry::ServerCall,
                &e,
                None,
                Some(&root),
                None,
                Host::Agy
            )),
            (root.clone(), StartSource::Root)
        );
        let e = env_with(Some(&exs));
        assert_eq!(
            sd(start_dir(
                Entry::ServerCall,
                &e,
                None,
                None,
                None,
                Host::Agy
            )),
            (ex.clone(), StartSource::Variable)
        );

        // Reminder: variable, workspace, process directory; the root does not enter.
        assert_eq!(
            sd(start_dir(
                Entry::Reminder,
                &e,
                None,
                Some(&root),
                Some(&ws),
                Host::Claude
            )),
            (ex.clone(), StartSource::Variable)
        );
        let e = env_with(None);
        assert_eq!(
            sd(start_dir(
                Entry::Reminder,
                &e,
                None,
                Some(&root),
                Some(&ws),
                Host::Claude
            )),
            (ws.clone(), StartSource::Workspace)
        );
        assert_eq!(
            sd(start_dir(
                Entry::Reminder,
                &e,
                None,
                Some(&root),
                None,
                Host::Claude
            )),
            (cwd.clone(), StartSource::ProcessDir)
        );
        assert_eq!(
            sd(start_dir(
                Entry::Reminder,
                &e,
                None,
                None,
                Some(&ws),
                Host::Agy
            )),
            (ws.clone(), StartSource::Workspace)
        );
        assert!(start_dir(Entry::Reminder, &e, None, Some(&root), None, Host::Agy).is_err());

        // Subcommand: argument, variable, process directory.
        let e = env_with(Some(&exs));
        assert_eq!(
            sd(start_dir(
                Entry::Subcommand,
                &e,
                Some(&arg),
                Some(&root),
                Some(&ws),
                Host::Claude
            )),
            (arg.clone(), StartSource::Argument)
        );
        assert_eq!(
            sd(start_dir(
                Entry::Subcommand,
                &e,
                None,
                Some(&root),
                Some(&ws),
                Host::Claude
            )),
            (ex.clone(), StartSource::Variable)
        );
        let e = env_with(None);
        assert_eq!(
            sd(start_dir(
                Entry::Subcommand,
                &e,
                None,
                Some(&root),
                Some(&ws),
                Host::Claude
            )),
            (cwd.clone(), StartSource::ProcessDir)
        );
        assert_eq!(
            sd(start_dir(
                Entry::Subcommand,
                &e,
                None,
                None,
                None,
                Host::Agy
            )),
            (cwd.clone(), StartSource::ProcessDir)
        );
        // No working directory at all is an error, not a guess.
        let e = Env::default();
        assert!(start_dir(Entry::Subcommand, &e, None, None, None, Host::Claude).is_err());
    }

    #[test]
    fn store_key_vector() {
        // printf '%s' '/home/user/project' | sha256sum, first 16 hex characters.
        assert_eq!(
            store_key(Path::new("/home/user/project")),
            "9dad1e4e08b0b11c"
        );
        assert_eq!(store_key(Path::new("/home/user/project")).len(), 16);
    }

    #[test]
    fn list_name_rules() {
        let ok64 = "a".repeat(64);
        assert!(valid_list_name(&ok64));
        assert!(valid_list_name("A_z-9"));
        assert!(!valid_list_name(&"a".repeat(65)));
        assert!(!valid_list_name(""));
        assert!(!valid_list_name("a b"));
        assert!(!valid_list_name("a.b"));
        assert!(!valid_list_name("é"));

        let base = std::env::temp_dir();
        let mut env = Env {
            home: Some(base.clone()),
            xdg_data_home: Some(base.clone()),
            local_app_data: Some(base.clone()),
            repo_tasks_list: Some(ok64.clone()),
            ..Env::default()
        };
        let s = locate(&base, &env).expect("64 characters pass");
        assert_eq!(s.list, ok64);
        for bad in ["a b".to_string(), "a".repeat(65), " x ".to_string()] {
            env.repo_tasks_list = Some(bad.clone());
            let err = locate(&base, &env).expect_err("bad name must fail");
            let text = err.to_string();
            assert!(text.contains("REPO_TASKS_LIST"), "{text}");
            assert!(text.contains(&bad), "value must not be altered: {text}");
        }
        // Blank is absent: the name is `default`.
        for blank in [None, Some(String::new()), Some("  ".to_string())] {
            env.repo_tasks_list = blank;
            assert_eq!(locate(&base, &env).unwrap().list, "default");
        }
    }
}
