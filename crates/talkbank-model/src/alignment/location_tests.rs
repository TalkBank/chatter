//! Alignment claims that are about an INTERNAL VALUE, not a diagnostic.
//!
//! # Why the six count-mismatch tests left
//!
//! They built a `MainTier` with `Span::from_usize(0, 20)` and a trailing
//! comment reading `// *CHI: one two .`, then asserted the error landed at
//! 0..20. The span and the comment are two statements of one fact joined by
//! nothing, and the number is only meaningful against bytes the test does not
//! contain. They are rows in
//! `talkbank-parser-tests/tests/integration/alignment_location_from_source.rs`
//! now, where the span comes from the parse, so the assertion is the one that
//! matters: the diagnostic points at the SAME span the main tier occupies,
//! whatever offset that is. A hardcoded number cannot say that.
//!
//! # Why the four `%wor` sidecar tests left too
//!
//! They asserted the `WorTimingSidecar` VALUE `resolve_wor_timing_sidecar`
//! returns, and this file used to say no end-to-end route could reach that
//! value. That was false: a parsed utterance owns both tiers, and
//! `talkbank-parser-tests/tests/integration/wor_timing_from_source.rs` (the
//! drift and timed-filler shapes) and `wor_terminator_alignment.rs` (the
//! terminator shape) call the same function on tiers the parser built.
//!
//! # What stays, and why it has to
//!
//! `test_mor_alignment_errors_have_no_bogus_context` asserts that an alignment
//! error carries NO `ErrorContext` at the moment it is created, because source
//! context is populated later by `enhance_errors_with_source`. By the time a
//! diagnostic reaches a user the context IS populated, so the property is
//! invisible from outside and this is the only place it can be checked. It
//! builds its input by hand ON PURPOSE, which is the line between the
//! fabrication that left and the fabrication that remains.
//!
//! CHAT references:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

use super::*;
use crate::Span;
use crate::model::{
    MainTier, Mor, MorTier, MorTierType, MorWord, PosCategory, Terminator, UtteranceContent, Word,
};

/// Helper: build a simple Mor item from POS and lemma strings.
fn simple_mor(pos: &str, lemma: &str) -> Mor {
    Mor::new(MorWord::new(PosCategory::new(pos), lemma))
}

/// Verifies that E706 errors have no bogus ErrorContext with empty source text.
///
/// The alignment module does not have access to the source text, so it must
/// create errors with `context: None` (via `at_span`), not with a dummy
/// `ErrorContext { source_text: "", span: <absolute bytes> }`.
#[test]
fn test_mor_alignment_errors_have_no_bogus_context() {
    let main = MainTier::new(
        "CHI",
        vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
            "one", "one",
        )))],
        Terminator::Period { span: Span::DUMMY },
    )
    .with_span(Span::from_usize(0, 15));

    let mor = MorTier::new(
        MorTierType::Mor,
        vec![simple_mor("num", "one"), simple_mor("num", "two")],
        Terminator::Period { span: Span::DUMMY },
    )
    .with_span(Span::from_usize(16, 40));

    let alignment = align_main_to_mor(&main, &mor);

    for error in &alignment.errors {
        // context should be None (no source text available at alignment time),
        // NOT Some(ErrorContext { source_text: "", ... })
        assert!(
            error.context.is_none(),
            "Alignment error should not have a dummy ErrorContext; \
             source context is populated later by enhance_errors_with_source"
        );
    }
}
