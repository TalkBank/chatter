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

//! Header ORDER and GEM balance, over header blocks the parser built.
//!
//! # What this replaces
//!
//! `talkbank-model`'s `validation/header/structure.rs` tested these rules by
//! handing `check_header_order` a `Vec<(&Header, Span)>` assembled in the test,
//! every position `Span::DUMMY`. Both facts under test are properties of a
//! header BLOCK: which header follows which, and whether a `@Bg` is closed. A
//! `.cha` file states a header block directly, so the hand-assembled sequence
//! was a second way of writing something the format already writes, with
//! nothing forcing the two to describe the same thing.
//!
//! # Why it moved crates, stated accurately
//!
//! Not because a test in `talkbank-model` cannot parse. It can: Cargo permits
//! a dev-dependency cycle, an integration test under `talkbank-model/tests/`
//! links against the non-test lib, and `talkbank-derive` already declares
//! `talkbank-model` as a dev-dependency the same way. What cannot parse is a
//! `#[cfg(test)] mod` inside `src/`, which is where these tests were: the
//! lib-test target is a second instantiation of the crate, so the parser's
//! `Header` and the test's `Header` are different types. The choice here is
//! the cheaper one: `talkbank-parser-tests` exists for parse-backed tests and
//! already depends on every parser, and a dev-dependency on tree-sitter would
//! make the model's own test build wait for the grammar to compile.
//!
//! # What each case asserts, and why it is more than "the code fired"
//!
//! The predecessor asserted three things per violating case: exactly ONE
//! error, its code, and a fragment of its message. The message fragment is not
//! decoration: E543 emits two different sentences under one code, one naming
//! `@Options` and one naming `@ID`, and a test that checks only the code passes
//! after the two arms' messages are swapped. Each case here carries the same
//! fragment, the exactly-once count, and, where the original asserted a
//! residual stack, every code in it.
//!
//! # The fixture itself is checked
//!
//! A `Legal` case is vacuous if its input never reached the rule, and a header
//! block reaches nothing if it does not parse: a typo in `@Bg` makes it an
//! unknown header, and every gem case passes for the wrong reason. So every
//! case, legal or not, is refused if any FIXTURE-FAULT code fires, which is
//! the guard the parse-then-assert shape needs and a hand-built sequence never
//! did.

use talkbank_model::ErrorCollector;
use talkbank_model::model::TranscriptName;
use talkbank_parser::TreeSitterParser;
use talkbank_parser_tests::test_error::TestError;

/// One code the case expects, exactly once, with a fragment of its message.
struct Fired {
    code: &'static str,
    /// A fragment the message must contain: what the predecessor pinned per
    /// case. For E543 it is the header the sentence names, for the gem codes
    /// the label.
    message: &'static str,
}

/// What a case asserts about its rule.
enum Expect {
    /// Every listed code fires exactly once with its fragment, and no other
    /// code from this table's rule family fires at all.
    Violates(&'static [Fired]),
    /// The named code must not fire. Other codes may.
    Legal(&'static str),
}

/// One header block and what validation must say about it.
struct Case {
    what: &'static str,
    /// Header lines between `@Languages` and the body, in order.
    headers: &'static [&'static str],
    /// Lines between the headers and `@End`.
    body: &'static [&'static str],
    expect: Expect,
}

/// Codes that mean the FIXTURE is broken rather than the rule violated.
///
/// Any of these on any case fails it: an unparsable line, a recovery
/// placeholder, an unknown header, a missing required header. A `Legal` case
/// that reported one of these would be passing because its construct never
/// existed.
const FIXTURE_FAULTS: &[&str] = &["E316", "E342", "E325", "E525", "E502", "E503", "E504"];

/// Every code this table is about, so a `Violates` case can assert that
/// nothing ELSE in the family fired: the predecessor's `len() == 1`.
const FAMILY: &[&str] = &[
    "E543", "E547", "E548", "E551", "E526", "E527", "E528", "E529", "E530",
];

const ID: &str = "@ID:\teng|corpus|CHI|||||Target_Child|||";
const PARTICIPANTS: &str = "@Participants:\tCHI Target_Child";
const PREAMBLE: &[&str] = &[PARTICIPANTS, ID];
const UTTERANCE: &[&str] = &["*CHI:\thello ."];

const CASES: &[Case] = &[
    // ── Header order ─────────────────────────────────────────────────
    Case {
        what: "@Options before @Participants",
        headers: &["@Options:\tCA", PARTICIPANTS, ID],
        body: UTTERANCE,
        expect: Expect::Violates(&[Fired {
            code: "E543",
            message: "@Options",
        }]),
    },
    Case {
        // ONE input, TWO rules, and only one may speak. E543 owns this
        // shape; E551 owns "@Options after @ID" and must stay quiet, or the
        // same fault is reported twice under two names. The predecessor had a
        // test with exactly this name.
        what: "@Options before @Participants is not also reported by E551",
        headers: &["@Options:\tCA", PARTICIPANTS, ID],
        body: UTTERANCE,
        expect: Expect::Legal("E551"),
    },
    Case {
        what: "@Options after @Participants",
        headers: &[PARTICIPANTS, "@Options:\tCA", ID],
        body: UTTERANCE,
        expect: Expect::Legal("E543"),
    },
    Case {
        what: "@ID before @Participants",
        headers: &[ID, PARTICIPANTS],
        body: UTTERANCE,
        expect: Expect::Violates(&[Fired {
            code: "E543",
            message: "@ID",
        }]),
    },
    Case {
        what: "@ID immediately after @Participants, on E543",
        headers: PREAMBLE,
        body: UTTERANCE,
        expect: Expect::Legal("E543"),
    },
    Case {
        what: "@ID immediately after @Participants, on E548",
        headers: PREAMBLE,
        body: UTTERANCE,
        expect: Expect::Legal("E548"),
    },
    Case {
        what: "a @Comment between @Participants and @ID",
        headers: &[PARTICIPANTS, "@Comment:\tnote", ID],
        body: UTTERANCE,
        expect: Expect::Violates(&[Fired {
            code: "E548",
            message: "@ID",
        }]),
    },
    Case {
        what: "@Options after @ID",
        headers: &[PARTICIPANTS, ID, "@Options:\tCA"],
        body: UTTERANCE,
        expect: Expect::Violates(&[Fired {
            code: "E551",
            message: "@Options",
        }]),
    },
    Case {
        what: "@Options between @Participants and @ID",
        headers: &[PARTICIPANTS, "@Options:\tCA", ID],
        body: UTTERANCE,
        expect: Expect::Legal("E551"),
    },
    Case {
        what: "a @Comment between @ID and @Birth",
        headers: &[
            PARTICIPANTS,
            ID,
            "@Comment:\tnote",
            "@Birth of CHI:\t01-JAN-2000",
        ],
        body: UTTERANCE,
        expect: Expect::Violates(&[Fired {
            code: "E547",
            message: "@Birth of",
        }]),
    },
    Case {
        what: "@Birth immediately after @ID",
        headers: &[PARTICIPANTS, ID, "@Birth of CHI:\t01-JAN-2000"],
        body: UTTERANCE,
        expect: Expect::Legal("E547"),
    },
    Case {
        what: "two @Birth headers in a row after the @IDs",
        headers: &[
            "@Participants:\tCHI Target_Child, MOT Mother",
            ID,
            "@ID:\teng|corpus|MOT|||||Mother|||",
            "@Birth of CHI:\t15-DEC-1970",
            "@Birth of MOT:\t01-JAN-1945",
        ],
        body: UTTERANCE,
        expect: Expect::Legal("E547"),
    },
    // ── Gem balance ──────────────────────────────────────────────────
    Case {
        what: "a @Bg that is never closed",
        headers: PREAMBLE,
        body: &["@Bg:\tstory", "*CHI:\thello ."],
        expect: Expect::Violates(&[Fired {
            code: "E526",
            message: "story",
        }]),
    },
    Case {
        what: "an @Eg with no @Bg",
        headers: PREAMBLE,
        body: &["*CHI:\thello .", "@Eg:\tstory"],
        expect: Expect::Violates(&[Fired {
            code: "E527",
            message: "story",
        }]),
    },
    Case {
        // The predecessor asserted the whole residual stack: three errors, the
        // mismatch itself, and the two labels on the right sides. So does
        // this.
        what: "an @Eg whose label is not the open one",
        headers: PREAMBLE,
        body: &["@Bg:\tepisode1", "*CHI:\thello .", "@Eg:\tepisode2"],
        expect: Expect::Violates(&[
            Fired {
                code: "E528",
                message: "episode",
            },
            Fired {
                code: "E526",
                message: "episode1",
            },
            Fired {
                code: "E527",
                message: "episode2",
            },
        ]),
    },
    Case {
        what: "a balanced labelled gem, on the opening rule",
        headers: PREAMBLE,
        body: &["@Bg:\tstory", "*CHI:\thello .", "@Eg:\tstory"],
        expect: Expect::Legal("E526"),
    },
    Case {
        what: "a balanced labelled gem, on the closing rule",
        headers: PREAMBLE,
        body: &["@Bg:\tstory", "*CHI:\thello .", "@Eg:\tstory"],
        expect: Expect::Legal("E527"),
    },
    Case {
        what: "a balanced UNLABELLED gem",
        headers: PREAMBLE,
        body: &["@Bg", "*CHI:\thello .", "@Eg"],
        expect: Expect::Legal("E526"),
    },
    Case {
        // Balanced on purpose, as the predecessor's was: with both scopes
        // closed the ONLY fault is the nesting, so the exactly-once count
        // proves E529 fires cleanly rather than alongside an E526 the fixture
        // itself caused.
        what: "a second @Bg with the label already open",
        headers: PREAMBLE,
        body: &[
            "@Bg:\tepisode1",
            "*CHI:\thello .",
            "@Bg:\tepisode1",
            "*CHI:\tbye .",
            "@Eg:\tepisode1",
            "@Eg:\tepisode1",
        ],
        expect: Expect::Violates(&[Fired {
            code: "E529",
            message: "episode1",
        }]),
    },
    Case {
        what: "a second UNLABELLED @Bg inside an open one",
        headers: PREAMBLE,
        body: &["@Bg", "*CHI:\thello .", "@Bg", "*CHI:\tbye .", "@Eg", "@Eg"],
        expect: Expect::Violates(&[Fired {
            code: "E529",
            message: "",
        }]),
    },
    Case {
        what: "a second @Bg with a DIFFERENT label, closed in order",
        headers: PREAMBLE,
        body: &[
            "@Bg:\tstory",
            "*CHI:\thello .",
            "@Bg:\tother",
            "*CHI:\tbye .",
            "@Eg:\tother",
            "@Eg:\tstory",
        ],
        expect: Expect::Legal("E529"),
    },
    Case {
        what: "an unlabelled gem nested inside a labelled one, closed in order",
        headers: PREAMBLE,
        body: &[
            "@Bg:\tepisode1",
            "*CHI:\thello .",
            "@Bg",
            "*CHI:\tbye .",
            "@Eg",
            "@Eg:\tepisode1",
        ],
        expect: Expect::Legal("E529"),
    },
    Case {
        what: "a lazy @G inside an open gem",
        headers: PREAMBLE,
        body: &["@Bg:\tstory", "*CHI:\thello .", "@G:\ttask1", "@Eg:\tstory"],
        expect: Expect::Violates(&[Fired {
            code: "E530",
            message: "task1",
        }]),
    },
    Case {
        what: "a lazy @G carrying its own label, inside an open gem",
        headers: PREAMBLE,
        body: &["@Bg:\tstory", "*CHI:\thello .", "@G:\tinner", "@Eg:\tstory"],
        expect: Expect::Violates(&[Fired {
            code: "E530",
            message: "inner",
        }]),
    },
    Case {
        what: "a lazy @G with no gem open",
        headers: PREAMBLE,
        body: &["@G:\tlazy", "*CHI:\thello ."],
        expect: Expect::Legal("E530"),
    },
    Case {
        what: "a lazy @G after the gem closed",
        headers: PREAMBLE,
        body: &[
            "@Bg:\tstory",
            "*CHI:\thello .",
            "@Eg:\tstory",
            "@G:\tlazy",
            "*CHI:\tbye .",
        ],
        expect: Expect::Legal("E530"),
    },
];

fn document(case: &Case) -> String {
    let mut out = String::from("@UTF8\n@Begin\n@Languages:\teng\n");
    for line in case.headers.iter().chain(case.body) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("@End\n");
    out
}

/// Every header-order and gem rule holds over a header block the parser built,
/// with the count and message each predecessor asserted.
///
/// SURVIVES a type change, and says which category: behaviour over real CHAT
/// bytes at the parser-to-validator seam. No signature relates a sequence of
/// header LINES to the `Vec<(&Header, Span)>` a validator walks.
#[test]
fn every_header_structure_rule_holds_over_a_parsed_block() -> Result<(), TestError> {
    let parser = TreeSitterParser::new().map_err(|err| TestError::ParserInit(err.to_string()))?;
    let mut wrong = Vec::new();

    for case in CASES {
        let source = document(case);
        let errors = ErrorCollector::new();
        let mut file = parser.parse_chat_file_streaming(&source, &errors);
        file.validate_with_alignment(&errors, TranscriptName::Anonymous);
        let reported = errors.to_vec();
        let codes: Vec<&str> = reported.iter().map(|e| e.code.as_str()).collect();

        // The fixture must be sound before its verdict means anything.
        if let Some(fault) = codes.iter().find(|code| FIXTURE_FAULTS.contains(code)) {
            wrong.push(format!(
                "{}: the FIXTURE is broken ({fault} fired), so the case says nothing: {codes:?}",
                case.what
            ));
            continue;
        }

        match &case.expect {
            Expect::Legal(code) => {
                if codes.contains(code) {
                    wrong.push(format!(
                        "{}: is legal for {code} and reported it anyway ({codes:?})",
                        case.what
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
                            case.what, want.code
                        )),
                        [one] => {
                            if !one.message.contains(want.message) {
                                wrong.push(format!(
                                    "{}: {}'s message does not name {:?}: {:?}",
                                    case.what, want.code, want.message, one.message
                                ));
                            }
                        }
                        many => wrong.push(format!(
                            "{}: {} fired {} times; the predecessor asserted exactly one",
                            case.what,
                            want.code,
                            many.len()
                        )),
                    }
                }
                // The predecessor's `len() == 1` (or 3): nothing else in the
                // family may fire.
                let expected: Vec<&str> = fired.iter().map(|f| f.code).collect();
                let extra: Vec<&&str> = codes
                    .iter()
                    .filter(|code| FAMILY.contains(code) && !expected.contains(code))
                    .collect();
                if !extra.is_empty() {
                    wrong.push(format!(
                        "{}: expected only {expected:?} from this family and also got {extra:?}",
                        case.what
                    ));
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "{} of {} header cases did not hold over a PARSED block:\n  {}",
        wrong.len(),
        CASES.len(),
        wrong.join("\n  ")
    );
    Ok(())
}
