//! Main-tier to `%wor` timing sidecar.
//!
//! `%wor` is **not** a structural tier alignment. It is a timing-annotation
//! sidecar: a record of bullets attached to the subset of main-tier words
//! that passed the Wor-domain filter at the moment forced alignment ran.
//!
//! The older `WorAlignment` / `align_main_to_wor` /
//! `WorTier: AlignableTier` design modeled `%wor` as a fifth positional
//! alignment alongside `%mor`, `%gra`, `%pho`, `%sin`. It didn't fit: count
//! mismatches are tolerated (stale `%wor` after main-tier edits is
//! legitimate), and error codes were never surfaced. This module replaces
//! that machinery with a sidecar summary plus [`WorTimingBinding`]. Binding
//! proves only whether counts match under a named membership policy. A
//! count-matched pair must then pass [`corroborate_wor_timing`], which compares
//! the canonical display-token sequences and refuses detectable same-count
//! edits without treating `%wor` text as lexical authority.
//! [`assess_wor_timing_sequence`] provides a third transition for
//! consumers that require complete positive timing plus explicit adjacency
//! geometry and a min/max timing hull.
//! None of these transitions claims acoustic accuracy.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

use crate::model::{Bullet, MainTier, WorTier, Word};
use schemars::JsonSchema;
use talkbank_derive::SpanShift;

/// Describes the correspondence between the main tier's Wor-filtered words
/// and the entries in a `%wor` tier, at the moment of read.
///
/// Variants express the observed count relationship, not *whether validation
/// passes* or whether common origin is proved. `%wor` has no validation
/// contract against the main tier; the presence of drift is a fact to report
/// to callers, not an error.
///
/// - [`Positional`](Self::Positional), filtered counts matched, so legacy
///   metadata records that the positional convention is available. Equal
///   counts do not prove that the two sequences share origin after a
///   same-count edit. Timing consumers should use [`bind_wor_timing`] so the
///   evidence limitation remains explicit in the returned state.
/// - [`Drifted`](Self::Drifted), filtered counts differ (typically a main
///   tier edit after `align` without a re-run). No positional correspondence
///   is available; timing recovery must be skipped or the file must be
///   re-aligned. Carries the two counts so callers can log or display them.
///
/// `None` at the containing [`Option<WorTimingSidecar>`] level on
/// [`AlignmentSet`](crate::model::AlignmentSet) means the utterance has no
/// `%wor` tier at all.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, JsonSchema, SpanShift,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorTimingSidecar {
    /// Filtered main-tier and `%wor` word counts match. Position is the legacy
    /// correspondence convention; `count` is the common length.
    Positional {
        /// Shared length of the Wor-filtered main sequence and `%wor.words()`.
        count: usize,
    },
    /// Filtered counts differ. No positional correspondence is defined.
    Drifted {
        /// Number of Wor-filtered main-tier words.
        main_count: usize,
        /// Number of `%wor.words()` entries.
        wor_count: usize,
    },
}

mod projection;
pub use projection::{WorMainTierProjection, WorSlotMembershipPolicy};

mod correspondence;
pub use correspondence::{
    CorroboratedWorTimingSlot, CorroboratedWorTimings, UncorroboratedWorTimings,
    WorLexicalMismatch, WorTimingCorrespondence, corroborate_wor_timing,
};

/// Number of main-tier slots admitted by the selected `%wor` policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MainWorSlotCount(usize);

impl MainWorSlotCount {
    /// Return the primitive count at reporting or comparison boundaries.
    pub fn get(self) -> usize {
        self.0
    }
}

impl std::fmt::Display for MainWorSlotCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Number of word entries physically present on a `%wor` tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorTierSlotCount(usize);

impl WorTierSlotCount {
    /// Return the primitive count at reporting or comparison boundaries.
    pub fn get(self) -> usize {
        self.0
    }
}

/// Parsed `%wor` timing state for one main tier.
///
/// [`CountMatched`](Self::CountMatched) permits the next correspondence check
/// but exposes no timing slots. A caller cannot zip a count-drifted pair
/// accidentally because [`Drifted`](Self::Drifted) carries only a diagnostic
/// witness. Absence is distinct from a present empty, count-matched tier
/// through [`Missing`](Self::Missing).
#[derive(Debug, PartialEq)]
#[must_use = "timing binding state must be handled before `%wor` timing is used"]
pub enum WorTimingBinding<'main> {
    /// No `%wor` tier was present.
    Missing(MissingWorTimings),
    /// Main-policy and `%wor` slot counts differed.
    Drifted(WorTimingDrift),
    /// Slot counts matched under the named policy. Position supplies the
    /// correspondence convention, but equal counts do not prove common origin.
    CountMatched(CountMatchedWorTimings<'main>),
}

/// Evidence that an utterance has no `%wor` tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingWorTimings {
    policy: WorSlotMembershipPolicy,
    main_count: MainWorSlotCount,
}

impl MissingWorTimings {
    /// Membership policy used to count eligible main-tier slots.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Eligible main-tier slot count despite the missing sidecar.
    pub fn main_count(&self) -> MainWorSlotCount {
        self.main_count
    }
}

/// Evidence that a main tier and `%wor` tier cannot be bound positionally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorTimingDrift {
    policy: WorSlotMembershipPolicy,
    main_count: MainWorSlotCount,
    wor_count: WorTierSlotCount,
}

impl WorTimingDrift {
    /// Membership policy used to count eligible main-tier slots.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Eligible main-tier slot count.
    pub fn main_count(&self) -> MainWorSlotCount {
        self.main_count
    }

    /// Physical `%wor` word-entry count.
    pub fn wor_count(&self) -> WorTierSlotCount {
        self.wor_count
    }
}

/// Main-tier and `%wor` lexical slots after equal counts were proved.
///
/// Construction is private. The only public constructor is
/// [`bind_wor_timing`], which proves only equal slot counts under the named
/// policy. This state exposes only the shared count. Its private sequences are
/// consumed by [`corroborate_wor_timing`], so a caller cannot read timing or
/// construct a positional zip before canonical token correspondence succeeds.
#[derive(Debug, PartialEq)]
pub struct CountMatchedWorTimings<'main> {
    policy: WorSlotMembershipPolicy,
    main_slots: Vec<&'main Word>,
    wor_slots: Vec<&'main Word>,
}

impl<'main> CountMatchedWorTimings<'main> {
    /// Membership policy whose equal-count result admitted this pairing.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Shared slot count proved by the count-matching transition.
    pub fn slot_count(&self) -> MainWorSlotCount {
        MainWorSlotCount(self.main_slots.len())
    }
}

/// Timing state of one count-matched and corroborated main-tier slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorSlotTiming {
    /// The `%wor` slot carried an inline media bullet.
    Timed(WorRecordedInterval),
    /// The `%wor` slot existed but carried no inline media bullet.
    Unaligned,
}

/// Zero-based position in a count-matched `%wor` sequence.
///
/// Construction is private. An index is evidence emitted by chatter's
/// sequence assessment, not a caller-supplied offset into an unrelated tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorSlotIndex(usize);

impl WorSlotIndex {
    /// Return the primitive index at reporting boundaries.
    pub fn get(self) -> usize {
        self.0
    }
}

impl std::fmt::Display for WorSlotIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Millisecond media offset recorded on a corroborated `%wor` bullet.
///
/// Construction is private so arbitrary arithmetic cannot be presented as a
/// recorded `%wor` coordinate. Use [`Self::get`] only at reporting or wire
/// boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorMediaOffsetMs(u64);

impl WorMediaOffsetMs {
    /// Return the primitive offset at reporting boundaries.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for WorMediaOffsetMs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Millisecond duration derived from complete `%wor` timing geometry.
///
/// This is a different type from a recorded media offset because subtraction,
/// not parsing, produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorDurationMs(u64);

impl WorDurationMs {
    /// Return the primitive duration at reporting boundaries.
    pub fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for WorDurationMs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Start and end offsets recorded on one admitted `%wor` bullet.
///
/// This type does not assert positive duration. Positivity is established by
/// [`assess_wor_timing_sequence`] before an interval reaches
/// [`CompleteWorTimingSlot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorRecordedInterval {
    start: WorMediaOffsetMs,
    end: WorMediaOffsetMs,
}

impl WorRecordedInterval {
    /// Recorded start offset.
    pub fn start(self) -> WorMediaOffsetMs {
        self.start
    }

    /// Recorded end offset.
    pub fn end(self) -> WorMediaOffsetMs {
        self.end
    }

    fn from_bullet(bullet: &Bullet) -> Self {
        Self {
            start: WorMediaOffsetMs(bullet.timing.start_ms),
            end: WorMediaOffsetMs(bullet.timing.end_ms),
        }
    }
}

/// Result of checking whether a count-matched `%wor` sequence can supply complete
/// positive word timing and a location hull.
///
/// This transition checks only facts represented in CHAT. It does not claim
/// that the boundaries are acoustically accurate.
#[derive(Debug, PartialEq)]
#[must_use = "timing sequence state must be handled before a `%wor` hull is used"]
pub enum WorTimingSequence<'main> {
    /// The binding was valid but contained no policy-selected word slots.
    Empty(EmptyWorTimingSequence),
    /// At least one slot was untimed or non-positive, so no complete timing
    /// sequence is exposed.
    Rejected(RejectedWorTimingSequence),
    /// Every slot was timed with a positive interval. Adjacency geometry is
    /// retained separately rather than treated as acoustic correctness.
    Complete(CompleteWorTimings<'main>),
}

/// Evidence that a corroborated `%wor` sequence contains no slots and has no hull.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyWorTimingSequence {
    policy: WorSlotMembershipPolicy,
    slot_count: MainWorSlotCount,
}

impl EmptyWorTimingSequence {
    /// Membership policy used by the empty binding.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Number of slots in the empty binding.
    pub fn slot_count(&self) -> MainWorSlotCount {
        self.slot_count
    }
}

/// Evidence that a corroborated `%wor` sequence cannot supply a complete timing hull.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedWorTimingSequence {
    policy: WorSlotMembershipPolicy,
    slot_count: MainWorSlotCount,
    issues: Vec<WorTimingSequenceIssue>,
}

impl RejectedWorTimingSequence {
    /// Membership policy used by the rejected binding.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Number of main-tier slots examined.
    pub fn slot_count(&self) -> MainWorSlotCount {
        self.slot_count
    }

    /// Exhaustive issues found while assessing the sequence.
    pub fn issues(&self) -> &[WorTimingSequenceIssue] {
        &self.issues
    }
}

/// CHAT-representable reason a corroborated `%wor` sequence has no complete hull.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorTimingSequenceIssue {
    /// The positional slot existed but carried no bullet.
    Unaligned {
        /// Main-tier slot without timing.
        slot: WorSlotIndex,
    },
    /// A bullet did not have positive duration.
    NonPositiveInterval {
        /// Main-tier slot carrying the interval.
        slot: WorSlotIndex,
        /// Recorded start offset.
        start: WorMediaOffsetMs,
        /// Recorded end offset.
        end: WorMediaOffsetMs,
    },
}

/// Relationship between two adjacent complete positive word intervals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorAdjacentTimingRelation {
    /// The current interval begins after the preceding interval ends.
    Gap {
        /// Preceding main-tier slot.
        previous_slot: WorSlotIndex,
        /// Current main-tier slot.
        current_slot: WorSlotIndex,
        /// Distance between the preceding end and current start.
        duration: WorDurationMs,
    },
    /// The current interval begins exactly when the preceding interval ends.
    Touching {
        /// Preceding main-tier slot.
        previous_slot: WorSlotIndex,
        /// Current main-tier slot.
        current_slot: WorSlotIndex,
    },
    /// Starts are nondecreasing, but the intervals overlap.
    Overlap {
        /// Preceding main-tier slot.
        previous_slot: WorSlotIndex,
        /// Current main-tier slot.
        current_slot: WorSlotIndex,
        /// Amount by which the current start precedes the previous end.
        duration: WorDurationMs,
    },
    /// The current start itself moves backwards in media time.
    BackwardStart {
        /// Preceding main-tier slot.
        previous_slot: WorSlotIndex,
        /// Current main-tier slot.
        current_slot: WorSlotIndex,
        /// Amount by which the current start precedes the previous start.
        regression: WorDurationMs,
    },
}

/// Complete positive word timings in main-tier order.
///
/// Only this state exposes a timing hull. Gap, touch, overlap, and backwards
/// start geometry remain explicit. This proves coverage and positive duration,
/// not acoustic boundary quality or model confidence.
#[derive(Debug, PartialEq)]
pub struct CompleteWorTimings<'main> {
    policy: WorSlotMembershipPolicy,
    slots: Vec<CompleteWorTimingSlot<'main>>,
    adjacencies: Vec<WorAdjacentTimingRelation>,
    hull: WorTimingHull,
}

impl<'main> CompleteWorTimings<'main> {
    /// Membership policy used by the complete binding.
    pub fn membership_policy(&self) -> WorSlotMembershipPolicy {
        self.policy
    }

    /// Complete timed slots in main-tier order.
    pub fn slots(&self) -> &[CompleteWorTimingSlot<'main>] {
        &self.slots
    }

    /// Geometry between every pair of adjacent slots.
    pub fn adjacencies(&self) -> &[WorAdjacentTimingRelation] {
        &self.adjacencies
    }

    /// Minimum recorded onset through maximum recorded offset.
    pub fn hull(&self) -> WorTimingHull {
        self.hull
    }
}

/// One main-tier word whose `%wor` timing passed completeness assessment.
#[derive(Debug, PartialEq)]
pub struct CompleteWorTimingSlot<'main> {
    main_word: &'main Word,
    timing: WorRecordedInterval,
}

impl<'main> CompleteWorTimingSlot<'main> {
    /// Typed main-tier word that owns lexical identity.
    pub fn main_word(&self) -> &'main Word {
        self.main_word
    }

    /// Cleaned lexical text from the main-tier word.
    pub fn main_text(&self) -> &str {
        self.main_word.cleaned_text()
    }

    /// Positive `%wor` timing observation.
    pub fn timing(&self) -> WorRecordedInterval {
        self.timing
    }

    /// Positive duration derived from this complete interval.
    pub fn duration(&self) -> WorDurationMs {
        WorDurationMs(self.timing.end.0 - self.timing.start.0)
    }
}

/// Derived min/max temporal hull of complete positive `%wor` timing.
///
/// Construction is private so callers cannot label arbitrary offsets as a
/// hull that passed sequence assessment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorTimingHull {
    start: WorMediaOffsetMs,
    end: WorMediaOffsetMs,
}

impl WorTimingHull {
    /// Minimum recorded start among all complete word intervals.
    pub fn start(self) -> WorMediaOffsetMs {
        self.start
    }

    /// Maximum recorded end among all complete word intervals.
    pub fn end(self) -> WorMediaOffsetMs {
        self.end
    }

    /// Positive duration covered by the hull.
    pub fn duration(self) -> WorDurationMs {
        WorDurationMs(self.end.0 - self.start.0)
    }
}

/// Assess a lexically corroborated `%wor` sidecar for complete positive timing.
///
/// A complete result requires every slot to carry a positive interval.
/// Adjacency geometry is retained as gap, touch, overlap, or backwards-start
/// evidence. Rejected states expose issues but no partial slot pairing or hull.
pub fn assess_wor_timing_sequence(
    corroborated: CorroboratedWorTimings<'_>,
) -> WorTimingSequence<'_> {
    let CorroboratedWorTimings { policy, slots } = corroborated;
    let slot_count = MainWorSlotCount(slots.len());
    let mut indexed_slots = slots.into_iter().enumerate();
    let Some((first_raw_index, first_slot)) = indexed_slots.next() else {
        return WorTimingSequence::Empty(EmptyWorTimingSequence { policy, slot_count });
    };

    let first_index = WorSlotIndex(first_raw_index);
    let first_complete = match assess_wor_timing_slot(first_index, first_slot) {
        Ok(complete) => complete,
        Err(first_issue) => {
            let mut issues = vec![first_issue];
            for (raw_index, slot) in indexed_slots {
                if let Err(issue) = assess_wor_timing_slot(WorSlotIndex(raw_index), slot) {
                    issues.push(issue);
                }
            }
            return WorTimingSequence::Rejected(RejectedWorTimingSequence {
                policy,
                slot_count,
                issues,
            });
        }
    };

    let mut hull = WorTimingHull {
        start: first_complete.timing.start,
        end: first_complete.timing.end,
    };
    let mut previous = (first_index, first_complete.timing);
    let mut complete_slots = Vec::with_capacity(slot_count.get());
    complete_slots.push(first_complete);
    let mut adjacencies = Vec::with_capacity(slot_count.get().saturating_sub(1));
    let mut issues = Vec::new();

    for (raw_index, slot) in indexed_slots {
        let index = WorSlotIndex(raw_index);
        match assess_wor_timing_slot(index, slot) {
            Ok(complete) => {
                if issues.is_empty() {
                    adjacencies.push(wor_adjacency(
                        previous.0,
                        previous.1,
                        index,
                        complete.timing,
                    ));
                    hull.start = hull.start.min(complete.timing.start);
                    hull.end = hull.end.max(complete.timing.end);
                    previous = (index, complete.timing);
                    complete_slots.push(complete);
                }
            }
            Err(issue) => issues.push(issue),
        }
    }

    if !issues.is_empty() {
        return WorTimingSequence::Rejected(RejectedWorTimingSequence {
            policy,
            slot_count,
            issues,
        });
    }

    WorTimingSequence::Complete(CompleteWorTimings {
        policy,
        slots: complete_slots,
        adjacencies,
        hull,
    })
}

fn assess_wor_timing_slot(
    index: WorSlotIndex,
    slot: CorroboratedWorTimingSlot<'_>,
) -> Result<CompleteWorTimingSlot<'_>, WorTimingSequenceIssue> {
    let WorSlotTiming::Timed(interval) = slot.timing else {
        return Err(WorTimingSequenceIssue::Unaligned { slot: index });
    };

    if interval.start >= interval.end {
        return Err(WorTimingSequenceIssue::NonPositiveInterval {
            slot: index,
            start: interval.start,
            end: interval.end,
        });
    }

    Ok(CompleteWorTimingSlot {
        main_word: slot.main_word,
        timing: interval,
    })
}

fn wor_adjacency(
    previous_slot: WorSlotIndex,
    previous: WorRecordedInterval,
    current_slot: WorSlotIndex,
    current: WorRecordedInterval,
) -> WorAdjacentTimingRelation {
    if current.start < previous.start {
        WorAdjacentTimingRelation::BackwardStart {
            previous_slot,
            current_slot,
            regression: WorDurationMs(previous.start.0 - current.start.0),
        }
    } else if current.start < previous.end {
        WorAdjacentTimingRelation::Overlap {
            previous_slot,
            current_slot,
            duration: WorDurationMs(previous.end.0 - current.start.0),
        }
    } else if current.start == previous.end {
        WorAdjacentTimingRelation::Touching {
            previous_slot,
            current_slot,
        }
    } else {
        WorAdjacentTimingRelation::Gap {
            previous_slot,
            current_slot,
            duration: WorDurationMs(current.start.0 - previous.end.0),
        }
    }
}

/// Bind a possibly absent `%wor` timing sidecar to its main tier.
///
/// Main-tier lexical slots are extracted with the canonical Wor-domain policy.
/// Equal slot counts produce [`WorTimingBinding::CountMatched`]; unequal counts
/// fail closed as [`WorTimingBinding::Drifted`]. Count matching does not expose
/// timing and does not prove common origin after a same-count main-tier edit.
/// The count-matched state retains both typed sequences privately for the
/// required [`corroborate_wor_timing`] transition. Generation and binding share
/// the same [`WorMainTierProjection`], so there is no second membership
/// implementation that can disagree.
pub fn bind_wor_timing<'main>(
    main: &'main MainTier,
    wor: Option<&'main WorTier>,
) -> WorTimingBinding<'main> {
    main.wor_projection().bind_timing(wor)
}

impl WorTimingSidecar {
    /// Returns `true` when the legacy metadata records equal counts.
    #[deprecated(
        note = "timing consumers should use bind_wor_timing followed by corroborate_wor_timing"
    )]
    pub fn is_positional(&self) -> bool {
        matches!(self, Self::Positional { .. })
    }

    /// Returns the shared legacy count when positional, otherwise `None`.
    #[deprecated(
        note = "timing consumers should use bind_wor_timing followed by corroborate_wor_timing"
    )]
    pub fn positional_count(&self) -> Option<usize> {
        match self {
            Self::Positional { count } => Some(*count),
            Self::Drifted { .. } => None,
        }
    }
}

/// Resolve legacy count metadata between a main tier and its `%wor` tier.
///
/// Counts Wor-filtered alignable words on the main tier (via
/// [`crate::alignment::TierDomain::Wor`]) and words on the `%wor` tier, then
/// returns either
/// [`WorTimingSidecar::Positional`] (counts match under the legacy positional
/// convention) or [`WorTimingSidecar::Drifted`] (counts differ). Equal counts
/// do not prove common origin. Timing-consuming code should use
/// [`bind_wor_timing`] followed by [`corroborate_wor_timing`].
///
/// This function never produces a [`ParseError`](crate::ParseError),
/// mismatch is a fact about the pair, not a validation failure.
pub fn resolve_wor_timing_sidecar(main: &MainTier, wor: &WorTier) -> WorTimingSidecar {
    let main_count = main.wor_projection().slot_count().get();
    let wor_count = wor.word_count();
    if main_count == wor_count {
        WorTimingSidecar::Positional { count: main_count }
    } else {
        WorTimingSidecar::Drifted {
            main_count,
            wor_count,
        }
    }
}

#[cfg(test)]
mod tests;
