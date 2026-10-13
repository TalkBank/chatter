//! Per-child `%wor` partition and the main-tier timing it can still measure.
use talkbank_model::alignment::{
    WorTimingBinding, WorTimingCorrespondence, WorTimingSequence, assess_wor_timing_sequence,
    bind_wor_timing, corroborate_wor_timing,
};
use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::model::{Bullet, MainTier, WorTier};

use super::content_groups::{ContentItemGroups, RebuiltChildren};

/// Build a per-child `%wor` from canonical, source-rooted timing slots.
///
/// Returns `None` unless chatter proves both equal policy-selected counts and
/// canonical lexical correspondence. On `None`, the caller drops the tier
/// from all children, matching the existing stale-`%wor`-is-fine behavior,
/// never raising a validation error.
///
/// Projection retains each selected word's root content position. That
/// position consumes the admitted groups' source-ordered content iterator;
/// no separate word walk or membership-agreement check can drift from it.
pub(super) fn partition_wor_tier<Affiliation>(
    parent: &WorTier,
    content_groups: &ContentItemGroups<'_, Affiliation>,
) -> Option<PartitionedWorTiers> {
    let main = &content_groups.source().main;
    let count_matched = match bind_wor_timing(main, Some(parent)) {
        WorTimingBinding::CountMatched(count_matched) => count_matched,
        WorTimingBinding::Drifted(drift) => {
            tracing::debug!(
                parent_wor_words = drift.wor_count().get(),
                main_eligible_words = drift.main_count().get(),
                "%wor count mismatch on split, dropping tier (stale %wor expected after prior edits)"
            );
            return None;
        }
        WorTimingBinding::Missing(_) => {
            tracing::debug!("%wor binding unexpectedly reported a missing tier during split");
            return None;
        }
    };
    let corroborated = match corroborate_wor_timing(count_matched) {
        WorTimingCorrespondence::Corroborated(corroborated) => corroborated,
        WorTimingCorrespondence::Uncorroborated(uncorroborated) => {
            tracing::debug!(
                lexical_mismatches = uncorroborated.mismatches().len(),
                "%wor lexical mismatch on split, dropping stale timing tier"
            );
            return None;
        }
    };
    // Both binding and grouping originate from content_groups.source().
    // Canonical root positions are nondecreasing, including repeated positions
    // for nested words, so this is one linear content scan without word walks
    // or a second independently indexed word/group vector.
    let mut per_child: Vec<Vec<WorItem>> = vec![Vec::new(); content_groups.group_count()];
    let mut by_content = content_groups.items_with_groups().enumerate().peekable();
    let mut source_group = content_groups.first_group();
    let mut last_seen_group = content_groups.first_group();
    let mut parent_items = parent.items.iter();
    for slot in corroborated.slots() {
        while let Some((_, (_, group))) =
            by_content.next_if(|(index, _)| *index <= slot.source_content().index())
        {
            source_group = group;
        }
        // Count matching proved one parent Word-item for each slot. Consume
        // its preceding separators using the last actual word affiliation;
        // leading separators retain the source's admitted first child.
        for item in parent_items.by_ref() {
            match item {
                WorItem::Word(_) => {
                    per_child[source_group].push(item.clone());
                    last_seen_group = source_group;
                    break;
                }
                WorItem::Separator { .. } => {
                    per_child[last_seen_group].push(item.clone());
                }
            }
        }
    }
    // Every Word-item was consumed above. Remaining items are separators,
    // including the all-separator case when the canonical slot set was empty.
    per_child[last_seen_group].extend(parent_items.cloned());

    // Build a WorTier for each child. Children with empty item lists get an
    // empty WorTier; the caller filters those out (we don't emit empty
    // `%wor:` tiers).
    Some(PartitionedWorTiers(
        per_child
            .into_iter()
            .map(|items| {
                // Each child tier is derived from the parent tier, so it
                // carries the parent's span as a coarse source pointer
                // rather than a zero span indistinguishable from byte 0.
                WorTier::new(items)
                    .with_language_code(parent.language_code.clone())
                    .with_span(parent.span)
            })
            .collect(),
    ))
}

/// Per-child `%wor` tiers admitted by count and lexical corroboration.
///
/// The inner vector stays private so split code cannot accidentally treat an
/// uncorroborated positional partition as reusable timing evidence.
pub(super) struct PartitionedWorTiers(Vec<WorTier>);

impl PartitionedWorTiers {
    /// Consume every tier in the admitted group's dense order, moving it once.
    pub(super) fn into_groups(self) -> impl Iterator<Item = WorTier> {
        self.0.into_iter()
    }
}

/// Complete child timing rederived from the partitioned `%wor` evidence.
///
/// Construction is private to [`split_main_timing_evidence`], which produces
/// exactly one bullet for every kept child or refuses this state entirely.
pub(super) struct CompletePerChildMainTiming {
    pub(super) bullets: Vec<Bullet>,
}

/// Mutually exclusive timing evidence available after an utterance split.
pub(super) enum SplitMainTimingEvidence {
    /// Every kept child's own `%wor` words were timed, so each child has a
    /// measured hull of its own.
    CompletePerChild(CompletePerChildMainTiming),
    /// No complete per-child evidence, so no child receives a main-tier
    /// bullet. The parent's bullet measures the whole parent: its start is
    /// where the first child began and its end where the last one finished,
    /// and nothing measured the boundaries between them. A split always keeps
    /// two or more children (an assignment naming one child is
    /// [`SplitOutcome::Unchanged`] and never reaches the rebuild), so the
    /// parent's span is no child's span, and writing it onto one would claim
    /// a time nobody observed.
    Unmeasured,
}

/// Derive one enclosing hull only when every `%wor` word is timed.
///
/// A partial tier cannot claim the full child span, so one missing word bullet
/// makes this return `None` and leaves the whole split unmeasured. Min/max is intentional: it encloses all admitted word spans even if
/// their serialized order contains a local timing inversion.
fn complete_wor_timing_hull(main: &MainTier, wor: &WorTier) -> Option<Bullet> {
    let count_matched = match bind_wor_timing(main, Some(wor)) {
        WorTimingBinding::CountMatched(count_matched) => count_matched,
        WorTimingBinding::Missing(_) | WorTimingBinding::Drifted(_) => return None,
    };
    let corroborated = match corroborate_wor_timing(count_matched) {
        WorTimingCorrespondence::Corroborated(corroborated) => corroborated,
        WorTimingCorrespondence::Uncorroborated(_) => return None,
    };
    let complete = match assess_wor_timing_sequence(corroborated) {
        WorTimingSequence::Complete(complete) => complete,
        WorTimingSequence::Empty(_) | WorTimingSequence::Rejected(_) => return None,
    };
    let hull = complete.hull();
    Some(Bullet::new(hull.start().get(), hull.end().get()))
}

pub(super) fn split_main_timing_evidence(children: &RebuiltChildren) -> SplitMainTimingEvidence {
    // RebuiltChildren already retains two or more children. Timing comes from
    // the actual attached tiers, not a second independently indexed partition.
    let complete = children
        .as_slice()
        .iter()
        .map(|child| {
            child
                .wor_tier()
                .and_then(|wor| complete_wor_timing_hull(&child.main, wor))
        })
        .collect::<Option<Vec<_>>>();
    match complete {
        Some(bullets) => {
            SplitMainTimingEvidence::CompletePerChild(CompletePerChildMainTiming { bullets })
        }
        None => SplitMainTimingEvidence::Unmeasured,
    }
}
