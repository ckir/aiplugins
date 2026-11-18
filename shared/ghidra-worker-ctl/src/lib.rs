// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Worker lifecycle + boot orchestration. The pure state machine (`state`) plus the OS-facing boot
//! path: config/argv (`config`), token handshake (`token`), spawn + pipe drain (`launch`), the
//! Windows Job Object kill-guard (`job_object`), framed TCP transport (`conn`), and boot/handshake
//! (`boot`). Health-driven RESPAWN wiring (re-boot a crashed worker mid-session) is still M1.

pub mod boot;
pub mod config;
pub mod conn;
pub mod job_object;
pub mod launch;
pub mod state;
pub mod token;
