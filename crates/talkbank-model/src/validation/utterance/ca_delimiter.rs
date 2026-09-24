//! Validation helpers for paired conversation-analysis (CA) delimiters.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Delimiters>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Overlaps>

use crate::model::{
    CADelimiterType, ContentStructure, Descend, Utterance, UtteranceContent, Word, WordContent,
};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use std::collections::HashMap;

/// A raw delimiter token discovered during utterance traversal.
///
/// Role (`Begin`/`End`) is assigned later once same-type pairing is resolved.
#[derive(Clone)]
struct CADelimiterOccurrence {
    delimiter_type: CADelimiterType,
    word_span: Span,
    word_text: String,
}

/// Role assigned to each delimiter occurrence after left-to-right pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CADelimiterRole {
    Begin,
    End,
}

/// Delimiter occurrence annotated with role and pairing status for diagnostics.
#[derive(Clone)]
pub(crate) struct CADelimiterRoleOccurrence {
    pub(crate) delimiter_type: CADelimiterType,
    pub(crate) role: CADelimiterRole,
    pub(crate) word_span: Span,
    pub(crate) word_text: String,
    pub(crate) is_paired: bool,
}

/// Validate CA delimiters are balanced across an utterance.
///
/// CA delimiters are paired prosodic markers (e.g., ∆, °) that must be balanced
/// in left-to-right order across the entire utterance, not per word. Errors are
/// emitted only for unmatched begin-role delimiters after role assignment.
pub(crate) fn check_ca_delimiter_balance(utterance: &Utterance, errors: &impl ErrorSink) {
    for delimiter in analyze_ca_delimiter_roles(utterance) {
        if matches!(delimiter.role, CADelimiterRole::Begin) && !delimiter.is_paired {
            errors.report(
                ParseError::new(
                    ErrorCode::UnbalancedCADelimiter,
                    Severity::Error,
                    SourceLocation::new(delimiter.word_span),
                    ErrorContext::new(delimiter.word_text.clone(), delimiter.word_span, &delimiter.word_text),
                    format!(
                        "Unbalanced CA delimiter {} ({:?}): missing closing delimiter",
                        delimiter.delimiter_type.to_symbol(),
                        delimiter.delimiter_type
                    ),
                )
                .with_suggestion(format!(
                    "CA delimiters mark prosodic features and must be paired. Add another {} ({:?}) delimiter to close the pair",
                    delimiter.delimiter_type.to_symbol(),
                    delimiter.delimiter_type
                )),
            );
        }
    }
}

/// Traverse an utterance and classify each delimiter as begin/end by type.
///
/// The returned sequence is ordered by appearance and includes pairing metadata
/// used by diagnostics and tests.
pub(crate) fn analyze_ca_delimiter_roles(utterance: &Utterance) -> Vec<CADelimiterRoleOccurrence> {
    let mut delimiters = Vec::new();
    for item in &utterance.main.content.content {
        collect_ca_delimiters_from_content(item, &mut delimiters);
    }
    assign_delimiter_roles(delimiters)
}

/// Assign begin/end roles by pairing same-type delimiters with a stack.
///
/// A delimiter is considered an `End` only when a prior unmatched delimiter of
/// the same type exists; otherwise it starts a new open region.
fn assign_delimiter_roles(
    delimiters: Vec<CADelimiterOccurrence>,
) -> Vec<CADelimiterRoleOccurrence> {
    let mut open_by_type: HashMap<CADelimiterType, Vec<usize>> = HashMap::new();
    let mut out = Vec::with_capacity(delimiters.len());

    for delimiter in delimiters {
        if let Some(open_stack) = open_by_type.get_mut(&delimiter.delimiter_type)
            && let Some(open_index) = open_stack.pop()
        {
            out.push(CADelimiterRoleOccurrence {
                delimiter_type: delimiter.delimiter_type,
                role: CADelimiterRole::End,
                word_span: delimiter.word_span,
                word_text: delimiter.word_text.clone(),
                is_paired: true,
            });
            out[open_index].is_paired = true;
            continue;
        }

        let out_index = out.len();
        out.push(CADelimiterRoleOccurrence {
            delimiter_type: delimiter.delimiter_type,
            role: CADelimiterRole::Begin,
            word_span: delimiter.word_span,
            word_text: delimiter.word_text,
            is_paired: false,
        });
        open_by_type
            .entry(delimiter.delimiter_type)
            .or_default()
            .push(out_index);
    }

    out
}

/// Collect delimiters using the canonical structural traversal.
///
/// The typed word view yields the produced form before replacement targets.
/// Group/retrace descent belongs to the structure producer, so consumers do
/// not reclassify containers or conditionally rediscover their content.
fn collect_ca_delimiters_from_content(
    item: &UtteranceContent,
    delimiters: &mut Vec<CADelimiterOccurrence>,
) {
    item.structure().walk(&mut |structure| {
        match structure {
            ContentStructure::Word(words) => {
                for word in words.words() {
                    collect_ca_delimiters_from_word(word, delimiters);
                }
            }
            ContentStructure::Group(_)
            | ContentStructure::Retrace(_)
            | ContentStructure::Leaf(_) => {}
        }
        Descend::Into
    });
}

/// Extracts all CA delimiter markers present in a single word token.
fn collect_ca_delimiters_from_word(word: &Word, delimiters: &mut Vec<CADelimiterOccurrence>) {
    for content in word.content() {
        if let WordContent::CADelimiter(delimiter) = content {
            delimiters.push(CADelimiterOccurrence {
                delimiter_type: delimiter.delimiter_type,
                word_span: word.span,
                word_text: word.cleaned_text().to_string(),
            });
        }
    }
}
