//! Parsing for background other-speaker events.
//! Generated child types own kind and position; recovery remains explicit.

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AsRawNode, KindSlot, NamedKind, OtherSpokenEventNode, extract_other_spoken_event,
};
use crate::model::{OtherSpokenEvent, UtteranceContent};
use crate::parser::tree_parsing::parser_helpers::{
    SlotState, expect_present, extract_utf8_text, surface_displaced,
};
use talkbank_model::ParseOutcome;

/// A required event child, with kind supplied by the generated wrapper type.
fn required<'a, 'tree, T: NamedKind + AsRawNode<'tree> + Copy>(
    slot: &'a KindSlot<'tree, T>,
    event: OtherSpokenEventNode<'tree>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<&'a T> {
    match expect_present(slot, "other_spoken_event", source, errors) {
        SlotState::Present(node) => ParseOutcome::parsed(node),
        SlotState::Recovered => ParseOutcome::rejected(),
        SlotState::Absent => {
            let node = event.raw_node();
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.byte_range(), node.kind()),
                format!("Missing required '{}' in other_spoken_event", T::KIND),
            ));
            ParseOutcome::rejected()
        }
    }
}

/// Lower the generated event shape, never a raw node plus guessed positions.
pub(crate) fn parse_other_spoken_event(
    event: OtherSpokenEventNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let children = extract_other_spoken_event(event);
    surface_displaced(&children.unexpected, "other_spoken_event", source, errors);
    if !children.unexpected.is_empty() {
        return ParseOutcome::rejected();
    }
    if required(children.child_0.slot(), event, source, errors).is_none()
        || required(children.child_1.slot(), event, source, errors).is_none()
        || required(children.child_3.slot(), event, source, errors).is_none()
    {
        return ParseOutcome::rejected();
    }
    let ParseOutcome::Parsed(speaker) = required(children.child_2.slot(), event, source, errors)
    else {
        return ParseOutcome::rejected();
    };
    let ParseOutcome::Parsed(text) = required(children.child_4.slot(), event, source, errors)
    else {
        return ParseOutcome::rejected();
    };
    let node = event.raw_node();
    let ParseOutcome::Parsed(speaker) =
        extract_utf8_text(speaker.raw_node(), source, errors, "speaker")
    else {
        return ParseOutcome::rejected();
    };
    let ParseOutcome::Parsed(text) =
        extract_utf8_text(text.raw_node(), source, errors, "other_spoken_text")
    else {
        return ParseOutcome::rejected();
    };
    ParseOutcome::parsed(UtteranceContent::OtherSpokenEvent(
        OtherSpokenEvent::with_span(
            speaker,
            text,
            Span::new(node.start_byte() as u32, node.end_byte() as u32),
        ),
    ))
}
