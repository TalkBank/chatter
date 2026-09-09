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

//! Long-feature (`&{l=` / `&}l=`) and nonvocal (`&{n=` / `&}n=`) scope
//! balance across utterances, over transcripts the parser built.
//!
//! # What this replaces
//!
//! The inline tests of `talkbank-model`'s
//! `validation/cross_utterance/scoped_markers.rs` built each utterance from
//! `UtteranceContent::LongFeatureBegin(LongFeatureBegin::new("singing"))`
//! beside a `Terminator::Period { span: Span::DUMMY }`, and a bare `Group` for
//! the nested cases, which no parse produces. Here each row is the utterance
//! sequence as written; the nested rows use the two parser-producible angle
//! shapes, a retrace (`<...> [/]`) and an annotated group (`<...> [!]`).
//!
//! # What each row asserts
//!
//! What the originals did, over the whole file: the exact set of codes, each
//! exactly once, with the label the message must name. The rules are on by
//! default, so the rows run under [`Rules::Default`].

use talkbank_parser_tests::from_source::{Dialogue, Fired, Rules, fired_mismatches};
use talkbank_parser_tests::test_error::TestError;

struct Row {
    what: &'static str,
    lines: &'static [&'static str],
    fired: &'static [Fired],
}

const ROWS: &[Row] = &[
    // ── long features ─────────────────────────────────────────────────
    Row {
        what: "E358: a long-feature begin with no end, naming the label",
        lines: &["I am &{l=singing happy ."],
        fired: &[Fired {
            code: "E358",
            message: "singing",
        }],
    },
    Row {
        what: "E359: a long-feature end with no begin, naming the label",
        lines: &["happy &}l=singing ."],
        fired: &[Fired {
            code: "E359",
            message: "singing",
        }],
    },
    Row {
        what: "a begin inside a retrace closes a later top-level end (the E359 regression on valid CHAT)",
        lines: &["<the &{l=soft dog> [/] the dog &}l=soft ."],
        fired: &[],
    },
    Row {
        what: "a begin inside an annotated group closes a later top-level end",
        lines: &["<the &{l=soft dog> [!] the dog &}l=soft ."],
        fired: &[],
    },
    Row {
        what: "an unmatched end inside a group is still reported",
        lines: &["<the dog &}l=soft> [!] ."],
        fired: &[Fired {
            code: "E359",
            message: "soft",
        }],
    },
    Row {
        what: "differing long-feature labels report both sides",
        lines: &["&{l=singing hi .", "hi &}l=whisper ."],
        fired: &[
            Fired {
                code: "E358",
                message: "singing",
            },
            Fired {
                code: "E359",
                message: "whisper",
            },
        ],
    },
    Row {
        what: "a balanced long feature reports nothing",
        lines: &["&{l=singing hi &}l=singing ."],
        fired: &[],
    },
    // ── nonvocals ─────────────────────────────────────────────────────
    Row {
        what: "a nonvocal begin inside a group closes a later top-level end",
        lines: &["<the &{n=THUMP dog> [!] the dog &}n=THUMP ."],
        fired: &[],
    },
    Row {
        what: "a self-closing nonvocal (`&{n=BANG}`) opens no scope, even inside a container that descent now reaches",
        lines: &["<the &{n=BANG} dog> [!] the dog ."],
        fired: &[],
    },
    Row {
        what: "E367: a nonvocal begin with no end, naming the label",
        lines: &["&{n=crying hi ."],
        fired: &[Fired {
            code: "E367",
            message: "crying",
        }],
    },
    Row {
        what: "E368: a nonvocal end with no begin, naming the label",
        lines: &["hi &}n=crying ."],
        fired: &[Fired {
            code: "E368",
            message: "crying",
        }],
    },
    Row {
        what: "differing nonvocal labels report both sides",
        lines: &["&{n=crying hi .", "hi &}n=laughing ."],
        fired: &[
            Fired {
                code: "E367",
                message: "crying",
            },
            Fired {
                code: "E368",
                message: "laughing",
            },
        ],
    },
    Row {
        what: "a balanced nonvocal reports nothing",
        lines: &["&{n=crying hi &}n=crying ."],
        fired: &[],
    },
];

/// Every row's verdict holds over the transcript the parser builds from it.
#[test]
fn every_scope_balance_rule_holds_over_a_parsed_transcript() -> Result<(), TestError> {
    let mut wrong = Vec::new();
    for row in ROWS {
        let lines: Vec<(&str, &str)> = row.lines.iter().map(|body| ("CHI", *body)).collect();
        let reported = match (Dialogue { lines: &lines }).diagnostics(Rules::Default) {
            Ok(reported) => reported,
            Err(err) => {
                wrong.push(format!("{}: {err}", row.what));
                continue;
            }
        };
        wrong.extend(fired_mismatches(row.what, &reported, row.fired));
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(TestError::Failure(wrong.join("\n")))
    }
}
