//! Underline-marker balance validation for utterance content trees.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

// Protected against the class this module's own history belongs to: a new
// `WordContent` variant must not silently join a `_ =>` that answers wrong.
// See `talkbank-parser-tests/src/content_catch_alls.rs`.
#![deny(clippy::wildcard_enum_match_arm)]

use crate::model::{
    BracketedContent, BracketedItem, Utterance, UtteranceContent, Word, WordContent,
};
use crate::{ErrorCode, ErrorSink, ParseError, Severity, Span};

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
    let tier_span = utterance.main.span;
    // Stack of spans for each open underline-begin marker
    let mut begin_spans: Vec<Span> = Vec::new();

    for content in &utterance.main.content.content {
        walk_underline_balance_in_content(content, &mut begin_spans, tier_span, errors);
    }

    // Check for unclosed begin markers
    for begin_span in &begin_spans {
        errors.report(
            ParseError::at_span(
                ErrorCode::UnmatchedUnderlineBegin,
                Severity::Error,
                *begin_span,
                "Unmatched underline begin: unclosed begin marker (␂␁)",
            )
            .with_suggestion("Ensure each underline begin (␂␁) has a matching underline end (␂␂)"),
        );
    }
}

/// Walk underline-balance state through one utterance-content node.
///
/// The traversal propagates a shared begin stack so nested groups and words
/// participate in the same pairing context.
fn walk_underline_balance_in_content(
    item: &UtteranceContent,
    begin_spans: &mut Vec<Span>,
    fallback_span: Span,
    errors: &impl ErrorSink,
) {
    match item {
        UtteranceContent::UnderlineBegin(marker) => {
            begin_spans.push(if marker.span.is_dummy() {
                fallback_span
            } else {
                marker.span
            });
        }
        UtteranceContent::UnderlineEnd(marker) => {
            let end_span = if marker.span.is_dummy() {
                fallback_span
            } else {
                marker.span
            };
            apply_underline_end(begin_spans, end_span, errors);
        }
        UtteranceContent::Word(word) => {
            walk_underline_balance_in_word(word, begin_spans, fallback_span, errors);
        }
        UtteranceContent::AnnotatedWord(word) => {
            walk_underline_balance_in_word(&word.inner, begin_spans, fallback_span, errors);
        }
        UtteranceContent::ReplacedWord(replaced) => {
            walk_underline_balance_in_word(&replaced.word, begin_spans, fallback_span, errors);
            for replacement in &replaced.replacement.words {
                walk_underline_balance_in_word(replacement, begin_spans, fallback_span, errors);
            }
        }
        // Containers: ONE arm. Every one of these recursed unconditionally
        // into its enclosed content, which is what `ContentStructure::enclosed`
        // is for; its own docstring names the walkers under `validation/` and
        // `alignment/` as the callers that had not adopted it. The annotations
        // sit on the wrapper and are not part of the enclosed content, which is
        // what each of the separate arms already did.
        UtteranceContent::Group(_)
        | UtteranceContent::AnnotatedGroup(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Quotation(_)
        | UtteranceContent::AnnotatedQuotation(_)
        | UtteranceContent::Retrace(_)
        | UtteranceContent::AnnotatedRetrace(_) => {
            if let Some(content) = item.structure().enclosed() {
                walk_underline_balance_in_bracketed(content, begin_spans, fallback_span, errors);
            }
        }
        UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Event(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::Action(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => {}
    }
}

/// Walk underline-balance state through bracketed content recursively.
fn walk_underline_balance_in_bracketed(
    content: &BracketedContent,
    begin_spans: &mut Vec<Span>,
    fallback_span: Span,
    errors: &impl ErrorSink,
) {
    for item in &content.content {
        match item {
            BracketedItem::UnderlineBegin(marker) => {
                begin_spans.push(if marker.span.is_dummy() {
                    fallback_span
                } else {
                    marker.span
                });
            }
            BracketedItem::UnderlineEnd(marker) => {
                let end_span = if marker.span.is_dummy() {
                    fallback_span
                } else {
                    marker.span
                };
                apply_underline_end(begin_spans, end_span, errors);
            }
            BracketedItem::Word(word) => {
                walk_underline_balance_in_word(word, begin_spans, fallback_span, errors);
            }
            BracketedItem::AnnotatedWord(word) => {
                walk_underline_balance_in_word(&word.inner, begin_spans, fallback_span, errors);
            }
            BracketedItem::ReplacedWord(replaced) => {
                walk_underline_balance_in_word(&replaced.word, begin_spans, fallback_span, errors);
                for replacement in &replaced.replacement.words {
                    walk_underline_balance_in_word(replacement, begin_spans, fallback_span, errors);
                }
            }
            // Containers: ONE arm. Every one of these recursed unconditionally
            // into its enclosed content, which is what `ContentStructure::enclosed`
            // is for; its own docstring names the walkers under `validation/` and
            // `alignment/` as the callers that had not adopted it. The annotations
            // sit on the wrapper and are not part of the enclosed content, which is
            // what each of the separate arms already did.
            BracketedItem::Group(_)
            | BracketedItem::AnnotatedGroup(_)
            | BracketedItem::PhoGroup(_)
            | BracketedItem::SinGroup(_)
            | BracketedItem::AnnotatedQuotation(_)
            | BracketedItem::Quotation(_)
            | BracketedItem::Retrace(_)
            | BracketedItem::AnnotatedRetrace(_) => {
                if let Some(content) = item.structure().enclosed() {
                    walk_underline_balance_in_bracketed(
                        content,
                        begin_spans,
                        fallback_span,
                        errors,
                    );
                }
            }
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::Separator(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}

/// Walk underline-balance state through inline word content markers.
fn walk_underline_balance_in_word(
    word: &Word,
    begin_spans: &mut Vec<Span>,
    fallback_span: Span,
    errors: &impl ErrorSink,
) {
    let word_span = if word.span.is_dummy() {
        fallback_span
    } else {
        word.span
    };
    for wc in &word.content {
        match wc {
            WordContent::UnderlineBegin(wb) => {
                let span = if wb.span.is_dummy() {
                    word_span
                } else {
                    wb.span
                };
                begin_spans.push(span);
            }
            WordContent::UnderlineEnd(we) => {
                let span = if we.span.is_dummy() {
                    word_span
                } else {
                    we.span
                };
                apply_underline_end(begin_spans, span, errors);
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
fn apply_underline_end(begin_spans: &mut Vec<Span>, end_span: Span, errors: &impl ErrorSink) {
    if begin_spans.pop().is_none() {
        errors.report(
            ParseError::at_span(
                ErrorCode::UnmatchedUnderlineEnd,
                Severity::Error,
                end_span,
                "Unmatched underline end (␂␂) without corresponding begin (␂␁)",
            )
            .with_suggestion(
                "Ensure each underline end (␂␂) has a matching underline begin (␂␁) before it",
            ),
        );
    }
}
