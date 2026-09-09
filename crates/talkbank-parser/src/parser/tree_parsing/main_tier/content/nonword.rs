//! Nonword content parsing, over the generated typed traversal.
//!
//! Handles the unified nonword category: events (`&=action`) and the
//! standalone zero/action (`0`). Other spoken events (`&*SPEAKER`) are a
//! separate rule, parsed in `base/other_spoken.rs`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Action_Code>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use crate::generated_traversal::{
    AsRawNode, EventNode, FromNodeKind, NonwordChoice, NonwordNode,
    NonwordWithOptionalAnnotationsNode, extract_event, extract_nonword,
    extract_nonword_with_optional_annotations,
};
use crate::model::{Action, Event, UtteranceContent};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::annotations::parse_scoped_annotations;
use super::marker_chain::fold_marker_chain;
use super::report_tree_shape;
use crate::parser::tree_parsing::parser_helpers::{
    SlotState, expect_delimiter, expect_present, surface_displaced,
};

/// Converts `nonword_with_optional_annotations` into `UtteranceContent`.
///
/// Grammar: `seq(field('nonword', nonword), field('annotations',
/// optional(base_annotations)))`, with `nonword: choice(event, zero)` and
/// `event: seq(event_marker, field('description', event_segment))`.
/// `extract_nonword_with_optional_annotations` places the two in typed
/// slots; the markers fold onto the nonword through `fold_marker_chain`,
/// giving a bare `Event`/`Action` or the annotated spelling when a marker
/// actually arrives. Until 2026-09-09 this walked the children by index and
/// `node.kind()` string, with `child(0)`/`child(1)` reaching into the event.
///
/// The dispatcher hands over the raw node; a node that is not a
/// `nonword_with_optional_annotations` is refused with a diagnostic.
pub(crate) fn parse_nonword_content(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let Some(typed) = NonwordWithOptionalAnnotationsNode::from_node(node) else {
        report_tree_shape(
            node,
            format!(
                "Expected a nonword_with_optional_annotations node, found '{}'",
                node.kind()
            ),
            source,
            errors,
        );
        return ParseOutcome::rejected();
    };
    let full_span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let children = extract_nonword_with_optional_annotations(typed);

    // Position `nonword`, required.
    let core = match expect_present(
        children.nonword.slot(),
        "nonword_with_optional_annotations",
        source,
        errors,
    ) {
        SlotState::Present(nonword) => nonword_core(*nonword, source, errors),
        SlotState::Absent | SlotState::Recovered => None,
    };

    // Position `annotations`, optional: at most one `base_annotations`, so
    // this assigns rather than accumulates.
    let markers = match children.annotations.slot() {
        Some(slot) => {
            match expect_present(slot, "nonword_with_optional_annotations", source, errors) {
                SlotState::Present(annotations) => {
                    parse_scoped_annotations(annotations.raw_node(), source, errors)
                }
                SlotState::Absent | SlotState::Recovered => Vec::new(),
            }
        }
        None => Vec::new(),
    };
    surface_displaced(
        &children.unexpected,
        "nonword_with_optional_annotations",
        source,
        errors,
    );

    ParseOutcome::from(core.map(|core| fold_marker_chain(core, markers, full_span)))
}

/// The bare content a `nonword` node names: an event, or the zero action.
/// Bare, exactly like the event: this used to wrap every action in an
/// `Annotated` carrying an empty list, because `UtteranceContent` had no bare
/// `Action` variant; the wrapper is now unconstructible without an
/// annotation, and the annotated spelling is reached only through
/// `fold_marker_chain`, when a marker actually arrives.
fn nonword_core(
    nonword: NonwordNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<UtteranceContent> {
    let raw = nonword.raw_node();
    let span = Span::new(raw.start_byte() as u32, raw.end_byte() as u32);
    let children = extract_nonword(nonword);
    surface_displaced(&children.unexpected, "nonword", source, errors);
    match expect_present(children.content.slot(), "nonword", source, errors) {
        SlotState::Present(NonwordChoice::Event(event)) => event_of(*event, span, source, errors),
        SlotState::Present(NonwordChoice::Zero(_)) => {
            Some(UtteranceContent::Action(Action::with_span(span)))
        }
        SlotState::Absent | SlotState::Recovered => None,
    }
}

/// The event an `event` node names: `&=` then its description segment. A
/// description whose bytes do not decode is reported and builds no event,
/// where the old walk silently built nothing.
fn event_of(
    event: EventNode<'_>,
    span: Span,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<UtteranceContent> {
    let children = extract_event(event);
    surface_displaced(&children.unexpected, "event", source, errors);
    expect_delimiter(children.child_0.slot(), |bad| {
        report_tree_shape(
            bad,
            format!("Expected '&=' opening an event, found '{}'", bad.kind()),
            source,
            errors,
        );
    });
    let SlotState::Present(segment) =
        expect_present(children.description.slot(), "event", source, errors)
    else {
        return None;
    };
    let raw = segment.raw_node();
    match raw.utf8_text(source.as_bytes()) {
        Ok(description) => Some(UtteranceContent::Event(
            Event::new(description).with_span(span),
        )),
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(raw.start_byte(), raw.end_byte()),
                ErrorContext::new(source, raw.start_byte()..raw.end_byte(), ""),
                format!("Failed to extract event description text: {err}"),
            ));
            None
        }
    }
}
