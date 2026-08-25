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

//! Every symbol in `spec/symbols/symbol_registry.json` must carry an example
//! that is valid CHAT, and this is what makes that a gate rather than a claim.
//!
//! The registry owns what each symbol means; a gloss is prose and prose rots.
//! The example is the part a machine can check, so it is generated onto the type
//! as `example()` and parsed here. A symbol whose documented usage stops being
//! valid CHAT fails the build instead of quietly becoming folklore.
//!
//! This test is deliberately NOT a mapping assertion. It never says "PitchUp is
//! ↑", because that is generated from the record and restating it would be the
//! mirror the registry exists to delete. It says the grammar accepts what we
//! tell people to write, which no type can hold: it is behaviour at the parser
//! boundary.
//!
//! Worked example of it earning its place: the uniform template
//! `<symbol>hello there .` is valid for 24 of the 25 symbols and INVALID for
//! `↫`, which brackets a repeated segment and needs a stem outside the
//! delimiters (E753). Without a runnable example that would have shipped as a
//! documented usage nobody could use.

use talkbank_model::model::FileStem;
use talkbank_model::model::TranscriptName;
use talkbank_model::model::content::word::{CADelimiterType, CAElementType};

/// Wraps a registry example's main tier in the smallest valid file around it.
fn file_around(main_tier: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n\
         @ID:\teng|registry|PAR|||||Participant|||\n{main_tier}\n@End\n"
    )
}

fn assert_example_is_valid_chat(gloss: &str, symbol: &str, example: &str) {
    let diagnostics = crate::common::parse_validate_and_collect_diagnostics(
        &file_around(example),
        TranscriptName::Named(FileStem::from_stem("symbol_registry_example")),
    );
    assert!(
        diagnostics.is_empty(),
        "the registry example for {gloss} ({symbol}) is not valid CHAT.\n  \
         example: {example:?}\n  diagnostics: {diagnostics:?}"
    );
}

/// Every word-attached symbol's documented example parses and validates clean.
#[test]
fn every_ca_element_example_is_valid_chat() {
    for element in CAElementType::ALL {
        assert_example_is_valid_chat(element.gloss(), element.to_symbol(), element.example());
    }
}

/// Every paired-stretch symbol's documented example parses and validates clean.
#[test]
fn every_ca_delimiter_example_is_valid_chat() {
    for delimiter in CADelimiterType::ALL {
        assert_example_is_valid_chat(
            delimiter.gloss(),
            delimiter.to_symbol(),
            delimiter.example(),
        );
    }
}

/// Every symbol round-trips through `from_char`, in both directions.
///
/// This is not a mapping assertion against hand-typed glyphs: it never names a
/// symbol. It checks that `to_symbol` and `from_char`, two separately generated
/// match tables, are inverses, which is a roundtrip between two functions and is
/// exactly the case a type cannot hold.
///
/// What this test USED to also assert, and no longer does: that a word-attached
/// symbol does not parse as a paired delimiter, and the reverse. That was the
/// same disjointness the registry validator used to check, and it is now
/// unrepresentable rather than merely untested. A symbol carries exactly one
/// `parse_role`, which is what sorts it into one of these two types, and
/// `registry.js` refuses duplicate codepoints across every symbol, so a registry
/// whose element and delimiter sets overlap cannot be built.
#[test]
fn from_char_inverts_to_symbol() {
    for element in CAElementType::ALL {
        let ch = element
            .to_symbol()
            .chars()
            .next()
            .expect("symbol is one char");
        assert_eq!(CAElementType::from_char(ch), Some(*element));
    }
    for delimiter in CADelimiterType::ALL {
        let ch = delimiter
            .to_symbol()
            .chars()
            .next()
            .expect("symbol is one char");
        assert_eq!(CADelimiterType::from_char(ch), Some(*delimiter));
    }
}
