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

//! A tier count mismatch is reported AT THE MAIN TIER, over real spans.
//!
//! # What this replaces, and why it is a stronger claim
//!
//! `talkbank-model`'s `alignment/location_tests.rs` built a `MainTier` with
//! `Span::from_usize(0, 20)` and a trailing comment reading
//! `// *CHI: one two .`, then asserted the error landed at 0..20. The span and
//! the comment are two statements of one fact, joined by nothing: change the
//! comment's text and the number stays right, change the number and the
//! comment stays plausible. Worse, 0..20 is only meaningful against bytes the
//! test does not contain.
//!
//! Here the span comes from the parse, so the assertion can be the one that
//! actually matters: the diagnostic points at the SAME span the main tier
//! occupies, whatever offset that is. A hardcoded number cannot say that, and
//! a test that hardcodes one passes just as happily when the diagnostic is
//! pinned to the wrong tier by an offset that happens to match.
//!
//! # Why the whole-file route
//!
//! `align_main_to_pho` and its siblings are `pub(crate)`, so nothing outside
//! `talkbank-model` calls them; the diagnostics they produce reach a user
//! through `validate`, and that is the path measured here.

use talkbank_model::ErrorCollector;
use talkbank_model::model::{Line, TranscriptName};
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// One transcript body and the count-mismatch code it must report.
struct Case {
    /// What the case is, for the failure report.
    what: &'static str,
    /// The main tier and its dependent tier, after the header block.
    body: &'static str,
    /// The code, or `None` for a case that must report no mismatch at all.
    code: Option<&'static str>,
}

/// Every tier whose item count is checked against the main tier, over and
/// under, with an aligned control.
const CASES: &[Case] = &[
    Case {
        what: "%pho with fewer items than the main tier",
        body: "*CHI:\tone two .\n%pho:\twan\n",
        code: Some("E714"),
    },
    Case {
        what: "%pho with more items than the main tier",
        body: "*CHI:\tone .\n%pho:\twan tu\n",
        code: Some("E715"),
    },
    Case {
        what: "%sin with fewer items than the main tier",
        body: "*CHI:\tI want cookie .\n%sin:\tPOINT REACH\n",
        code: Some("E718"),
    },
    Case {
        what: "%sin with more items than the main tier",
        body: "*CHI:\twant cookie .\n%sin:\tPOINT REACH GRAB\n",
        code: Some("E719"),
    },
    Case {
        what: "%mor with fewer items than the main tier",
        body: "*CHI:\tone two .\n%mor:\tn|one .\n",
        code: Some("E705"),
    },
    Case {
        what: "%mor with more items than the main tier",
        body: "*CHI:\tone .\n%mor:\tn|one n|two .\n",
        code: Some("E706"),
    },
    Case {
        what: "an aligned %mor tier",
        body: "*CHI:\tone two .\n%mor:\tn|one n|two .\n",
        code: None,
    },
];

/// Every count mismatch is reported at the main tier's own span.
///
/// SURVIVES a type change, and says which category: this is where a
/// DIAGNOSTIC POINTS, which no signature describes. The alignment layer
/// deliberately anchors a count mismatch at the main tier rather than at the
/// dependent one, so an editor highlights the utterance a person has to
/// repair; that is a policy with a real alternative, and it is what this pins.
#[test]
fn a_count_mismatch_points_at_the_main_tier() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let mut wrong = Vec::new();

    for case in CASES {
        let source = format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n{}@End\n",
            case.body
        );
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);

        // The main tier's span, read off the PARSE rather than written down.
        // This is the whole difference from the predecessor: there is no
        // number here to disagree with the text.
        let main_span = file.lines.iter().find_map(|line| match line {
            Line::Utterance(utterance) => Some(utterance.main.span),
            _ => None,
        });
        let Some(main_span) = main_span else {
            wrong.push(format!("{}: the fixture built no utterance", case.what));
            continue;
        };

        let reported = errors.to_vec();
        match case.code {
            None => {
                let mismatches: Vec<&str> = reported
                    .iter()
                    .map(|e| e.code.as_str())
                    .filter(|code| CASES.iter().any(|c| c.code == Some(*code)))
                    .collect();
                if !mismatches.is_empty() {
                    wrong.push(format!(
                        "{}: aligned tiers reported {mismatches:?}",
                        case.what
                    ));
                }
            }
            Some(code) => {
                let found: Vec<_> = reported
                    .iter()
                    .filter(|e| e.code.as_str() == code)
                    .collect();
                match found.as_slice() {
                    [] => {
                        let got: Vec<&str> = reported.iter().map(|e| e.code.as_str()).collect();
                        wrong.push(format!("{}: expected {code} and got {got:?}", case.what));
                    }
                    [error] => {
                        if error.location.span != main_span {
                            wrong.push(format!(
                                "{}: {code} points at {:?} and the main tier is at {main_span:?}",
                                case.what, error.location.span
                            ));
                        }
                    }
                    many => wrong.push(format!(
                        "{}: {code} was reported {} times, and a count mismatch is one fault",
                        case.what,
                        many.len()
                    )),
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "{} of {} alignment cases were wrong:\n  {}",
        wrong.len(),
        CASES.len(),
        wrong.join("\n  ")
    );
    Ok(())
}
