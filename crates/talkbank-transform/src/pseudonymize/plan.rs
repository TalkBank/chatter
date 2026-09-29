//! Lexical and aligned morphology review, before complete output admission.

use std::collections::HashMap;
use std::fmt;

use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::{Span, Word};

use super::word::WordPlan;
use super::{ComponentDecision, PseudonymizationInput, WordRefusal};

/// Which lexical spelling at a main-tier word position owns a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordSpelling {
    /// Original speech, including retraced speech.
    Spoken,
    /// Zero-based target within this word's editorial replacement annotation.
    ReplacementTarget(usize),
}

/// Location assigned by the canonical lexical traversal, not inferred from text.
/// Indices are zero-based; the word index counts original words (including
/// retraces), not separators or replacement targets. It is not a `%mor` or
/// phonology alignment index. Targets retain their original word's position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordLocation {
    utterance: usize,
    word: usize,
    spelling: WordSpelling,
}

impl WordLocation {
    /// Zero-based utterance in the admitted document.
    pub fn utterance(self) -> usize {
        self.utterance
    }
    /// Zero-based original lexical word in this utterance.
    pub fn word(self) -> usize {
        self.word
    }
    /// Original speech or a specific editorial target.
    pub fn spelling(self) -> WordSpelling {
        self.spelling
    }
}

/// Sensitive original/proposed word pair from one admitted document.
/// This is a review preview, not validated document output.
pub struct WordPreview<'input> {
    location: WordLocation,
    plan: WordPlan<'input>,
    proposed: Word,
    edits: Vec<super::LexicalEdit<'input>>,
}

impl WordPreview<'_> {
    /// Location of the decision in the original document.
    pub fn location(&self) -> WordLocation {
        self.location
    }
    /// Carry this proposal's decisions into an already aligned timing word.
    pub(super) fn project_onto<'target>(
        &self,
        word: &'target Word,
    ) -> Result<WordPlan<'target>, WordRefusal> {
        self.plan.project_onto(word)
    }
    /// Exact lexical field edits, excluding markers, suffixes and annotations.
    /// These previews do not authorize whole-document output.
    pub fn edits(&self) -> &[super::LexicalEdit<'_>] {
        &self.edits
    }
    /// Original protected word; its span addresses the plan's original source.
    pub fn original(&self) -> &Word {
        self.plan.original()
    }

    /// Proposed typed word, not authority to publish or write a document.
    /// Its span still addresses the original, not a rewritten file.
    pub fn proposed(&self) -> &Word {
        &self.proposed
    }

    /// The same sensitive decisions used to construct this proposal.
    pub fn decisions(&self) -> impl Iterator<Item = &ComponentDecision> {
        self.plan.decisions()
    }
}

/// A word that could not be safely planned; no proposal is created for it.
pub struct RefusedWord {
    location: WordLocation,
    span: Span,
    reason: WordRefusal,
}

impl RefusedWord {
    /// Location retained even when no safe proposal exists.
    pub fn location(&self) -> WordLocation {
        self.location
    }
    /// Location in the plan's protected original source.
    pub fn span(&self) -> Span {
        self.span
    }

    /// Value-free reason, suitable for ordinary diagnostic output.
    pub fn reason(&self) -> WordRefusal {
        self.reason
    }
}

/// Lexical and morphology review stage bound to the source it inspected.
///
/// Original and replacement-target words, including retraced speech, are
/// inspected through the canonical word walker. Only findings allocate previews;
/// unrelated words are not cloned. There is deliberately no writer, serializer,
/// or "ready" verdict: even a refusal-free lexical plan still owes cross-tier,
/// header/free-text and final output checks.
pub struct LexicalPlan<'input> {
    source: &'input str,
    previews: Vec<WordPreview<'input>>,
    refusals: Vec<RefusedWord>,
    morphology: Vec<super::MorphologyReview<'input>>,
    lemma_findings: Vec<super::LemmaFinding<'input>>,
    timing: Vec<super::TimingReview<'input>>,
    pronunciation: Vec<super::PronunciationRefusal<'input>>,
    header_fields: Result<Vec<super::HeaderFieldReview<'input>>, super::HeaderFieldRefusal>,
    free_text: Result<Vec<super::FreeTextFinding<'input>>, super::FreeTextRefusal>,
}

impl LexicalPlan<'_> {
    /// Unicode whole-word findings from typed prose fields; no output authority.
    pub fn free_text(&self) -> Result<&[super::FreeTextFinding<'_>], super::FreeTextRefusal> {
        self.free_text.as_deref().map_err(|error| *error)
    }
    /// Source-bound whole-word findings within participant-name and ID metadata fields.
    pub fn header_fields(
        &self,
    ) -> Result<&[super::HeaderFieldReview<'_>], super::HeaderFieldRefusal> {
        self.header_fields.as_deref().map_err(|error| *error)
    }
    /// Mapped morphology lemmas not changed by an aligned main-tier proposal.
    pub fn lemma_findings(&self) -> &[super::LemmaFinding<'_>] {
        &self.lemma_findings
    }
    /// Pronunciation that cannot safely follow a selected orthographic change.
    pub fn pronunciation(&self) -> &[super::PronunciationRefusal<'_>] {
        &self.pronunciation
    }
    /// Corroborated timing-tier proposals or drift refusals.
    pub fn timing(&self) -> &[super::TimingReview<'_>] {
        &self.timing
    }

    /// Aligned morphology findings driven by this plan's changed words.
    pub fn morphology(&self) -> &[super::MorphologyReview<'_>] {
        &self.morphology
    }

    /// Protected original source associated with every reported word span.
    pub fn source(&self) -> &str {
        self.source
    }

    /// Changed or near-miss words in document order, with private decisions.
    pub fn previews(&self) -> &[WordPreview<'_>] {
        &self.previews
    }

    /// Unsafe word plans; these must prevent eventual document output.
    pub fn refusals(&self) -> &[RefusedWord] {
        &self.refusals
    }
}

impl PseudonymizationInput<'_, '_> {
    /// Review selective words and aligned morphology without writing the input.
    pub fn plan_words(&self) -> LexicalPlan<'_> {
        let mut result = LexicalPlan {
            source: self.source(),
            previews: Vec::new(),
            refusals: Vec::new(),
            morphology: Vec::new(),
            lemma_findings: Vec::new(),
            timing: Vec::new(),
            pronunciation: Vec::new(),
            header_fields: super::header_fields::plan(self),
            free_text: super::free_text::plan(self),
        };
        for (utterance_index, utterance) in self.document().document().utterances().enumerate() {
            let mut word_index = 0;
            walk_words(
                &utterance.main.content.content,
                None,
                &mut |item| match item {
                    WordItem::Word(word) => {
                        assess_word(
                            word,
                            WordLocation {
                                utterance: utterance_index,
                                word: word_index,
                                spelling: WordSpelling::Spoken,
                            },
                            self,
                            &mut result,
                        );
                        word_index += 1;
                    }
                    WordItem::ReplacedWord(replaced) => {
                        let location = WordLocation {
                            utterance: utterance_index,
                            word: word_index,
                            spelling: WordSpelling::Spoken,
                        };
                        assess_word(&replaced.word, location, self, &mut result);
                        for (index, target) in replaced.replacement.words.iter().enumerate() {
                            assess_word(
                                target,
                                WordLocation {
                                    spelling: WordSpelling::ReplacementTarget(index),
                                    ..location
                                },
                                self,
                                &mut result,
                            );
                        }
                        word_index += 1;
                    }
                    WordItem::Separator(_) => {}
                },
            );
        }
        let selected = selected_words(&result.previews);
        result.morphology = super::morphology::plan(self, &selected);
        result.lemma_findings = super::morphology::lemma_findings(self, &result.morphology);
        result.timing = super::timing::plan(self, &selected);
        result.pronunciation = super::phonology::plan(self, &selected);
        result
    }
}

/// Sparse identity index shared by cross-tier owners. Spelling and source spans
/// cannot identify repeated or nested lexical nodes unambiguously.
pub(super) type SelectedWords<'plan, 'input> = HashMap<*const Word, &'plan WordPreview<'input>>;

fn selected_words<'plan, 'input>(
    previews: &'plan [WordPreview<'input>],
) -> SelectedWords<'plan, 'input> {
    previews
        .iter()
        .filter(|preview| {
            preview
                .decisions()
                .any(|decision| matches!(decision, ComponentDecision::Replace { .. }))
        })
        .map(|preview| (preview.original() as *const Word, preview))
        .collect()
}

fn assess_word<'input>(
    word: &'input Word,
    location: WordLocation,
    input: &'input PseudonymizationInput<'_, '_>,
    result: &mut LexicalPlan<'input>,
) {
    match WordPlan::assess(word, input.names()) {
        Ok(plan) => {
            if plan
                .decisions()
                .all(|decision| matches!(decision, ComponentDecision::Keep))
            {
                return;
            }
            let proposed = plan.apply();
            match plan.source_edits(input.parsed_source()) {
                Ok(edits) => result.previews.push(WordPreview {
                    location,
                    plan,
                    proposed,
                    edits,
                }),
                Err(reason) => result.refusals.push(RefusedWord {
                    location,
                    span: word.span,
                    reason,
                }),
            }
        }
        Err(reason) => result.refusals.push(RefusedWord {
            location,
            span: word.span,
            reason,
        }),
    }
}

impl fmt::Debug for LexicalPlan<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LexicalPlan(<private>)")
    }
}
