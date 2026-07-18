//! Source-spacing validation: E751 (pause glued to the preceding word)
//! and E757 (bracketed code glued to the following content).
//!
//! Sibling of the comma-spacing rules in `comma.rs` (E258/E259/E749):
//! CHAT items are space-delimited in the source, and these rules detect
//! glued items by SPAN ADJACENCY over the in-order content walk, which
//! works because the parser preserves byte spans on words and pauses.
//! Dummy (0,0) spans are skipped: the re2c oracle fills dummy spans and
//! mirrors each rule as a token-stream scan in its own front end.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>

use crate::alignment::helpers::{ContentItem, walk_content};
use crate::model::{MainTier, Utterance, UtteranceContent};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

/// The source start byte of a top-level content item, for variants that
/// carry a real span. Variants without a span field (or with a dummy
/// span, as the re2c oracle produces) return `None`.
fn item_source_start(item: &UtteranceContent) -> Option<u32> {
    let span = match item {
        UtteranceContent::Word(word) => word.span,
        UtteranceContent::AnnotatedWord(annotated) => annotated.inner.span,
        UtteranceContent::ReplacedWord(replaced) => replaced.word.span,
        UtteranceContent::Pause(pause) => pause.span,
        UtteranceContent::Retrace(retrace) => retrace.span,
        UtteranceContent::Freecode(freecode) => freecode.span,
        UtteranceContent::Quotation(quotation) => quotation.span,
        UtteranceContent::AnnotatedAction(annotated) => annotated.span,
        // No span field on these variants; a tier starting with one of
        // them simply opts out of span-arithmetic checks.
        UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Group(_)
        | UtteranceContent::AnnotatedGroup(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => return None,
    };
    non_dummy_start(span)
}

/// The start byte of `span`, or `None` if it is the dummy span. Centralizes
/// the "a dummy span carries no real position, so opt out" rule shared by the
/// source-spacing checks (the re2c oracle fills dummy spans).
fn non_dummy_start(span: crate::Span) -> Option<u32> {
    (!span.is_dummy()).then_some(span.start)
}

/// The source start byte of the main tier's FIRST content item, when
/// that item carries a real span (used by E758's leading-whitespace
/// arithmetic). Deliberately looks at the first item only: skipping
/// past a span-less leading item (e.g. the `+"` quotation linker) to a
/// later word would measure the linker's own width as a false
/// whitespace gap (caught by corpus/reference/edge-cases/
/// special-terminators.cha). A tier starting with a span-less item
/// simply opts out of the check.
fn first_content_start(main: &MainTier) -> Option<u32> {
    main.content.content.0.first().and_then(item_source_start)
}

/// The source start byte of the main tier's FIRST real element, in document
/// order: the earliest non-dummy start among the leading discourse linker,
/// the `[- code]` language precode, and the first content item.
///
/// Linkers and the precode are real spanned source tokens (a leading linker or
/// precode sits between the tab delimiter and the first content item), so a
/// leading space before ANY of them is measurable: taking the minimum start
/// makes the earliest source token the anchor regardless of which kind it is.
/// Returns `None` only when no leading element carries a real span (for
/// example the re2c oracle's dummy spans, or a content-first tier whose first
/// item is a span-less variant), in which case the leading-space check opts
/// out because it has nothing to measure against.
pub(crate) fn first_element_start(main: &MainTier) -> Option<u32> {
    let linker_start = main
        .content
        .linkers
        .0
        .first()
        .map(|linker| linker.span)
        .and_then(non_dummy_start);
    let precode_start = main.content.language_code_span.and_then(non_dummy_start);
    let content_start = first_content_start(main);
    [linker_start, precode_start, content_start]
        .into_iter()
        .flatten()
        .min()
}

/// The source end byte of a content item, when the item is a word whose
/// trailing edge can glue a following pause. Non-word items return
/// `None`: CHECK 57 fires on the word-then-`(` shape specifically.
fn word_end(item: &ContentItem<'_>) -> Option<u32> {
    match item {
        ContentItem::Word(word) => Some(word.span.end),
        ContentItem::ReplacedWord(replaced) => Some(replaced.word.span.end),
        ContentItem::Separator(_)
        | ContentItem::Event(_)
        | ContentItem::Pause(_)
        | ContentItem::Action(_)
        | ContentItem::OverlapPoint(_)
        | ContentItem::OtherSpokenEvent(_)
        | ContentItem::Freecode(_)
        | ContentItem::InternalBullet(_)
        | ContentItem::LongFeatureBegin(_)
        | ContentItem::LongFeatureEnd(_)
        | ContentItem::UnderlineBegin(_)
        | ContentItem::UnderlineEnd(_)
        | ContentItem::NonvocalBegin(_)
        | ContentItem::NonvocalEnd(_)
        | ContentItem::NonvocalSimple(_) => None,
    }
}

/// The source start byte of a top-level item whose leading edge can be
/// glued onto a preceding code's `]` (the word family). Other variants
/// return `None`: the grounded CHECK-19 shape is code-then-word; further
/// glue shapes get their own grounding before extension (see the spec).
fn word_family_start(item: &UtteranceContent) -> Option<u32> {
    match item {
        UtteranceContent::Word(word) => Some(word.span.start),
        UtteranceContent::AnnotatedWord(annotated) => Some(annotated.inner.span.start),
        UtteranceContent::ReplacedWord(replaced) => Some(replaced.word.span.start),
        // Deliberately not glue targets in the grounded shape: groups,
        // events, pauses (E751's territory), markers, and separators.
        UtteranceContent::Event(_)
        | UtteranceContent::AnnotatedEvent(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::Group(_)
        | UtteranceContent::AnnotatedGroup(_)
        | UtteranceContent::Retrace(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Quotation(_)
        | UtteranceContent::AnnotatedAction(_)
        | UtteranceContent::Freecode(_)
        | UtteranceContent::Separator(_)
        | UtteranceContent::OverlapPoint(_)
        | UtteranceContent::InternalBullet(_)
        | UtteranceContent::LongFeatureBegin(_)
        | UtteranceContent::LongFeatureEnd(_)
        | UtteranceContent::UnderlineBegin(_)
        | UtteranceContent::UnderlineEnd(_)
        | UtteranceContent::NonvocalBegin(_)
        | UtteranceContent::NonvocalEnd(_)
        | UtteranceContent::NonvocalSimple(_)
        | UtteranceContent::OtherSpokenEvent(_) => None,
    }
}

/// E757: a bracketed code's closing `]` must not run directly into the
/// next word (`hello [/]x`; CLAN CHECK 19). Fires when a retrace's span
/// ends exactly where the next top-level word-family item starts. The
/// retrace span covers its content plus the `[...]` marker, so its end
/// byte is the byte after `]`.
pub(crate) fn check_code_glued_to_following_content(
    utterance: &Utterance,
    errors: &impl ErrorSink,
) {
    for pair in utterance.main.content.content.0.windows(2) {
        let UtteranceContent::Retrace(retrace) = &pair[0] else {
            continue;
        };
        if retrace.span == crate::Span::DUMMY {
            continue;
        }
        let Some(next_start) = word_family_start(&pair[1]) else {
            continue;
        };
        if next_start == retrace.span.end {
            errors.report(
                ParseError::new(
                    ErrorCode::CodeGluedToFollowingContent,
                    Severity::Error,
                    SourceLocation::new(retrace.span),
                    ErrorContext::new("]", retrace.span, "]"),
                    "Bracketed code must be separated from the following word by a space",
                )
                .with_suggestion("Add a space after the closing bracket"),
            );
        }
    }
}

/// E751: a pause must not open directly at the end of a word
/// (`hello(.)`; CLAN CHECK 57). Fires when a pause's span starts at the
/// byte where the previous in-order word's span ends.
pub(crate) fn check_pause_glued_to_word(utterance: &Utterance, errors: &impl ErrorSink) {
    let mut prev_word_end: Option<u32> = None;

    walk_content(&utterance.main.content.content.0, None, &mut |item| {
        if let ContentItem::Pause(pause) = item
            && pause.span != crate::Span::DUMMY
            && let Some(end) = prev_word_end
            && pause.span.start == end
        {
            errors.report(
                ParseError::new(
                    ErrorCode::PauseGluedToWord,
                    Severity::Error,
                    SourceLocation::new(pause.span),
                    ErrorContext::new("(", pause.span, "("),
                    "Pause must be separated from the preceding word by a space",
                )
                .with_suggestion("Add a space between the word and the pause"),
            );
        }
        prev_word_end = word_end(&item);
    });
}
