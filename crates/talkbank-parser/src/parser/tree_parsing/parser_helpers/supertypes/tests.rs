//! Tests for this subsystem.
//!

use super::{is_dependent_tier, is_header, is_terminator};

// `test_is_base_annotation` was DELETED here on 2026-08-25, and nothing replaced
// it, which is the point.
//
// It asserted membership kind by kind against a hand-written `matches!` list:
// two spellings of one fact, and a test whose only job was to notice they had
// drifted. It did not notice. By the time it was removed the list named three
// kinds the grammar's `base_annotation` choice does not contain
// (`duration_annotation`, `retrace_uncertain`, `scoped_best_guess`) and omitted
// `code_switch_annotation`, which the grammar had just gained, so the predicate
// rejected a construct the parser accepted.
//
// What guards the list now is behaviour rather than a second copy of it: the
// spec-generated construct corpus tests parse each member through a real file,
// and they are what failed loudly when `code_switch_annotation` was missing.
// `is_base_annotation`'s own docstring is the single owner of the rest of the
// explanation, including why deriving the list from the generated traversal was
// tried and backed out.

/// Tests is terminator.
#[test]
fn test_is_terminator() {
    assert!(is_terminator("period"));
    assert!(is_terminator("question"));
    assert!(is_terminator("interruption"));
    assert!(is_terminator("terminator"));
    assert!(!is_terminator("word"));
}

/// Tests is header.
#[test]
fn test_is_header() {
    assert!(is_header("languages_header"));
    assert!(is_header("participants_header"));
    assert!(is_header("id_header"));
    assert!(is_header("header"));
    assert!(!is_header("utterance"));
}

/// Tests is dependent tier.
#[test]
fn test_is_dependent_tier() {
    assert!(is_dependent_tier("mor_dependent_tier"));
    assert!(is_dependent_tier("gra_dependent_tier"));
    assert!(is_dependent_tier("pho_dependent_tier"));
    assert!(is_dependent_tier("dependent_tier"));
    assert!(!is_dependent_tier("main_tier"));
}
