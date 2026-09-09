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

//! Overlap-marker regions (`⌈ ⌉` top, `⌊ ⌋` bottom), over main tiers the
//! parser built.
//!
//! # What this replaces
//!
//! Seven of the eight inline tests of `talkbank-model`'s
//! `alignment/helpers/overlap.rs` built content from
//! `OverlapPoint::new(OverlapPointKind::TopOverlapBegin, None)` between
//! `Word::new_unchecked` items, and one wrote an intra-word marker into a
//! `WordContent` list by hand. Here each case is the utterance as written.
//! The eighth, `test_estimate_onset_ms`, stays there: it builds an
//! `OverlapMarkerInfo` value directly to test arithmetic on it, and holds no
//! fabricated span.
//!
//! # Two spellings, two AST shapes
//!
//! A marker with whitespace on both sides (`⌈ four five ⌉`) is a STANDALONE
//! `OverlapPoint` item; a marker glued to a word (`⌈four five⌉`, `butt⌈er⌉`)
//! is an intra-word `WordContent::OverlapPoint` on that word. The extractor
//! walks both routes and the originals covered the standalone one in six of
//! seven tests, so the rows spell their markers with spaces except the one
//! that is about the intra-word route. The first draft glued every marker and
//! passed with identical numbers while leaving the standalone route
//! unexercised; a review caught it.

//! # The unpaired rows are valid CHAT on purpose
//!
//! `⌈word .` and `word⌉ .` validate clean. That is E348, "unpaired overlap
//! marker within utterance", DELIBERATELY suppressed and registered
//! `not_implemented`: onset-only marking is a legitimate Jeffersonian CA
//! convention, and enabling the check produced 2,152 false positives on
//! hand-edited data (`spec/errors/E348.md`). The extractor still reports the
//! region as not well paired, which is what those two rows pin.

use talkbank_model::alignment::helpers::{OverlapRegionKind, extract_overlap_info};
use talkbank_model::model::{Line, MainTier, OverlapIndex};
use talkbank_parser_tests::from_source::{Dialogue, Rules, SingleSpeaker};
use talkbank_parser_tests::test_error::TestError;

fn main_tier(utterance: &str) -> Result<MainTier, TestError> {
    let lines = format!("*CHI:\t{utterance}");
    SingleSpeaker::english(&lines).main_tier(&[])
}

/// No markers: every word counted, no region.
#[test]
fn no_markers_means_no_regions() -> Result<(), TestError> {
    let info = extract_overlap_info(&main_tier("hello world .")?.content.content);
    assert_eq!(info.total_words, 2);
    assert!(!info.has_any_markers());
    Ok(())
}

/// A top overlap over the last two of five words begins at word 3, ends at
/// word 5, and puts the onset at 0.6 of the utterance.
#[test]
fn a_mid_utterance_top_overlap_is_one_well_paired_region() -> Result<(), TestError> {
    let info = extract_overlap_info(
        &main_tier("one two three \u{2308} four five \u{2309} .")?
            .content
            .content,
    );
    assert_eq!(info.total_words, 5);
    assert_eq!(info.regions.len(), 1);
    let region = &info.regions[0];
    assert_eq!(region.kind, OverlapRegionKind::Top);
    assert_eq!(region.begin_at_word, Some(3));
    assert_eq!(region.end_at_word, Some(5));
    assert!(region.is_well_paired());
    let frac = info.top_onset_fraction().unwrap();
    assert!((frac - 0.6).abs() < 0.001);
    Ok(())
}

/// A bottom overlap around the only word.
#[test]
fn a_bottom_overlap_is_a_bottom_region() -> Result<(), TestError> {
    let info = extract_overlap_info(&main_tier("\u{230A} yeah \u{230B} .")?.content.content);
    assert_eq!(info.total_words, 1);
    assert!(info.has_bottom_overlap());
    assert_eq!(info.regions.len(), 1);
    let region = &info.regions[0];
    assert_eq!(region.kind, OverlapRegionKind::Bottom);
    assert_eq!(region.begin_at_word, Some(0));
    assert_eq!(region.end_at_word, Some(1));
    Ok(())
}

/// Markers INSIDE a word (`butt⌈er⌉`), the intra-word route, are found and
/// land on that word.
#[test]
fn intra_word_markers_are_found() -> Result<(), TestError> {
    let info = extract_overlap_info(
        &main_tier("butt\u{2308}er\u{2309} please .")?
            .content
            .content,
    );
    assert_eq!(info.total_words, 2);
    assert!(info.has_top_overlap());
    assert_eq!(info.regions.len(), 1);
    assert_eq!(info.regions[0].begin_at_word, Some(0));
    assert_eq!(info.regions[0].end_at_word, Some(1));
    Ok(())
}

/// Indexed markers pair by index, so an unindexed pair and a `2` pair are two
/// regions.
///
/// An INDEXED top overlap must be answered by a bottom overlap of the same
/// index from another speaker (E347), so this one is a two-speaker file and
/// the tier under test is the first speaker's.
#[test]
fn indexed_overlaps_pair_by_index() -> Result<(), TestError> {
    let file = Dialogue {
        lines: &[
            ("CHI", "\u{2308} one \u{2309} \u{2308}2 two \u{2309}2 ."),
            ("MOT", "\u{230A} a \u{230B} \u{230A}2 b \u{230B}2 ."),
        ],
    }
    .parsed(&[], Rules::Default)?;
    let main = file
        .lines
        .into_iter()
        .find_map(|line| match line {
            Line::Utterance(utterance) => Some(utterance.main),
            _ => None,
        })
        .ok_or_else(|| TestError::Failure("the dialogue built no utterance".into()))?;
    let info = extract_overlap_info(&main.content.content);
    assert_eq!(info.total_words, 2);
    assert_eq!(info.regions.len(), 2);
    assert_eq!(info.regions[0].index, None);
    assert_eq!(info.regions[0].begin_at_word, Some(0));
    assert_eq!(info.regions[0].end_at_word, Some(1));
    assert_eq!(info.regions[1].index, Some(OverlapIndex::new(2)));
    assert_eq!(info.regions[1].begin_at_word, Some(1));
    assert_eq!(info.regions[1].end_at_word, Some(2));
    Ok(())
}

/// A begin with no end is a region with no end, not well paired, and still
/// gives an onset.
#[test]
fn an_unpaired_opening_has_no_end() -> Result<(), TestError> {
    let info = extract_overlap_info(&main_tier("\u{2308} word .")?.content.content);
    assert_eq!(info.regions.len(), 1);
    assert_eq!(info.regions[0].begin_at_word, Some(0));
    assert_eq!(info.regions[0].end_at_word, None);
    assert!(!info.regions[0].is_well_paired());
    assert!(info.top_onset_fraction().is_some());
    Ok(())
}

/// An end with no begin is a region with no begin, and is not a top overlap.
#[test]
fn an_orphaned_closing_has_no_begin() -> Result<(), TestError> {
    let info = extract_overlap_info(&main_tier("word \u{2309} .")?.content.content);
    assert_eq!(info.regions.len(), 1);
    assert_eq!(info.regions[0].begin_at_word, None);
    assert_eq!(info.regions[0].end_at_word, Some(1));
    assert!(!info.regions[0].is_well_paired());
    assert!(!info.has_top_overlap());
    Ok(())
}

/// A replaced word inside a group counts once on the overlap scale, as it
/// does on the `%wor` scale: `%wor` times the original spoken word and the
/// replacement is editorial. Until 2026-09-08 the collector's bracketed arm
/// scanned the replacement words too, so `<doggie [: dog]>` under a marker
/// counted two words where the projection counts one, and the marker's
/// position fraction drifted with it.
#[test]
fn a_replaced_word_inside_a_group_counts_once() -> Result<(), TestError> {
    let main = main_tier("<\u{2308} doggie [: dog] \u{2309}> [/] yes .")?;
    let info = extract_overlap_info(&main.content.content);
    assert_eq!(
        info.total_words,
        main.wor_projection().slot_count().get(),
        "total_words is the %wor projection's slot count"
    );
    let region = info.top_regions().next().expect("one top region");
    assert_eq!(region.begin_at_word, Some(0));
    assert_eq!(region.end_at_word, Some(1));
    Ok(())
}
