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
//! Temporal validation (E701, E704) over transcripts the PARSER built.
//!
//! # What this replaces
//!
//! `talkbank-model/tests/temporal_validation_tests.rs` built each transcript
//! by hand: a `MainTier` struct literal per utterance with `Span::DUMMY` in
//! three fields, a `Bullet::new(start, end)` beside a `Word::simple("foo")`,
//! and an `Utterance` literal around it with every computed field set to its
//! uncomputed variant. Nine `Span::DUMMY` sentinels per file, and a transcript
//! no CHAT text ever described. Each case is a row here: the utterance lines
//! as written, bullets included, under a declared `@Media`, and the code set
//! the whole transcript reports.
//!
//! # What the move found that the originals could not
//!
//! A parsed transcript runs every rule. Where a speaker's second bullet
//! starts before an earlier one, E362 (a bullet timestamp before the previous
//! one) fires beside the temporal code the original asserted, and a
//! same-speaker step backwards of 500 ms is E704 as well as E701, since the
//! validator measures the overlap from the previous utterance's end (2,000
//! ms) to the new start (500 ms): 1,500 ms, past the tolerance. Each row states the whole set, measured through
//! `chatter validate` before it was written. The originals asserted only
//! that one code was present or absent.
//!
//! # Policy rows
//!
//! The two boundary rows (exactly 500 ms of overlap tolerated, 501 ms
//! reported) survive on purpose: which side of the tolerance is accepted is a
//! choice with a real alternative (CLAN Error 133 parity), not an invariant a
//! type could refuse, and `temporal.rs` reads `overlap > TOLERANCE`, so
//! flipping the operator changes behaviour at exactly one value.

use talkbank_parser_tests::from_source::{Rules, diagnostics_of};
use talkbank_parser_tests::test_error::TestError;

/// A bullet `•start_end•` after `text`, on `speaker`'s line.
struct Turn {
    speaker: &'static str,
    text: &'static str,
    bullet: Option<(u64, u64)>,
}

const fn timed(speaker: &'static str, start: u64, end: u64) -> Turn {
    Turn {
        speaker,
        text: "hello .",
        bullet: Some((start, end)),
    }
}

/// A transcript and the codes it reports.
struct Row {
    why: &'static str,
    /// Whether the transcript declares `@Media`; every timed row does, and
    /// the two no-bullet rows differ only in this.
    media: bool,
    turns: &'static [Turn],
    codes: &'static [&'static str],
}

const ROWS: &[Row] = &[
    Row {
        media: true,
        why: "a speaker whose later utterance starts before an earlier one of theirs is E701, and E704 for the overlap, and E362 for the bullet that steps back",
        turns: &[
            timed("CHI", 1000, 2000),
            timed("MOT", 3000, 4000),
            timed("CHI", 500, 1500),
        ],
        codes: &["E362", "E701", "E704"],
    },
    Row {
        media: true,
        why: "another speaker starting earlier is conversational overlap, not E701; the bullet stepping back is still E362",
        turns: &[timed("CHI", 1000, 2000), timed("MOT", 500, 1500)],
        codes: &["E362"],
    },
    Row {
        media: true,
        why: "monotonic start times across speakers report nothing",
        turns: &[timed("CHI", 1000, 2000), timed("MOT", 2000, 3000)],
        codes: &[],
    },
    Row {
        media: true,
        why: "a speaker overlapping themself by 1,000 ms is E704",
        turns: &[timed("CHI", 1000, 3000), timed("CHI", 2000, 4000)],
        codes: &["E704"],
    },
    Row {
        media: true,
        why: "policy: an overlap of exactly the 500 ms tolerance is accepted",
        turns: &[timed("CHI", 1000, 2000), timed("CHI", 1500, 2500)],
        codes: &[],
    },
    Row {
        media: true,
        why: "policy: one millisecond past the tolerance is E704",
        turns: &[timed("CHI", 1000, 2000), timed("CHI", 1499, 2499)],
        codes: &["E704"],
    },
    Row {
        media: true,
        why: "an overlap within the tolerance is accepted",
        turns: &[timed("CHI", 1000, 2000), timed("CHI", 1600, 2600)],
        codes: &[],
    },
    Row {
        media: true,
        why: "different speakers may overlap",
        turns: &[timed("CHI", 1000, 3000), timed("MOT", 2000, 4000)],
        codes: &[],
    },
    Row {
        media: true,
        why: "untranscribed-only `www` turns are ignored by E704, as CHECK ignores them",
        turns: &[
            Turn {
                speaker: "INV",
                text: "www .",
                bullet: Some((355_600, 653_182)),
            },
            Turn {
                speaker: "INV",
                text: "www .",
                bullet: Some((562_690, 729_500)),
            },
        ],
        codes: &[],
    },
    Row {
        media: false,
        why: "no bullets and no media: no temporal codes, nothing at all",
        turns: &[Turn {
            speaker: "CHI",
            text: "hello .",
            bullet: None,
        }],
        codes: &[],
    },
    Row {
        media: true,
        why: "no bullets under a declared media: no temporal codes, and E544 for the media with no timing",
        turns: &[Turn {
            speaker: "CHI",
            text: "hello .",
            bullet: None,
        }],
        codes: &["E544"],
    },
];

/// The row's transcript: three participants, media declared, one line per
/// turn.
fn source(row: &Row) -> String {
    let mut source = String::from(
        "@UTF8\n@Begin\n@Languages:\teng\n\
         @Participants:\tCHI Target_Child, MOT Mother, INV Investigator\n\
         @ID:\teng|corpus|CHI|||||Target_Child|||\n\
         @ID:\teng|corpus|MOT|||||Mother|||\n\
         @ID:\teng|corpus|INV|||||Investigator|||\n",
    );
    if row.media {
        source.push_str("@Media:\tcorpus, audio\n");
    }
    for turn in row.turns {
        source.push('*');
        source.push_str(turn.speaker);
        source.push_str(":\t");
        source.push_str(turn.text);
        if let Some((start, end)) = turn.bullet {
            source.push_str(&format!(" \u{15}{start}_{end}\u{15}"));
        }
        source.push('\n');
    }
    source.push_str("@End\n");
    source
}

#[test]
fn every_transcript_reports_the_codes_its_row_says() -> Result<(), TestError> {
    for row in ROWS {
        let reported = diagnostics_of(&source(row), Rules::Default)?;
        let mut got: Vec<&str> = reported.iter().map(|e| e.code.as_str()).collect();
        got.sort_unstable();
        got.dedup();
        let mut want: Vec<&str> = row.codes.to_vec();
        want.sort_unstable();
        if got != want {
            return Err(TestError::Failure(format!(
                "{}: reported {got:?}, the row says {want:?}: {}",
                row.why,
                reported
                    .iter()
                    .map(|e| format!("{} {}", e.code.as_str(), e.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
    }
    Ok(())
}
