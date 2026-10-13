//! Source-bound admission and partitioning of typed CHAT utterances.
//!
//! This proves a structural split, not complete CHAT validity or output-write
//! permission. Rebuilt children still require checked construction.

use talkbank_model::alignment::helpers::PositionalDomain;
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
///     UtteranceSplitPlan { content_groups: None }
/// }
/// ```
#[must_use]
pub struct UtteranceSplitPlan<'source> {
    content_groups: Option<ContentItemGroups<'source>>,
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
        Self::admit(source, assignments, word_timing_content_map(source))
    }

    fn admit(
        source: &'source Utterance,
        assignments: &[usize],
        word_to_content: Vec<usize>,
    ) -> Result<Self, SplitRefusal> {
        let groups = ContentItemGroups::admit(source, &word_to_content, assignments)?;
        Ok(Self {
            content_groups: groups,
        })
    }

    /// Rebuild from the retained plan and source; no parser or independent rematch.
    /// An assignment that splits nothing copies nothing.
    pub fn execute(self) -> SplitOutcome<'source> {
        let Some(content_groups) = self.content_groups else {
            return SplitOutcome::Unchanged;
        };
        SplitOutcome::Split(execute_split(&content_groups, |_| {
            content_groups.source().main.speaker.clone()
        }))
    }
}

/// Finish every admitted group, keeping loss receipts bound to its source.
fn execute_split<'source, Affiliation>(
    content_groups: &ContentItemGroups<'source, Affiliation>,
    speaker: impl FnMut(
        &content_groups::ContentGroup<'source, Affiliation>,
    ) -> talkbank_model::SpeakerCode,
) -> SplitChildren<'source> {
    let source = content_groups.source();
    let children = rebuild_split(content_groups, speaker).into_vec();
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
    SplitChildren {
        children,
        invalidated,
    }
}

/// The same policy-selected source-word domain for ordinary and speaker splits.
fn word_timing_content_map(source: &Utterance) -> Vec<usize> {
    source
        .main
        .wor_projection()
        .source_content()
        .map(|location| location.index())
        .collect()
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
mod word_ownership;
mod word_speaker;

use content_groups::{ChildPosition, ContentItemGroups, RebuiltChildren};
use wor_partition::{
    CompletePerChildMainTiming, SplitMainTimingEvidence, partition_wor_tier,
    split_main_timing_evidence,
};
pub use word_ownership::WordSpeakerOwnership;
pub use word_speaker::{
    WordSpeakerPartition, WordSpeakerSource, WordSpeakerSplitOutcome, WordSpeakerSplitParts,
    WordSpeakerSplitPlan, WordSpeakerSplitRefusal,
};

fn rebuild_split<'source, Affiliation>(
    content_groups: &ContentItemGroups<'source, Affiliation>,
    mut speaker: impl FnMut(
        &content_groups::ContentGroup<'source, Affiliation>,
    ) -> talkbank_model::SpeakerCode,
) -> RebuiltChildren {
    let utt = content_groups.source();
    // The parent's main-tier bullet is not carried: it measures no child of a
    // split (see `SplitMainTimingEvidence::Unmeasured`).

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
        .and_then(|wor| partition_wor_tier(wor, content_groups));
    let mut result = content_groups.rebuild(|position, group| {
        let mut main = MainTier::new(
            speaker(group),
            group.cloned_content(),
            // Earlier children need a derived ending with a parent source
            // pointer. Positional metadata below replaces the last ending.
            Terminator::Period {
                span: parent_main_span,
            },
        );
        // Language code applies to every child (utterance-scope), set
        // it at construction time. Linkers, terminator and postcodes use the
        // admitted position below; word-derived bullets follow tier attachment.
        if let Some(ref lang) = utt.main.content.language_code {
            main.content = main.content.with_language_code(lang.clone());
        }
        // Source spans: inherit the parent's so children retain a
        // useful (if coarse) source pointer instead of `Span::DUMMY`.
        main.span = parent_main_span;
        main.speaker_span = parent_speaker_span;
        let mut child = Utterance::new(main);
        match position {
            ChildPosition::First => {
                child.main.content.linkers = utt.main.content.linkers.clone();
                for tier in &utt.dependent_tiers {
                    if matches!(policy_for_tier(&tier.tier), TierSplitPolicy::AttachFirst) {
                        child.dependent_tiers.push(tier.clone());
                    }
                }
            }
            ChildPosition::Interior => {}
            ChildPosition::Last => {
                // Absence is authored too (CA unmarked endings). Do not keep
                // the temporary period when the source had no terminator.
                child.main.content.terminator = utt.main.content.terminator.clone();
                child.main.content.postcodes = utt.main.content.postcodes.clone();
            }
        }
        child
    });

    // Attach partitioned %wor after child main-tier terminators are final, so
    // the dependent tier cannot retain the parent's terminator on an earlier
    // child or retain the default period on the last child.
    // Both producers retain every admitted group in the same dense order.
    // An empty word partition is omitted rather than emitted as an empty tier;
    // missing content groups and independent index rematches are not possible.
    if let Some(per_group) = partitioned_wor {
        for (child, child_wor) in result
            .as_mut_slice()
            .iter_mut()
            .zip(per_group.into_groups())
        {
            if !child_wor.items.is_empty() {
                let child_wor = child_wor.with_terminator(child.main.content.terminator.clone());
                child
                    .dependent_tiers
                    .push(DependentTier::Wor(child_wor).into());
            }
        }
    }

    match split_main_timing_evidence(&result) {
        // One bullet per kept child, by construction (see
        // `split_main_timing_evidence`).
        SplitMainTimingEvidence::CompletePerChild(CompletePerChildMainTiming { bullets }) => {
            for (child, bullet) in result.as_mut_slice().iter_mut().zip(bullets) {
                child.main.content.bullet = Some(bullet);
            }
        }
        SplitMainTimingEvidence::Unmeasured => {}
    }

    result
}
