//! Converts `contents` lists inside bracketed groups.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

use crate::error::ErrorSink;
use crate::model::{BracketedItem, UtteranceContent};
use crate::node_types::{
    CA_CONTINUATION_MARKER, CA_NO_BREAK, CA_TECHNICAL_BREAK, COLON, COMMA, CONTENT_ITEM,
    FALLING_TO_LOW, FALLING_TO_MID, LEVEL_PITCH, NON_COLON_SEPARATOR, OVERLAP_POINT,
    RISING_TO_HIGH, RISING_TO_MID, SEMICOLON, SEPARATOR, TAG_MARKER, UNMARKED_ENDING,
    UPTAKE_SYMBOL, VOCATIVE_MARKER, WHITESPACES,
};
use tree_sitter::Node;

use super::nested::parse_nested_content;
use crate::parser::tree_parsing::helpers::unexpected_node_error;

/// Converts a `contents` CST node into `BracketedItem`s.
///
/// The `contents` rule enumerates the tokens that can live inside bracketed tiers (e.g., `%mor`, `%gra`),
/// including explicit overlap/continuation markers. This parser walks the CST children, decoys whitespace,
/// and delegates to `parse_nested_content` so each nested utterance item ends up in the `BracketedItem`
/// vector reported back to the caller. That way the bracketed tiers keep the same ordering and annotated types
/// described in the manual’s Scoped Symbols chapter.
///
/// **Grammar Rule:**
/// ```text
/// contents: $ => repeat1($.content_item)
/// ```
pub(crate) fn parse_group_contents(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<BracketedItem> {
    let child_count = node.child_count();
    // Pre-allocate: each child is typically one content item
    let mut group_items = Vec::with_capacity(child_count);

    for idx in 0..child_count {
        if let Some(child) = node.child(idx as u32) {
            match child.kind() {
                // One arm: `CONTENT_ITEM` and the CA/separator kinds had
                // byte-identical bodies once the converter became total, and
                // the sibling group parsers already merged them.
                CONTENT_ITEM
                | OVERLAP_POINT
                | SEPARATOR
                | NON_COLON_SEPARATOR
                | COLON
                | COMMA
                | SEMICOLON
                | TAG_MARKER
                | VOCATIVE_MARKER
                | CA_CONTINUATION_MARKER
                | UNMARKED_ENDING
                | UPTAKE_SYMBOL
                | CA_NO_BREAK
                | CA_TECHNICAL_BREAK
                | RISING_TO_HIGH
                | RISING_TO_MID
                | LEVEL_PITCH
                | FALLING_TO_MID
                | FALLING_TO_LOW => {
                    for content in parse_nested_content(child, source, errors) {
                        group_items.push(convert_to_group_content(content));
                    }
                }
                // Expected: whitespace between content items (no model representation needed)
                WHITESPACES => {}
                _ => {
                    errors.report(unexpected_node_error(
                        child,
                        source,
                        "contents (expected content_item)",
                    ));
                }
            }
        }
    }

    group_items
}

/// Convert `UtteranceContent` into `BracketedItem` when the content is valid inside a bracketed tier.
///
/// Convert one piece of utterance content for use inside brackets.
///
/// # Total, and that is the whole guarantee
///
/// Every variant has an answer and none of them is a failure, so there is no
/// arm a caller can discard. It returned `Result<BracketedItem, Group>` until
/// 2026-08-26, and five of its six call sites wrote `if let Ok(item)`,
/// silently dropping a group and every word inside it.
///
/// Replacing the `Result` with a two-variant enum was the first attempt, and
/// it did not actually close the hole: `if let (item)`
/// compiles and drops exactly as `if let Ok(item)` did, so the rename made the
/// drop unidiomatic rather than unrepresentable. A total function has no
/// second arm to ignore.
///
/// The one caller that genuinely discriminates is `marker_chain::retrace`,
/// where a BARE group hands its brackets to the retrace rather than nesting:
/// `<a b> [/]` is one retrace wearing the group's brackets, recorded by
/// `Retrace::is_group`, and nesting it would serialize `<<a b>> [/]`. That
/// caller tests for `UtteranceContent::Group` itself, before converting, which
/// is where the distinction actually lives.
pub(crate) fn convert_to_group_content(content: UtteranceContent) -> BracketedItem {
    match content {
        UtteranceContent::Word(word) => BracketedItem::Word(word),
        UtteranceContent::AnnotatedWord(ann) => BracketedItem::AnnotatedWord(ann),
        UtteranceContent::ReplacedWord(rw) => BracketedItem::ReplacedWord(rw),
        UtteranceContent::Event(event) => BracketedItem::Event(event),
        UtteranceContent::AnnotatedEvent(ann) => BracketedItem::AnnotatedEvent(ann),
        UtteranceContent::Pause(pause) => BracketedItem::Pause(pause),
        UtteranceContent::Action(action) => BracketedItem::Action(action),
        UtteranceContent::AnnotatedAction(ann) => BracketedItem::AnnotatedAction(ann),
        UtteranceContent::Group(group) => BracketedItem::Group(group),
        UtteranceContent::OverlapPoint(marker) => BracketedItem::OverlapPoint(marker),
        UtteranceContent::Separator(sep) => BracketedItem::Separator(sep.clone()),
        UtteranceContent::InternalBullet(bullet) => BracketedItem::InternalBullet(bullet),
        UtteranceContent::Freecode(freecode) => BracketedItem::Freecode(freecode),
        UtteranceContent::LongFeatureBegin(marker) => BracketedItem::LongFeatureBegin(marker),
        UtteranceContent::LongFeatureEnd(marker) => BracketedItem::LongFeatureEnd(marker),
        UtteranceContent::NonvocalBegin(marker) => BracketedItem::NonvocalBegin(marker),
        UtteranceContent::NonvocalEnd(marker) => BracketedItem::NonvocalEnd(marker),
        UtteranceContent::NonvocalSimple(marker) => BracketedItem::NonvocalSimple(marker),
        UtteranceContent::UnderlineBegin(marker) => BracketedItem::UnderlineBegin(marker),
        UtteranceContent::UnderlineEnd(marker) => BracketedItem::UnderlineEnd(marker),
        UtteranceContent::OtherSpokenEvent(event) => BracketedItem::OtherSpokenEvent(event.clone()),
        // Groups CAN contain annotated groups (e.g., retraces inside pho groups)
        UtteranceContent::AnnotatedGroup(ann) => BracketedItem::AnnotatedGroup(ann),
        UtteranceContent::Retrace(retrace) => BracketedItem::Retrace(retrace),
        UtteranceContent::AnnotatedRetrace(annotated) => BracketedItem::AnnotatedRetrace(annotated),
        UtteranceContent::PhoGroup(pho) => BracketedItem::PhoGroup(pho),
        UtteranceContent::SinGroup(sin) => BracketedItem::SinGroup(sin),
        UtteranceContent::Quotation(quot) => BracketedItem::Quotation(quot),
        UtteranceContent::AnnotatedQuotation(ann) => BracketedItem::AnnotatedQuotation(ann),
    }
}
