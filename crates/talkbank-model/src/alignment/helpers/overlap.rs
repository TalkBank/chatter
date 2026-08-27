//! Overlap marker extraction from CHAT content.
//!
//! Provides [`extract_overlap_info`], an in-order traversal of main-tier
//! content that counts alignable words and records the word-relative positions
//! of CA overlap markers (⌈⌉⌊⌋). Handles markers at all three content levels:
//!
//! - `UtteranceContent::OverlapPoint`, space-separated: `⌈ word ⌉`
//! - `BracketedItem::OverlapPoint`, inside groups: `<⌈ word ⌉> [/]`
//! - `WordContent::OverlapPoint`, intra-word: `butt⌈er⌉`
//!
//! This parallels the overlap collection in `validation/utterance/overlap.rs`
//! but tracks word positions rather than collecting points for validation.
// Every match over the content enums in this file is exhaustive, so the lint
// costs nothing today and makes it stay that way: a new `UtteranceContent` or
// `BracketedItem` variant becomes a COMPILE ERROR here rather than a silent
// `_ =>` that answers wrong. Four such catch-alls have already shipped as
// defects; see `talkbank-parser-tests/src/content_catch_alls.rs`.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::alignment::helpers::descent::descend;
use crate::alignment::helpers::{TierDomain, counts_for_tier};
use crate::model::{
    BracketedItem, OverlapIndex, OverlapPointKind, UtteranceContent, Word, WordContent,
};

/// A single paired overlap region within an utterance, matched by index.
///
/// A region is a begin-end pair of the same kind (top or bottom) with the
/// same optional index. For example, `⌈2 word word ⌉2` is one region with
/// `index = Some(2)`.
///
/// Unpaired markers (opening without closing, or vice versa) produce regions
/// with `begin_at_word` or `end_at_word` set to `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlapRegion {
    /// Whether this is a top (⌈⌉) or bottom (⌊⌋) region.
    pub kind: OverlapRegionKind,
    /// Disambiguation index (None = unindexed, Some = indexed 2..=9).
    pub index: Option<OverlapIndex>,
    /// Word position of the opening marker, if present.
    pub begin_at_word: Option<usize>,
    /// Word position of the closing marker, if present.
    pub end_at_word: Option<usize>,
}

/// Whether an overlap region is top (first speaker) or bottom (second speaker).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapRegionKind {
    /// ⌈...⌉; this speaker started talking first.
    Top,
    /// ⌊...⌋; this speaker started talking during the top speaker's turn.
    Bottom,
}

impl OverlapRegion {
    /// Whether both begin and end markers are present (well-paired).
    pub fn is_well_paired(&self) -> bool {
        match (self.begin_at_word, self.end_at_word) {
            (Some(b), Some(e)) => b <= e,
            _ => false,
        }
    }

    /// Whether the opening marker is present (possibly without closing).
    pub fn has_begin(&self) -> bool {
        self.begin_at_word.is_some()
    }
}

/// Overlap marker analysis for one utterance.
///
/// Contains all paired overlap regions found by in-order traversal,
/// matched by index. Each ⌈ is matched with the next ⌉ of the same index
/// (or unindexed with unindexed), and similarly for ⌊/⌋.
#[derive(Debug, Clone, Default)]
pub struct OverlapMarkerInfo {
    /// Total alignable words (Wor domain) in the utterance.
    pub total_words: usize,
    /// All overlap regions found, in document order by opening marker.
    pub regions: Vec<OverlapRegion>,
}

impl OverlapMarkerInfo {
    /// Whether this utterance contains any CA overlap markers.
    pub fn has_any_markers(&self) -> bool {
        !self.regions.is_empty()
    }

    /// Whether this utterance has any ⌊ (bottom overlap begin), indicating it
    /// overlaps with a preceding speaker's ⌈-marked region.
    pub fn has_bottom_overlap(&self) -> bool {
        self.regions
            .iter()
            .any(|r| r.kind == OverlapRegionKind::Bottom && r.has_begin())
    }

    /// Whether this utterance has any ⌈ (top overlap begin).
    pub fn has_top_overlap(&self) -> bool {
        self.regions
            .iter()
            .any(|r| r.kind == OverlapRegionKind::Top && r.has_begin())
    }

    /// Proportional onset of the *first* ⌈ marker within the utterance (0.0-1.0).
    ///
    /// Returns `None` if no ⌈ marker is present or the utterance has no words.
    /// A value of 0.6 means the earliest overlap begins 60% through the utterance.
    ///
    /// Works with unpaired ⌈ (onset-only marking is a legitimate CA practice).
    pub fn top_onset_fraction(&self) -> Option<f64> {
        let first_top = self
            .regions
            .iter()
            .find(|r| r.kind == OverlapRegionKind::Top && r.has_begin())?;
        let word_pos = first_top.begin_at_word?;
        if self.total_words == 0 {
            return None;
        }
        Some(word_pos as f64 / self.total_words as f64)
    }

    /// Estimate the overlap onset time in milliseconds given the utterance's
    /// bullet timing.
    ///
    /// Linearly interpolates: `start + fraction * (end - start)`.
    pub fn estimate_onset_ms(&self, utt_start_ms: u64, utt_end_ms: u64) -> Option<u64> {
        let fraction = self.top_onset_fraction()?;
        let duration = utt_end_ms.saturating_sub(utt_start_ms);
        Some(utt_start_ms + (fraction * duration as f64) as u64)
    }

    /// Top regions only.
    pub fn top_regions(&self) -> impl Iterator<Item = &OverlapRegion> {
        self.regions
            .iter()
            .filter(|r| r.kind == OverlapRegionKind::Top)
    }

    /// Bottom regions only.
    pub fn bottom_regions(&self) -> impl Iterator<Item = &OverlapRegion> {
        self.regions
            .iter()
            .filter(|r| r.kind == OverlapRegionKind::Bottom)
    }
}

/// Extract overlap marker positions from utterance content.
///
/// Walks the content in document order, counting alignable words (Wor domain)
/// and collecting overlap markers with their word positions. Then matches
/// begin/end markers by (kind, index) to form paired regions.
///
/// Handles markers at all three content levels: `UtteranceContent`,
/// `BracketedItem`, and `WordContent`.
pub fn extract_overlap_info(content: &[UtteranceContent]) -> OverlapMarkerInfo {
    let mut markers: Vec<MarkerOccurrence> = Vec::new();
    let mut word_count: usize = 0;

    walk_content(content, &mut word_count, &mut markers);

    let regions = pair_markers(&markers);

    OverlapMarkerInfo {
        total_words: word_count,
        regions,
    }
}

/// Context passed to the [`walk_overlap_points`] closure for each marker.
#[derive(Debug, Clone)]
pub struct OverlapPointVisit<'a> {
    /// The overlap point marker.
    pub point: &'a crate::model::OverlapPoint,
    /// Number of alignable words (Wor domain) seen before this marker.
    pub word_position: usize,
}

/// Visit every overlap marker in document order, with word-position context.
///
/// This is the closure-based internal iterator for overlap markers, analogous
/// to [`walk_words`](super::walk_words) for words. It walks all three
/// content levels (UtteranceContent, BracketedItem, WordContent) and calls
/// `visitor` for each `OverlapPoint` encountered.
///
/// Use this when you need per-marker access (e.g., collecting raw points for
/// validation) rather than the pre-paired regions from [`extract_overlap_info`].
pub fn walk_overlap_points(
    content: &[UtteranceContent],
    visitor: &mut impl FnMut(OverlapPointVisit<'_>),
) {
    let mut word_count: usize = 0;
    walk_content_visiting(content, &mut word_count, visitor);
}

/// Walk top-level content, calling visitor for each overlap point.
fn walk_content_visiting(
    items: &[UtteranceContent],
    word_count: &mut usize,
    visitor: &mut impl FnMut(OverlapPointVisit<'_>),
) {
    for item in items {
        match item {
            UtteranceContent::OverlapPoint(m) => {
                visitor(OverlapPointVisit {
                    point: m,
                    word_position: *word_count,
                });
            }
            UtteranceContent::Word(word) => {
                scan_word_visiting(word, word_count, visitor);
            }
            UtteranceContent::AnnotatedWord(word) => {
                scan_word_visiting(&word.inner, word_count, visitor);
            }
            UtteranceContent::ReplacedWord(replaced) => {
                scan_word_visiting(&replaced.word, word_count, visitor);
            }
            // Containers: ONE arm, through `descent` at the `%wor` domain.
            //
            // It descended UNCONDITIONALLY via `enclosed()` until 2026-08-26,
            // which agrees with `descent` only because the `Wor` row of the
            // table is `Enter` for every container kind. That agreement was a
            // coincidence nothing in the types recorded, in the one traversal
            // whose scale must match `count_tier_positions(content, Wor)` and
            // which has already drifted from it once, moving real timing
            // anchors. Naming the domain makes the row load-bearing.
            UtteranceContent::Group(_)
            | UtteranceContent::AnnotatedGroup(_)
            | UtteranceContent::Quotation(_)
            | UtteranceContent::AnnotatedQuotation(_)
            | UtteranceContent::PhoGroup(_)
            | UtteranceContent::SinGroup(_)
            | UtteranceContent::Retrace(_)
            | UtteranceContent::AnnotatedRetrace(_) => {
                if let Some(into) = descend(item.structure(), Some(TierDomain::Wor)).entered() {
                    walk_bracketed_visiting(into.content().content.as_slice(), word_count, visitor);
                }
            }
            UtteranceContent::AnnotatedEvent(_)
            | UtteranceContent::Event(_)
            | UtteranceContent::Pause(_)
            | UtteranceContent::Action(_)
            | UtteranceContent::AnnotatedAction(_)
            | UtteranceContent::Freecode(_)
            | UtteranceContent::Separator(_)
            | UtteranceContent::InternalBullet(_)
            | UtteranceContent::LongFeatureBegin(_)
            | UtteranceContent::LongFeatureEnd(_)
            | UtteranceContent::UnderlineBegin(_)
            | UtteranceContent::UnderlineEnd(_)
            | UtteranceContent::NonvocalBegin(_)
            | UtteranceContent::NonvocalEnd(_)
            | UtteranceContent::NonvocalSimple(_)
            | UtteranceContent::OtherSpokenEvent(_) => {}
        }
    }
}

/// Walk bracketed items, calling visitor for each overlap point.
fn walk_bracketed_visiting(
    items: &[BracketedItem],
    word_count: &mut usize,
    visitor: &mut impl FnMut(OverlapPointVisit<'_>),
) {
    for item in items {
        match item {
            BracketedItem::OverlapPoint(m) => {
                visitor(OverlapPointVisit {
                    point: m,
                    word_position: *word_count,
                });
            }
            BracketedItem::Word(word) => {
                scan_word_visiting(word, word_count, visitor);
            }
            BracketedItem::AnnotatedWord(annotated) => {
                scan_word_visiting(&annotated.inner, word_count, visitor);
            }
            BracketedItem::ReplacedWord(replaced) => {
                scan_word_visiting(&replaced.word, word_count, visitor);
            }
            // Containers: ONE arm, through `descent` at the `%wor` domain.
            //
            // It descended UNCONDITIONALLY via `enclosed()` until 2026-08-26,
            // which agrees with `descent` only because the `Wor` row of the
            // table is `Enter` for every container kind. That agreement was a
            // coincidence nothing in the types recorded, in the one traversal
            // whose scale must match `count_tier_positions(content, Wor)` and
            // which has already drifted from it once, moving real timing
            // anchors. Naming the domain makes the row load-bearing.
            BracketedItem::Group(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::AnnotatedQuotation(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::AnnotatedRetrace(_) => {
                if let Some(into) = descend(item.structure(), Some(TierDomain::Wor)).entered() {
                    walk_bracketed_visiting(into.content().content.as_slice(), word_count, visitor);
                }
            }
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::Separator(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}

/// Scan word content for overlap points, calling visitor for each.
fn scan_word_visiting(
    word: &Word,
    word_count: &mut usize,
    visitor: &mut impl FnMut(OverlapPointVisit<'_>),
) {
    for wc in word.content.iter() {
        if let WordContent::OverlapPoint(m) = wc {
            visitor(OverlapPointVisit {
                point: m,
                word_position: *word_count,
            });
        }
    }
    if counts_for_tier(word, TierDomain::Wor) {
        *word_count += 1;
    }
}

/// A single overlap marker occurrence with its word position.
#[derive(Debug)]
struct MarkerOccurrence {
    kind: OverlapPointKind,
    index: Option<OverlapIndex>,
    word_position: usize,
}

/// Record an overlap marker occurrence.
fn record_marker(
    markers: &mut Vec<MarkerOccurrence>,
    kind: OverlapPointKind,
    index: Option<OverlapIndex>,
    word_position: usize,
) {
    markers.push(MarkerOccurrence {
        kind,
        index,
        word_position,
    });
}

/// Match begin/end markers by (kind-pair, index) to form regions.
///
/// For each ⌈, find the next unmatched ⌉ with the same index (or both
/// unindexed). Same for ⌊/⌋. Unmatched markers produce regions with
/// `None` for the missing endpoint.
fn pair_markers(markers: &[MarkerOccurrence]) -> Vec<OverlapRegion> {
    let mut regions: Vec<OverlapRegion> = Vec::new();
    let mut used: Vec<bool> = vec![false; markers.len()];

    // First pass: match begins with ends
    for (i, m) in markers.iter().enumerate() {
        let (region_kind, end_point_kind) = match m.kind {
            OverlapPointKind::TopOverlapBegin => {
                (OverlapRegionKind::Top, OverlapPointKind::TopOverlapEnd)
            }
            OverlapPointKind::BottomOverlapBegin => (
                OverlapRegionKind::Bottom,
                OverlapPointKind::BottomOverlapEnd,
            ),
            // Named, not `_`: this pass pairs BEGIN markers, and a future
            // point kind must be classified here rather than silently skipped.
            OverlapPointKind::TopOverlapEnd | OverlapPointKind::BottomOverlapEnd => continue,
        };

        if used[i] {
            continue;
        }
        used[i] = true;

        // Find the next unmatched end marker with the same index
        let mut end_at_word = None;
        for (j, candidate) in markers.iter().enumerate().skip(i + 1) {
            if !used[j] && candidate.kind == end_point_kind && candidate.index == m.index {
                end_at_word = Some(candidate.word_position);
                used[j] = true;
                break;
            }
        }

        regions.push(OverlapRegion {
            kind: region_kind,
            index: m.index,
            begin_at_word: Some(m.word_position),
            end_at_word,
        });
    }

    // Second pass: orphaned end markers (no matching begin)
    for (i, m) in markers.iter().enumerate() {
        if used[i] {
            continue;
        }
        let region_kind = match m.kind {
            OverlapPointKind::TopOverlapEnd => OverlapRegionKind::Top,
            OverlapPointKind::BottomOverlapEnd => OverlapRegionKind::Bottom,
            // This pass collects ORPHANED ENDS; begins were handled above.
            OverlapPointKind::TopOverlapBegin | OverlapPointKind::BottomOverlapBegin => continue,
        };
        regions.push(OverlapRegion {
            kind: region_kind,
            index: m.index,
            begin_at_word: None,
            end_at_word: Some(m.word_position),
        });
    }

    regions
}

/// Scan a word's internal content for overlap markers and count the word.
///
/// Intra-word markers like `butt⌈er⌉` have the overlap point embedded in
/// `WordContent::OverlapPoint`. Opening markers (⌈⌊) are recorded before
/// the word is counted; closing markers (⌉⌋) are recorded after.
fn scan_word(word: &Word, word_count: &mut usize, markers: &mut Vec<MarkerOccurrence>) {
    // Opening markers (⌈⌊) record position BEFORE the word.
    for wc in word.content.iter() {
        if let WordContent::OverlapPoint(marker) = wc
            && matches!(
                marker.kind,
                OverlapPointKind::TopOverlapBegin | OverlapPointKind::BottomOverlapBegin
            )
        {
            record_marker(markers, marker.kind, marker.index, *word_count);
        }
    }

    if counts_for_tier(word, TierDomain::Wor) {
        *word_count += 1;
    }

    // Closing markers (⌉⌋) record position AFTER the word.
    for wc in word.content.iter() {
        if let WordContent::OverlapPoint(marker) = wc
            && matches!(
                marker.kind,
                OverlapPointKind::TopOverlapEnd | OverlapPointKind::BottomOverlapEnd
            )
        {
            record_marker(markers, marker.kind, marker.index, *word_count);
        }
    }
}

/// Walk top-level content items, collecting marker occurrences.
fn walk_content(
    items: &[UtteranceContent],
    word_count: &mut usize,
    markers: &mut Vec<MarkerOccurrence>,
) {
    for item in items {
        match item {
            UtteranceContent::OverlapPoint(m) => {
                record_marker(markers, m.kind, m.index, *word_count);
            }
            UtteranceContent::Word(word) => {
                scan_word(word, word_count, markers);
            }
            UtteranceContent::AnnotatedWord(word) => {
                scan_word(&word.inner, word_count, markers);
            }
            UtteranceContent::ReplacedWord(replaced) => {
                // The ORIGINAL only. `word_count` is a %wor position index
                // (both scanners increment on `counts_for_tier(_, Wor)`), and
                // the canonical %wor rule in `count::count_alignable_replaced_word`
                // is explicit: "%wor aligns to the originally spoken surface
                // form, not the editorial replacement", contributing at most 1.
                //
                // This arm used to scan the replacement words too, so on any
                // utterance carrying both a replacement and an overlap marker
                // the positions this walker produced were on a DIFFERENT SCALE
                // from `count_tier_positions(content, Wor)` and from its own
                // sibling `walk_content_visiting`, which always scanned the
                // original alone. `total_words` then fed `top_onset_fraction`
                // and `estimate_onset_ms`, so the disagreement moved real
                // timing anchors. Two copies of one traversal, drifted.
                scan_word(&replaced.word, word_count, markers);
            }
            // Containers: ONE arm, through `descent` at the `%wor` domain.
            //
            // It descended UNCONDITIONALLY via `enclosed()` until 2026-08-26,
            // which agrees with `descent` only because the `Wor` row of the
            // table is `Enter` for every container kind. That agreement was a
            // coincidence nothing in the types recorded, in the one traversal
            // whose scale must match `count_tier_positions(content, Wor)` and
            // which has already drifted from it once, moving real timing
            // anchors. Naming the domain makes the row load-bearing.
            UtteranceContent::Group(_)
            | UtteranceContent::AnnotatedGroup(_)
            | UtteranceContent::Quotation(_)
            | UtteranceContent::AnnotatedQuotation(_)
            | UtteranceContent::PhoGroup(_)
            | UtteranceContent::SinGroup(_)
            | UtteranceContent::Retrace(_)
            | UtteranceContent::AnnotatedRetrace(_) => {
                if let Some(into) = descend(item.structure(), Some(TierDomain::Wor)).entered() {
                    walk_bracketed(into.content().content.as_slice(), word_count, markers);
                }
            }
            UtteranceContent::AnnotatedEvent(_)
            | UtteranceContent::Event(_)
            | UtteranceContent::Pause(_)
            | UtteranceContent::Action(_)
            | UtteranceContent::AnnotatedAction(_)
            | UtteranceContent::Freecode(_)
            | UtteranceContent::Separator(_)
            | UtteranceContent::InternalBullet(_)
            | UtteranceContent::LongFeatureBegin(_)
            | UtteranceContent::LongFeatureEnd(_)
            | UtteranceContent::UnderlineBegin(_)
            | UtteranceContent::UnderlineEnd(_)
            | UtteranceContent::NonvocalBegin(_)
            | UtteranceContent::NonvocalEnd(_)
            | UtteranceContent::NonvocalSimple(_)
            | UtteranceContent::OtherSpokenEvent(_) => {}
        }
    }
}

/// Walk bracketed items (inside groups), collecting marker occurrences.
fn walk_bracketed(
    items: &[BracketedItem],
    word_count: &mut usize,
    markers: &mut Vec<MarkerOccurrence>,
) {
    for item in items {
        match item {
            BracketedItem::OverlapPoint(m) => {
                record_marker(markers, m.kind, m.index, *word_count);
            }
            BracketedItem::Word(word) => {
                scan_word(word, word_count, markers);
            }
            BracketedItem::AnnotatedWord(annotated) => {
                scan_word(&annotated.inner, word_count, markers);
            }
            BracketedItem::ReplacedWord(replaced) => {
                scan_word(&replaced.word, word_count, markers);
                for word in &replaced.replacement.words {
                    scan_word(word, word_count, markers);
                }
            }
            // Containers: ONE arm, through `descent` at the `%wor` domain.
            //
            // It descended UNCONDITIONALLY via `enclosed()` until 2026-08-26,
            // which agrees with `descent` only because the `Wor` row of the
            // table is `Enter` for every container kind. That agreement was a
            // coincidence nothing in the types recorded, in the one traversal
            // whose scale must match `count_tier_positions(content, Wor)` and
            // which has already drifted from it once, moving real timing
            // anchors. Naming the domain makes the row load-bearing.
            BracketedItem::Group(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::AnnotatedQuotation(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::AnnotatedRetrace(_) => {
                if let Some(into) = descend(item.structure(), Some(TierDomain::Wor)).entered() {
                    walk_bracketed(into.content().content.as_slice(), word_count, markers);
                }
            }
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::Separator(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::model::{OverlapPoint, Word};

    fn make_word(text: &str) -> UtteranceContent {
        UtteranceContent::Word(Box::new(Word::new_unchecked(text, text)))
    }

    fn make_overlap(kind: OverlapPointKind) -> UtteranceContent {
        UtteranceContent::OverlapPoint(OverlapPoint::new(kind, None))
    }

    #[test]
    fn test_no_markers() {
        let content = vec![make_word("hello"), make_word("world")];
        let info = extract_overlap_info(&content);
        assert_eq!(info.total_words, 2);
        assert!(!info.has_any_markers());
    }

    #[test]
    fn test_top_overlap_mid_utterance() {
        // one two three ⌈ four five ⌉
        let content = vec![
            make_word("one"),
            make_word("two"),
            make_word("three"),
            make_overlap(OverlapPointKind::TopOverlapBegin),
            make_word("four"),
            make_word("five"),
            make_overlap(OverlapPointKind::TopOverlapEnd),
        ];
        let info = extract_overlap_info(&content);
        assert_eq!(info.total_words, 5);
        assert_eq!(info.regions.len(), 1);
        let region = &info.regions[0];
        assert_eq!(region.kind, OverlapRegionKind::Top);
        assert_eq!(region.begin_at_word, Some(3));
        assert_eq!(region.end_at_word, Some(5));
        assert!(region.is_well_paired());
        let frac = info.top_onset_fraction().unwrap();
        assert!((frac - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_bottom_overlap() {
        // ⌊ yeah ⌋
        let content = vec![
            make_overlap(OverlapPointKind::BottomOverlapBegin),
            make_word("yeah"),
            make_overlap(OverlapPointKind::BottomOverlapEnd),
        ];
        let info = extract_overlap_info(&content);
        assert_eq!(info.total_words, 1);
        assert!(info.has_bottom_overlap());
        assert_eq!(info.regions.len(), 1);
        let region = &info.regions[0];
        assert_eq!(region.kind, OverlapRegionKind::Bottom);
        assert_eq!(region.begin_at_word, Some(0));
        assert_eq!(region.end_at_word, Some(1));
    }

    #[test]
    fn test_estimate_onset_ms() {
        let info = OverlapMarkerInfo {
            total_words: 10,
            regions: vec![OverlapRegion {
                kind: OverlapRegionKind::Top,
                index: None,
                begin_at_word: Some(6),
                end_at_word: Some(10),
            }],
        };
        let onset = info.estimate_onset_ms(12660, 15585).unwrap();
        assert_eq!(onset, 14415);
    }

    #[test]
    fn test_intra_word_markers() {
        // butt⌈er⌉, opening at word 0, closing after word 0
        let word = Word::new_unchecked("butt⌈er⌉", "butter").with_content(vec![
            WordContent::Text(crate::model::WordText::new_unchecked("butt")),
            WordContent::OverlapPoint(OverlapPoint::new(OverlapPointKind::TopOverlapBegin, None)),
            WordContent::Text(crate::model::WordText::new_unchecked("er")),
            WordContent::OverlapPoint(OverlapPoint::new(OverlapPointKind::TopOverlapEnd, None)),
        ]);
        let content = vec![UtteranceContent::Word(Box::new(word)), make_word("please")];
        let info = extract_overlap_info(&content);
        assert_eq!(info.total_words, 2);
        assert!(info.has_top_overlap());
        assert_eq!(info.regions.len(), 1);
        assert_eq!(info.regions[0].begin_at_word, Some(0));
        assert_eq!(info.regions[0].end_at_word, Some(1));
    }

    #[test]
    fn test_indexed_overlaps_pair_by_index() {
        // ⌈ one ⌉ ⌈2 two ⌉2, two separate top regions
        use crate::model::OverlapIndex;
        let content = vec![
            make_overlap(OverlapPointKind::TopOverlapBegin),
            make_word("one"),
            UtteranceContent::OverlapPoint(OverlapPoint::new(
                OverlapPointKind::TopOverlapEnd,
                None,
            )),
            UtteranceContent::OverlapPoint(OverlapPoint::new(
                OverlapPointKind::TopOverlapBegin,
                Some(OverlapIndex::new(2)),
            )),
            make_word("two"),
            UtteranceContent::OverlapPoint(OverlapPoint::new(
                OverlapPointKind::TopOverlapEnd,
                Some(OverlapIndex::new(2)),
            )),
        ];
        let info = extract_overlap_info(&content);
        assert_eq!(info.total_words, 2);
        assert_eq!(info.regions.len(), 2);
        // First region: unindexed
        assert_eq!(info.regions[0].index, None);
        assert_eq!(info.regions[0].begin_at_word, Some(0));
        assert_eq!(info.regions[0].end_at_word, Some(1));
        // Second region: index 2
        assert_eq!(info.regions[1].index, Some(OverlapIndex::new(2)));
        assert_eq!(info.regions[1].begin_at_word, Some(1));
        assert_eq!(info.regions[1].end_at_word, Some(2));
    }

    #[test]
    fn test_unpaired_opening_only() {
        // ⌈ word, opening without closing (onset-only annotation)
        let content = vec![
            make_overlap(OverlapPointKind::TopOverlapBegin),
            make_word("word"),
        ];
        let info = extract_overlap_info(&content);
        assert_eq!(info.regions.len(), 1);
        assert_eq!(info.regions[0].begin_at_word, Some(0));
        assert_eq!(info.regions[0].end_at_word, None);
        assert!(!info.regions[0].is_well_paired());
        // Still usable for onset estimation
        assert!(info.top_onset_fraction().is_some());
    }

    #[test]
    fn test_orphaned_closing() {
        // word ⌉, closing without opening
        let content = vec![
            make_word("word"),
            make_overlap(OverlapPointKind::TopOverlapEnd),
        ];
        let info = extract_overlap_info(&content);
        assert_eq!(info.regions.len(), 1);
        assert_eq!(info.regions[0].begin_at_word, None);
        assert_eq!(info.regions[0].end_at_word, Some(1));
        assert!(!info.regions[0].is_well_paired());
        // No onset estimation possible
        assert!(!info.has_top_overlap());
    }
}
