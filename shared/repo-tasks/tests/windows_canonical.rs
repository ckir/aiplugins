// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Spec 15.6: one directory reached under two spellings of its path (lower-case
//! and upper-case) must resolve to one store key. Windows only; an empty target
//! elsewhere.
#![cfg(windows)]

use repo_tasks::resolve::{locate, store_key, Env};
use std::io::Write;

fn note(line: &str) {
    eprintln!("{line}");
    if let Some(file) = std::env::var_os("REPO_TASKS_CI_NOTES") {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// Removes the temporary directory however the test ends, unwrap panics included.
struct Cleanup(std::path::PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn differently_cased_spellings_give_one_store_key() {
    let base = std::env::temp_dir().join(format!("repo-tasks-canon-{}", std::process::id()));
    let _cleanup = Cleanup(base.clone());
    let dir = base.join("MixedCase");
    std::fs::create_dir_all(&dir).unwrap();
    let real = std::fs::canonicalize(&dir)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    // ASCII only: the point is two spellings of the same path, not Unicode case mapping.
    let lower = std::path::PathBuf::from(real.to_ascii_lowercase());
    let upper = std::path::PathBuf::from(real.to_ascii_uppercase());
    assert_ne!(lower, upper, "the two spellings must differ");
    let env = Env {
        local_app_data: Some(base.clone()),
        ..Env::default()
    };
    let a = locate(&lower, &env).unwrap();
    let b = locate(&upper, &env).unwrap();
    note(&format!(
        "canonical: {} | {}",
        a.start.display(),
        b.start.display()
    ));
    let (ka, kb) = (store_key(&a.start), store_key(&b.start));
    assert_eq!(ka, kb, "two spellings must give one store key");
    assert_eq!(a.dir, b.dir);
}
