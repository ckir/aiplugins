// Copyright 2026 AI Plugins Marketplace Contributors
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
//
// Source-available software. It is NOT open source.
// Licensed under the PolyForm Noncommercial License 1.0.0 together with the
// additional terms (evaluation grant, anti-circumvention) that apply to this
// repository. See the LICENSE file at the repository root.

//! Shared engine for the opencode-ctl MCP servers.

pub mod cli;
pub mod clock;
pub mod compact;
pub mod created_set;
pub mod doctor;
pub mod engine;
pub mod error;
pub mod gc;
pub mod host;
pub mod launcher;
pub mod liveness;
pub mod logging;
pub mod openapi;
pub mod pairing;
pub mod server;
pub mod settings;
pub mod skill_asset;
pub mod startup;
pub mod sweep;
pub mod testing;
pub mod tools;
pub mod transport;
