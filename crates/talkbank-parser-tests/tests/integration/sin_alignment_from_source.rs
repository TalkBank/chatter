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

//! Main-tier to `%sin` alignment, over tiers the parser built.
//!
//! # What this replaces
//!
//! Five of the six inline tests of `talkbank-model`'s `alignment/sin.rs`
//! built a `MainTier` of `Word::simple` items beside a `Terminator::Period {
//! span: Span::DUMMY }` and a `SinTier` of `SinToken::new(..)` items. Here each
//! case is the two tiers as written. The sixth, empty on empty, stays there:
//! a `%sin:` line with no token is E342 at parse, so that state is reachable
//! only from a tier built by hand.

use talkbank_model::alignment::{SinAlignment, align_main_to_sin};
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

/// Parse a main tier with its `%sin`, checked to report exactly `codes`, and
/// align them.
fn aligned(main: &str, sin: &str, codes: &[&str]) -> Result<SinAlignment, TestError> {
    let lines = format!("*CHI:\t{main}\n%sin:\t{sin}");
    let utterance = SingleSpeaker::english(&lines).utterance(codes)?;
    let sin = utterance
        .sin_tier()
        .ok_or_else(|| TestError::Failure("the fixture built no %sin tier".into()))?;
    Ok(align_main_to_sin(&utterance.main, sin))
}

/// Matched counts align completely; the terminator has no `%sin` slot.
#[test]
fn perfect_match_aligns_every_word() -> Result<(), TestError> {
    let alignment = aligned("one two .", "g:toy:dpoint 0", &[])?;
    assert_eq!(alignment.pairs.len(), 2);
    assert!(alignment.errors.is_empty());
    assert!(alignment.pairs.iter().all(|p| p.is_complete()));
    Ok(())
}

/// A longer main tier gets a placeholder and one E718.
#[test]
fn a_longer_main_tier_is_e718() -> Result<(), TestError> {
    let alignment = aligned("one two three .", "g:toy:dpoint 0", &["E718"])?;
    assert_eq!(alignment.pairs.len(), 3);
    assert_eq!(alignment.errors.len(), 1);
    assert_eq!(alignment.errors[0].code.as_str(), "E718");
    Ok(())
}

/// A longer `%sin` tier gets a placeholder and one E719.
#[test]
fn a_longer_sin_tier_is_e719() -> Result<(), TestError> {
    let alignment = aligned("one .", "g:toy:dpoint 0", &["E719"])?;
    assert_eq!(alignment.pairs.len(), 2);
    assert_eq!(alignment.errors.len(), 1);
    assert_eq!(alignment.errors[0].code.as_str(), "E719");
    Ok(())
}

/// A literal `0` is a valid alignable `%sin` token.
#[test]
fn zero_placeholders_are_alignable_tokens() -> Result<(), TestError> {
    let alignment = aligned("what shall we get ?", "0 0 0 0", &[])?;
    assert_eq!(alignment.pairs.len(), 4);
    assert!(alignment.errors.is_empty());
    Ok(())
}

/// A gesture token aligns to one word.
#[test]
fn a_gesture_token_aligns_to_one_word() -> Result<(), TestError> {
    let alignment = aligned("junk .", "g:toy:dpoint", &[])?;
    assert_eq!(alignment.pairs.len(), 1);
    assert!(alignment.errors.is_empty());
    Ok(())
}
