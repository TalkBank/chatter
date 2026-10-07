//! Per-child `%wor` partition and the main-tier timing it can still measure.
use talkbank_model::alignment::{
    WorTimingBinding, WorTimingCorrespondence, WorTimingSequence, assess_wor_timing_sequence,
    bind_wor_timing, corroborate_wor_timing,
};
use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::model::{Bullet, MainTier, Utterance, WorTier};

/// Build a per-child `%wor` from the parent tier by walking main-tier words
/// in lockstep with `%wor` Word-items.
///
/// Returns `None` unless chatter proves both equal policy-selected counts and
/// canonical lexical correspondence. On `None`, the caller drops the tier
/// from all children, matching the existing stale-`%wor`-is-fine behavior,
/// never raising a validation error.
///
/// `main_word_groups` is the per-main-tier-word child-group assignment, in
/// main-tier word order, restricted to `%wor`-eligible words (the same
/// filtering `TierDomain::Wor` uses: untranscribed, fragments, and nonwords
/// are excluded; fillers are included).
pub(super) fn partition_wor_tier(
    main: &MainTier,
    parent: &WorTier,
    main_word_groups: &[usize],
    num_groups: usize,
) -> Option<PartitionedWorTiers> {
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
    if corroborated.slots().len() != main_word_groups.len() {
        tracing::debug!(
            chatter_projection_words = corroborated.slots().len(),
            batchalign_projection_words = main_word_groups.len(),
            "%wor membership implementations disagree on split, dropping timing tier"
        );
        return None;
    }

    // Walk parent items, tracking which main-tier word index we're on for
    // Word items. Separators have no main-tier counterpart; we attach them
    // to the same child as the most recent Word, falling back to group 0
    // if we haven't seen any Word yet.
    let mut per_child: Vec<Vec<WorItem>> = vec![Vec::new(); num_groups];
    let mut next_word_idx = 0usize;
    let mut last_seen_group: Option<usize> = None;
    for item in &parent.items {
        match item {
            WorItem::Word(_) => {
                let group = main_word_groups[next_word_idx];
                last_seen_group = Some(group);
                per_child[group].push(item.clone());
                next_word_idx += 1;
            }
            WorItem::Separator { .. } => {
                let group = last_seen_group.unwrap_or(0);
                per_child[group].push(item.clone());
            }
        }
    }

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
    pub(super) fn get(&self, group_idx: usize) -> Option<&WorTier> {
        self.0.get(group_idx)
    }

    /// Consume the partition: every per-child tier with its group index, in
    /// group order, moved rather than copied.
    pub(super) fn into_groups(self) -> impl Iterator<Item = (usize, WorTier)> {
        self.0.into_iter().enumerate()
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

pub(super) fn split_main_timing_evidence(
    partitioned_wor: Option<&PartitionedWorTiers>,
    children: &[(usize, Utterance)],
) -> SplitMainTimingEvidence {
    // One route into the complete state: a partitioned tier, a hull for EVERY
    // kept child, and at least one child. The three refusals used to be three
    // early returns that each named the fallback, which is how the fallback
    // came to be spelled out three times over.
    let complete = partitioned_wor.and_then(|per_group| {
        children
            .iter()
            .map(|(group_idx, child)| {
                per_group
                    .get(*group_idx)
                    .and_then(|wor| complete_wor_timing_hull(&child.main, wor))
            })
            .collect::<Option<Vec<_>>>()
            .filter(|bullets| !bullets.is_empty())
    });
    match complete {
        Some(bullets) => {
            SplitMainTimingEvidence::CompletePerChild(CompletePerChildMainTiming { bullets })
        }
        None => SplitMainTimingEvidence::Unmeasured,
    }
}
