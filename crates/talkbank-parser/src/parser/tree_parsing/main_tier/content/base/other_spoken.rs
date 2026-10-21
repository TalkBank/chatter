//! Parsing for background other-speaker events.
//! Generated child types own kind and position; recovery remains explicit.

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, OtherSpokenEventNode, SourceBound, SourceBoundKind, SourceSlotView,
};
use crate::model::{OtherSpokenEvent, UtteranceContent};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};
use crate::parser::typed_cst::RequiredLexicalField;
use talkbank_model::ParseOutcome;

/// Read a required lexical child from its own source; Missing remains recovery.
fn required<'value, 'tree: 'value, 'source, T: SourceBoundKind<'tree>>(
    slot: RequiredLexicalField<'value, 'tree, 'source, T>,
    errors: &impl ErrorSink,
) -> Option<SourceBound<'tree, 'source, T>> {
    match slot.view() {
        SourceSlotView::Present(node) => crate::parser::typed_cst::read_source_field(node, errors),
        SourceSlotView::Missing(node) => {
            check_not_missing(node.raw_node(), node.source(), errors, "other_spoken_event");
            None
        }
    }
}

/// Lower the generated event shape, never a raw node plus guessed positions.
pub(crate) fn parse_other_spoken_event<'tree>(
    event: SourceBound<'tree, '_, OtherSpokenEventNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = event.source();
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_inner(event),
        event.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::rejected();
    };
    let children = associated.children();
    surface_displaced(&children.unexpected, "other_spoken_event", source, errors);
    if !children.unexpected.is_empty() {
        return ParseOutcome::rejected();
    }
    if required(associated.field_child_0().slot(), errors).is_none()
        || required(associated.field_child_1().slot(), errors).is_none()
        || required(associated.field_child_3().slot(), errors).is_none()
    {
        return ParseOutcome::rejected();
    }
    let Some(speaker) = required(associated.field_child_2().slot(), errors) else {
        return ParseOutcome::rejected();
    };
    let SourceSlotView::Present(text) = associated.field_child_4().slot().view();
    let Some(text) = crate::parser::typed_cst::read_source_field(text, errors) else {
        return ParseOutcome::rejected();
    };
    let node = event.raw_node();
    ParseOutcome::parsed(UtteranceContent::OtherSpokenEvent(OtherSpokenEvent::new(
        span_of(node),
        speaker.text(),
        text.text(),
    )))
}
