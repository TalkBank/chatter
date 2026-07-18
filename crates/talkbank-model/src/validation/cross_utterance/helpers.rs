//! Helper functions for cross-utterance validation
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Utterance_Linkers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotedNewLine_Terminator>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OtherCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SelfCompletion_Linker>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::model::{LinkerKind, Utterance};

/// Helper: Check if utterance has quoted linker (+")
pub(super) fn has_quoted_linker(utterance: &Utterance) -> bool {
    utterance
        .main
        .content
        .linkers
        .iter()
        .any(|l| matches!(l.kind, LinkerKind::QuotationFollows))
}

/// Helper: Check if utterance has other-completion linker (++)
///
/// Other-completion (++) means a different speaker is finishing or continuing
/// another speaker's incomplete thought.
pub(super) fn has_other_completion_linker(utterance: &Utterance) -> bool {
    utterance
        .main
        .content
        .linkers
        .iter()
        .any(|l| matches!(l.kind, LinkerKind::OtherCompletion))
}
