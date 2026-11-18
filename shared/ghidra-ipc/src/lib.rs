// Copyright 2026 Costas Kirgoussios
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
// with the additional terms in LICENSE-ADDENDUM (evaluation grant,
// anti-circumvention). See both files at the repository root.

//! Wire types, framing, JSON-RPC, and address canonicalization for the
//! host <-> Ghidra-worker socket. No Ghidra dependency: pure, unit-testable.
pub mod address;
pub mod error;
pub mod error_map;
pub mod frame;
pub mod owner_only;
pub mod protocol;
pub mod rpc;
