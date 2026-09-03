//! Measure the static token footprint a plugin imposes on a context window.
//!
//! See `docs/specs/2026-09-03-plugin-footprint.md`. This crate is maintainer
//! tooling: it is never shipped in a plugin and never appears in a marketplace
//! manifest.

pub mod canonical;
pub mod child_env;
pub mod document;
pub mod manifest;
pub mod probe;
