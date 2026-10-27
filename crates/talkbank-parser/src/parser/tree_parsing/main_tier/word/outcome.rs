//! A lowered word is not necessarily a model word: recovery can retain a
//! prefix or suffix without producing any body pieces.

use crate::error::ErrorSink;
use talkbank_model::model::annotation::replacement_omission_diagnostic;
use talkbank_model::model::{Word, WordCategory};
use talkbank_model::{ParseOutcome, Span};

/// Only this module's producer can distinguish complete and incomplete syntax.
pub(in crate::parser) struct LoweredWord(Result<Word, WordRefusal>);

enum WordRefusal {
    Incomplete {
        span: Span,
        category: Option<WordCategory>,
    },
    DiagnosedRejection,
}

impl LoweredWord {
    pub(super) fn complete(word: Word) -> Self {
        Self(Ok(word))
    }

    pub(super) fn incomplete(span: Span, category: Option<WordCategory>) -> Self {
        Self(Err(WordRefusal::Incomplete { span, category }))
    }

    pub(super) fn rejected() -> Self {
        Self(Err(WordRefusal::DiagnosedRejection))
    }

    /// Incomplete syntax cannot cross the model construction boundary.
    pub(in crate::parser) fn into_word(self) -> ParseOutcome<Word> {
        match self.0 {
            Ok(word) => ParseOutcome::Parsed(word),
            Err(WordRefusal::Incomplete { .. } | WordRefusal::DiagnosedRejection) => {
                ParseOutcome::Rejected
            }
        }
    }

    /// Preserve the replacement rule for a written omission prefix without
    /// fabricating its missing word body. Complete words are validated normally.
    pub(in crate::parser) fn into_replacement_word(
        self,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        match self.0 {
            Ok(word) => ParseOutcome::Parsed(word),
            Err(WordRefusal::Incomplete { span, category }) => {
                if let Some(error) = replacement_omission_diagnostic(category.as_ref(), span.into())
                {
                    errors.report(error);
                }
                ParseOutcome::Rejected
            }
            Err(WordRefusal::DiagnosedRejection) => ParseOutcome::Rejected,
        }
    }
}
