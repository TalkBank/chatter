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

//! Which code a dependent tier's recovery ERROR gets, over files the parser
//! built.
//!
//! The dependent-tier analyzer classifies an ERROR node by its text. Until
//! 2026-09-08 one of its branches fired on the substring `%gra:` anywhere in
//! that text, whatever the tier: an `%eng` body that happened to mention
//! `%gra:` was reported as E710, "invalid GRA relation, non-numeric index",
//! and so was junk after a well-formed `%gra` relation. E710 has one honest
//! producer, the typed relation parser (E710.md); an ERROR inside any tier
//! is the generic E316 unless a rule about its TEXT applies (E258 for a
//! double comma, E760 for a `%mor` item with an empty part of speech). Every
//! row names every code the file reports.

use talkbank_parser_tests::from_source::{Rules, diagnostics_of};
use talkbank_parser_tests::test_error::TestError;

/// A CHAT file around one `CHI` utterance and one dependent tier line.
fn file(tier: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\thello .\n{tier}\n@End\n"
    )
}

/// Each row: the tier line and every code the file reports, sorted.
const ROWS: &[(&str, &[&str])] = &[
    // The substring `%gra:` inside an `%eng` body's recovery node is E316,
    // beside E330 for the content node the reader does not have.
    ("%eng:\t\u{15}has %gra: inside", &["E316", "E330"]),
    // The same inside a `%x` tier.
    ("%xfoo:\t\u{15}with %gra: text", &["E316", "E330"]),
    // Junk after a well-formed relation is E316 at the junk, not E710.
    ("%gra:\t1|0|ROOT %gra: x", &["E316", "E604"]),
    // A double comma is its own rule on any tier, and stays.
    ("%eng:\t\u{15}one,,two", &["E258", "E330"]),
    // The substring `%mor:` inside an `%eng` body, beside a token that would
    // be a `%mor` item with an empty part of speech, is still E316: E760 is
    // about `%mor` tiers, and this is not one.
    ("%eng:\t\u{15}note %mor: |bad text", &["E316", "E330"]),
];

/// The code a dependent tier's recovery ERROR gets does not depend on a
/// substring the tier's text happens to contain.
#[test]
fn a_recovery_error_is_classified_by_its_tier_not_by_a_substring() -> Result<(), TestError> {
    for (tier, expected_codes) in ROWS {
        let reported = diagnostics_of(&file(tier), Rules::Default)?;
        let mut got: Vec<String> = reported.iter().map(|e| e.code.to_string()).collect();
        got.sort_unstable();
        let mut expected: Vec<&str> = expected_codes.to_vec();
        expected.sort_unstable();
        assert_eq!(got, expected, "codes reported for {tier:?}");
    }
    Ok(())
}
