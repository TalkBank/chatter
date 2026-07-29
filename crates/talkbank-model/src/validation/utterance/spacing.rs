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
//! E758 (leading/trailing space between a tab delimiter and tier
//! content) used to live here as a main-tier-only span reconstruction
//! (`first_element_start`); it is now read uniformly from every source
//! line's [`crate::model::TierSeparator`] (main tier, dependent tiers,
//! and headers alike), so that reconstruction was deleted.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Pauses>

use crate::alignment::helpers::{ContentItem, walk_content};
use crate::model::{Utterance, UtteranceContent};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

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

/// The source end byte of an item that ENDS with a bracketed code, so
/// that glued following material is a separate item rather than part of
/// this one. Two shapes qualify:
///
/// - a retrace, whose span covers its content plus the `[...]` marker;
/// - any annotated item CARRYING scoped annotations (`hello [!]`,
///   `bobo [= toy]`), whose wrapper span covers the annotations.
///
/// An annotated item with no scoped annotations is deliberately excluded:
/// its wrapper span ends at the payload, and word-glued-to-word is a
/// different question (`dogdog` is ONE word, by definition).
fn glued_code_end(item: &UtteranceContent) -> Option<u32> {
    /// The wrapper span, but only when scoped annotations are present.
    fn annotated_end<T>(annotated: &crate::model::Annotated<T>) -> Option<u32> {
        (!annotated.scoped_annotations.is_empty()).then_some(annotated.span.end)
    }
    match item {
        UtteranceContent::Retrace(retrace) => Some(retrace.span.end),
        UtteranceContent::AnnotatedWord(annotated) => annotated_end(annotated),
        UtteranceContent::AnnotatedGroup(annotated) => annotated_end(annotated),
        UtteranceContent::AnnotatedEvent(annotated) => annotated_end(annotated),
        UtteranceContent::AnnotatedAction(annotated) => annotated_end(annotated),
        // Not code-terminated: nothing here ends with a `]`.
        UtteranceContent::Word(_)
        | UtteranceContent::ReplacedWord(_)
        | UtteranceContent::Event(_)
        | UtteranceContent::Pause(_)
        | UtteranceContent::Group(_)
        | UtteranceContent::PhoGroup(_)
        | UtteranceContent::SinGroup(_)
        | UtteranceContent::Quotation(_)
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
/// next word (`hello [/]x`, `hello [!]x`; CLAN CHECK 19). Fires when a
/// code-terminated item's span ends exactly where the next top-level
/// word-family item starts.
pub(crate) fn check_code_glued_to_following_content(
    utterance: &Utterance,
    errors: &impl ErrorSink,
) {
    for pair in utterance.main.content.content.0.windows(2) {
        let Some(code_end) = glued_code_end(&pair[0]) else {
            continue;
        };
        if code_end == crate::Span::DUMMY.end {
            continue;
        }
        let Some(next_start) = word_family_start(&pair[1]) else {
            continue;
        };
        if next_start == code_end {
            errors.report(
                ParseError::new(
                    ErrorCode::CodeGluedToFollowingContent,
                    Severity::Error,
                    SourceLocation::new(crate::Span::new(code_end, code_end)),
                    ErrorContext::new("]", crate::Span::new(code_end, code_end), "]"),
                    "Bracketed code must be separated from the following word by a space",
                )
                .with_suggestion("Add a space after the closing bracket"),
            );
        }
    }
}

/// The `&`-prefix categories: each introduces a word of its own, so each
/// needs a space before it. `Omission` (`0word`) and `CAOmission`
/// (`(word)`) are deliberately excluded: they are not `&` forms, they do
/// not split a glued token into two words, and the glued shape is already
/// rejected elsewhere (E220).
fn is_ampersand_prefixed(word: &crate::model::Word) -> bool {
    matches!(
        word.category,
        Some(
            crate::model::WordCategory::Filler
                | crate::model::WordCategory::Nonword
                | crate::model::WordCategory::PhonologicalFragment
        )
    )
}

/// E764: a `&`-prefixed form must not run directly into the preceding
/// word (`dog&-um`). Fires when such a word's span starts at the byte
/// where the previous in-order word's span ends. Mirror of
/// [`check_pause_glued_to_word`]: same walk, same adjacency test, same
/// dummy-span opt-out; only the glued item's identity differs.
pub(crate) fn check_prefixed_form_glued_to_preceding_word(
    utterance: &Utterance,
    errors: &impl ErrorSink,
) {
    let mut prev_word_end: Option<u32> = None;

    walk_content(&utterance.main.content.content.0, None, &mut |item| {
        if let ContentItem::Word(word) = &item
            && word.span != crate::Span::DUMMY
            && is_ampersand_prefixed(word)
            && let Some(end) = prev_word_end
            && word.span.start == end
        {
            errors.report(
                ParseError::new(
                    ErrorCode::PrefixedFormGluedToPrecedingWord,
                    Severity::Error,
                    SourceLocation::new(word.span),
                    ErrorContext::new("&", word.span, "&"),
                    "Prefixed form must be separated from the preceding word by a space",
                )
                .with_suggestion("Add a space between the word and the prefixed form"),
            );
        }
        prev_word_end = word_end(&item);
    });
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
