//! Cross-utterance overlap group analysis.
//!
//! Matches top overlap regions (⌈...⌉) with bottom overlap regions (⌊...⌋)
//! across utterances to form [`OverlapGroup`]s. Supports 1:N matching,
//! one top region from speaker A can be matched by bottom regions from
//! speakers B, C, etc. (multiple respondents to the same turn).
//!
//! The matching algorithm:
//! 1. Collect all top and bottom regions across all utterances
//! 2. For each bottom region, find the nearest preceding top region from a
//!    different speaker with the same index
//! 3. Group bottoms by their matched top
//! 4. Tops with no matching bottoms are orphaned
//! 5. Bottoms with no matching tops are orphaned
//!
//! ## Example
//!
//! ```text
//! *A: I was ⌈ saying that ⌉ .     → top region, unindexed
//! *B:       ⌊ yeah ⌋ .             → bottom, matches A's top
//! *C:       ⌊ right ⌋ .            → bottom, also matches A's top (1:N)
//! *A: and ⌈2 then ⌉2 .            → top region, index 2
//! *B:     ⌊2 mhm ⌋2 .             → bottom, matches A's index-2 top
//! ```
//!
//! Produces two groups: one with A's unindexed top + B and C's bottoms,
//! one with A's index-2 top + B's index-2 bottom.

use crate::model::Line;

use super::overlap::{OverlapMarkerInfo, OverlapRegion, OverlapRegionKind, extract_overlap_info};
use crate::model::SpeakerCode;

/// An overlap region anchored to a specific utterance.
#[derive(Debug, Clone)]
pub struct OverlapAnchor {
    /// Index of the utterance in the file's utterance list (0-based).
    pub utterance_index: usize,
    /// Speaker code of the utterance.
    ///
    /// `SpeakerCode`, not `String`: the code is an interned `Arc<str>` and a
    /// field typed `String` is what made `speaker.to_string()` look like the
    /// natural way to fill it, heap-allocating a fresh copy per utterance and
    /// discarding the interning the type exists to provide.
    pub speaker: SpeakerCode,
    /// The overlap region within this utterance.
    pub region: OverlapRegion,
    /// Utterance-level timing bullet, if present.
    pub bullet: Option<(u64, u64)>,
}

/// A matched overlap group: one top region paired with 1..N bottom regions.
///
/// Represents the semantic relationship "speaker A was talking, and speakers
/// B (and possibly C, D, ...) started overlapping during A's turn."
#[derive(Debug, Clone)]
pub struct OverlapGroup {
    /// The top region (⌈...⌉), the speaker who was talking first.
    pub top: OverlapAnchor,
    /// Bottom regions (⌊...⌋) from different speakers who overlapped.
    /// May be empty if the top has no matching bottoms (orphaned top that
    /// was grouped because it shares an index with other tops).
    pub bottoms: Vec<OverlapAnchor>,
}

/// Complete cross-utterance overlap analysis for a file.
#[derive(Debug, Clone)]
pub struct FileOverlapAnalysis {
    /// Matched overlap groups (1 top : N bottoms).
    pub groups: Vec<OverlapGroup>,
    /// Top regions with no matching bottom from any other speaker.
    pub orphaned_tops: Vec<OverlapAnchor>,
    /// Bottom regions with no matching top from any other speaker.
    pub orphaned_bottoms: Vec<OverlapAnchor>,
    /// Per-utterance overlap info (cached from extraction).
    pub per_utterance: Vec<PerUtteranceOverlap>,
}

/// Per-utterance overlap data extracted during analysis.
#[derive(Debug, Clone)]
pub struct PerUtteranceOverlap {
    /// Utterance index.
    pub utterance_index: usize,
    /// Speaker code. See [`OverlapAnchor::speaker`] for why it is not a `String`.
    pub speaker: SpeakerCode,
    /// Overlap marker info for this utterance.
    pub info: OverlapMarkerInfo,
    /// Utterance-level timing bullet.
    pub bullet: Option<(u64, u64)>,
}

impl FileOverlapAnalysis {
    /// Total number of matched groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total number of bottom regions across all groups.
    pub fn total_bottoms(&self) -> usize {
        self.groups.iter().map(|g| g.bottoms.len()).sum()
    }

    /// Groups where the top has timing and at least one bottom has timing.
    pub fn timed_groups(&self) -> impl Iterator<Item = &OverlapGroup> {
        self.groups
            .iter()
            .filter(|g| g.top.bullet.is_some() && g.bottoms.iter().any(|b| b.bullet.is_some()))
    }

    /// Whether there are any overlap markers in the file at all.
    pub fn has_overlaps(&self) -> bool {
        !self.groups.is_empty()
            || !self.orphaned_tops.is_empty()
            || !self.orphaned_bottoms.is_empty()
    }
}

/// Analyze cross-utterance overlap groups for an entire file.
///
/// Extracts overlap regions from each utterance, then matches top regions
/// (⌈...⌉) with bottom regions (⌊...⌋) across speakers. Supports 1:N
/// matching, one top can have multiple bottoms from different speakers.
pub fn analyze_file_overlaps(lines: &[Line]) -> FileOverlapAnalysis {
    // Step 1: Extract per-utterance overlap info.
    let mut per_utterance: Vec<PerUtteranceOverlap> = Vec::new();
    for line in lines {
        if let Line::Utterance(utt) = line {
            let info = extract_overlap_info(utt.main.content.content.as_slice());
            let bullet = utt
                .main
                .content
                .bullet
                .as_ref()
                .map(|b| (b.timing.start_ms, b.timing.end_ms));
            per_utterance.push(PerUtteranceOverlap {
                utterance_index: per_utterance.len(),
                speaker: utt.main.speaker.clone(),
                info,
                bullet,
            });
        }
    }

    // Step 2: Collect all top and bottom anchors.
    let mut tops: Vec<OverlapAnchor> = Vec::new();
    let mut bottoms: Vec<OverlapAnchor> = Vec::new();

    for pu in &per_utterance {
        for region in &pu.info.regions {
            let anchor = OverlapAnchor {
                utterance_index: pu.utterance_index,
                speaker: pu.speaker.clone(),
                region: region.clone(),
                bullet: pu.bullet,
            };
            match region.kind {
                OverlapRegionKind::Top if region.has_begin() => tops.push(anchor),
                OverlapRegionKind::Bottom if region.has_begin() => bottoms.push(anchor),
                _ => {} // Orphaned closings without openings, skip
            }
        }
    }

    // Step 3: Match each bottom to its nearest preceding top from a different
    // speaker with the same index. Build groups.
    let mut top_to_bottoms: Vec<Vec<OverlapAnchor>> = vec![Vec::new(); tops.len()];
    let mut bottom_matched: Vec<bool> = vec![false; bottoms.len()];

    for (bi, bottom) in bottoms.iter().enumerate() {
        // Search backward through tops for a match.
        let mut best_top: Option<usize> = None;
        for (ti, top) in tops.iter().enumerate().rev() {
            // Must be from a different speaker.
            if top.speaker == bottom.speaker {
                continue;
            }
            // Must have the same index (or both unindexed).
            if top.region.index != bottom.region.index {
                continue;
            }
            // Must precede or be at the same utterance position.
            if top.utterance_index > bottom.utterance_index {
                continue;
            }
            // Distribution guard for same-speaker-pair siblings: when this
            // bottom's speaker already has a sibling attached to this top AND
            // a vacant sibling top exists (same speaker, same utterance, same
            // index, no bottom from this speaker yet), skip so the bottoms
            // distribute instead of collapsing. Without it two FM bottoms
            // land on one AM top while a second AM top sits empty.
            //
            // This lived only in a near-copy of this function inside
            // `validation::cross_utterance`, so the validator and this
            // analysis disagreed about the same transcript. The copy is gone;
            // this is the one owner.
            let already_has_same_speaker = top_to_bottoms[ti]
                .iter()
                .any(|b| b.speaker == bottom.speaker);
            if already_has_same_speaker {
                let has_vacant_sibling = tops.iter().enumerate().any(|(oti, other_top)| {
                    oti != ti
                        && other_top.speaker == top.speaker
                        && other_top.utterance_index == top.utterance_index
                        && other_top.region.index == top.region.index
                        && other_top.utterance_index <= bottom.utterance_index
                        && !top_to_bottoms[oti]
                            .iter()
                            .any(|b| b.speaker == bottom.speaker)
                });
                if has_vacant_sibling {
                    continue;
                }
            }
            best_top = Some(ti);
            break;
        }

        if let Some(ti) = best_top {
            top_to_bottoms[ti].push(bottom.clone());
            bottom_matched[bi] = true;
        }
    }

    // Step 4: Build groups and collect orphans.
    let mut groups: Vec<OverlapGroup> = Vec::new();
    let mut orphaned_tops: Vec<OverlapAnchor> = Vec::new();

    for (ti, top) in tops.into_iter().enumerate() {
        let matched_bottoms = std::mem::take(&mut top_to_bottoms[ti]);
        if matched_bottoms.is_empty() {
            orphaned_tops.push(top);
        } else {
            groups.push(OverlapGroup {
                top,
                bottoms: matched_bottoms,
            });
        }
    }

    let orphaned_bottoms: Vec<OverlapAnchor> = bottoms
        .into_iter()
        .zip(bottom_matched)
        .filter(|(_, matched)| !*matched)
        .map(|(b, _)| b)
        .collect();

    FileOverlapAnalysis {
        groups,
        orphaned_tops,
        orphaned_bottoms,
        per_utterance,
    }
}
