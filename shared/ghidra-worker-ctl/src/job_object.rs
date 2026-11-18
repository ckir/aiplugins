// Copyright 2026 AI Plugins Marketplace Contributors
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 together with the
// additional terms (evaluation grant, anti-circumvention) that apply to this
// repository. See the LICENSE file at the repository root.

//! Re-export shim. The kill-guard moved to the `proc-guard` crate, but `crate::job_object::JobObject`
//! stays a stable path for the worker lifecycle (`launch.rs`) — on Windows a Job Object with
//! `KILL_ON_JOB_CLOSE`, on Unix a `setsid` process group, and a no-op stub everywhere else.

pub use proc_guard::JobObject;
