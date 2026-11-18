// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Locked, atomic writes and lock-free reads of the list file (8.2, 8.3).

use crate::error::{Error, Result};
use crate::model::List;
use crate::resolve::{FileKind, Store};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The waits of 8.2 and 8.3; tests shorten them.
#[derive(Debug, Clone)]
pub struct Tuning {
    pub lock_deadline: Duration,
    pub rename_retry: Duration,
    pub read_retry: Duration,
    pub poll: Duration,
    /// Where `retry_io` records each refused attempt (the error kind, once per refusal); `None`
    /// records nothing. CI notes read it; nothing is printed.
    pub retry_log: Option<RetryLog>,
}

/// The shared list of refused attempts a `Tuning` can carry.
pub type RetryLog = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            lock_deadline: Duration::from_secs(5),
            rename_retry: Duration::from_secs(5),
            read_retry: Duration::from_secs(1),
            poll: Duration::from_millis(10),
            retry_log: None,
        }
    }
}

fn io_err(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn file_err(path: &Path, source: Error) -> Error {
    Error::ListFile {
        path: path.to_path_buf(),
        source: Box::new(source),
    }
}

/// The two kinds 8.2 and 8.3 retry on Windows: `PermissionDenied` and a sharing violation (32).
#[cfg(windows)]
fn windows_retryable(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(32)
}

/// Only Windows retries; elsewhere every error fails at once.
fn platform_retryable(e: &io::Error) -> bool {
    #[cfg(windows)]
    {
        windows_retryable(e)
    }
    #[cfg(not(windows))]
    {
        let _ = e;
        false
    }
}

/// What the retry log keeps of a refused attempt: its kind, and the OS error number if any.
fn refusal_label(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(n) => format!("{:?} (os error {n})", e.kind()),
        None => format!("{:?}", e.kind()),
    }
}

/// Run `op`; while it fails with an error `retryable` accepts, repeat it until `budget`
/// has passed, counted from the first refusal. Each refusal is pushed on `log` when there is one.
fn retry_io<T>(
    budget: Duration,
    poll: Duration,
    log: Option<&RetryLog>,
    retryable: fn(&io::Error) -> bool,
    mut op: impl FnMut() -> io::Result<T>,
) -> io::Result<T> {
    let mut first_refusal: Option<Instant> = None;
    loop {
        match op() {
            Err(e) if retryable(&e) => {
                let start = *first_refusal.get_or_insert_with(Instant::now);
                if let Some(log) = log {
                    log.lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .push(refusal_label(&e));
                }
                if start.elapsed() >= budget {
                    return Err(e);
                }
                std::thread::sleep(poll);
            }
            other => return other,
        }
    }
}

/// `Ok(None)` when the file does not exist.
fn read_file(path: &Path, tuning: &Tuning) -> io::Result<Option<Vec<u8>>> {
    match retry_io(
        tuning.read_retry,
        tuning.poll,
        tuning.retry_log.as_ref(),
        platform_retryable,
        || fs::read(path),
    ) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn read_parsed(path: &Path, tuning: &Tuning) -> Result<List> {
    match read_file(path, tuning).map_err(|e| io_err(path, e))? {
        None => Ok(List::empty()),
        Some(bytes) => List::parse(&bytes).map_err(|e| file_err(path, e)),
    }
}

/// Read the list without a lock. A missing file or folder is the empty list and
/// creates nothing; a bad file is an error naming it and is never repaired.
pub fn read_list(store: &Store, tuning: &Tuning) -> Result<List> {
    read_parsed(&store.file(FileKind::List), tuning)
}

fn ensure_dir(dir: &Path) -> Result<()> {
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir).map_err(|e| io_err(dir, e))
}

fn open_options() -> OpenOptions {
    let mut o = OpenOptions::new();
    o.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o
}

fn take_lock(path: &Path, tuning: &Tuning) -> Result<File> {
    let file = open_options()
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| io_err(path, e))?;
    let deadline = Instant::now() + tuning.lock_deadline;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) => {
                if Instant::now() >= deadline {
                    return Err(Error::LockTimeout {
                        path: path.to_path_buf(),
                        secs: tuning.lock_deadline.as_secs(),
                    });
                }
                std::thread::sleep(tuning.poll);
            }
            Err(TryLockError::Error(e)) => return Err(io_err(path, e)),
        }
    }
}

/// The bytes to write, refused when empty. `List::to_bytes` turns a serialisation
/// failure into an empty body followed by a line break, so a body with nothing but
/// white space is refused too.
fn checked_bytes(bytes: Vec<u8>, path: &Path) -> Result<Vec<u8>> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(Error::EmptyWrite {
            path: path.to_path_buf(),
        });
    }
    Ok(bytes)
}

fn write_tmp(tmp: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut f = open_options().create_new(true).open(tmp)?;
    f.write_all(bytes)?;
    f.flush()?;
    f.sync_all()
}

fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(dir)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        Ok(())
    }
}

/// Step 4 of 8.2: temp file, flush, rename, flush the folder. On failure the old
/// list stands and the temp file is removed.
fn replace_list(store: &Store, bytes: &[u8], tuning: &Tuning) -> Result<()> {
    let (tmp, list) = (store.file(FileKind::Tmp), store.file(FileKind::List));
    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_err(&tmp, e)),
    }
    let result = write_tmp(&tmp, bytes)
        .map_err(|e| io_err(&tmp, e))
        .and_then(|()| {
            retry_io(
                tuning.rename_retry,
                tuning.poll,
                tuning.retry_log.as_ref(),
                platform_retryable,
                || fs::rename(&tmp, &list),
            )
            .map_err(|e| io_err(&list, e))
        });
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    after_rename(&store.dir, sync_dir);
    Ok(())
}

/// The folder flush after a successful rename is best effort: the new list is already in place,
/// so failing the call would invite a retry that duplicates a create.
fn after_rename(dir: &Path, flush: impl FnOnce(&Path) -> io::Result<()>) {
    let _ = flush(dir);
}

/// 7.5: outside git the store folder names the directory it belongs to.
fn write_directory_txt(store: &Store) -> Result<()> {
    if store.common_dir.is_some() {
        return Ok(());
    }
    let path: PathBuf = store.dir.join("directory.txt");
    match open_options().create_new(true).open(&path) {
        Ok(mut f) => f
            .write_all(store.start.to_string_lossy().as_bytes())
            .and_then(|()| f.sync_all())
            .map_err(|e| io_err(&path, e)),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(io_err(&path, e)),
    }
}

/// The five steps of 8.2. `f` sees the whole list; the file is written only when
/// `f` returned `Ok` and the list changed.
pub fn mutate<T>(
    store: &Store,
    tuning: &Tuning,
    f: impl FnOnce(&mut List) -> Result<T>,
) -> Result<T> {
    ensure_dir(&store.dir)?;
    let _lock = take_lock(&store.file(FileKind::Lock), tuning)?;
    let before = read_parsed(&store.file(FileKind::List), tuning)?;
    let mut after = before.clone();
    let out = f(&mut after)?;
    if after != before {
        after
            .validate()
            .map_err(|e| file_err(&store.file(FileKind::List), e))?;
        let bytes = checked_bytes(after.to_bytes(), &store.file(FileKind::List))?;
        write_directory_txt(store)?;
        replace_list(store, &bytes, tuning)?;
    }
    Ok(out)
    // The lock is released when `_lock` drops, after the write.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn empty_bytes_are_never_written() {
        let p = Path::new("/x/default.json");
        assert!(matches!(
            checked_bytes(Vec::new(), p),
            Err(Error::EmptyWrite { .. })
        ));
        assert!(matches!(
            checked_bytes(b"\n".to_vec(), p),
            Err(Error::EmptyWrite { .. })
        ));
        assert_eq!(checked_bytes(b"{}\n".to_vec(), p).unwrap(), b"{}\n");
        assert!(!List::empty().to_bytes().iter().all(u8::is_ascii_whitespace));
    }

    #[test]
    fn retry_repeats_only_accepted_errors_until_budget() {
        let calls = Cell::new(0);
        let always = |_: &io::Error| true;
        let never = |_: &io::Error| false;
        let denied = || io::Error::from(io::ErrorKind::PermissionDenied);
        // Accepted error: repeated until the budget, then the error is returned.
        let r: io::Result<()> = retry_io(
            Duration::from_millis(50),
            Duration::from_millis(1),
            None,
            always,
            || {
                calls.set(calls.get() + 1);
                Err(denied())
            },
        );
        assert!(r.is_err());
        assert!(calls.get() > 1);
        // Refused error: one call only.
        calls.set(0);
        let r: io::Result<()> = retry_io(
            Duration::from_secs(5),
            Duration::from_millis(1),
            None,
            never,
            || {
                calls.set(calls.get() + 1);
                Err(denied())
            },
        );
        assert!(r.is_err());
        assert_eq!(calls.get(), 1);
        // Recovery after two refusals.
        calls.set(0);
        let r = retry_io(
            Duration::from_secs(5),
            Duration::from_millis(1),
            None,
            always,
            || {
                calls.set(calls.get() + 1);
                if calls.get() < 3 {
                    Err(denied())
                } else {
                    Ok(7)
                }
            },
        );
        assert_eq!(r.unwrap(), 7);
    }

    #[test]
    fn observer_gets_one_entry_per_refusal() {
        let log: RetryLog = Default::default();
        let calls = Cell::new(0);
        let r = retry_io(
            Duration::from_secs(5),
            Duration::from_millis(1),
            Some(&log),
            |_| true,
            || {
                calls.set(calls.get() + 1);
                if calls.get() < 4 {
                    Err(io::Error::from(io::ErrorKind::PermissionDenied))
                } else {
                    Ok(())
                }
            },
        );
        assert!(r.is_ok());
        let seen = log.lock().unwrap().clone();
        assert_eq!(seen, vec!["PermissionDenied"; 3]);
        // A raw OS error keeps its number; a refused error is not recorded.
        let log: RetryLog = Default::default();
        let _ = retry_io(
            Duration::ZERO,
            Duration::from_millis(1),
            Some(&log),
            |_| true,
            || Err::<(), _>(io::Error::from_raw_os_error(32)),
        );
        assert_eq!(log.lock().unwrap().len(), 1);
        assert!(log.lock().unwrap()[0].ends_with("(os error 32)"));
        let _ = retry_io(
            Duration::ZERO,
            Duration::from_millis(1),
            Some(&log),
            |_| false,
            || Err::<(), _>(io::Error::from_raw_os_error(32)),
        );
        assert_eq!(log.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_failed_folder_flush_after_the_rename_is_not_a_failure() {
        let ran = Cell::new(false);
        after_rename(Path::new("/x"), |_| {
            ran.set(true);
            Err(io::Error::other("disk gone"))
        });
        assert!(ran.get());
    }
}
