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

//! Whole-file alignment validation (`ChatFile::validate_alignments` and the
//! alignment pass of `validate_chat_file_with_options`), over files the
//! parser built.
//!
//! # What this replaces
//!
//! Five inline tests of `talkbank-model`'s `model/file/chat_file/validate.rs`
//! assembled a `ChatFile` from `Line::header(Header::Utf8)` onward around a
//! `MainTier` of `Word::new_unchecked` items and a `%gra` of synthetic `MOD`
//! relations. Here each case is the file as written. The synthetic labels
//! become Universal Dependencies ones so the fixtures are valid CHAT where
//! the originals were unparseable as such.

use talkbank_model::model::{ChatFile, Line, ParseHealthTier};
use talkbank_model::{ErrorCode, ParseValidateOptions, validate_chat_file_with_options};
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

const MATCHING: &str =
    "*CHI:\tI go .\n%mor:\tpron|I verb|go .\n%gra:\t1|2|NSUBJ 2|0|ROOT 3|2|PUNCT";
const GRA_SHORT: &str = "*CHI:\tI go .\n%mor:\tpron|I verb|go .\n%gra:\t1|2|NSUBJ 2|0|ROOT";
const MOR_SHORT: &str =
    "*CHI:\tI go home .\n%mor:\tpron|I verb|go .\n%gra:\t1|2|NSUBJ 2|0|ROOT 3|2|PUNCT";

fn file(lines: &str, codes: &[&str]) -> Result<ChatFile, TestError> {
    SingleSpeaker::english(lines).parsed(codes)
}

fn codes_of(errors: &[talkbank_model::ParseError]) -> Vec<&str> {
    errors.iter().map(|e| e.code.as_str()).collect()
}

/// Consistent main, `%mor` and `%gra` cardinalities: no alignment error.
#[test]
fn matching_tiers_align_without_error() -> Result<(), TestError> {
    let errors = file(MATCHING, &[])?.validate_alignments();
    assert!(errors.is_empty(), "{:?}", codes_of(&errors));
    Ok(())
}

/// Too few `%gra` relations is an alignment error (E720).
#[test]
fn a_short_gra_tier_is_an_alignment_error() -> Result<(), TestError> {
    let errors = file(GRA_SHORT, &["E720"])?.validate_alignments();
    assert!(
        codes_of(&errors).contains(&"E720"),
        "{:?}",
        codes_of(&errors)
    );
    Ok(())
}

/// A tainted `%gra` domain is skipped: the same short `%gra` reports nothing
/// once the utterance carries a `%gra` parse taint.
///
/// The taint is applied to the parsed utterance the way a recovering parse
/// would apply it; no CHAT text produces a clean `%gra` that is also tainted,
/// so this is the one hand-applied step, and it is the state under test.
#[test]
fn a_tainted_gra_domain_is_skipped() -> Result<(), TestError> {
    let mut chat = file(GRA_SHORT, &["E720"])?;
    for line in &mut chat.lines {
        if let Line::Utterance(utterance) = line {
            utterance.mark_parse_taint(ParseHealthTier::Gra);
        }
    }
    let errors = chat.validate_alignments();
    assert!(errors.is_empty(), "{:?}", codes_of(&errors));
    Ok(())
}

/// Main-word and `%mor` counts diverging is an alignment error (E705).
#[test]
fn a_short_mor_tier_is_an_alignment_error() -> Result<(), TestError> {
    let errors = file(MOR_SHORT, &["E705"])?.validate_alignments();
    assert!(
        codes_of(&errors).contains(&"E705"),
        "{:?}",
        codes_of(&errors)
    );
    Ok(())
}

/// An out-of-bounds `%gra` head is E713 and does not cascade into the
/// structural no-root (E722) or cycle (E724) diagnostics.
#[test]
fn an_out_of_bounds_head_does_not_cascade() -> Result<(), TestError> {
    let mut chat = file(
        "*CHI:\tI go .\n%mor:\tpron|I verb|go .\n%gra:\t1|5|DEP 2|1|OBJ 3|2|PUNCT",
        &["E713"],
    )?;
    let errors = validate_chat_file_with_options(
        &mut chat,
        &ParseValidateOptions::default().with_alignment(),
    )
    .expect_err("an out-of-bounds gra head must fail validation");
    assert!(
        errors
            .iter()
            .any(|e| e.code == ErrorCode::GraInvalidHeadIndex)
    );
    assert!(!errors.iter().any(|e| e.code == ErrorCode::GraNoRoot));
    assert!(
        !errors
            .iter()
            .any(|e| e.code == ErrorCode::GraCircularDependency)
    );
    Ok(())
}
