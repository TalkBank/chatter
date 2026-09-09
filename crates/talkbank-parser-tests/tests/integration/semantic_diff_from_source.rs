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

//! The semantic-diff REPORT, over two parsed files that differ in one word.
//!
//! `SemanticEq` is the parity oracle's verdict; `SemanticDiff` is the
//! instrument that says WHERE two models differ. Until 2026-09-08 the report
//! machinery (`SemanticDiffReport`, `SemanticDiffContext`, the container and
//! scalar impls) ran only inside the `#[ignore]`d full-corpus equivalence
//! test, so a regression in it would have been found the next time somebody
//! ran that test by hand. This pins, over parsed input, that equal files
//! report nothing, that a one-word change is exactly one difference, and that
//! every difference sits on that one word and names both sides.
//!
//! A changed word is TWO differences, not one: the derive walks `raw_text`
//! and the word's content element separately, and both differ. That is the
//! derive's behaviour as measured, pinned here so a change to it is noticed.
//!
//! SURVIVES a type change, and says which category: behaviour of a derive over
//! a real model, which no signature describes.

use talkbank_model::model::ChatFile;
use talkbank_model::{
    SemanticDiff, SemanticDiffContext, SemanticDiffReport, SemanticEq, SemanticPath,
};
use talkbank_parser_tests::from_source::SingleSpeaker;
use talkbank_parser_tests::test_error::TestError;

fn report(left: &ChatFile, right: &ChatFile, max: usize) -> SemanticDiffReport {
    let mut report = SemanticDiffReport::new(max);
    let mut path = SemanticPath::new();
    let mut ctx = SemanticDiffContext::new();
    left.semantic_diff_into(right, &mut path, &mut report, &mut ctx);
    report
}

/// Two parses of the same text are equal and report no difference.
#[test]
fn the_same_text_reports_no_difference() -> Result<(), TestError> {
    let a = SingleSpeaker::english("*CHI:\thello world .").parsed(&[])?;
    let b = SingleSpeaker::english("*CHI:\thello world .").parsed(&[])?;
    assert!(a.semantic_eq(&b));
    assert!(report(&a, &b, 20).differences().is_empty());
    Ok(())
}

/// One changed word is two differences (`raw_text` and the content element),
/// both located inside the utterance's content, both rendering the two words.
#[test]
fn one_changed_word_is_two_located_differences_on_one_word() -> Result<(), TestError> {
    let a = SingleSpeaker::english("*CHI:\thello world .").parsed(&[])?;
    let b = SingleSpeaker::english("*CHI:\thello there .").parsed(&[])?;
    assert!(!a.semantic_eq(&b));
    let report = report(&a, &b, 20);
    let differences = report.differences();
    assert_eq!(differences.len(), 2, "{differences:?}");
    for difference in differences {
        let path = difference.path.to_string();
        assert!(
            path.contains(".main.content.content["),
            "every difference sits inside the main tier's content: {path}"
        );
        assert!(difference.left.contains("world"), "{}", difference.left);
        assert!(difference.right.contains("there"), "{}", difference.right);
    }
    Ok(())
}

/// The report's cap stops the walk: with a limit of one, two changed words
/// (four differences) yield one recorded difference and a truncated report.
#[test]
fn the_cap_truncates_the_walk() -> Result<(), TestError> {
    let a = SingleSpeaker::english("*CHI:\thello world .\n*CHI:\tbye now .").parsed(&[])?;
    let b = SingleSpeaker::english("*CHI:\thello there .\n*CHI:\tbye then .").parsed(&[])?;
    let capped = report(&a, &b, 1);
    assert_eq!(capped.differences().len(), 1);
    assert!(capped.is_truncated());
    let full = report(&a, &b, 20);
    assert_eq!(full.differences().len(), 4, "{:?}", full.differences());
    assert!(!full.is_truncated());
    Ok(())
}
