//! Pronunciation evidence is never replaced with an orthographic placeholder.

use talkbank_model::alignment::helpers::visit_pho_words;
use talkbank_model::{DependentTier, PhoItem, PhoTierType, Word};

use super::PseudonymizationInput;
use super::plan::SelectedWords;

/// Evidence requiring refusal, not permission to drop or rewrite pronunciation.
pub enum PronunciationEvidence<'input> {
    /// Item addressed by the admitted main-to-phonology alignment pair.
    AlignedItem(&'input PhoItem),
    /// Structured pronunciation companion needs its own correspondence review.
    CompanionReview,
    /// A present phonology tier has no verified pair for the selected word.
    MissingAlignment,
}

/// Protected pronunciation evidence associated with a selected spoken word.
pub struct PronunciationRefusal<'input> {
    location: super::WordLocation,
    word: &'input Word,
    tier: &'input DependentTier,
    evidence: PronunciationEvidence<'input>,
}

impl PronunciationRefusal<'_> {
    /// Original spoken-word position retained by the selecting decision.
    pub fn location(&self) -> super::WordLocation {
        self.location
    }
    /// Original spoken word, never an independently matched pronunciation.
    pub fn word(&self) -> &Word {
        self.word
    }
    /// Original pronunciation tier retained without mutation.
    pub fn tier(&self) -> &DependentTier {
        self.tier
    }
    /// Sensitive evidence for the private receipt; all variants block output.
    pub fn evidence(&self) -> &PronunciationEvidence<'_> {
        &self.evidence
    }
}

pub(super) fn plan<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
    selected: &SelectedWords<'_, 'input>,
) -> Vec<PronunciationRefusal<'input>> {
    let mut refusals = Vec::new();
    for utterance in input.document().document().utterances() {
        visit_pho_words(&utterance.main.content.content, &mut |position| {
            let word = position.word();
            let Some(preview) = selected.get(&(word as *const Word)) else {
                return;
            };
            for entry in &utterance.dependent_tiers {
                let evidence = match &entry.tier {
                    DependentTier::Pho(tier) | DependentTier::Mod(tier) => {
                        let alignment =
                            utterance
                                .alignments
                                .as_ref()
                                .and_then(|set| match tier.tier_type {
                                    PhoTierType::Pho => set.pho.as_ref(),
                                    PhoTierType::Mod => set.mod_.as_ref(),
                                });
                        let item = alignment
                            .filter(|a| a.is_error_free())
                            .and_then(|a| a.pairs.get(position.index().as_usize()))
                            .filter(|pair| pair.source_index == Some(position.index()))
                            .and_then(|pair| pair.target_index)
                            .and_then(|index| tier.items.get(index.as_usize()));
                        match item {
                            Some(item) => PronunciationEvidence::AlignedItem(item),
                            None => PronunciationEvidence::MissingAlignment,
                        }
                    }
                    DependentTier::Modsyl(_)
                    | DependentTier::Phosyl(_)
                    | DependentTier::Phoaln(_)
                    | DependentTier::Xphoint(_) => PronunciationEvidence::CompanionReview,
                    DependentTier::Mor(_)
                    | DependentTier::Gra(_)
                    | DependentTier::Sin(_)
                    | DependentTier::Act(_)
                    | DependentTier::Cod(_)
                    | DependentTier::Add(_)
                    | DependentTier::Com(_)
                    | DependentTier::Exp(_)
                    | DependentTier::Gpx(_)
                    | DependentTier::Int(_)
                    | DependentTier::Sit(_)
                    | DependentTier::Spa(_)
                    | DependentTier::Alt(_)
                    | DependentTier::Coh(_)
                    | DependentTier::Def(_)
                    | DependentTier::Eng(_)
                    | DependentTier::Err(_)
                    | DependentTier::Fac(_)
                    | DependentTier::Flo(_)
                    | DependentTier::Gls(_)
                    | DependentTier::Ort(_)
                    | DependentTier::Par(_)
                    | DependentTier::Tim(_)
                    | DependentTier::Wor(_)
                    | DependentTier::UserDefined(_)
                    | DependentTier::Unsupported(_) => continue,
                };
                refusals.push(PronunciationRefusal {
                    location: preview.location(),
                    word,
                    tier: &entry.tier,
                    evidence,
                });
            }
        });
    }
    refusals
}
