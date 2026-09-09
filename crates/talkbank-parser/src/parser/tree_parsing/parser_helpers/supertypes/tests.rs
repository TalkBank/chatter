//! Tests for this subsystem.
//!

use super::is_header;

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
// The predicate itself went on 2026-09-09: its last caller, the annotation
// list parser, matches the generated `BaseAnnotationChoice`, which is the
// grammar's `base_annotation` choice as a type, exhaustively, so the list has
// no hand-written copy left to drift. Deriving the KIND question from the
// generated traversal had been tried and backed out on 2026-08-25 because a
// MISSING `retrace_complete` classifies as `NodeSlot::Missing`, not as a
// present member, and a derived predicate reported it as "expected annotation,
// found 'retrace_complete'"; the typed list parser reads that state as the
// placeholder it is and reports nothing of its own (the whole-tree pass names
// it, E342), which is the answer to that objection.
//
// `test_is_terminator` and `test_is_dependent_tier` went the same way on
// 2026-09-08, for a sharper reason: their SUBJECTS were dead. Nothing in the
// tree-sitter parser called either predicate, and `is_dependent_tier`'s last
// caller had already been replaced by a typed check, with a comment in
// `tier_parsers/dependent_tier.rs` saying so. The tests were what made two dead
// hand-written lists look maintained.

/// Tests is header.
#[test]
fn test_is_header() {
    assert!(is_header("languages_header"));
    assert!(is_header("participants_header"));
    assert!(is_header("id_header"));
    assert!(is_header("header"));
    assert!(!is_header("utterance"));
}
