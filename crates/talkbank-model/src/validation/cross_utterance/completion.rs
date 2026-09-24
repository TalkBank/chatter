//! Cross-utterance validation patterns for completion linkers
//!
//! ## Completion Patterns
//!
//! - **Self-completion (`+,`)**: Resumes an interrupted utterance by the same speaker.
//!   Requires preceding utterance from same speaker to end with interruption terminator (`+/.`).
//!
//! - **Other-completion (`++`)**: Completes another speaker's incomplete utterance.
//!   Requires preceding utterance from different speaker to end with trailing-off terminator (`+...`).
//!
//! ## Performance Optimization (2025-12-29)
//!
//! Self-completion validation was optimized from O(n²) to O(n) using a stack-based algorithm:
//!
//! **Before** (O(n²)): Each utterance with `+,` searched backward through all prior
//! same-speaker utterances to find matching `+/.` terminator.
//!
//! **After** (O(n)): One forward pass maintains per-speaker interruption tokens.
//! A `+,` consumes a token for an O(1) match; tokens carry no unused indices.
//!
//! This is critical for large conversational files with many completion patterns.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use super::{FileUtterances, UtterancePosition};
use crate::model::{LinkerKind, Terminator, Utterance};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use std::collections::{HashMap, hash_map::Entry};

/// Single-use evidence issued only for a typed interruption terminator.
struct PendingInterruption;

/// Distinct refusal states of consuming a speaker's prior interruption.
enum CompletionRefusal {
    NoPriorTurn,
    NoPendingInterruption,
}

/// Validate all `+,` self-completion linkers in one forward pass.
///
/// This replaces the O(n²) per-utterance backward search with a stack-based
/// approach that processes all utterances in a single forward pass.
///
/// A vacant history means E351 (no previous turn). An occupied history with
/// no remaining token means E352. Tokens are consumed before this turn's own
/// terminator can issue a new one, so self-completing interruptions keep the
/// correct lifecycle without two maps or a separate match flag.
pub(super) fn check_self_completion_all(utterances: &FileUtterances<'_>, errors: &impl ErrorSink) {
    let mut histories: HashMap<&str, Vec<PendingInterruption>> = HashMap::new();

    for utterance in utterances.iter() {
        let speaker = utterance.main.speaker.as_str();
        let mut history = histories.entry(speaker);

        // Check if this has self-completion linker (+,)
        if has_self_completion_linker_internal(utterance) {
            let completion = match &mut history {
                Entry::Occupied(prior) => prior
                    .get_mut()
                    .pop()
                    .ok_or(CompletionRefusal::NoPendingInterruption),
                Entry::Vacant(_) => Err(CompletionRefusal::NoPriorTurn),
            };
            match completion {
                Ok(_consumed) => {}
                Err(CompletionRefusal::NoPendingInterruption) => {
                    // E352: prior turns exist, but no unconsumed interruption remains.
                    errors.report(
                        ParseError::new(
                            ErrorCode::MissingQuoteEnd,
                            Severity::Error,
                            SourceLocation::new(utterance.main.span),
                            ErrorContext::new(
                                format!("*{}: +, ...", speaker),
                                utterance.main.span,
                                "self-completion linker",
                            ),
                            format!(
                                "Self-completion linker (+,) but preceding same-speaker utterance doesn't end with +/. (interruption terminator) from speaker {}",
                                speaker
                            ),
                        )
                        .with_suggestion("Change the preceding utterance terminator to +/. to mark it as interrupted")
                    );
                }
                Err(CompletionRefusal::NoPriorTurn) => {
                    // E351: no prior utterance from this speaker at all.
                    errors.report(
                        ParseError::new(
                            ErrorCode::MissingQuoteBegin,
                            Severity::Error,
                            SourceLocation::new(utterance.main.span),
                            ErrorContext::new(
                                format!("*{}: +, ...", speaker),
                                utterance.main.span,
                                "self-completion linker",
                            ),
                            format!(
                                "Self-completion linker (+,) without any preceding utterance from same speaker ({})",
                                speaker
                            ),
                        )
                        .with_suggestion("Self-completion is used to resume an interrupted utterance; ensure there's a prior interrupted utterance with +/. terminator")
                    );
                }
            }
        }

        // Occupancy records that the speaker has now had a turn, even if that
        // turn had no interruption. No independent last-seen map can drift.
        let pending = history.or_default();

        // If it ends with `+/.`, also push onto the interruption stack.
        if let Some(ref term) = utterance.main.content.terminator
            && matches!(term, Terminator::Interruption { .. })
        {
            pending.push(PendingInterruption);
        }
    }
}

/// Returns whether an utterance includes the self-completion linker (`+,`).
fn has_self_completion_linker_internal(utterance: &Utterance) -> bool {
    utterance
        .main
        .content
        .linkers
        .iter()
        .any(|linker| matches!(linker.kind, LinkerKind::SelfCompletion))
}

/// Validate one `++` other-completion linker usage.
///
/// Requires: Most recent utterance by DIFFERENT speaker ended with +... (trailing off)
pub(super) fn check_other_completion(position: &UtterancePosition<'_, '_>) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let utterance = position.current();
    let speaker = utterance.main.speaker.as_str();

    // Check if there's any preceding utterance at all
    let Some(prev_utt) = position.preceding().next() else {
        errors.push(
            ParseError::new(
                ErrorCode::MissingOtherCompletionContext,
                Severity::Error,
                SourceLocation::new(utterance.main.span),
                ErrorContext::new(
                    format!("*{}: ++ ...", speaker),
                    utterance.main.span,
                    "other-completion linker",
                ),
                "Other-completion linker (++) without any preceding utterance from different speaker",
            )
            .with_suggestion("Other-completion is used to finish another speaker's incomplete thought; ensure there's a prior incomplete utterance with +... terminator from a different speaker")
        );
        return errors;
    };

    // Check if same speaker - should use +, instead
    if prev_utt.main.speaker.as_str() == speaker {
        errors.push(
            ParseError::new(
                ErrorCode::InterleavedContentAnnotations,
                Severity::Error,
                SourceLocation::new(utterance.main.span),
                ErrorContext::new(
                    format!("*{}: ++ ...", speaker),
                    utterance.main.span,
                    "other-completion linker",
                ),
                format!(
                    "Other-completion linker (++) but preceding utterance is from same speaker ({}); use +, for self-completion",
                    speaker
                ),
            )
            .with_suggestion("Change ++ to +, when completing your own interrupted utterance")
        );
        return errors;
    }

    // Now check if it ended with +... (trailing off)
    let has_trailing_off = if let Some(ref term) = prev_utt.main.content.terminator {
        matches!(term, Terminator::TrailingOff { .. })
    } else {
        false
    };

    if !has_trailing_off {
        let prev_speaker = prev_utt.main.speaker.as_str();
        errors.push(
            ParseError::new(
                ErrorCode::MissingTrailingOffTerminator,
                Severity::Error,
                SourceLocation::new(utterance.main.span),
                ErrorContext::new(
                    format!("*{}: ++ ...", speaker),
                    utterance.main.span,
                    "other-completion linker",
                ),
                format!(
                    "Other-completion linker (++) but preceding different-speaker utterance (by {}) doesn't end with +... (trailing off terminator)",
                    prev_speaker
                ),
            )
            .with_suggestion(format!(
                "Change the preceding utterance by {} to end with +... to mark it as trailing off/incomplete",
                prev_speaker
            ))
        );
    }

    errors
}
