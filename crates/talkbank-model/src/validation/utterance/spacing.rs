//! Source-spacing validation: E751 (pause glued to the preceding word).
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
use crate::model::Utterance;
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
