//! Nonword content parsing over source-bound generated traversal.
//!
//! Handles events (`&=action`) and the standalone zero/action (`0`). Other
//! spoken events (`&*SPEAKER`) belong to `base/other_spoken.rs`.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Action_Code>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, EventNode, NonwordChoiceBoundView, NonwordNode, NonwordWithOptionalAnnotationsNode,
    SourceBound, SourceSlotView,
};
use crate::model::{Action, Event, UtteranceContent};
use talkbank_model::ParseOutcome;

use super::super::annotations::parse_scoped_annotations;
use super::marker_chain::fold_marker_chain;
use super::report_tree_shape;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, expect_delimiter, surface_displaced,
};
use crate::parser::typed_cst::read_source_field;

/// Fold the ordered annotations onto the nonword. Compiled kind admission
/// excludes a Missing composite slot; Error/absence and lexical recovery
/// inside the nonword remain independently handled. Every text reader and
/// annotation caller retains the producing node's canonical source.
pub(crate) fn parse_nonword_content<'tree>(
    typed: SourceBound<'tree, '_, NonwordWithOptionalAnnotationsNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = typed.source();
    let node = typed.raw_node();
    let full_span = span_of(node);
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_inner(typed),
        node,
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let core = match associated.field_nonword().slot().view() {
        SourceSlotView::Present(nonword) => {
            read_source_field(nonword, errors).and_then(|nonword| nonword_core(nonword, errors))
        }
    };
    let markers = match associated.field_annotations().slot().optional() {
        Some(slot) => match slot.view() {
            SourceSlotView::Present(annotations) => match read_source_field(annotations, errors) {
                Some(annotations) => parse_scoped_annotations(annotations, errors),
                None => Vec::new(),
            },
            SourceSlotView::Error(bad) => {
                errors.report(unexpected_node_error(
                    bad.raw_node(),
                    bad.source(),
                    "nonword_with_optional_annotations",
                ));
                Vec::new()
            }
            SourceSlotView::Absent(never) => match never {},
        },
        None => Vec::new(),
    };
    surface_displaced(
        &associated.children().unexpected,
        "nonword_with_optional_annotations",
        source,
        errors,
    );
    ParseOutcome::from(core.map(|core| fold_marker_chain(core, markers, full_span)))
}

/// Bare events and actions stay bare until a marker actually arrives.
fn nonword_core<'tree>(
    nonword: SourceBound<'tree, '_, NonwordNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<UtteranceContent> {
    let associated = nonword.extract();
    surface_displaced(
        &associated.children().unexpected,
        "nonword",
        nonword.source(),
        errors,
    );
    match associated.field_content().slot().view() {
        SourceSlotView::Present(choice) => match read_source_field(choice, errors)?.view() {
            NonwordChoiceBoundView::Event(event) => event_of(event, errors),
            NonwordChoiceBoundView::Zero(zero) => {
                let raw = zero.raw_node();
                Some(UtteranceContent::Action(Action::new(span_of(raw))))
            }
        },
        SourceSlotView::Missing(placeholder) => {
            check_not_missing(
                placeholder.raw_node(),
                associated.source(),
                errors,
                "nonword",
            );
            None
        }
        SourceSlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                bad.source(),
                "nonword",
            ));
            None
        }
        SourceSlotView::Absent(_) => None,
    }
}

/// The description comes only from the event's generated associated field.
/// A source-read refusal is reported and builds no event.
fn event_of<'tree>(
    event: SourceBound<'tree, '_, EventNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<UtteranceContent> {
    let associated = crate::parser::typed_cst::report_admission(
        crate::parser::typed_cst::extract_inner(event),
        event.raw_node(),
        event.source(),
        errors,
    )
    .ok()?;
    let children = associated.children();
    surface_displaced(&children.unexpected, "event", event.source(), errors);
    expect_delimiter(children.child_0.slot(), |bad| {
        report_tree_shape(
            bad,
            format!("Expected '&=' opening an event, found '{}'", bad.kind()),
            event.source(),
            errors,
        );
    });
    let description = match associated.field_description().slot().view() {
        SourceSlotView::Present(segment) => read_source_field(segment, errors)?,
        SourceSlotView::Missing(bad) => {
            check_not_missing(bad.raw_node(), bad.source(), errors, "event");
            return None;
        }
    };
    let raw = event.raw_node();
    Some(UtteranceContent::Event(Event::new(
        span_of(raw),
        description.text(),
    )))
}
