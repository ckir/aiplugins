// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Test helper: take the exclusive lock on the file named by the first argument,
//! print "locked", then hold it until killed or 60 seconds have passed.

use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;

fn main() {
    let path = std::env::args().nth(1).expect("lock path argument");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .expect("open lock file");
    file.lock().expect("take lock");
    println!("locked");
    std::io::stdout().flush().expect("flush");
    std::thread::sleep(Duration::from_secs(60));
}
