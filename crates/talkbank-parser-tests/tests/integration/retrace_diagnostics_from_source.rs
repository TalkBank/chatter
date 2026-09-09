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

//! Where a retrace diagnostic points, over utterances the parser built.
//!
//! # What this pins
//!
//! E370 (a retrace marker with nothing after it to retrace) is reported at
//! the marker. Until 2026-09-08 the rule found that position by RENDERING
//! the main tier back to CHAT text and taking the marker's offset in the
//! rendering, then adding the tier's start: right only when the source was
//! already canonical. On `<hello  there> [/] .` (two spaces) the label sat
//! one byte early, over ` [/`, and on `< hello there> [/] .` likewise. The
//! parser now records the marker token's own span on the retrace
//! (`Retrace::marker_span`); these rows pin that the reported span is the
//! marker's bytes in the SOURCE, whatever the spacing around it and
//! whatever else the marker's chain carries after it.
//!
//! Every row is invalid CHAT on purpose (E370 is the point), and names
//! every code it expects.

use talkbank_parser_tests::from_source::{Rules, diagnostics_of};
use talkbank_parser_tests::test_error::TestError;

/// A CHAT file around one `CHI` utterance.
fn file(utterance: &str) -> String {
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t{utterance}\n@End\n"
    )
}

/// Each row: the utterance, the marker E370 must point at, and every code
/// the file reports.
const ROWS: &[(&str, &str, &[&str])] = &[
    ("<hello there> [/] .", "[/]", &["E370"]),
    ("<hello  there> [/] .", "[/]", &["E370"]),
    ("< hello there> [/] .", "[/]", &["E750", "E370"]),
    ("hi <hello there> [//] .", "[//]", &["E370"]),
    ("hi <hello there> [///] .", "[///]", &["E370"]),
    ("the cat [/-] .", "[/-]", &["E370"]),
    // The marker sits in a chain with another annotation after it: the
    // retrace's own span runs through `[?]`, so the marker's bytes have to
    // be recorded separately, which is what `Retrace::marker_span` is.
    ("<hello there> [/] [?] .", "[/]", &["E370"]),
    // Two markers, each reported at its own bytes (the first is checked).
    ("hello [/] [//] .", "[/]", &["E370", "E370", "E377"]),
];

/// E370's span is the marker's own bytes in the source.
#[test]
fn e370_points_at_the_marker_in_the_source() -> Result<(), TestError> {
    for (utterance, marker, expected_codes) in ROWS {
        let source = file(utterance);
        let reported = diagnostics_of(&source, Rules::Default)?;
        let codes: Vec<String> = reported.iter().map(|e| e.code.to_string()).collect();
        let mut expected: Vec<&str> = expected_codes.to_vec();
        expected.sort_unstable();
        let mut got: Vec<&str> = codes.iter().map(String::as_str).collect();
        got.sort_unstable();
        assert_eq!(got, expected, "codes reported for {utterance:?}");

        let e370 = reported
            .iter()
            .find(|e| e.code.to_string() == "E370")
            .expect("E370 reported");
        let span = e370.location.span;
        let pointed = &source[span.start as usize..span.end as usize];
        assert_eq!(
            pointed, *marker,
            "E370 on {utterance:?} points at {pointed:?}, bytes {}..{}",
            span.start, span.end
        );
    }
    Ok(())
}
