// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Re-export shim. The kill-guard moved to the `proc-guard` crate, but `crate::job_object::JobObject`
//! stays a stable path for the worker lifecycle (`launch.rs`) — on Windows a Job Object with
//! `KILL_ON_JOB_CLOSE`, on Unix a `setsid` process group, and a no-op stub everywhere else.

pub use proc_guard::JobObject;
