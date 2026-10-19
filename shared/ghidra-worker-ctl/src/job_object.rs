//! Re-export shim. The kill-guard moved to the `proc-guard` crate, but `crate::job_object::JobObject`
//! stays a stable path for the worker lifecycle (`launch.rs`) — on Windows a Job Object with
//! `KILL_ON_JOB_CLOSE`, on Unix a `setsid` process group, and a no-op stub everywhere else.

pub use proc_guard::JobObject;
