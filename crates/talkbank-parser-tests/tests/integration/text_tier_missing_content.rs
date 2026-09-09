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

//! A bullet-payload tier whose body cannot start, for every tier that has one.
//!
//! # Why this exists
//!
//! `report_missing_text_content` in `tier_parsers/text/helpers.rs` is the only
//! producer of E330 for these tiers, and a whole-suite branch-coverage run on
//! 2026-09-08 found it with ZERO covered regions: nothing in the suite reached
//! it, across 436 error fixtures, 138 construct tests and 107 reference files.
//!
//! That is not because the code is unreachable. `spec/errors/E330.md` has an
//! example that emits E330, and the observation snapshot records it doing so.
//! But the snapshot is produced by `spec/runtime-tools`, which lives in a
//! SEPARATE Cargo workspace, so `cargo llvm-cov --workspace --tests` over the
//! main workspace never runs it. The rule was demonstrated and unexercised at
//! the same time, in two senses of the word that nobody had told apart.
//!
//! # The input, and why it is this one
//!
//! A bullet-payload tier body is `text_with_bullets`, and none of its
//! alternatives can BEGIN with a bare bullet delimiter (U+0015). tree-sitter
//! therefore parks an ERROR where the body belongs, the body slot is
//! `NodeSlot::Error`, and `parse_text_tier_content` reports E330 rather than
//! inventing an empty body. Any other malformed body either parses or is
//! rejected further up, so this is the shortest input that reaches the arm.
//!
//! # One case per tier, deliberately
//!
//! The nine tiers share one generic function, so nine monomorphizations of it
//! exist and coverage counts them separately. A single `%com` case would leave
//! eight uncovered and read as though the rule were exercised.

use talkbank_model::errors::ErrorCollector;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// The bullet delimiter, as a `char` so the fixture text carries the real byte
/// rather than an escape a reader has to decode.
const BULLET: char = '\u{15}';

/// Every dependent tier whose body is `text_with_bullets`.
///
/// Read off the `parse_optional_text_tier_content` callers rather than recalled:
/// a tier missing from this list is a monomorphization nothing covers, which is
/// the exact hole this module was written to close.
const BULLET_PAYLOAD_TIERS: &[&str] = &[
    "act", "add", "cod", "com", "exp", "gpx", "int", "sit", "spa",
];

/// A minimal file whose `%{label}` tier body opens with a bare bullet.
fn file_with_unstartable_tier(label: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n\
         *CHI:\thello .\n\
         %{label}:\t{BULLET}hello\n\
         @End\n"
    )
}

/// Every bullet-payload tier reports E330 when its body cannot start.
///
/// SURVIVES a type change, and says which category: this is behaviour at the
/// PARSER boundary over real CHAT bytes, which no signature describes. The
/// model cannot express "this tier body was an ERROR node" as a type, because
/// the tier is the thing that failed to be built.
#[test]
fn a_tier_body_that_cannot_start_reports_missing_content() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;
    let mut without = Vec::new();

    for label in BULLET_PAYLOAD_TIERS {
        let errors = ErrorCollector::new();
        let source = file_with_unstartable_tier(label);
        let _ = parser.parse_chat_file_streaming(&source, &errors);

        let reported = errors.to_vec();
        let has_e330 = reported.iter().any(|error| error.code.as_str() == "E330");
        if !has_e330 {
            let codes: Vec<&str> = reported.iter().map(|error| error.code.as_str()).collect();
            without.push(format!("%{label}: got {codes:?}"));
        }
    }

    assert!(
        without.is_empty(),
        "every bullet-payload tier must report E330 when its body cannot start, \
         because the alternative is inventing an empty body for a tier the \
         author wrote content into. Missing:\n  {}",
        without.join("\n  ")
    );
    Ok(())
}

/// The same tiers, with a body that CAN start, report no E330.
///
/// The pair is the point. Without it the test above passes if E330 begins
/// firing on every tier, or if the parser stops building these tiers at all.
#[test]
fn a_tier_body_that_starts_normally_reports_nothing_missing() -> Result<(), TestError> {
    let parser = TreeSitterParser::new()?;
    let mut wrongly_reported = Vec::new();

    for label in BULLET_PAYLOAD_TIERS {
        let errors = ErrorCollector::new();
        let source = format!(
            "@UTF8\n@Begin\n@Languages:\teng\n\
             @Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n\
             *CHI:\thello .\n\
             %{label}:\thello\n\
             @End\n"
        );
        let _ = parser.parse_chat_file_streaming(&source, &errors);

        if errors
            .to_vec()
            .iter()
            .any(|error| error.code.as_str() == "E330")
        {
            wrongly_reported.push(format!("%{label}"));
        }
    }

    assert!(
        wrongly_reported.is_empty(),
        "a tier with ordinary text content has content: {wrongly_reported:?}"
    );
    Ok(())
}
