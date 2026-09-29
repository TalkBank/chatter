//! Source-bound `%wor` proposals only after count and lexical corroboration.

use talkbank_model::alignment::{
    WorTimingBinding, WorTimingCorrespondence, corroborate_wor_timing,
};
use talkbank_model::{DependentTier, WorTier, Word};

use super::PseudonymizationInput;
use super::plan::SelectedWords;

/// Why an existing timing tier cannot safely follow the main-tier changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingRefusal {
    /// The timing tier has a different slot count.
    CountDrift,
    /// Equal counts conceal differing lexical content.
    LexicalDrift,
    /// A present tier unexpectedly failed to bind.
    MissingBinding,
    /// Aligned word decisions or their source fields could not transfer safely.
    UnsafeWord(super::WordRefusal),
}

/// Sensitive original/proposed timing word pair; timing remains unchanged.
pub struct TimingWordPreview<'input> {
    location: super::WordLocation,
    original: &'input Word,
    proposed: Word,
    edits: Vec<super::LexicalEdit<'input>>,
}

impl TimingWordPreview<'_> {
    /// Original main-tier position, carried through the corroborated binding.
    pub fn location(&self) -> super::WordLocation {
        self.location
    }
    /// Source-bound lexical edits; timing bullets and other markers are excluded.
    pub fn edits(&self) -> &[super::LexicalEdit<'_>] {
        &self.edits
    }
    /// Original word in the protected timing tier.
    pub fn original(&self) -> &Word {
        self.original
    }
    /// Proposed display token with the original timing and metadata.
    pub fn proposed(&self) -> &Word {
        &self.proposed
    }
}

/// Review result for an existing `%wor` tier, not output authority.
pub enum TimingOutcome<'input> {
    /// Selective proposals after the entire tier corroborated.
    Corroborated(Vec<TimingWordPreview<'input>>),
    /// No independent rewrite fallback is permitted.
    Refused(TimingRefusal),
}

/// Review bound to its original typed timing tier.
pub struct TimingReview<'input> {
    original: &'input WorTier,
    outcome: TimingOutcome<'input>,
}

impl TimingReview<'_> {
    /// Protected original tier, retained even for drift refusals.
    pub fn original(&self) -> &WorTier {
        self.original
    }
    /// Sensitive proposals or a value-free refusal.
    pub fn outcome(&self) -> &TimingOutcome<'_> {
        &self.outcome
    }
}

pub(super) fn plan<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
    selected: &SelectedWords<'_, 'input>,
) -> Vec<TimingReview<'input>> {
    let mut reviews = Vec::new();
    for utterance in input.document().document().utterances() {
        for entry in &utterance.dependent_tiers {
            let DependentTier::Wor(tier) = &entry.tier else {
                continue;
            };
            let outcome = match utterance.main.wor_projection().bind_timing(Some(tier)) {
                WorTimingBinding::Missing(_) => {
                    TimingOutcome::Refused(TimingRefusal::MissingBinding)
                }
                WorTimingBinding::Drifted(_) => TimingOutcome::Refused(TimingRefusal::CountDrift),
                WorTimingBinding::CountMatched(matched) => match corroborate_wor_timing(matched) {
                    WorTimingCorrespondence::Uncorroborated(_) => {
                        TimingOutcome::Refused(TimingRefusal::LexicalDrift)
                    }
                    WorTimingCorrespondence::Corroborated(bound) => {
                        let changes = (|| {
                            let mut changes = Vec::new();
                            for slot in bound.slots() {
                                let Some(preview) =
                                    selected.get(&(slot.main_word() as *const Word))
                                else {
                                    continue;
                                };
                                let original = slot.wor_word();
                                let plan = preview
                                    .project_onto(original)
                                    .map_err(TimingRefusal::UnsafeWord)?;
                                let edits = plan
                                    .source_edits(input.parsed_source())
                                    .map_err(TimingRefusal::UnsafeWord)?;
                                let proposed = plan.apply();
                                changes.push(TimingWordPreview {
                                    location: preview.location(),
                                    original,
                                    proposed,
                                    edits,
                                });
                            }
                            Ok(changes)
                        })();
                        match changes {
                            Ok(changes) => TimingOutcome::Corroborated(changes),
                            Err(reason) => TimingOutcome::Refused(reason),
                        }
                    }
                },
            };
            reviews.push(TimingReview {
                original: tier,
                outcome,
            });
        }
    }
    reviews
}
