//! Overlap marker extraction from CHAT content.
//!
//! Provides [`extract_overlap_info`], an in-order traversal of main-tier
//! content (the shared `walk_content` at the `%wor` domain) that counts
//! alignable words and records the word-relative positions of CA overlap
//! markers (⌈⌉⌊⌋). Handles markers at all three content levels:
//!
//! - `UtteranceContent::OverlapPoint`, space-separated: `⌈ word ⌉`
//! - `BracketedItem::OverlapPoint`, inside groups: `<⌈ word ⌉> [/]`
//! - `WordContent::OverlapPoint`, intra-word: `butt⌈er⌉`
//!
//! This parallels the overlap collection in `validation/utterance/overlap.rs`
//! but tracks word positions rather than collecting points for validation.
// Every match in this file (over the shared walker's `ContentItem`, and over
// `WordContent` in `scan_word`) is exhaustive, so the lint costs nothing today
// and makes it stay that way: a new content variant becomes a COMPILE ERROR
// in the walker's own match first and then here, rather than a silent `_ =>`
// that answers wrong. Four such catch-alls have already shipped as defects;
// see `talkbank-parser-tests/src/content_catch_alls.rs`.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::alignment::helpers::{ContentItem, TierDomain, counts_for_tier, walk_content};
use crate::model::{OverlapIndex, OverlapPointKind, UtteranceContent, Word, WordContent};

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

    // The shared walker at the `%wor` domain: the same descent and the same
    // leaf set as `WorMainTierProjection`, so `total_words` IS the
    // projection's slot count. Until 2026-09-08 this file walked with two
    // private traversals of its own, and the bracketed one scanned a
    // replaced word's replacement too, counting `<doggie [: dog]>` twice
    // where the projection counts it once.
    walk_content(content, Some(TierDomain::Wor), &mut |item| match item {
        ContentItem::OverlapPoint(m) => record_marker(&mut markers, m.kind, m.index, word_count),
        ContentItem::Word(word) => scan_word(word, &mut word_count, &mut markers),
        // `%wor` times the original spoken word; the replacement is editorial.
        ContentItem::ReplacedWord(replaced) => {
            scan_word(&replaced.word, &mut word_count, &mut markers);
        }
        ContentItem::Separator(_)
        | ContentItem::Event(_)
        | ContentItem::Pause(_)
        | ContentItem::Action(_)
        | ContentItem::OtherSpokenEvent(_)
        | ContentItem::Freecode(_)
        | ContentItem::InternalBullet(_)
        | ContentItem::LongFeatureBegin(_)
        | ContentItem::LongFeatureEnd(_)
        | ContentItem::UnderlineBegin(_)
        | ContentItem::UnderlineEnd(_)
        | ContentItem::NonvocalBegin(_)
        | ContentItem::NonvocalEnd(_)
        | ContentItem::NonvocalSimple(_) => {}
    });

    let regions = pair_markers(&markers);

    OverlapMarkerInfo {
        total_words: word_count,
        regions,
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
    for wc in word.content().iter() {
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
    for wc in word.content().iter() {
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

#[cfg(test)]
mod tests {
    //! Arithmetic on an `OverlapMarkerInfo` value, built directly.
    //!
    //! Seven tests that used to sit here built content from `OverlapPoint::new`
    //! between hand-made words; they are
    //! `talkbank-parser-tests/tests/integration/overlap_regions_from_source.rs`
    //! now, over parsed main tiers. This one stays: it tests the onset
    //! estimate on an info value, not the extraction, and holds no span.
    use super::*;

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
}
