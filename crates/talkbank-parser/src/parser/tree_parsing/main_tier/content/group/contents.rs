//! The `contents` inside a bracketed construct, as `BracketedItem`s.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#OverlapMarkers>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, ContentsChildren, ContentsNode, NoChild, NonMissingKindSlot, SourceChildren,
    SourceField, SourceSlotView,
};
use crate::model::{BracketedItem, UtteranceContent};
use tree_sitter::Node;

use crate::parser::tree_parsing::main_tier::structure::contents::parse_contents;

/// The extracted children of a construct's `contents` slot, or nothing.
///
/// Present is extracted. Compiled grammar admission proves that the composite
/// `contents` node cannot itself be Missing; lexical recovery within it is
/// still owned by the shared contents walker. An ERROR or displaced node at the
/// position is the construct losing its shape there, which `on_bad` reports
/// in the construct's own words, and Absent means the construct has no
/// contents at all; both yield nothing, and the caller rejects an empty
/// construct.
pub(crate) fn contents_of<'tree, 'source>(
    slot: SourceField<'_, 'tree, 'source, NonMissingKindSlot<'tree, ContentsNode<'tree>>>,
    errors: &impl ErrorSink,
    on_bad: impl FnOnce(Node<'tree>),
) -> Option<SourceChildren<'tree, 'source, ContentsChildren<'tree>>> {
    match slot.view() {
        SourceSlotView::Present(contents) => {
            let bound = crate::parser::typed_cst::read_source_field(contents, errors)?;
            crate::parser::typed_cst::report_reconstruction(
                bound.extract(),
                bound.raw_node(),
                bound.source(),
                errors,
            )
            .ok()
        }
        SourceSlotView::Error(bad) => {
            on_bad(bad.raw_node());
            None
        }
        SourceSlotView::Absent(NoChild) => None,
        SourceSlotView::Missing(never) => match never {},
    }
}

/// Parse an extracted `contents` node inside brackets into `BracketedItem`s.
///
/// The one `contents` grammar rule serves the tier body and every bracketed
/// construct, so this is the one walker
/// ([`parse_contents`]) in its bracketed region, followed by the total
/// conversion below. Until 2026-09-08 the angle group, the quotation, the
/// pho group and the sin group each walked `contents` with their own copy
/// of a `node.kind()` match and a shared second dispatcher
/// (`group/nested.rs`) beneath it.
pub(crate) fn parse_group_contents<'tree>(
    contents: &SourceChildren<'tree, '_, ContentsChildren<'tree>>,
    errors: &impl ErrorSink,
) -> Vec<BracketedItem> {
    parse_contents(contents, errors)
        .into_iter()
        .map(convert_to_group_content)
        .collect()
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
