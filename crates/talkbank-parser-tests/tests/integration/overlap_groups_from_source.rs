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

//! Cross-speaker overlap grouping (a top overlap and the bottom overlaps
//! that answer it), over dialogues the parser built.
//!
//! # What this replaces
//!
//! The seven inline tests of `talkbank-model`'s
//! `alignment/helpers/overlap_groups.rs` built each utterance from
//! `OverlapPoint::new` items around `Word::new_unchecked` words with speakers
//! `A`, `B`, `C`. Here each case is the dialogue as written, with `CHI`,
//! `MOT`, `EXP` standing in (only same-speaker versus different-speaker
//! matters to the grouping).
//!
//! Markers are spelled with whitespace on both sides, which is the
//! STANDALONE `OverlapPoint` item the originals built; glued to a word they
//! would be the intra-word route instead.
//!
//! # Fixtures that are invalid on purpose
//!
//! An INDEXED top overlap with no matching bottom from another speaker is
//! E347, and a speaker answering their own overlap is E704, so those rows
//! say so. The UNINDEXED orphan rows are valid CHAT because E347 deliberately
//! skips unindexed orphans as inherently ambiguous
//! (`validation/cross_utterance/mod.rs`), and the grouping still reports
//! them as orphaned, which is what those rows pin.

use talkbank_model::alignment::helpers::{FileOverlapAnalysis, analyze_file_overlaps};
use talkbank_parser_tests::from_source::{Dialogue, Rules};
use talkbank_parser_tests::test_error::TestError;

fn analysis(lines: &[(&str, &str)], codes: &[&str]) -> Result<FileOverlapAnalysis, TestError> {
    let file = Dialogue { lines }.parsed(codes, Rules::Default)?;
    Ok(analyze_file_overlaps(&file.lines))
}

const TOP: (&str, &str) = ("CHI", "\u{2308} hello \u{2309} .");

/// One top, one bottom from another speaker: one group, no orphans.
#[test]
fn one_top_and_one_bottom_pair() -> Result<(), TestError> {
    let analysis = analysis(&[TOP, ("MOT", "\u{230A} hi \u{230B} .")], &[])?;
    assert_eq!(analysis.groups.len(), 1);
    assert_eq!(analysis.groups[0].top.speaker.as_str(), "CHI");
    assert_eq!(analysis.groups[0].bottoms.len(), 1);
    assert_eq!(analysis.groups[0].bottoms[0].speaker.as_str(), "MOT");
    assert!(analysis.orphaned_tops.is_empty());
    assert!(analysis.orphaned_bottoms.is_empty());
    Ok(())
}

/// One top answered by two bottoms is one group with two bottoms.
#[test]
fn one_top_and_two_bottoms_are_one_group() -> Result<(), TestError> {
    let analysis = analysis(
        &[
            TOP,
            ("MOT", "\u{230A} yeah \u{230B} ."),
            ("EXP", "\u{230A} right \u{230B} ."),
        ],
        &[],
    )?;
    assert_eq!(analysis.groups.len(), 1, "one top, one group");
    assert_eq!(analysis.groups[0].bottoms.len(), 2, "two bottoms matched");
    assert_eq!(analysis.groups[0].bottoms[0].speaker.as_str(), "MOT");
    assert_eq!(analysis.groups[0].bottoms[1].speaker.as_str(), "EXP");
    assert!(analysis.orphaned_tops.is_empty());
    assert!(analysis.orphaned_bottoms.is_empty());
    Ok(())
}

/// An unindexed pair and an index-2 pair are two groups, each with its own
/// bottom.
#[test]
fn indexed_markers_group_by_index() -> Result<(), TestError> {
    let analysis = analysis(
        &[
            ("CHI", "\u{2308} one \u{2309} \u{2308}2 two \u{2309}2 ."),
            ("MOT", "\u{230A} one \u{230B} \u{230A}2 two \u{230B}2 ."),
        ],
        &[],
    )?;
    assert_eq!(analysis.groups.len(), 2, "two separate groups by index");
    assert_eq!(analysis.groups[0].top.region.index, None);
    assert_eq!(analysis.groups[0].bottoms.len(), 1);
    assert_eq!(
        analysis.groups[1].top.region.index.map(|i| i.get()),
        Some(2)
    );
    assert_eq!(analysis.groups[1].bottoms.len(), 1);
    Ok(())
}

/// A bottom from the SAME speaker does not answer a top: both orphaned. That
/// shape is E704 (a speaker overlapping themself across adjacent
/// utterances), so the fixture says so.
#[test]
fn a_same_speaker_bottom_does_not_match() -> Result<(), TestError> {
    let analysis = analysis(&[TOP, ("CHI", "\u{230A} nope \u{230B} .")], &["E704"])?;
    assert_eq!(analysis.groups.len(), 0);
    assert_eq!(analysis.orphaned_tops.len(), 1);
    assert_eq!(analysis.orphaned_bottoms.len(), 1);
    Ok(())
}

/// A top with no bottom anywhere is an orphaned top.
#[test]
fn a_top_with_no_bottom_is_orphaned() -> Result<(), TestError> {
    let analysis = analysis(&[TOP, ("MOT", "no overlap .")], &[])?;
    assert_eq!(analysis.groups.len(), 0);
    assert_eq!(analysis.orphaned_tops.len(), 1);
    assert_eq!(analysis.orphaned_tops[0].speaker.as_str(), "CHI");
    Ok(())
}

/// A bottom with no top anywhere is an orphaned bottom.
#[test]
fn a_bottom_with_no_top_is_orphaned() -> Result<(), TestError> {
    let analysis = analysis(
        &[
            ("CHI", "no overlap ."),
            ("MOT", "\u{230A} random \u{230B} ."),
        ],
        &[],
    )?;
    assert_eq!(analysis.groups.len(), 0);
    assert_eq!(analysis.orphaned_bottoms.len(), 1);
    Ok(())
}

/// A top of index 2 and a bottom of index 3 do not match.
#[test]
fn an_index_mismatch_does_not_match() -> Result<(), TestError> {
    let analysis = analysis(
        &[
            ("CHI", "\u{2308}2 hello \u{2309}2 ."),
            ("MOT", "\u{230A}3 hi \u{230B}3 ."),
        ],
        &["E347"],
    )?;
    assert_eq!(analysis.groups.len(), 0);
    assert_eq!(analysis.orphaned_tops.len(), 1);
    assert_eq!(analysis.orphaned_bottoms.len(), 1);
    Ok(())
}
