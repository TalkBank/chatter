//! Morphology proposals driven only by selected, admitted main-tier words.

use std::collections::HashSet;
use talkbank_model::alignment::helpers::visit_mor_positions;
use talkbank_model::{Mor, MorStem, MorTier, MorWord, Word};
use talkbank_parser::generated_traversal::{
    AsRawNode, MorContentNode, MorDependentTierNode, MorLemmaNode, SourceBound, SourceSlotView,
};

use super::plan::SelectedWords;
use super::{ComponentDecision, NameDecision, PseudonymizationInput, WordPreview};

/// A mapped lemma retained unchanged, for explicit private review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LemmaFindingKind {
    /// Exact map match not replaced through a selected main-tier word.
    UnreplacedMatch,
    /// Case-only match, never authority for an automatic change.
    CaseNearMiss,
}

/// Part of one typed morphology item, distinct from a main-tier word position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LemmaPart {
    /// Main morphological word.
    Main,
    /// Zero-based post-clitic within the item's own collection.
    PostClitic(usize),
}

/// Producer-assigned location of an independently reported morphology lemma.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LemmaLocation {
    utterance: usize,
    item: usize,
    part: LemmaPart,
}

impl LemmaLocation {
    /// Zero-based utterance in the admitted document.
    pub fn utterance(self) -> usize {
        self.utterance
    }
    /// Zero-based morphology item, not a main-tier lexical word index.
    pub fn item(self) -> usize {
        self.item
    }
    /// Main lemma or a specific post-clitic.
    pub fn part(self) -> LemmaPart {
        self.part
    }
}

/// Sensitive source-bound lemma finding; it does not propose a replacement.
pub struct LemmaFinding<'input> {
    location: LemmaLocation,
    tier: &'input MorTier,
    word: &'input MorWord,
    kind: LemmaFindingKind,
}

impl LemmaFinding<'_> {
    /// Typed morphology location, even when no main-tier word was selected.
    pub fn location(&self) -> LemmaLocation {
        self.location
    }
    /// Owning original tier, whose source span locates the finding.
    pub fn tier(&self) -> &MorTier {
        self.tier
    }
    /// Exact original morphological word, including post-clitics.
    pub fn word(&self) -> &MorWord {
        self.word
    }
    /// Value-free review classification.
    pub fn kind(&self) -> LemmaFindingKind {
        self.kind
    }
}

/// Report every mapped lemma not already changed by the aligned proposal.
/// Pointer identity avoids conflating repeated lemmas or post-clitic siblings.
pub(super) fn lemma_findings<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
    reviews: &[MorphologyReview<'input>],
) -> Vec<LemmaFinding<'input>> {
    let changed: HashSet<*const MorWord> = reviews
        .iter()
        .filter_map(|review| match &review.outcome {
            MorphologyOutcome::Proposed { original, .. } => Some(&original.main as *const MorWord),
            MorphologyOutcome::Refused { .. } => None,
        })
        .collect();
    let mut findings = Vec::new();
    for (utterance_index, utterance) in input.document().document().utterances().enumerate() {
        let Some(tier) = utterance.mor_tier() else {
            continue;
        };
        for (item_index, item) in tier.items().iter().enumerate() {
            for (part, word) in std::iter::once((LemmaPart::Main, &item.main)).chain(
                item.post_clitics
                    .iter()
                    .enumerate()
                    .map(|(index, word)| (LemmaPart::PostClitic(index), word)),
            ) {
                if changed.contains(&(word as *const MorWord)) {
                    continue;
                }
                let kind = match input.names().decide(word.lemma.as_ref()) {
                    NameDecision::Keep => continue,
                    NameDecision::Replace(_) => LemmaFindingKind::UnreplacedMatch,
                    NameDecision::CaseNearMiss => LemmaFindingKind::CaseNearMiss,
                };
                findings.push(LemmaFinding {
                    location: LemmaLocation {
                        utterance: utterance_index,
                        item: item_index,
                        part,
                    },
                    tier,
                    word,
                    kind,
                });
            }
        }
    }
    findings
}

/// Why the selected main-tier change cannot safely determine a lemma change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MorphologyRefusal {
    /// No verified pair addresses this position in the admitted document.
    MissingAlignment,
    /// Typed main-tier components have no established lemma-part correspondence.
    ComponentCorrespondence,
    /// The aligned lemma is not the exact selected main-tier spelling.
    LemmaCorrespondence,
    /// The aligned item could not be bound to its exact source lemma field.
    SourceCorrespondence,
}

/// Exact main-lemma source field, admitted from the input's retained CST.
/// This is sensitive review evidence, not document-writing authority.
pub struct LemmaSource<'input>(SourceBound<'input, 'input, MorLemmaNode<'input>>);

impl LemmaSource<'_> {
    /// Original protected lemma, excluding POS, feature and clitic syntax.
    pub fn original(&self) -> &str {
        self.0.text()
    }

    /// Producer-owned UTF-8 byte range in the original document.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.0.raw_node().byte_range()
    }
}

/// Sensitive morphology review; neither variant grants output authority.
pub enum MorphologyOutcome<'input> {
    /// Proposed item preserving POS, features and post-clitics.
    Proposed {
        /// Borrowed source item that justified this proposal.
        original: &'input Mor,
        /// Proposed item, not validated document output.
        proposed: Box<Mor>,
        /// Exact main-lemma field; never a whole-tier serialization span.
        source: LemmaSource<'input>,
    },
    /// Keep the original and block eventual output pending review.
    Refused {
        /// Source item, absent when no alignment could be established.
        original: Option<&'input Mor>,
        /// Value-free refusal classification.
        reason: MorphologyRefusal,
    },
}

/// A morphology finding bound to a word in the admitted source document.
pub struct MorphologyReview<'input> {
    location: super::WordLocation,
    word: &'input Word,
    outcome: MorphologyOutcome<'input>,
}

impl MorphologyReview<'_> {
    /// Original main-tier position whose decision drives this review.
    pub fn location(&self) -> super::WordLocation {
        self.location
    }
    /// The selected main-tier word, not an independently matched lemma.
    pub fn word(&self) -> &Word {
        self.word
    }

    /// Aligned original item, when the document supplies a verified pair.
    pub fn original(&self) -> Option<&Mor> {
        match &self.outcome {
            MorphologyOutcome::Proposed { original, .. } => Some(original),
            MorphologyOutcome::Refused { original, .. } => *original,
        }
    }

    /// Sensitive proposal or value-free refusal reason.
    pub fn outcome(&self) -> &MorphologyOutcome<'_> {
        &self.outcome
    }
}

pub(super) fn plan<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
    selected: &SelectedWords<'_, 'input>,
) -> Vec<MorphologyReview<'input>> {
    let mut reviews = Vec::new();
    for utterance in input.document().document().utterances() {
        let Some(tier) = utterance.mor_tier() else {
            continue;
        };
        let alignment = utterance
            .alignments
            .as_ref()
            .and_then(|set| set.mor.as_ref());
        // Resolve source fields only if this tier contains a selected word,
        // and at most once even when several aligned items change.
        let sources = std::cell::OnceCell::new();
        visit_mor_positions(&utterance.main.content.content, &mut |position| {
            let Some(word) = position.word() else { return };
            let Some(preview) = selected.get(&(word as *const Word)) else {
                return;
            };
            let original = alignment
                .filter(|alignment| alignment.is_error_free())
                .and_then(|alignment| alignment.pairs.get(position.index().as_usize()))
                .filter(|pair| pair.source_index == Some(position.index()))
                .and_then(|pair| pair.target_index)
                .and_then(|index| tier.items().get(index.as_usize()).map(|item| (index, item)));
            let outcome = match original {
                None => MorphologyOutcome::Refused {
                    original: None,
                    reason: MorphologyRefusal::MissingAlignment,
                },
                Some((index, item)) => {
                    match sources
                        .get_or_init(|| lemma_sources(input, tier))
                        .as_ref()
                        .ok()
                        .and_then(|fields| fields.get(index.as_usize()))
                    {
                        Some(source) => propose(preview, item, *source),
                        None => MorphologyOutcome::Refused {
                            original: Some(item),
                            reason: MorphologyRefusal::SourceCorrespondence,
                        },
                    }
                }
            };
            reviews.push(MorphologyReview {
                location: preview.location(),
                word,
                outcome,
            });
        });
    }
    reviews
}

fn propose<'input>(
    preview: &WordPreview<'_>,
    item: &'input Mor,
    source: SourceBound<'input, 'input, MorLemmaNode<'input>>,
) -> MorphologyOutcome<'input> {
    let mut decisions = preview.decisions();
    let first = decisions.next();
    if decisions.next().is_some() {
        return MorphologyOutcome::Refused {
            original: Some(item),
            reason: MorphologyRefusal::ComponentCorrespondence,
        };
    }
    match first {
        Some(ComponentDecision::Replace {
            original,
            replacement,
        }) if item.main.lemma.as_ref() == original.as_str() => {
            let mut proposed = item.clone();
            proposed.main.lemma = MorStem::new(replacement.as_ref());
            MorphologyOutcome::Proposed {
                original: item,
                proposed: Box::new(proposed),
                source: LemmaSource(source),
            }
        }
        _ => MorphologyOutcome::Refused {
            original: Some(item),
            reason: MorphologyRefusal::LemmaCorrespondence,
        },
    }
}

/// The admitted parser lowers each `mor_content` into exactly one `Mor`, in
/// source order. Select those generated nodes within this tier, then project
/// only their named main word's lemma (never a post-clitic's lemma). Count and
/// lexical corroboration refuse drift; they do not search text for a match.
fn lemma_sources<'input>(
    input: &'input PseudonymizationInput<'_, '_>,
    tier: &MorTier,
) -> Result<Vec<SourceBound<'input, 'input, MorLemmaNode<'input>>>, MorphologyRefusal> {
    let refused = MorphologyRefusal::SourceCorrespondence;
    let parsed = input.parsed_source();
    let raw = parsed
        .root_node()
        .named_descendant_for_byte_range(tier.span.start as usize, tier.span.end as usize)
        .ok_or(refused)?;
    let bound = parsed.bind(raw).map_err(|_| refused)?;
    let tier_node = bound.typed::<MorDependentTierNode>().ok_or(refused)?;
    let mut fields = Vec::with_capacity(tier.items().len());
    for descendant in tier_node.descendants() {
        let Some(item) = descendant.map_err(|_| refused)?.typed::<MorContentNode>() else {
            continue;
        };
        let item = item.extract().map_err(|_| refused)?;
        let main = match item.field_main().slot().view() {
            SourceSlotView::Present(field) => field.read().map_err(|_| refused)?,
            SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(_) => {
                return Err(refused);
            }
        };
        let main = main.extract().map_err(|_| refused)?;
        let lemma = match main.field_child_2().slot().view() {
            SourceSlotView::Present(field) => field.read().map_err(|_| refused)?,
            SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(_) => {
                return Err(refused);
            }
        };
        fields.push(lemma);
    }
    if fields.len() != tier.items().len()
        || fields
            .iter()
            .zip(tier.items())
            .any(|(field, item)| field.text() != item.main.lemma.as_ref())
    {
        return Err(refused);
    }
    Ok(fields)
}
