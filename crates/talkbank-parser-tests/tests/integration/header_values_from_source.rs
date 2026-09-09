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

//! Header VALUE rules (unknown header, unsupported option, media type and
//! status, number, recording quality, transcription, time duration and
//! start), over headers the parser built.
//!
//! # What this replaces
//!
//! Nine tests in `talkbank-model`'s `validation/header/mod.rs` built a
//! `Header` value by hand (`Header::Options { options: ChatOptionFlags::new(
//! vec![ChatOptionFlag::Unsupported("NewThing".to_string())]) }`) and called
//! `check_header` on it at `Span::DUMMY`; the variant stood in for the text
//! `@Options:\tCA, NewThing`, and nothing checked that the text builds the
//! variant. Four more in `validation/header/metadata.rs` called the leaf
//! time-format validators on a bare string (`check_time_duration_format(
//! "anything", Span::DUMMY, ..)`), never reaching the header that carries
//! the string. Here each row is the header line as written.
//!
//! Two tests stay there: they build a `SpeakerCode` containing `:`, which no
//! parse produces (`@Participants:\tCH:I Child` is E320 and E316 at parse),
//! so the invalid-character check they exercise is reachable only from a
//! value built by hand.
//!
//! # `@Media` rows carry a status
//!
//! A bare `@Media` line beside an utterance with no bullets is E544, which is
//! about timing evidence, not the value under test; a status (`unlinked`, or
//! `missing` beside the `missing` type, the one pairing that is coherent for
//! an absent file) says so. E531 (the filename must match the transcript's own
//! name) is outside the verdict's scope, which validates an unnamed
//! transcript; the originals could not reach it either.

use talkbank_model::model::{
    Header, Line, MediaStatus, MediaType, Number, RecordingQuality, Sex, Transcription, WriteChat,
};
use talkbank_parser_tests::from_source::{
    Fired, Rules, diagnostics_of, fired_mismatches, parsed_document,
};
use talkbank_parser_tests::test_error::TestError;

struct Row {
    what: &'static str,
    /// The header line under test; placed after `@ID` unless it is
    /// `@Options`, whose one legal position is before it.
    header: &'static str,
    /// Every code the file must report, each exactly once; empty means the
    /// header is valid.
    fired: &'static [Fired],
}

const ROWS: &[Row] = &[
    Row {
        what: "an unknown header is exactly one E525, naming it",
        header: "@Unknown:\tsomething",
        fired: &[Fired {
            code: "E525",
            message: "Unknown or malformed header",
        }],
    },
    Row {
        what: "an unsupported @Options value is E534",
        header: "@Options:\tCA, NewThing",
        fired: &[Fired {
            code: "E534",
            message: "NewThing",
        }],
    },
    Row {
        what: "the two structured options together are fine",
        header: "@Options:\tCA, NoAlign",
        fired: &[],
    },
    Row {
        what: "an unsupported @Media type is E535",
        header: "@Media:\ttest, hologram, unlinked",
        fired: &[Fired {
            code: "E535",
            message: "hologram",
        }],
    },
    Row {
        what: "an unsupported @Media status is E536",
        header: "@Media:\ttest, audio, archived",
        fired: &[Fired {
            code: "E536",
            message: "archived",
        }],
    },
    Row {
        what: "an unsupported @Number value is E537",
        header: "@Number:\tseventeen",
        fired: &[Fired {
            code: "E537",
            message: "seventeen",
        }],
    },
    Row {
        what: "an unsupported @Recording Quality value is E538",
        header: "@Recording Quality:\texcellent",
        fired: &[Fired {
            code: "E538",
            message: "excellent",
        }],
    },
    Row {
        what: "an unsupported @Transcription value is E539",
        header: "@Transcription:\tsloppy",
        fired: &[Fired {
            code: "E539",
            message: "sloppy",
        }],
    },
    Row {
        what: "an unrecognised @Time Duration value is exactly one E540",
        header: "@Time Duration:\tanything",
        fired: &[Fired {
            code: "E540",
            message: "anything",
        }],
    },
    Row {
        what: "an empty @Time Duration value is not reported",
        header: "@Time Duration:\t",
        fired: &[],
    },
    Row {
        what: "an unrecognised @Time Start value is exactly one E541",
        header: "@Time Start:\tanything",
        fired: &[Fired {
            code: "E541",
            message: "anything",
        }],
    },
    Row {
        what: "an empty @Time Start value is not reported",
        header: "@Time Start:\t",
        fired: &[],
    },
    Row {
        what: "a known media type and status are fine",
        header: "@Media:\ttest, audio, unlinked",
        fired: &[],
    },
];

/// The whole file around one header line.
fn document(header: &str) -> String {
    let (before_id, after_id) = if header.starts_with("@Options:") {
        (format!("{header}\n"), String::new())
    } else {
        (String::new(), format!("{header}\n"))
    };
    format!(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
         {before_id}@ID:\teng|corpus|CHI|||||Target_Child|||\n{after_id}*CHI:\thello .\n@End\n"
    )
}

/// Every row's verdict holds over the header the parser builds from it.
#[test]
fn every_header_value_rule_holds_over_a_parsed_header() -> Result<(), TestError> {
    let mut wrong = Vec::new();
    for row in ROWS {
        let reported = match diagnostics_of(&document(row.header), Rules::Default) {
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

/// One value of a closed header vocabulary: the header line as written and
/// the variant the model must hold for it.
struct Vocabulary {
    header: &'static str,
    /// `true` when `header` is the variant this row is about.
    holds: fn(&Header) -> bool,
}

/// Every value of every closed header vocabulary, one row each. The
/// `Unsupported` arms are the E53x rows above; these are the legal values,
/// which no example exercised beyond the first of each set until
/// 2026-09-08 (a whole-workspace coverage run showed `2`..`5`, `partial`
/// through `anonymized` and `more`/`audience` unreached).
const VOCABULARY: &[Vocabulary] = &[
    Vocabulary {
        header: "@Recording Quality:\t1",
        holds: |h| {
            matches!(
                h,
                Header::RecordingQuality {
                    quality: RecordingQuality::Quality1
                }
            )
        },
    },
    Vocabulary {
        header: "@Recording Quality:\t2",
        holds: |h| {
            matches!(
                h,
                Header::RecordingQuality {
                    quality: RecordingQuality::Quality2
                }
            )
        },
    },
    Vocabulary {
        header: "@Recording Quality:\t3",
        holds: |h| {
            matches!(
                h,
                Header::RecordingQuality {
                    quality: RecordingQuality::Quality3
                }
            )
        },
    },
    Vocabulary {
        header: "@Recording Quality:\t4",
        holds: |h| {
            matches!(
                h,
                Header::RecordingQuality {
                    quality: RecordingQuality::Quality4
                }
            )
        },
    },
    Vocabulary {
        header: "@Recording Quality:\t5",
        holds: |h| {
            matches!(
                h,
                Header::RecordingQuality {
                    quality: RecordingQuality::Quality5
                }
            )
        },
    },
    Vocabulary {
        header: "@Transcription:\teye_dialect",
        holds: |h| {
            matches!(
                h,
                Header::Transcription {
                    transcription: Transcription::EyeDialect
                }
            )
        },
    },
    Vocabulary {
        header: "@Transcription:\tpartial",
        holds: |h| {
            matches!(
                h,
                Header::Transcription {
                    transcription: Transcription::Partial
                }
            )
        },
    },
    Vocabulary {
        header: "@Transcription:\tfull",
        holds: |h| {
            matches!(
                h,
                Header::Transcription {
                    transcription: Transcription::Full
                }
            )
        },
    },
    Vocabulary {
        header: "@Transcription:\tdetailed",
        holds: |h| {
            matches!(
                h,
                Header::Transcription {
                    transcription: Transcription::Detailed
                }
            )
        },
    },
    Vocabulary {
        header: "@Transcription:\tcoarse",
        holds: |h| {
            matches!(
                h,
                Header::Transcription {
                    transcription: Transcription::Coarse
                }
            )
        },
    },
    Vocabulary {
        header: "@Transcription:\tchecked",
        holds: |h| {
            matches!(
                h,
                Header::Transcription {
                    transcription: Transcription::Checked
                }
            )
        },
    },
    Vocabulary {
        header: "@Transcription:\tanonymized",
        holds: |h| {
            matches!(
                h,
                Header::Transcription {
                    transcription: Transcription::Anonymized
                }
            )
        },
    },
    Vocabulary {
        header: "@Number:\t1",
        holds: |h| {
            matches!(
                h,
                Header::Number {
                    number: Number::Number1
                }
            )
        },
    },
    Vocabulary {
        header: "@Number:\t2",
        holds: |h| {
            matches!(
                h,
                Header::Number {
                    number: Number::Number2
                }
            )
        },
    },
    Vocabulary {
        header: "@Number:\t3",
        holds: |h| {
            matches!(
                h,
                Header::Number {
                    number: Number::Number3
                }
            )
        },
    },
    Vocabulary {
        header: "@Number:\t4",
        holds: |h| {
            matches!(
                h,
                Header::Number {
                    number: Number::Number4
                }
            )
        },
    },
    Vocabulary {
        header: "@Number:\t5",
        holds: |h| {
            matches!(
                h,
                Header::Number {
                    number: Number::Number5
                }
            )
        },
    },
    Vocabulary {
        header: "@Number:\tmore",
        holds: |h| {
            matches!(
                h,
                Header::Number {
                    number: Number::More
                }
            )
        },
    },
    Vocabulary {
        header: "@Number:\taudience",
        holds: |h| {
            matches!(
                h,
                Header::Number {
                    number: Number::Audience
                }
            )
        },
    },
    Vocabulary {
        header: "@Media:\ttest, audio, unlinked",
        holds: |h| matches!(h, Header::Media(m) if m.media_type == MediaType::Audio && m.status == Some(MediaStatus::Unlinked)),
    },
    Vocabulary {
        header: "@Media:\ttest, video, unlinked",
        holds: |h| matches!(h, Header::Media(m) if m.media_type == MediaType::Video && m.status == Some(MediaStatus::Unlinked)),
    },
    Vocabulary {
        header: "@Media:\ttest, missing, missing",
        holds: |h| matches!(h, Header::Media(m) if m.media_type == MediaType::Missing && m.status == Some(MediaStatus::Missing)),
    },
    Vocabulary {
        header: "@Media:\ttest, audio, missing",
        holds: |h| matches!(h, Header::Media(m) if m.media_type == MediaType::Audio && m.status == Some(MediaStatus::Missing)),
    },
    Vocabulary {
        header: "@Media:\ttest, audio, notrans",
        holds: |h| matches!(h, Header::Media(m) if m.media_type == MediaType::Audio && m.status == Some(MediaStatus::Notrans)),
    },
    Vocabulary {
        header: "@ID:\teng|corpus|CHI||male|||Target_Child|||",
        holds: |h| matches!(h, Header::ID(id) if id.sex == Some(Sex::Male)),
    },
    Vocabulary {
        header: "@ID:\teng|corpus|CHI||female|||Target_Child|||",
        holds: |h| matches!(h, Header::ID(id) if id.sex == Some(Sex::Female)),
    },
];

/// The whole file around one header line, where an `@ID` row REPLACES the
/// skeleton's own `@ID` rather than adding a second one.
fn vocabulary_document(header: &str) -> String {
    if header.starts_with("@ID:") {
        format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
             {header}\n*CHI:\thello .\n@End\n"
        )
    } else {
        document(header)
    }
}

/// Every legal value parses to its variant, validates clean, and writes
/// back byte for byte, which is where `as_str` is exercised.
#[test]
fn every_closed_vocabulary_value_builds_its_variant_and_writes_back() -> Result<(), TestError> {
    let mut wrong = Vec::new();
    for row in VOCABULARY {
        let source = vocabulary_document(row.header);
        let file = match parsed_document(&source, &[], Rules::Default) {
            Ok(file) => file,
            Err(err) => {
                wrong.push(format!("{}: {err}", row.header));
                continue;
            }
        };
        let held = file.lines.iter().any(|line| match line {
            Line::Header { header, .. } => (row.holds)(header),
            _ => false,
        });
        if !held {
            wrong.push(format!(
                "{}: no header holds the expected variant",
                row.header
            ));
        }
        let written = file.to_chat_string();
        if written != source {
            wrong.push(format!("{}: writes back as {written:?}", row.header));
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(TestError::Failure(wrong.join("\n")))
    }
}
