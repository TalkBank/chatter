//! Cross-utterance validation patterns
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use super::UtterancePosition;
use super::helpers::has_quoted_linker;
use crate::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation};

/// Validates `+".` quotation-precedes terminators against prior `+"` linkers.
///
/// The nearest preceding same-speaker utterance must be marked as quoted
/// speech; an older quote cannot cross intervening ordinary same-speaker speech.
pub(super) fn check_quotation_precedes(position: &UtterancePosition<'_, '_>) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let utterance = position.current();
    let speaker = utterance.main.speaker.as_str();

    // The file-issued position traverses nearest first. Selection yields one
    // optional turn, so an ordinary intervening turn cannot be skipped later.
    let preceding = position
        .preceding()
        .find(|previous| previous.main.speaker.as_str() == speaker);
    if !preceding.is_some_and(has_quoted_linker) {
        errors.push(
            ParseError::new(
                ErrorCode::InvalidContentAnnotationNesting,
                Severity::Error,
                SourceLocation::new(utterance.main.span),
                ErrorContext::new(
                    format!("*{}: ... +\". ", speaker),
                    utterance.main.span,
                    "quotation precedes terminator",
                ),
                format!(
                    "Quotation precedes terminator (+\". ) without preceding quoted utterances (+\") from same speaker ({})",
                    speaker
                ),
            )
            .with_suggestion(format!(
                "Add +\" linker to preceding utterance(s) by {} to mark them as quoted speech",
                speaker
            ))
        );
    }

    errors
}
