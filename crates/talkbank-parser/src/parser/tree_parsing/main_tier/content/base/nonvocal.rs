//! Parsing for nonvocal markers in base content, over the generated typed
//! traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, NonvocalChoice, NonvocalNode, extract_nonvocal,
    extract_nonvocal_begin, extract_nonvocal_end, extract_nonvocal_simple,
};
use crate::model::{NonvocalBegin, NonvocalEnd, NonvocalLabel, NonvocalSimple, UtteranceContent};
use crate::parser::tree_parsing::parser_helpers::{SlotState, expect_present, surface_displaced};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::report_tree_shape;
use super::{delimiter, marker_label, span_of};

/// Parse one `nonvocal` node: a begin marker (`&{n=label`), an end marker
/// (`&}n=label`) or a self-contained one (`&{n=label}`), over the generated
/// traversal.
///
/// Grammar: `nonvocal: choice(nonvocal_begin, nonvocal_end,
/// nonvocal_simple)`, each `seq(ampersand, <marker>, long_feature_label)`,
/// the simple form closed by `right_brace`. The label is read from its
/// typed position; the delimiters are structure, reported when they lose
/// their shape, and a marker whose shape is not intact is not built. Until 2026-09-09 this asserted child counts and kinds
/// by position and matched `kind()` strings.
pub(crate) fn parse_nonvocal(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let Some(typed) = NonvocalNode::from_node(node) else {
        report_tree_shape(
            node,
            format!("Expected a nonvocal node, found '{}'", node.kind()),
            source,
            errors,
        );
        return ParseOutcome::rejected();
    };
    let children = extract_nonvocal(typed);
    surface_displaced(&children.unexpected, "nonvocal", source, errors);
    let SlotState::Present(choice) =
        expect_present(children.content.slot(), "nonvocal", source, errors)
    else {
        return ParseOutcome::rejected();
    };
    let built = match choice {
        NonvocalChoice::NonvocalBegin(node) => {
            let inner = extract_nonvocal_begin(*node);
            surface_displaced(&inner.unexpected, "nonvocal_begin", source, errors);
            // Every delimiter is checked (and reported) before the verdict;
            // a marker whose shape is not intact is not built.
            let intact = inner.unexpected.is_empty()
                & delimiter(
                    inner.child_0.slot(),
                    "'&'",
                    "nonvocal_begin",
                    source,
                    errors,
                )
                & delimiter(
                    inner.child_1.slot(),
                    "'{n='",
                    "nonvocal_begin",
                    source,
                    errors,
                );
            let span = span_of(node.raw_node());
            marker_label(
                inner.child_2.slot(),
                "nonvocal_begin",
                "nonvocal_begin_label",
                source,
                errors,
            )
            .filter(|_| intact)
            .map(|label| {
                UtteranceContent::NonvocalBegin(
                    NonvocalBegin::new(NonvocalLabel::new(label)).with_span(span),
                )
            })
        }
        NonvocalChoice::NonvocalEnd(node) => {
            let inner = extract_nonvocal_end(*node);
            surface_displaced(&inner.unexpected, "nonvocal_end", source, errors);
            // Every delimiter is checked (and reported) before the verdict;
            // a marker whose shape is not intact is not built.
            let intact = inner.unexpected.is_empty()
                & delimiter(inner.child_0.slot(), "'&'", "nonvocal_end", source, errors)
                & delimiter(
                    inner.child_1.slot(),
                    "'}n='",
                    "nonvocal_end",
                    source,
                    errors,
                );
            let span = span_of(node.raw_node());
            marker_label(
                inner.child_2.slot(),
                "nonvocal_end",
                "nonvocal_end_label",
                source,
                errors,
            )
            .filter(|_| intact)
            .map(|label| {
                UtteranceContent::NonvocalEnd(
                    NonvocalEnd::new(NonvocalLabel::new(label)).with_span(span),
                )
            })
        }
        NonvocalChoice::NonvocalSimple(node) => {
            let inner = extract_nonvocal_simple(*node);
            surface_displaced(&inner.unexpected, "nonvocal_simple", source, errors);
            // Every delimiter is checked (and reported) before the verdict;
            // a marker whose shape is not intact is not built.
            let intact = inner.unexpected.is_empty()
                & delimiter(
                    inner.child_0.slot(),
                    "'&'",
                    "nonvocal_simple",
                    source,
                    errors,
                )
                & delimiter(
                    inner.child_1.slot(),
                    "'{n='",
                    "nonvocal_simple",
                    source,
                    errors,
                )
                & delimiter(
                    inner.child_3.slot(),
                    "'}'",
                    "nonvocal_simple",
                    source,
                    errors,
                );
            let span = span_of(node.raw_node());
            marker_label(
                inner.child_2.slot(),
                "nonvocal_simple",
                "nonvocal_simple_label",
                source,
                errors,
            )
            .filter(|_| intact)
            .map(|label| {
                UtteranceContent::NonvocalSimple(
                    NonvocalSimple::new(NonvocalLabel::new(label)).with_span(span),
                )
            })
        }
    };
    ParseOutcome::from(built)
}
