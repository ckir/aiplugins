// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! The engine's error type. Every `Display` text is one line.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("REPO_TASKS_DIR must be an absolute path to an existing directory, got '{value}'")]
    EnvDir { value: String },
    #[error("the host gave no workspace: set REPO_TASKS_DIR to the directory whose list to use")]
    NoWorkspace,
    #[error("no working directory is available: set REPO_TASKS_DIR or pass --dir")]
    NoWorkingDir,
    #[error("unknown --host value '{value}' (only 'opencode' is accepted)")]
    BadHost { value: String },
    #[error("REPO_TASKS_LIST '{value}' is not a valid list name: use 1 to 64 characters from A-Z, a-z, 0-9, '_' and '-'")]
    ListName { value: String },
    #[error("the git entry {} does not lead to a git directory: {reason}", entry.display())]
    GitEntry { entry: PathBuf, reason: String },
    #[error("no per-user data directory is available: {reason}")]
    NoDataDir { reason: String },
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the list breaks a rule: {reason}")]
    ListInvalid { reason: String },
    #[error(
        "the list has format {found} but this engine knows format {newest}: update the plugin"
    )]
    NewerFormat { found: u64, newest: u32 },
    #[error("{}: {source}", path.display())]
    ListFile {
        path: PathBuf,
        #[source]
        source: Box<Error>,
    },
    #[error("could not take the lock {} within {secs} seconds: another process holds it", path.display())]
    LockTimeout { path: PathBuf, secs: u64 },
    #[error("{reason}")]
    Refused { reason: String },
    #[error("refusing to write an empty list file: {}", path.display())]
    EmptyWrite { path: PathBuf },
}
