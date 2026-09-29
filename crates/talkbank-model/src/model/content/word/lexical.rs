//! One owner of lexical contribution, retaining typed structure for editors.

use super::{CADelimiterType, WordContent};

/// How a typed word leaf contributes to lexical identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexicalContribution<'word> {
    /// Text included in the word's cleaned lexical spelling.
    Spoken(&'word str),
    /// Text inside CA segment-repetition delimiters, excluded from cleaned text
    /// but still present in the transcription and relevant to privacy review.
    Repeated(&'word str),
    /// A structural marker, including compound and clitic boundaries.
    Structural,
}

/// A borrowed leaf classified in the context of its owning word.
///
/// Private construction prevents a caller from attaching a classification from
/// a different word. The content remains available: cleaning must not erase the
/// information an editor needs to preserve compound, clitic or prosodic markers.
pub struct WordLexicalPart<'word> {
    content: &'word WordContent,
    contribution: LexicalContribution<'word>,
}

impl<'word> WordLexicalPart<'word> {
    /// The original typed leaf, with no copying or normalization.
    pub fn content(&self) -> &'word WordContent {
        self.content
    }

    /// Its contribution under the owning word's CA repetition context.
    pub fn contribution(&self) -> LexicalContribution<'word> {
        self.contribution
    }
}

enum RepetitionState {
    Outside,
    Inside,
}

/// Ordered lexical view of a word, constructed by [`super::Word::lexical_parts`].
/// Repetition state travels across all leaves, including compound boundaries.
pub struct WordLexicalParts<'word> {
    leaves: std::slice::Iter<'word, WordContent>,
    repetition: RepetitionState,
}

impl<'word> WordLexicalParts<'word> {
    pub(super) fn new(content: &'word [WordContent]) -> Self {
        Self {
            leaves: content.iter(),
            repetition: RepetitionState::Outside,
        }
    }
}

impl<'word> Iterator for WordLexicalParts<'word> {
    type Item = WordLexicalPart<'word>;

    fn next(&mut self) -> Option<Self::Item> {
        let content = self.leaves.next()?;
        let text = match content {
            WordContent::Text(text) => Some(text.as_ref()),
            WordContent::Shortening(text) => Some(text.as_ref()),
            WordContent::Phonetic(text) => Some(text.as_ref()),
            WordContent::CADelimiter(delimiter) => {
                if delimiter.delimiter_type == CADelimiterType::SegmentRepetition {
                    self.repetition = match self.repetition {
                        RepetitionState::Outside => RepetitionState::Inside,
                        RepetitionState::Inside => RepetitionState::Outside,
                    };
                }
                None
            }
            WordContent::OverlapPoint(_)
            | WordContent::CAElement(_)
            | WordContent::StressMarker(_)
            | WordContent::Lengthening(_)
            | WordContent::SyllablePause(_)
            | WordContent::UnderlineBegin(_)
            | WordContent::UnderlineEnd(_)
            | WordContent::CompoundMarker(_)
            | WordContent::CliticBoundary(_) => None,
        };
        let contribution = match (text, &self.repetition) {
            (Some(text), RepetitionState::Outside) => LexicalContribution::Spoken(text),
            (Some(text), RepetitionState::Inside) => LexicalContribution::Repeated(text),
            (None, _) => LexicalContribution::Structural,
        };
        Some(WordLexicalPart {
            content,
            contribution,
        })
    }
}
