//! Shared test helpers for cross-utterance validation suites.
//!
//! Helper functions keep fixture construction compact and make individual tests
//! emphasize dialogue sequencing over boilerplate model assembly.

use crate::ParseError;
use crate::model::{Linker, MainTier, Terminator, Utterance, UtteranceContent, Word};
use crate::validation::ValidationContext;

/// Executes cross-utterance validation with quotation/linker validation enabled.
///
/// Test fixtures use this to focus on utterance sequencing rather than context
/// construction boilerplate. Quotation validation is enabled so that E341-E355
/// checks fire during tests.
pub fn check_cross_utterance_patterns(utterances: &[Utterance]) -> Vec<ParseError> {
    let context = ValidationContext::default().with_quotation_validation(true);
    // A real `ChatFile`, not a hand-built sequence. There USED to be a
    // `#[cfg(test)]` constructor that took the slice directly; it was the one
    // door into the invariant the sequence type exists to hold, and it turned
    // out to be unnecessary, because `Line::Utterance` and `ChatFile::new` are
    // both public. Going through the front door also keeps the PUBLIC entry
    // point tested: routing these fixtures around it left it with no coverage
    // at all, and it is the LSP's only way in.
    let file = crate::model::ChatFile::new(
        utterances
            .iter()
            .cloned()
            .map(|utt| crate::model::Line::Utterance(Box::new(utt)))
            .collect(),
    );
    crate::validation::cross_utterance::check_cross_utterance_patterns(&file, &context)
}

/// Builds a minimal utterance fixture for cross-utterance tests.
///
/// The helper wires words, linkers, and terminator into a `MainTier` so tests
/// can describe dialogue sequences compactly.
pub fn make_utterance(
    speaker: &str,
    words: Vec<&str>,
    linkers: Vec<Linker>,
    terminator: Terminator,
) -> Utterance {
    let content: Vec<UtteranceContent> = words
        .into_iter()
        .map(|w| UtteranceContent::Word(Box::new(Word::new_unchecked(w, w))))
        .collect();

    let main = MainTier::new(speaker.to_string(), content, Some(terminator)).with_linkers(linkers);
    Utterance::new(main)
}
