//! Underline-marker balance validation for utterance content trees.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

// Protected against the class this module's own history belongs to: a new
// `WordContent` variant must not silently join a `_ =>` that answers wrong.
// See `talkbank-parser-tests/src/content_catch_alls.rs`.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::model::{ContentStructure, LeafContent, Utterance, Word, WordContent};
use crate::{ErrorCode, ErrorSink, ParseError, Severity};

/// Validate underline markers are balanced in an utterance.
///
/// CHAT format uses control characters `\u0002\u0001` for underline begin
/// and `\u0002\u0002` for underline end.
///
/// This validates that within a single utterance:
/// - Every underline begin has a corresponding underline end
/// - Underline markers are properly paired (no crossing/interleaving)
///
/// Uses stack-based validation to ensure proper pairing, not just counting.
///
/// Note: Underline markers are within-utterance only (they do not cross utterances).
pub(crate) fn check_underline_balance(utterance: &Utterance, errors: &impl ErrorSink) {
    let tier_location = utterance.main.location;
    // Stack of spans for each open underline-begin marker
    let mut begin_spans: Vec<crate::Location> = Vec::new();

    for content in &utterance.main.content.content {
        walk_underline_balance_in_content(
            content.structure(),
            &mut begin_spans,
            tier_location,
            errors,
        );
    }

    // Check for unclosed begin markers
    for begin_location in &begin_spans {
        errors.report(
            ParseError::at_location(
                ErrorCode::UnmatchedUnderlineBegin,
                Severity::Error,
                *begin_location,
                "Unmatched underline begin: unclosed begin marker (␂␁)",
            )
            .with_suggestion("Ensure each underline begin (␂␁) has a matching underline end (␂␂)"),
        );
    }
}

/// Walk the shared structural view without losing underline leaf provenance.
///
/// Group and retrace variants carry their content infallibly. Words retain
/// authored replacement targets; all descendants share the same begin stack.
fn walk_underline_balance_in_content(
    item: ContentStructure<'_>,
    begin_spans: &mut Vec<crate::Location>,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    let content = match item {
        ContentStructure::Word(word) => {
            for word in word.words() {
                walk_underline_balance_in_word(word, begin_spans, location, errors);
            }
            return;
        }
        ContentStructure::Group(group) => group.content(),
        ContentStructure::Retrace(retrace) => &retrace.inner().content,
        ContentStructure::Leaf(leaf) => {
            match leaf.content {
                LeafContent::UnderlineBegin(marker) => {
                    begin_spans.push(marker.or(location));
                }
                LeafContent::UnderlineEnd(marker) => {
                    apply_underline_end(begin_spans, marker.or(location), errors);
                }
                LeafContent::Spoken | LeafContent::Notation => {}
            }
            return;
        }
    };
    for child in &content.content {
        walk_underline_balance_in_content(child.structure(), begin_spans, location, errors);
    }
}

/// Walk underline-balance state through inline word content markers.
fn walk_underline_balance_in_word(
    word: &Word,
    begin_spans: &mut Vec<crate::Location>,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    // A marker falls back to its word, and a word to its tier.
    let word_location = word.location.or(location);
    for wc in word.content() {
        match wc {
            WordContent::UnderlineBegin(wb) => {
                begin_spans.push(wb.location().or(word_location));
            }
            WordContent::UnderlineEnd(we) => {
                apply_underline_end(begin_spans, we.location().or(word_location), errors);
            }
            // Named, not swept. Every one of these carries no underline
            // marker, so doing nothing is the right answer today; the point of
            // writing them out is that a NEW `WordContent` variant cannot join
            // them by default. Four catch-alls of exactly this shape have
            // shipped as defects (see `content_catch_alls`).
            WordContent::Text(_)
            | WordContent::Phonetic(_)
            | WordContent::Shortening(_)
            | WordContent::OverlapPoint(_)
            | WordContent::CAElement(_)
            | WordContent::CADelimiter(_)
            | WordContent::StressMarker(_)
            | WordContent::Lengthening(_)
            | WordContent::SyllablePause(_)
            | WordContent::CompoundMarker(_)
            | WordContent::CliticBoundary(_) => {}
        }
    }
}

/// Apply one underline-end marker against the current begin stack.
///
/// If no open begin exists, emit `UnmatchedUnderlineEnd` at the end marker span.
fn apply_underline_end(
    begin_spans: &mut Vec<crate::Location>,
    location: crate::Location,
    errors: &impl ErrorSink,
) {
    if begin_spans.pop().is_none() {
        errors.report(
            ParseError::at_location(
                ErrorCode::UnmatchedUnderlineEnd,
                Severity::Error,
                location,
                "Unmatched underline end (␂␂) without corresponding begin (␂␁)",
            )
            .with_suggestion(
                "Ensure each underline end (␂␂) has a matching underline begin (␂␁) before it",
            ),
        );
    }
}
