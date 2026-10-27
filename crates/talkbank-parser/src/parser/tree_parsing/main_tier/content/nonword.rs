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
    AsRawNode, EventNode, NonwordChoiceBoundView, NonwordMissingSourceView, NonwordNode,
    NonwordWithOptionalAnnotationsNode, SlotView, SourceBound, SourceSlotView,
};
use crate::model::{Action, Event, EventType, UtteranceContent};
use talkbank_model::ParseOutcome;

use super::super::annotations::parse_scoped_annotations;
use super::marker_chain::{MarkerSeed, fold_marker_chain};
use super::report_tree_shape;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, expect_delimiter, surface_displaced,
};
use crate::parser::typed_cst::{ExtractCanonical, read_source_field};

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
        crate::parser::typed_cst::extract_inner(typed)
            .and_then(|children| children.admit_ranges().map_err(Into::into)),
        node,
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let core = match associated.children().nonword.slot.view() {
        SlotView::Present(nonword) => nonword_core(*nonword, errors),
    };
    let markers = match associated.children().annotations.slot.as_ref() {
        Some(slot) => match slot.view() {
            SlotView::Present(annotations) => parse_scoped_annotations(*annotations, errors),
            SlotView::Error(bad) => {
                errors.report(unexpected_node_error(
                    bad.raw_node(),
                    bad.source(),
                    "nonword_with_optional_annotations",
                ));
                Ok(Vec::new())
            }
            SlotView::Absent(never) => match never {},
        },
        None => Ok(Vec::new()),
    };
    surface_displaced(
        &associated.children().unexpected,
        "nonword_with_optional_annotations",
        source,
        errors,
    );
    let Ok(markers) = markers else {
        return ParseOutcome::Rejected;
    };
    ParseOutcome::from(core.map(|core| fold_marker_chain(core, markers, full_span)))
}

/// Bare events and actions stay bare until a marker actually arrives.
fn nonword_core<'tree>(
    nonword: SourceBound<'tree, '_, NonwordNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<MarkerSeed> {
    let associated = crate::parser::typed_cst::report_admission(
        nonword.extract_canonical(),
        nonword.raw_node(),
        nonword.source(),
        errors,
    )
    .ok()?;
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
                Some(MarkerSeed::Action(Action::new(span_of(raw))))
            }
        },
        SourceSlotView::Missing(NonwordMissingSourceView::Event(never)) => match never.0 {},
        SourceSlotView::Missing(NonwordMissingSourceView::Zero(placeholder)) => {
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
) -> Option<MarkerSeed> {
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
    let event_type = match EventType::new(description.text()) {
        Ok(event_type) => event_type,
        Err(error) => {
            report_tree_shape(raw, error.to_string(), event.source(), errors);
            return None;
        }
    };
    Some(MarkerSeed::Event(Event::new(span_of(raw), event_type)))
}
