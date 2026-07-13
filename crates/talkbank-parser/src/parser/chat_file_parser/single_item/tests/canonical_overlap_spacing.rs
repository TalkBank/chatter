//! Canonical serialization: contents-level overlap-point spacing.
//!
//! Ratified 2026-07-11 (maintainer decision, "tight" form): a contents-level
//! OPENING overlap marker glues to the FOLLOWING content item (`⌈is`), and a
//! CLOSING marker glues to the PRECEDING one (`here⌉`). All other contents
//! items are single-space joined.
//!
//! The roundtrip INVARIANT is semantic, not textual: serializing a model and
//! reparsing must yield a semantically equal model. These tests pin (a) the
//! canonical FORM the writer emits, (b) idempotence (canonical text is a
//! serialization fixpoint), and (c) the semantic invariant across
//! non-canonical spellings of the same content.

use talkbank_model::model::{SemanticEq, WriteChat};

use super::parse_main_tier;

/// Parse a main-tier line and serialize it back (the canonicalizer).
fn canon(line: &str) -> String {
    let tier = parse_main_tier(line)
        .unwrap_or_else(|e| panic!("canonical-spacing fixture must parse: {line:?}: {e:?}"));
    let mut out = String::new();
    tier.write_chat(&mut out)
        .unwrap_or_else(|e| panic!("serialization must not fail: {e}"));
    out
}

/// Comma glues LEFT (the grammar's negotiated exception accepts
/// `one, two` and rejects `one ,two`; canonical form is the accepted
/// glued spelling, so normalizing wild corpora does not churn commas).
#[test]
fn comma_glues_left() {
    assert_eq!(canon("*CHI:\tone , two ."), "*CHI:\tone, two .");
    assert_eq!(canon("*CHI:\tone, two ."), "*CHI:\tone, two .");
}

/// Spaced source form canonicalizes to the tight form.
#[test]
fn spaced_overlap_becomes_tight() {
    assert_eq!(
        canon("*CHI:\twho \u{2308} is here \u{2309} ."),
        "*CHI:\twho \u{2308}is here\u{2309} ."
    );
}

/// Already-tight source is already canonical.
#[test]
fn tight_overlap_is_fixpoint() {
    let tight = "*CHI:\twho \u{2308}is here\u{2309} .";
    assert_eq!(canon(tight), tight);
}

/// The golden-tier shape that motivated the redefinition: a glued closing
/// edge marker (`h⌋`) and a spaced opening one normalize to tight.
#[test]
fn mixed_spacing_normalizes() {
    assert_eq!(
        canon("*LSN:\te f \u{230A} g h\u{230B} ."),
        "*LSN:\te f \u{230A}g h\u{230B} ."
    );
}

/// Canonicalization is idempotent on every fixture here.
#[test]
fn canonicalization_is_idempotent() {
    for line in [
        "*CHI:\twho \u{2308} is here \u{2309} .",
        "*CHI:\twho \u{2308}is here\u{2309} .",
        "*LSN:\te f \u{230A} g h\u{230B} .",
        "*SPK:\t\u{2308} a b\u{2309} c d .",
    ] {
        let once = canon(line);
        assert_eq!(canon(&once), once, "not a fixpoint for {line:?}");
    }
}

/// The semantic roundtrip invariant: every spelling of the same content
/// parses, canonicalizes, reparses, and compares semantically equal to the
/// original parse.
#[test]
fn semantic_roundtrip_holds_across_spellings() {
    for line in [
        "*CHI:\twho \u{2308} is here \u{2309} .",
        "*CHI:\twho \u{2308}is here\u{2309} .",
        "*SPK:\t\u{2308} a b\u{2309} c d .",
        "*LSN:\te f \u{230A} g h\u{230B} .",
    ] {
        let original = parse_main_tier(line).expect("fixture parses");
        let mut serialized = String::new();
        original.write_chat(&mut serialized).expect("serializes");
        let reparsed = parse_main_tier(&serialized)
            .unwrap_or_else(|e| panic!("canonical text must reparse: {serialized:?}: {e:?}"));
        assert!(
            reparsed.semantic_eq(&original),
            "semantic roundtrip failed for {line:?} via {serialized:?}"
        );
    }
}
