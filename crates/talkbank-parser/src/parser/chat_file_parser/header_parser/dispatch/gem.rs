//! GEM headers retain source ownership through optional group projection.
//!
//! Bare markers and recovery retain their header kind. Missing/error slots
//! remain the whole-tree backstop's responsibility; displaced nodes at both
//! group and header boundaries are surfaced independently.

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BgHeaderNode, EgHeaderNode, GHeaderNode, NoChild, SourceBound, SourceSlotView,
};
use crate::model::Header;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::{read_source_field, report_reconstruction};
use talkbank_model::ParseOutcome;

use super::super::helpers::parse_optional_gem_label;

// These three grammar rules share an optional (separator, free_text) group.
// The generated field API, not raw child indexes or text, owns that shape.
macro_rules! gem_header {
    ($name:ident, $node:ident, $variant:ident, $context:literal) => {
        pub(super) fn $name<'tree>(
            typed: SourceBound<'tree, '_, $node<'tree>>,
            errors: &impl ErrorSink,
        ) -> ParseOutcome<Header> {
            let input = typed.source();
            let Ok(children) =
                report_reconstruction(typed.extract(), typed.raw_node(), input, errors)
            else {
                return ParseOutcome::Rejected;
            };
            let label = match children.field_child_1().slot().optional() {
                None => ParseOutcome::Parsed(None),
                Some(slot) => match slot.view() {
                    SourceSlotView::Present(group) => {
                        let result = match group.field_child_1().slot().view() {
                            SourceSlotView::Present(field) => {
                                match read_source_field(field, errors) {
                                    Some(bound) => parse_optional_gem_label(Some(bound), errors),
                                    None => ParseOutcome::Rejected,
                                }
                            }
                            SourceSlotView::Missing(_)
                            | SourceSlotView::Error(_)
                            | SourceSlotView::Absent(NoChild) => ParseOutcome::Parsed(None),
                            SourceSlotView::Unexpected(never) => match never {},
                        };
                        for displaced in group.field_unexpected().iter() {
                            surface_displaced(&[displaced.raw_node()], $context, input, errors);
                        }
                        result
                    }
                    SourceSlotView::Error(_) => ParseOutcome::Parsed(None),
                    SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => {
                        match never {}
                    }
                },
            };
            for displaced in children.field_unexpected().iter() {
                surface_displaced(&[displaced.raw_node()], $context, input, errors);
            }
            match label {
                ParseOutcome::Parsed(label) => ParseOutcome::Parsed(Header::$variant { label }),
                ParseOutcome::Rejected => ParseOutcome::Rejected,
            }
        }
    };
}

gem_header!(bg, BgHeaderNode, BeginGem, "bg_header");
gem_header!(eg, EgHeaderNode, EndGem, "eg_header");
gem_header!(g, GHeaderNode, LazyGem, "g_header");
