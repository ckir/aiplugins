//! The one serialisation every measured byte count is taken against (spec §5).
//!
//! JSON is not canonical by default: key order, whitespace and unicode escaping
//! are all free choices, and two encoders can emit the same document at
//! different lengths. A footprint measured under an unpinned encoding would
//! drift for reasons having nothing to do with the plugin, and the snapshot test
//! that reviews these numbers would flake on the encoder rather than on the
//! schema.
//!
//! The pinned form is: object keys sorted, no insignificant whitespace,
//! non-ASCII emitted literally as UTF-8. `serde_json` gives all three by
//! default — its `Map` is a `BTreeMap`, `to_string` is compact, and it does not
//! `\u`-escape above ASCII — so this module is thin on purpose. What earns its
//! keep is `tests/canonical.rs`, which fails if any of those defaults ever
//! changes underneath us. The realistic way that happens is a crate anywhere in
//! the dependency tree enabling `serde_json`'s `preserve_order` feature, which
//! swaps the `BTreeMap` for an `IndexMap`; feature unification would then apply
//! it here, silently making every measurement depend on the order a server
//! happened to send its keys in.

use serde_json::Value;

/// Serialise `value` in the pinned canonical form.
pub fn canonical_json(value: &Value) -> String {
    // `to_string` cannot fail for a `Value`: it contains no map with non-string
    // keys and no type whose `Serialize` can error.
    serde_json::to_string(value).expect("a serde_json::Value always serialises")
}

/// The measured size of `value`: the UTF-8 byte length of its canonical form.
///
/// Bytes, not characters. A schema carrying non-ASCII would otherwise be
/// reported smaller than the host actually sends.
pub fn canonical_len(value: &Value) -> usize {
    canonical_json(value).len()
}
