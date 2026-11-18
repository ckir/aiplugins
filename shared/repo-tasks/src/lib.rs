// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Shared engine for repo-tasks: one task list per repository.

pub mod archive;
pub mod clock;
pub mod error;
pub mod host;
pub mod model;
pub mod ops;
pub mod resolve;
pub mod roots;
pub mod store;
