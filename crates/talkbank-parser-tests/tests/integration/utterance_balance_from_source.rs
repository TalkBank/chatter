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

//! Balance rules inside one utterance, over utterances the parser built.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `validation/utterance/tests.rs` built `Utterance` values
//! by hand and called `check_quotation_balance`, `check_ca_delimiter_balance`,
//! `check_overlap_index_values` and `check_underline_balance` directly. That is
//! the input stated twice, and the coverage it produced was fabrication-backed:
//! the check ran and no CHAT text was read.
//!
//! # Why it moved crates, stated accurately
//!
//! Not because a test in `talkbank-model` cannot parse. Cargo permits a
//! dev-dependency cycle, an integration test under `talkbank-model/tests/`
//! links against the non-test lib, and this workspace already uses the pattern
//! on purpose (`talkbank-derive` dev-depends on `talkbank-model`). What cannot
//! parse is a `#[cfg(test)] mod` inside `src/`, where these tests were: the
//! lib-test target is a second instantiation of the crate, so the parser's
//! `Utterance` and the test's are different types. This crate is the cheaper
//! home: it exists for parse-backed tests and already depends on every parser.
//!
//! # Two rules share E242, and only one of them is the validator's
//!
//! `check_quotation_balance` reads the utterance's POSTCODES, matching `"/`
//! and `"/.` exactly; curly quotes in the text are a different producer
//! entirely, a scan in the parser's error analysis, and the spec examples for
//! E242 exercise that one. The first draft of this table replaced the
//! postcode tests with curly-quote rows and so deleted the validator rule's
//! only coverage while the code kept passing. The postcode rows below are
//! that coverage, restored with the messages the originals asserted.
//!
//! # What each row asserts
//!
//! The predecessor asserted, per violating case, exactly ONE error, its code
//! and a fragment of its message; each `Violates` row carries all three, and
//! nothing else in this table's rule family may fire. A `Legal` row asserts
//! the code is absent, and every row is refused if the FIXTURE itself failed
//! to parse, because a rule that never ran is silent for the wrong reason.

use talkbank_model::ErrorCollector;
use talkbank_model::model::TranscriptName;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// One code the row expects, exactly once, with a fragment of its message.
struct Fired {
    code: &'static str,
    message: &'static str,
}

/// What a row asserts about its rule.
enum Expect {
    /// Every listed code fires exactly once with its fragment, and no other
    /// code from this table's family fires.
    Violates(&'static [Fired]),
    /// The named code must not fire.
    Legal(&'static str),
}

/// One utterance body and what validation must say about it.
struct Row {
    what: &'static str,
    /// The main-tier content after `*CHI:` and a tab, INCLUDING the
    /// terminator and any postcode.
    utterance: &'static str,
    expect: Expect,
}

/// Codes meaning the fixture is broken rather than the rule violated.
const FIXTURE_FAULTS: &[&str] = &["E316", "E342", "E325", "E525", "E502", "E503", "E504"];

/// Every code this table is about, so `Violates` can assert exclusivity.
const FAMILY: &[&str] = &["E230", "E242", "E356", "E357", "E373"];

const ROWS: &[Row] = &[
    // ── Quotation balance (E242), the VALIDATOR's rule: postcodes ────
    Row {
        what: "a quotation-begin postcode never closed",
        utterance: "hello . [+ \"/]",
        expect: Expect::Violates(&[Fired {
            code: "E242",
            message: "unclosed quotation begin",
        }]),
    },
    Row {
        what: "a quotation-end postcode with no begin",
        utterance: "hello . [+ \"/.]",
        expect: Expect::Violates(&[Fired {
            code: "E242",
            message: "without corresponding begin",
        }]),
    },
    Row {
        what: "a balanced quotation postcode pair",
        utterance: "hello . [+ \"/] [+ \"/.]",
        expect: Expect::Legal("E242"),
    },
    Row {
        // The predecessor's control carried a NON-quotation postcode, so it
        // also pinned that the rule ignores postcodes that are not markers.
        what: "a postcode that is not a quotation marker",
        utterance: "hello . [+ bch]",
        expect: Expect::Legal("E242"),
    },
    Row {
        // Near miss, from the predecessor: `"/.` followed by more text is not
        // the close marker and must not be counted as one.
        what: "a begin whose would-be close is a near miss",
        utterance: "hello . [+ \"/] [+ \"/. x]",
        expect: Expect::Violates(&[Fired {
            code: "E242",
            message: "unclosed quotation begin",
        }]),
    },
    // ── CA delimiter balance (E230) ──────────────────────────────────
    Row {
        what: "a CA delimiter with no partner",
        utterance: "hello\u{2206}world .",
        expect: Expect::Violates(&[Fired {
            code: "E230",
            message: "\u{2206}",
        }]),
    },
    Row {
        what: "a balanced CA delimiter pair spanning words",
        utterance: "he said \u{2206}hello there\u{2206} today .",
        expect: Expect::Legal("E230"),
    },
    Row {
        // INTERLEAVED, as the predecessor's was: two types open, then both
        // close. Two disjoint pairs prove less than this does.
        what: "two CA delimiter types interleaved, each balanced",
        utterance: "he said \u{2206}\u{2207}a b\u{2206}\u{2207} today .",
        expect: Expect::Legal("E230"),
    },
    Row {
        // The shape the surviving role-analysis test in `talkbank-model` uses:
        // one balanced pair and one unpaired delimiter of another type. Exactly
        // one E230, naming the unpaired one and not the pair.
        what: "a balanced pair beside an unpaired delimiter of another type",
        utterance: "\u{b0}soft more\u{b0} \u{2206}fast .",
        expect: Expect::Violates(&[Fired {
            code: "E230",
            message: "\u{2206}",
        }]),
    },
    // ── Underline balance (E356, E357) ───────────────────────────────
    //
    // The markers are CONTROL CHARACTERS, written as escapes throughout:
    // `\u{2}\u{1}` opens and `\u{2}\u{2}` closes. Literally they would be
    // invisible here and indistinguishable from each other.
    Row {
        what: "an underline that never closes",
        utterance: "hello \u{2}\u{1}world .",
        expect: Expect::Violates(&[Fired {
            code: "E356",
            message: "Unmatched underline begin",
        }]),
    },
    Row {
        what: "an underline end with no begin",
        utterance: "hello \u{2}\u{2}world .",
        expect: Expect::Violates(&[Fired {
            code: "E357",
            message: "Unmatched underline end",
        }]),
    },
    Row {
        what: "a balanced underline",
        utterance: "hello \u{2}\u{1}world\u{2}\u{2} again .",
        expect: Expect::Legal("E356"),
    },
    Row {
        what: "a balanced underline, on the closing rule",
        utterance: "hello \u{2}\u{1}world\u{2}\u{2} again .",
        expect: Expect::Legal("E357"),
    },
    Row {
        // End, begin, end: the predecessor's shape. The first end has nothing
        // open, and the begin IS closed by the second end, so exactly ONE
        // error, E357. A version with the begin left open reports both codes
        // and tests something else.
        what: "an underline end, then a balanced pair",
        utterance: "hello \u{2}\u{2}one \u{2}\u{1}two\u{2}\u{2} .",
        expect: Expect::Violates(&[Fired {
            code: "E357",
            message: "Unmatched underline end",
        }]),
    },
    Row {
        // Begin, begin, end: the predecessor's shape. One begin is closed and
        // one is not, so exactly ONE E356: a partial close still leaves a
        // residue.
        what: "two underlines opened and one closed",
        utterance: "hello \u{2}\u{1}one \u{2}\u{1}two\u{2}\u{2} .",
        expect: Expect::Violates(&[Fired {
            code: "E356",
            message: "unclosed begin marker",
        }]),
    },
    Row {
        // Nested structures obey the same rule: the predecessor assembled a
        // `Group` around a `Word` ending in an `UnderlineEnd`. A retrace group
        // spells the same shape.
        what: "an underline end inside a retrace group",
        utterance: "<x\u{2}\u{2}> [//] hello .",
        expect: Expect::Violates(&[Fired {
            code: "E357",
            message: "Unmatched underline end",
        }]),
    },
    Row {
        what: "a balanced underline inside a retrace group",
        utterance: "<\u{2}\u{1}x\u{2}\u{2}> [//] hello .",
        expect: Expect::Legal("E357"),
    },
    // ── Overlap index range (E373) ───────────────────────────────────
    //
    // The rule reads CA overlap markers (`⌈⌉`) only; a scoped `[<n]` is never
    // collected by the traversal it walks, so a `[<n]` row would be a control
    // that the rule never sees. The control is an IN-RANGE CA index.
    Row {
        what: "a CA overlap index below the legal range",
        utterance: "\u{2308}1 hello \u{2309} .",
        expect: Expect::Violates(&[Fired {
            code: "E373",
            message: "1",
        }]),
    },
    Row {
        what: "a CA overlap index at the top of the legal range",
        utterance: "\u{2308}9 hello \u{2309} .",
        expect: Expect::Legal("E373"),
    },
];

/// Every balance rule holds over an utterance the parser built, with the count
/// and message each predecessor asserted.
///
/// SURVIVES a type change, and says which category: behaviour over real CHAT
/// bytes at the parser-to-validator seam.
#[test]
fn every_balance_rule_holds_over_a_parsed_utterance() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let mut wrong = Vec::new();

    for row in ROWS {
        let source = format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             @ID:\teng|corpus|CHI|||||Target_Child|||\n*CHI:\t{}\n@End\n",
            row.utterance
        );
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let reported = errors.to_vec();
        let codes: Vec<&str> = reported.iter().map(|e| e.code.as_str()).collect();

        if let Some(fault) = codes.iter().find(|code| FIXTURE_FAULTS.contains(code)) {
            wrong.push(format!(
                "{}: the FIXTURE is broken ({fault} fired), so the row says nothing: {codes:?}",
                row.what
            ));
            continue;
        }

        match &row.expect {
            Expect::Legal(code) => {
                if codes.contains(code) {
                    wrong.push(format!(
                        "{}: is legal for {code} and reported it anyway ({codes:?})",
                        row.what
                    ));
                }
            }
            Expect::Violates(fired) => {
                for want in *fired {
                    let matching: Vec<_> = reported
                        .iter()
                        .filter(|e| e.code.as_str() == want.code)
                        .collect();
                    match matching.as_slice() {
                        [] => wrong.push(format!(
                            "{}: expected {} and got {codes:?}",
                            row.what, want.code
                        )),
                        [one] => {
                            if !one.message.contains(want.message) {
                                wrong.push(format!(
                                    "{}: {}'s message does not contain {:?}: {:?}",
                                    row.what, want.code, want.message, one.message
                                ));
                            }
                        }
                        many => wrong.push(format!(
                            "{}: {} fired {} times; the predecessor asserted exactly one",
                            row.what,
                            want.code,
                            many.len()
                        )),
                    }
                }
                let expected: Vec<&str> = fired.iter().map(|f| f.code).collect();
                let extra: Vec<&&str> = codes
                    .iter()
                    .filter(|code| FAMILY.contains(code) && !expected.contains(code))
                    .collect();
                if !extra.is_empty() {
                    wrong.push(format!(
                        "{}: expected only {expected:?} from this family and also got {extra:?}",
                        row.what
                    ));
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "{} of {} balance rows did not hold over a PARSED utterance:\n  {}",
        wrong.len(),
        ROWS.len(),
        wrong.join("\n  ")
    );
    Ok(())
}
