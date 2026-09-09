// Test code: the panic-family clippy lints are relaxed by policy
// (assertions and fixture unwraps are the testing idiom); the
// workspace [lints] table holds production code to deny.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! Separators at the end of tier content, over utterances the parser built.
//!
//! # What this replaces
//!
//! The two inline tests of `talkbank-model`'s `model/content/tier_content.rs`
//! built a `TierContentItems` of `Word::simple("hi")` and
//! `Separator::Comma { span: Span::DUMMY }` and validated it under a context
//! whose `field_span` and `field_text` they filled by hand. Here each is the
//! utterance as written, and the claim is the same: valid CHAT, no
//! diagnostic at all. Both were regressions once, which is why they are
//! pinned at all.

use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

/// A separator right before the terminator is fine.
#[test]
fn a_trailing_separator_is_valid() -> Result<(), TestError> {
    SingleSpeaker::english("*CHI:\thi , .").parsed(&[])?;
    Ok(())
}

/// An overlap marker after a separator still counts as content.
#[test]
fn a_separator_followed_by_an_overlap_is_valid() -> Result<(), TestError> {
    SingleSpeaker::english("*CHI:\thello , \u{2308} world \u{2309} .").parsed(&[])?;
    Ok(())
}
