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

//! E305, a main tier with no terminator, and its CA-mode exemption, over
//! files the parser built.
//!
//! # What this replaces
//!
//! The two inline tests of `talkbank-model`'s `validation/main_tier.rs`
//! built `MainTier::new("CHI", content, None)` and validated it under a
//! context with `ca_mode` set by hand. Here the utterance is `*CHI:\thi` with
//! nothing after the word, and CA mode is what it is in a transcript: the
//! `@Options:\tCA` header.

use talkbank_parser_tests::from_source::{Media, SingleSpeaker};
use talkbank_parser_tests::test_error::TestError;

/// Outside CA mode a missing terminator is E305, and never E304.
#[test]
fn a_missing_terminator_is_e305_outside_ca_mode() -> Result<(), TestError> {
    // The verdict is exactly E305: E304 not firing is part of the set check.
    SingleSpeaker::english("*CHI:\thi").parsed(&["E305"])?;
    Ok(())
}

/// Under `@Options: CA` a missing terminator is allowed.
#[test]
fn a_missing_terminator_is_allowed_in_ca_mode() -> Result<(), TestError> {
    SingleSpeaker {
        languages: "eng",
        options: Some("CA"),
        media: Media::Undeclared,
        lines: "*CHI:\thi",
    }
    .parsed(&[])?;
    Ok(())
}
