use super::FragmentSemanticContext;
use crate::model::{ChatFile, Line, Utterance};

/// Why a parsed population cannot be returned by a single-utterance API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UtteranceCardinalityError {
    /// No utterance was produced.
    #[error("No utterance found in parsed output")]
    Missing,
    /// Returning one utterance would discard another.
    #[error("Expected one utterance, but the input contains multiple utterances")]
    Multiple,
}

/// Exactly one utterance admitted from the parser's complete population.
///
/// This proves cardinality only, not CHAT validity or absence of recovery.
/// Document admission retains semantic options until the utterance is released.
#[derive(Debug, talkbank_derive::SpanShift)]
pub struct SingleUtterance {
    utterance: Utterance,
    #[span_shift(skip)]
    context: FragmentSemanticContext,
}

impl SingleUtterance {
    /// Consume a parsed document, retaining its effective options and refusing
    /// absence or a second utterance before any headers can be discarded.
    ///
    /// # Errors
    /// Returns [`UtteranceCardinalityError::Missing`] for an empty population
    /// and [`UtteranceCardinalityError::Multiple`] when a second value exists.
    pub fn admit(file: ChatFile) -> Result<Self, UtteranceCardinalityError> {
        let context = FragmentSemanticContext::new()
            .with_option_flags(file.option_flags().cloned().collect());
        let mut utterances = file.lines.into_iter().filter_map(|line| match line {
            Line::Utterance(utterance) => Some(*utterance),
            Line::Header { .. } => None,
        });
        let utterance = utterances
            .next()
            .ok_or(UtteranceCardinalityError::Missing)?;
        if utterances.next().is_some() {
            return Err(UtteranceCardinalityError::Multiple);
        }
        Ok(Self { utterance, context })
    }

    /// Transform the admitted utterance while retaining its document context.
    pub fn map_utterance(self, map: impl FnOnce(Utterance) -> Utterance) -> Self {
        Self {
            utterance: map(self.utterance),
            context: self.context,
        }
    }

    /// Release the utterance together with the options admitted from its document.
    pub fn into_parts(self) -> (Utterance, FragmentSemanticContext) {
        (self.utterance, self.context)
    }

    /// Release the single admitted utterance without repeating cardinality checks.
    pub fn into_utterance(self) -> Utterance {
        self.utterance
    }
}
