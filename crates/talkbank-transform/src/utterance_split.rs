//! Source-bound admission and partitioning of typed CHAT utterances.
//!
//! This proves a structural split, not complete CHAT validity or output-write
//! permission. Rebuilt children still require checked construction.

use std::collections::HashSet;

use talkbank_model::alignment::WorSlotMembershipPolicy;
use talkbank_model::alignment::helpers::{PositionalDomain, TierDomain, WordItem, walk_words};
use talkbank_model::model::{DependentTier, MainTier, Terminator, Utterance, UtteranceContent};

use crate::extract;

/// A proposed boundary that cannot preserve the producing CHAT structure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplitRefusal {
    /// The selected domain and assignment vector do not describe the same slots.
    #[error("split assignments have {actual} slots; this source has {expected}")]
    SlotCount {
        /// Source-owned slot count.
        expected: usize,
        /// Caller-supplied slot count.
        actual: usize,
    },
    /// A child label occurs in disjoint runs and would reorder source content.
    #[error("split group {group} reappears after another child")]
    DisjointGroup {
        /// The proposed child label.
        group: usize,
    },
    /// One indivisible top-level content item would belong to different children.
    #[error("split boundary crosses indivisible content item {content_index}")]
    IndivisibleContent {
        /// Index in this source's main-tier content.
        content_index: usize,
    },
    /// A boundary would strand punctuation at the start of a child.
    #[error("split boundary strands a separator at content item {content_index}")]
    LeadingSeparator {
        /// Index in this source's main-tier content.
        content_index: usize,
    },
}

/// Why an existing dependent tier cannot describe the partitioned children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierInvalidationReason {
    /// Analysis or positional references depended on the original boundaries.
    BoundaryDependent,
    /// The original word tier could not establish count and lexical correspondence.
    UncorroboratedWordTiming,
}

/// A source-bound receipt for one invalidated dependent tier.
pub struct InvalidatedTier<'source> {
    index: usize,
    tier: &'source DependentTier,
    reason: TierInvalidationReason,
}

impl InvalidatedTier<'_> {
    /// Index in the producing utterance's dependent-tier collection.
    pub const fn index(&self) -> usize {
        self.index
    }

    /// The actual original tier, not a guessed label.
    pub const fn tier(&self) -> &DependentTier {
        self.tier
    }

    /// The policy that prevented its reuse.
    pub const fn reason(&self) -> TierInvalidationReason {
        self.reason
    }
}

/// What executing an admitted partition did to its source.
///
/// Most utterances of a document are not split, so the unsplit case is a
/// variant of its own rather than a one-element copy of the source: the
/// caller already holds the utterance and keeps it.
#[must_use]
pub enum SplitOutcome<'source> {
    /// Every word slot named the same child (or there were no slots), so the
    /// source stands exactly as it is, with every dependent tier. Nothing was
    /// rebuilt or copied.
    Unchanged,
    /// Two or more children replace the source.
    Split(SplitChildren<'source>),
}

/// Partitioned children together with explicit information-loss receipts.
///
/// This is not a complete-validity capability. Consumers must admit checked
/// construction before writing the resulting document.
#[must_use]
pub struct SplitChildren<'source> {
    children: Vec<Utterance>,
    invalidated: Vec<InvalidatedTier<'source>>,
}

impl<'source> SplitChildren<'source> {
    /// Inspect the rebuilt children.
    pub fn children(&self) -> &[Utterance] {
        &self.children
    }

    /// Inspect every dependent tier that was not retained.
    pub fn invalidated_tiers(&self) -> &[InvalidatedTier<'source>] {
        &self.invalidated
    }

    /// Consume both children and loss receipts; no lossy children-only transition.
    pub fn into_parts(self) -> (Vec<Utterance>, Vec<InvalidatedTier<'source>>) {
        (self.children, self.invalidated)
    }
}

/// A partition admitted against the very utterance it will rebuild.
///
/// Fields and construction are private. Execution accepts no second utterance
/// or independently supplied assignment vector, and mutation of the source is
/// excluded while this borrow is live.
///
/// Callers cannot mint a split by supplying their own content mapping:
///
/// ```compile_fail,E0451
/// use talkbank_model::model::Utterance;
/// use talkbank_transform::utterance_split::UtteranceSplitPlan;
/// fn forge(source: &Utterance) -> UtteranceSplitPlan<'_> {
///     UtteranceSplitPlan { source, content_groups: None }
/// }
/// ```
#[must_use]
pub struct UtteranceSplitPlan<'source> {
    source: &'source Utterance,
    content_groups: Option<ContentItemGroups>,
}

impl<'source> UtteranceSplitPlan<'source> {
    /// Admit assignments in the morphology/extraction domain used by utseg.
    ///
    /// # Errors
    /// Refuses count drift, disjoint child runs and boundaries inside indivisible
    /// content or before separators.
    pub fn for_morphology(
        source: &'source Utterance,
        assignments: &[usize],
    ) -> Result<Self, SplitRefusal> {
        Self::admit(
            source,
            assignments,
            build_word_to_content_map(&source.main.content.content),
        )
    }

    /// Admit assignments in the same original-word domain used by word timing.
    ///
    /// The model's word-slot membership policy owns exclusions; replacements
    /// therefore occupy their spoken original slot, not replacement-target slots.
    ///
    /// # Errors
    /// Refuses count drift, disjoint child runs and indivisible-content boundaries.
    pub fn for_word_timing(
        source: &'source Utterance,
        assignments: &[usize],
    ) -> Result<Self, SplitRefusal> {
        let mut word_to_content = Vec::new();
        for (index, item) in source.main.content.content.iter().enumerate() {
            walk_words(
                std::slice::from_ref(item),
                Some(TierDomain::Wor),
                &mut |item| {
                    let admitted = match item {
                        WordItem::Word(word) => WOR_SLOT_POLICY.admits(word),
                        WordItem::ReplacedWord(replaced) => WOR_SLOT_POLICY.admits(&replaced.word),
                        WordItem::Separator(_) => false,
                    };
                    if admitted {
                        word_to_content.push(index);
                    }
                },
            );
        }
        Self::admit(source, assignments, word_to_content)
    }

    fn admit(
        source: &'source Utterance,
        assignments: &[usize],
        word_to_content: Vec<usize>,
    ) -> Result<Self, SplitRefusal> {
        if assignments.len() != word_to_content.len() {
            return Err(SplitRefusal::SlotCount {
                expected: word_to_content.len(),
                actual: assignments.len(),
            });
        }
        // Normalize only labels, never word order. Allocation is bounded by the
        // source's slot count rather than the magnitude of a supplied label.
        let mut seen = HashSet::new();
        let mut previous = None;
        let mut group_count = 0usize;
        let mut normalized = Vec::with_capacity(assignments.len());
        for &label in assignments {
            if previous != Some(label) {
                if !seen.insert(label) {
                    return Err(SplitRefusal::DisjointGroup { group: label });
                }
                group_count += 1;
                previous = Some(label);
            }
            normalized.push(group_count - 1);
        }
        let mut by_content = vec![None; source.main.content.content.len()];
        for (&index, &group) in word_to_content.iter().zip(&normalized) {
            match by_content[index] {
                Some(previous) if previous != group => {
                    return Err(SplitRefusal::IndivisibleContent {
                        content_index: index,
                    });
                }
                Some(_) | None => by_content[index] = Some(group),
            }
        }
        let groups =
            ContentItemGroups::assign(&source.main.content.content, &word_to_content, &normalized);
        if let Some(groups) = &groups {
            let mut previous = None;
            for (index, (item, group)) in source
                .main
                .content
                .content
                .iter()
                .zip(groups.in_content_order())
                .enumerate()
            {
                if previous != Some(group) && matches!(item, UtteranceContent::Separator(_)) {
                    return Err(SplitRefusal::LeadingSeparator {
                        content_index: index,
                    });
                }
                previous = Some(group);
            }
        }
        Ok(Self {
            source,
            content_groups: groups,
        })
    }

    /// Rebuild from the retained plan and source; no parser or independent rematch.
    /// An assignment that splits nothing copies nothing.
    pub fn execute(self) -> SplitOutcome<'source> {
        let source = self.source;
        let Some(content_groups) = self.content_groups else {
            return SplitOutcome::Unchanged;
        };
        let children = rebuild_split(source, &content_groups);
        let retained_wor = children.iter().any(|child| child.wor_tier().is_some());
        let invalidated = source
            .dependent_tiers
            .iter()
            .enumerate()
            .filter_map(|(index, tier)| {
                let reason = match policy_for_tier(&tier.tier) {
                    TierSplitPolicy::Drop => TierInvalidationReason::BoundaryDependent,
                    TierSplitPolicy::Partition if !retained_wor => {
                        TierInvalidationReason::UncorroboratedWordTiming
                    }
                    TierSplitPolicy::Partition | TierSplitPolicy::AttachFirst => return None,
                };
                Some(InvalidatedTier {
                    index,
                    tier: &tier.tier,
                    reason,
                })
            })
            .collect();
        SplitOutcome::Split(SplitChildren {
            children,
            invalidated,
        })
    }
}

/// Build a mapping from extracted-word index to top-level content item index.
///
/// Counts each item's extracted words with extraction's own walk
/// ([`extract::count_utterance_content`]) instead of building them: only the
/// number is read, and building a word costs two owned strings.
pub fn build_word_to_content_map(content: &[UtteranceContent]) -> Vec<usize> {
    let mut word_to_content = Vec::new();
    for (content_idx, item) in content.iter().enumerate() {
        let words =
            extract::count_utterance_content(std::slice::from_ref(item), PositionalDomain::Mor);
        word_to_content.extend(std::iter::repeat_n(content_idx, words));
    }
    word_to_content
}

/// Per-tier behavior when an utterance is split into multiple children.
///
/// Splitting an utterance is a transformation that invalidates some
/// dependent-tier data and not others. This enum makes the per-tier
/// decision explicit and grep-able. `policy_for_tier` is the single
/// dispatch site; tests cover each variant.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TierSplitPolicy {
    /// Walk the parent's items in lockstep with main-tier words and
    /// distribute them across children by the existing word→child
    /// mapping. Falls back to `Drop` if positional counts mismatch
    /// (stale `%wor` from prior edits, or tokenization drift). The
    /// Any invalidation is returned in the split outcome.
    Partition,
    /// Drop the tier from all children. The tier's data is
    /// semantically invalidated by the split: morphological analysis
    /// assumed the original utterance boundary; dependency arcs
    /// reference word indices that no longer match; coreference
    /// chains span document positions that the split changes. The
    /// user regenerates via `morphotag` / `coref`.
    Drop,
    /// Attach the tier (unchanged) to the first child only. The data
    /// is utterance-level free-form (`%com` comments, `%xtra`
    /// translations, user-defined `%x*` annotations) with no
    /// positional semantics to violate. Stale-on-first-child is
    /// strictly better than silent loss: the user can re-translate or
    /// correct manually, and improves on BA2 which dropped these
    /// unconditionally.
    AttachFirst,
}

/// Map a dependent tier to its split policy.
///
/// Word-positional, context-free tiers (`%wor`) get [`Partition`]. Word-positional
/// but context-dependent tiers (`%mor`, `%gra`) get [`Drop`], the data is
/// invalid in the new context. Document- or analysis-scoped tiers (`%xcoref`)
/// also `Drop`. Other word-positional tiers we don't yet have a partition
/// implementation for (`%pho`, `%mod`, `%sin`, etc.) `Drop` rather than
/// `AttachFirst`, because attaching the parent's full per-word data to one
/// child would falsely claim the data covers all the original words. Free-form
/// utterance-level tiers (`%com`, `%xtra`, `%add`, etc., text tiers, user-defined,
/// unsupported) default to `AttachFirst`, preserve the data on the first child.
///
/// [`Partition`]: TierSplitPolicy::Partition
/// [`Drop`]: TierSplitPolicy::Drop
/// [`AttachFirst`]: TierSplitPolicy::AttachFirst
/// The coreference tier `batchalign3 coref` injects.
///
/// Named here rather than spelled inline so the policy below and the injector
/// cannot disagree about which label they mean.
const XCOREF_LABEL: &str = "xcoref";

fn policy_for_tier(tier: &DependentTier) -> TierSplitPolicy {
    match tier {
        // Per-word timing: partitionable by word index.
        DependentTier::Wor(_) => TierSplitPolicy::Partition,

        // Context-dependent or reference-structured: drop, regenerate downstream.
        DependentTier::Mor(_) | DependentTier::Gra(_) => TierSplitPolicy::Drop,

        // Word-positional but no partition implementation yet. Dropping is
        // honest: attaching to first child would claim phonological / sign
        // data covering all original words, which is wrong. Add to Partition
        // explicitly when a partition impl lands for each shape.
        DependentTier::Pho(_)
        | DependentTier::Mod(_)
        | DependentTier::Sin(_)
        | DependentTier::Modsyl(_)
        | DependentTier::Phosyl(_)
        | DependentTier::Phoaln(_) => TierSplitPolicy::Drop,

        // `%xphoint` indexes INTO `%pho`: it is per-phone time intervals
        // segmenting each `%pho` word. `%pho` is dropped two arms above, so
        // attaching these to the first child leaves interval bullets for
        // phones that are no longer in that utterance, and nothing on the
        // rest. It reached `AttachFirst` through the wildcard this match used
        // to end with, which is the failure the arm above is written to avoid.
        DependentTier::Xphoint(_) => TierSplitPolicy::Drop,

        // Free-form / loosely-structured utterance-level annotations:
        // preserve on first child rather than silently lose.
        //
        // Enumerated so a tier added to chatter stops compiling here until
        // someone decides what splitting it does.
        DependentTier::Act(_)
        | DependentTier::Add(_)
        | DependentTier::Alt(_)
        | DependentTier::Cod(_)
        | DependentTier::Coh(_)
        | DependentTier::Com(_)
        | DependentTier::Def(_)
        | DependentTier::Eng(_)
        | DependentTier::Err(_)
        | DependentTier::Exp(_)
        | DependentTier::Fac(_)
        | DependentTier::Flo(_)
        | DependentTier::Gls(_)
        | DependentTier::Gpx(_)
        | DependentTier::Int(_)
        | DependentTier::Ort(_)
        | DependentTier::Par(_)
        | DependentTier::Sit(_)
        | DependentTier::Spa(_)
        | DependentTier::Tim(_)
        | DependentTier::Unsupported(_) => TierSplitPolicy::AttachFirst,

        // `UserDefined` is a FAMILY, not a policy class, and enumerating the
        // variants gave it one arm. This pipeline emits exactly two labels:
        // `%xtra` is free-form utterance-level, but `%xcoref` is coreference
        // chains, whose links span document positions the split changes, which
        // is the same reason `%mor` and `%gra` are dropped above. The doc for
        // this function has always said document-scoped tiers drop; before the
        // variants were enumerated, `%xcoref` reached `AttachFirst` through the
        // wildcard and contradicted it silently.
        DependentTier::UserDefined(tier) if tier.label.as_str() == XCOREF_LABEL => {
            TierSplitPolicy::Drop
        }
        DependentTier::UserDefined(_) => TierSplitPolicy::AttachFirst,
    }
}

mod content_groups;
#[cfg(test)]
mod tests;
mod wor_partition;
mod word_speaker;

use content_groups::ContentItemGroups;
use wor_partition::{
    CompletePerChildMainTiming, SplitMainTimingEvidence, partition_wor_tier,
    split_main_timing_evidence,
};
pub use word_speaker::{
    WordSpeakerPartition, WordSpeakerSource, WordSpeakerSplitOutcome, WordSpeakerSplitParts,
    WordSpeakerSplitPlan, WordSpeakerSplitRefusal,
};

/// The `%wor` slot-membership policy this module's decisions travel through.
///
/// Named once, and taken from chatter rather than restated. The doc on
/// [`WorSlotMembershipPolicy::admits`] says the method is public precisely so
/// that a per-content-item count in an utterance splitter can ask it instead
/// of spelling the rule out beside its own walk; this module is one of the
/// two trees that doc names, and this is the deletion it describes.
const WOR_SLOT_POLICY: WorSlotMembershipPolicy = WorSlotMembershipPolicy::FilteredLexicalV1;

/// Compute the child-group assignment for each main-tier word that occupies a
/// `%wor` slot.
///
/// Which words those are is [`WOR_SLOT_POLICY`]'s answer, asked per word. A
/// replaced word is admitted by its ORIGINAL, as the projection admits it.
///
/// The returned Vec has one entry per admitted word, in main-tier order;
/// entries are child-group indices. The walk is per content item because that
/// is what pairs a word with the group its item travels with, which is also
/// why this cannot be `WorMainTierProjection` itself: that is constructible
/// only from a whole `MainTier`, and it reports slots rather than the content
/// items they came from.
fn wor_eligible_word_groups(
    content_items: &[UtteranceContent],
    content_groups: &ContentItemGroups,
) -> Vec<usize> {
    let mut groups = Vec::new();
    for (item, group) in content_items.iter().zip(content_groups.in_content_order()) {
        walk_words(
            std::slice::from_ref(item),
            Some(TierDomain::Wor),
            &mut |word| {
                let admitted = match word {
                    WordItem::Word(word) => WOR_SLOT_POLICY.admits(word),
                    WordItem::ReplacedWord(replaced) => WOR_SLOT_POLICY.admits(&replaced.word),
                    WordItem::Separator(_) => false,
                };
                if admitted {
                    groups.push(group);
                }
            },
        );
    }
    groups
}

fn rebuild_split(utt: &Utterance, content_groups: &ContentItemGroups) -> Vec<Utterance> {
    let content_items = &utt.main.content.content;
    let mut groups: Vec<Vec<UtteranceContent>> = vec![Vec::new(); content_groups.group_count()];
    for (item, group_id) in content_items.iter().zip(content_groups.in_content_order()) {
        groups[group_id].push(item.clone());
    }

    let speaker = &utt.main.speaker;
    // The parent's main-tier bullet is not carried: it measures no child of a
    // split (see `SplitMainTimingEvidence::Unmeasured`).

    // Capture the rest of the parent's main-tier metadata so each child
    // can inherit the right slice of it. Per-field propagation policy
    // (linkers → first only, terminator/postcodes → last only, language
    // code/spans → all).
    let parent_linkers = utt.main.content.linkers.clone();
    let parent_terminator = utt.main.content.terminator.clone();
    let parent_language_code = utt.main.content.language_code.clone();
    let parent_postcodes = utt.main.content.postcodes.clone();
    let parent_main_span = utt.main.span;
    let parent_speaker_span = utt.main.speaker_span;

    // Compute per-child %wor item lists only when the parent tier is count-
    // matched and lexically corroborated against the typed main tier. None
    // means absent or stale evidence (graceful drop).
    let partitioned_wor = utt
        .dependent_tiers
        .iter()
        // A search for one tier, not a policy over all of them, so the
        // "everything else" case is genuinely "not the tier I am looking for"
        // and a tier added later is correctly not it. Written as a `let-else`
        // rather than a match with a wildcard so that distinction is visible:
        // the file denies wildcard matches precisely because the OTHER one in
        // it was a policy decision wearing the same syntax.
        .find_map(|tier| {
            let DependentTier::Wor(wor) = &tier.tier else {
                return None;
            };
            Some(wor)
        })
        .and_then(|wor| {
            let main_groups = wor_eligible_word_groups(content_items, content_groups);
            partition_wor_tier(&utt.main, wor, &main_groups, content_groups.group_count())
        });

    // Track (original_group_idx, utterance) so we can later look up the
    // partitioned %wor for each kept child even after empty/all-separator
    // groups are skipped.
    let mut result: Vec<(usize, Utterance)> = Vec::new();

    for (group_idx, group_content) in groups.into_iter().enumerate() {
        if group_content.is_empty() {
            continue;
        }

        let mut main = MainTier::new(
            speaker.clone(),
            group_content,
            // A split child's terminator is synthesized; it points at the
            // parent utterance it was derived from, like the child's spans.
            Terminator::Period {
                span: parent_main_span,
            },
        );
        // Language code applies to every child (utterance-scope), set
        // it at construction time. Linkers, terminator, postcodes, and
        // bullet are positional and applied to the right child after
        // the loop.
        if let Some(ref lang) = parent_language_code {
            main.content = main.content.with_language_code(lang.clone());
        }
        // Source spans: inherit the parent's so children retain a
        // useful (if coarse) source pointer instead of `Span::DUMMY`.
        main.span = parent_main_span;
        main.speaker_span = parent_speaker_span;
        let new_utt = Utterance::new(main);
        result.push((group_idx, new_utt));
    }

    let main_timing = split_main_timing_evidence(partitioned_wor.as_ref(), &result);

    // Per-tier policy. Walk the parent's dependent tiers once, dispatching
    // each to its policy. `partitioned_wor` (if Some) is the precomputed
    // per-group payload; AttachFirst tiers go to result[0]; Drop tiers
    // produce no output.
    if let Some((_, first_child)) = result.first_mut() {
        for tier in &utt.dependent_tiers {
            if matches!(policy_for_tier(&tier.tier), TierSplitPolicy::AttachFirst) {
                first_child.dependent_tiers.push(tier.clone());
            }
        }
    }

    // Linkers go on the FIRST child only, they describe relation to the
    // *prior* (different) utterance, which only the first piece is adjacent
    // to. Use a non-empty check so we don't bother cloning the empty
    // SmallVec for the common case.
    if !parent_linkers.is_empty()
        && let Some((_, first_child)) = result.first_mut()
    {
        first_child.main.content.linkers = parent_linkers;
    }

    // Terminator and postcodes go on the LAST child only. Terminator
    // describes how the original utterance ended, that's the last child.
    // Postcodes are utterance-level analysis tags; placing them on the
    // last child matches the conventional after-terminator serialization.
    if let Some((_, last)) = result.last_mut() {
        if let Some(term) = parent_terminator {
            last.main.content.terminator = Some(term);
        }
        if !parent_postcodes.is_empty() {
            last.main.content.postcodes = parent_postcodes;
        }
    }

    // Attach partitioned %wor after child main-tier terminators are final, so
    // the dependent tier cannot retain the parent's terminator on an earlier
    // child or retain the default period on the last child.
    // Each per-child tier moves into its child; kept children are in group
    // order, so one forward pass over both pairs them.
    if let Some(per_group) = partitioned_wor {
        let mut per_group = per_group.into_groups();
        for (group_idx, child) in result.iter_mut() {
            if let Some((_, child_wor)) = per_group.find(|(index, _)| index == group_idx)
                && !child_wor.items.is_empty()
            {
                let child_wor = child_wor.with_terminator(child.main.content.terminator.clone());
                child
                    .dependent_tiers
                    .push(DependentTier::Wor(child_wor).into());
            }
        }
    }

    match main_timing {
        // One bullet per kept child, by construction (see
        // `split_main_timing_evidence`).
        SplitMainTimingEvidence::CompletePerChild(CompletePerChildMainTiming { bullets }) => {
            for ((_, child), bullet) in result.iter_mut().zip(bullets) {
                child.main.content.bullet = Some(bullet);
            }
        }
        SplitMainTimingEvidence::Unmeasured => {}
    }

    result.into_iter().map(|(_, u)| u).collect()
}
