//! The serialisation every byte count is measured against (spec §5).
//!
//! A footprint is only comparable across runs if the same payload always
//! serialises to the same text. `serde_json`'s default `Map` is a `BTreeMap`,
//! so keys come out sorted for free — but the `preserve_order` feature swaps it
//! for an `IndexMap` and silently makes ordering depend on what the server
//! happened to send. Enabling that anywhere in the dependency tree, by any
//! crate, would change every measured number without failing a build. These
//! tests are what notices.

use plugin_footprint::canonical::{canonical_json, canonical_len};

#[test]
fn keys_are_sorted_and_insignificant_whitespace_is_dropped() {
    let value: serde_json::Value =
        serde_json::from_str(r#"{ "zebra": 1, "alpha": { "y": 2, "x": 3 } }"#).expect("parses");

    assert_eq!(
        canonical_json(&value),
        r#"{"alpha":{"x":3,"y":2},"zebra":1}"#
    );
}

#[test]
fn non_ascii_is_emitted_literally_never_escaped() {
    let value = serde_json::json!({ "name": "café ✓" });

    let text = canonical_json(&value);

    assert!(
        text.contains("café ✓"),
        "non-ASCII must survive as UTF-8, got: {text}"
    );
    assert!(
        !text.contains(r"\u"),
        "escaping would inflate the byte count against what the host actually sends: {text}"
    );
}

#[test]
fn length_counts_utf8_bytes_not_characters() {
    // 'é' is one character but two UTF-8 bytes. A footprint reported in
    // characters would understate every schema carrying non-ASCII.
    let value = serde_json::json!({ "k": "é" });
    let expected = r#"{"k":"é"}"#;

    assert_eq!(canonical_json(&value), expected);
    assert_eq!(canonical_len(&value), expected.len());
    assert_ne!(
        canonical_len(&value),
        expected.chars().count(),
        "this assertion is what makes the one above meaningful"
    );
}

#[test]
fn array_order_is_preserved_because_it_is_semantic() {
    // Object keys are sorted for determinism; array order is data and must not be.
    let value = serde_json::json!({ "tools": ["zebra", "alpha"] });

    assert_eq!(canonical_json(&value), r#"{"tools":["zebra","alpha"]}"#);
}
