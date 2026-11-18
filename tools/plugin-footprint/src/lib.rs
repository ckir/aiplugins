// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Measure the static token footprint a plugin imposes on a context window.
//!
//! See `docs/specs/2026-09-03-plugin-footprint.md`. This crate is maintainer
//! tooling: it is never shipped in a plugin and never appears in a marketplace
//! manifest.

pub mod canonical;
pub mod child_env;
pub mod document;
pub mod gate;
pub mod manifest;
pub mod probe;
pub mod publish;
pub mod registry;
pub mod sources;
