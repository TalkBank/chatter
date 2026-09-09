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

//! The opt-in cross-utterance linker rules, over dialogues the parser built.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `validation/cross_utterance/tests/` (six files and a
//! helper) built each utterance from a speaker, a word list, a linker and a
//! `Terminator::QuotedNewLine { span: Span::DUMMY }`, then called the
//! cross-utterance checker on the sequence. The terminator variant stood in
//! for the text `+"/.`, and nothing checked that the text builds the variant.
//! Here each row is the dialogue as written: speaker and line, in order.
//!
//! # What each row asserts
//!
//! What the originals did, over the whole file: the EXACT set of codes the
//! file reports under `--strict-linkers` (a valid dialogue reports nothing at
//! all, which is stronger than "the checker returned an empty list"), each
//! expected code exactly once, and the message fragment the original pinned.
//! The rules are opt-in, so the rows run under [`Rules::StrictLinkers`]; that
//! is the same switch the deleted helper flipped.
//!
//! # Speakers
//!
//! The originals used `HEL`/`WIN` in two rows; the fixture offers `CHI`,
//! `MOT` and `EXP`, and only same-speaker versus different-speaker matters to
//! these rules, so those rows use `CHI`/`MOT`.

use talkbank_parser_tests::from_source::{Dialogue, Fired, Rules, fired_mismatches};
use talkbank_parser_tests::test_error::TestError;

struct Row {
    what: &'static str,
    lines: &'static [(&'static str, &'static str)],
    /// Every code the file must report, each exactly once; empty means the
    /// dialogue is valid.
    fired: &'static [Fired],
}

const ROWS: &[Row] = &[
    // ── self-completion, `+,` after `+/.` ─────────────────────────────
    Row {
        what: "self-completion: an interrupted utterance and the same speaker's `+,` continuation pair cleanly",
        lines: &[
            ("CHI", "so after the tower +/."),
            ("EXP", "yeah ."),
            ("CHI", "+, I go straight ahead ."),
        ],
        fired: &[],
    },
    Row {
        what: "E351: `+,` with no preceding utterance from that speaker",
        lines: &[("CHI", "+, I go ahead .")],
        fired: &[Fired {
            code: "E351",
            message: "without any preceding utterance",
        }],
    },
    Row {
        what: "E352: `+,` whose preceding same-speaker utterance does not end with `+/.`",
        lines: &[
            ("CHI", "so after the tower ."),
            ("EXP", "yeah ."),
            ("CHI", "+, I go ahead ."),
        ],
        fired: &[Fired {
            code: "E352",
            message: "doesn't end with +/. ",
        }],
    },
    // ── other-completion, `++` after `+...` ───────────────────────────
    Row {
        what: "other-completion: a trailed-off utterance and another speaker's `++` pair cleanly",
        lines: &[
            ("CHI", "if Bill had known +..."),
            ("MOT", "++ he would have come ."),
        ],
        fired: &[],
    },
    Row {
        what: "E353: `++` with no preceding utterance at all",
        lines: &[("MOT", "++ he would have come .")],
        fired: &[Fired {
            code: "E353",
            message: "without any preceding utterance from different speaker",
        }],
    },
    Row {
        what: "E354: `++` whose preceding different-speaker utterance does not end with `+...`",
        lines: &[
            ("CHI", "if Bill had known ."),
            ("MOT", "++ he would have come ."),
        ],
        fired: &[Fired {
            code: "E354",
            message: "doesn't end with +...",
        }],
    },
    Row {
        what: "E355: `++` after the SAME speaker, which is what `+,` is for",
        lines: &[
            ("CHI", "if Bill had known +..."),
            ("CHI", "++ he would have come ."),
        ],
        fired: &[Fired {
            code: "E355",
            message: "same speaker",
        }],
    },
    // ── quotation follows, `+"/.` then `+"` ───────────────────────────
    Row {
        what: "quotation follows: `+\"/.` then the same speaker's `+\"` line",
        lines: &[
            ("CHI", "the bear said +\"/."),
            ("CHI", "+\" please give me honey ."),
        ],
        fired: &[],
    },
    Row {
        what: "quotation follows: several `+\"` lines after one `+\"/.`",
        lines: &[
            ("CHI", "the bear said +\"/."),
            ("CHI", "+\" please give me honey ."),
            ("CHI", "+\" I'll carry you ."),
        ],
        fired: &[],
    },
    Row {
        what: "E341: `+\"/.` with no later utterance from that speaker at all",
        lines: &[("CHI", "the bear said +\"/."), ("MOT", "what happened ?")],
        fired: &[Fired {
            code: "E341",
            message: "not followed by any subsequent utterance",
        }],
    },
    Row {
        what: "E341: `+\"/.` followed by the same speaker WITHOUT the `+\"` linker",
        lines: &[
            ("CHI", "the bear said +\"/."),
            ("CHI", "please give me honey ."),
        ],
        fired: &[Fired {
            code: "E341",
            message: "not followed by quoted utterance",
        }],
    },
    Row {
        what: "E341: `+\"/.` and `+\".` mixed in one sequence",
        lines: &[
            ("CHI", "the bear said +\"/."),
            ("CHI", "+\" give me honey ."),
            ("CHI", "+\" I'll carry you +\"."),
        ],
        fired: &[Fired {
            code: "E341",
            message: "Mixed quotation patterns",
        }],
    },
    // ── quotation precedes, `+"` then `+".` ───────────────────────────
    Row {
        what: "quotation precedes: a `+\"` line then the same speaker's `+\".`",
        lines: &[
            ("CHI", "+\" please give me honey ."),
            ("CHI", "the bear said +\"."),
        ],
        fired: &[],
    },
    Row {
        what: "quotation precedes: several `+\"` lines before one `+\".`",
        lines: &[
            ("CHI", "+\" please give me honey ."),
            ("CHI", "+\" I'll carry you ."),
            ("CHI", "the bear said +\"."),
        ],
        fired: &[],
    },
    Row {
        what: "E344: `+\".` with no preceding `+\"` line from that speaker (two originals, one shape)",
        lines: &[("CHI", "the bear said +\".")],
        fired: &[Fired {
            code: "E344",
            message: "without preceding quoted utterances",
        }],
    },
    Row {
        what: "E346: a `+\"` line whose speaker never closes it with `+\".`",
        lines: &[
            ("CHI", "+\" please give me honey ."),
            ("MOT", "nice story ."),
        ],
        fired: &[Fired {
            code: "E346",
            message: "missing required terminator",
        }],
    },
    Row {
        what: "E346: a `+\"` line ending in a plain period, alone",
        lines: &[("CHI", "+\" hello there .")],
        fired: &[Fired {
            code: "E346",
            message: "missing required terminator",
        }],
    },
    // ── edge cases ────────────────────────────────────────────────────
    Row {
        what: "an intervening speaker between `+\"/.` and its `+\"` line is fine",
        lines: &[
            ("CHI", "the bear said +\"/."),
            ("MOT", "uh huh ."),
            ("CHI", "+\" please give me honey ."),
        ],
        fired: &[],
    },
    Row {
        what: "two independent faults in one sequence produce two diagnostics; neither masks the other",
        lines: &[
            ("CHI", "she said +\"/."),
            ("MOT", "okay ."),
            ("EXP", "+, continue ."),
        ],
        fired: &[
            Fired {
                code: "E341",
                message: "not followed by any subsequent utterance",
            },
            Fired {
                code: "E351",
                message: "without any preceding utterance",
            },
        ],
    },
];

/// Every row's verdict holds over the dialogue the parser builds from it.
///
/// SURVIVES a type change, and says which category: this is behaviour a
/// signature cannot describe, the pairing of a linker on one line with a
/// terminator on an earlier or later one, across speakers.
#[test]
fn every_linker_rule_holds_over_a_parsed_dialogue() -> Result<(), TestError> {
    let mut wrong = Vec::new();
    for row in ROWS {
        let reported = match (Dialogue { lines: row.lines }).diagnostics(Rules::StrictLinkers) {
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
