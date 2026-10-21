//! Parsing for nonvocal markers in base content, over the generated typed
//! traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::ErrorSink;
use crate::generated_traversal::{AsRawNode, NonvocalNode};
use crate::model::{NonvocalBegin, NonvocalEnd, NonvocalLabel, NonvocalSimple, UtteranceContent};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::ExtractCanonical;
use talkbank_model::ParseOutcome;

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
pub(crate) fn parse_nonvocal<'tree>(
    typed: crate::generated_traversal::SourceBound<'tree, '_, NonvocalNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let source = typed.source();
    // Admitted: every head of this choice is a rule, which tree-sitter never
    // inserts MISSING, so the compiled grammar's proof leaves the slot no
    // `Missing` state to handle.
    let Ok(associated) = crate::parser::typed_cst::report_admission(
        typed.extract_canonical(),
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::rejected();
    };
    let children = associated.children();
    surface_displaced(&children.unexpected, "nonvocal", source, errors);
    let choice = match associated.field_content().slot().view() {
        crate::generated_traversal::SourceSlotView::Present(choice) => {
            let Some(choice) = crate::parser::typed_cst::read_source_field(choice, errors) else {
                return ParseOutcome::rejected();
            };
            choice
        }
        crate::generated_traversal::SourceSlotView::Error(bad) => {
            errors.report(crate::parser::tree_parsing::helpers::unexpected_node_error(
                bad.raw_node(),
                source,
                "nonvocal",
            ));
            return ParseOutcome::rejected();
        }
        crate::generated_traversal::SourceSlotView::Absent(_) => return ParseOutcome::rejected(),
    };
    let built = match choice.view() {
        crate::generated_traversal::NonvocalChoiceBoundView::NonvocalBegin(node) => {
            let Ok(associated) = crate::parser::typed_cst::report_admission(
                crate::parser::typed_cst::extract_inner(node),
                node.raw_node(),
                source,
                errors,
            ) else {
                return ParseOutcome::rejected();
            };
            let inner = associated.children();
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
            marker_label(associated.field_child_2().slot(), "nonvocal_begin", errors)
                .filter(|_| intact)
                .map(|label| {
                    UtteranceContent::NonvocalBegin(NonvocalBegin::new(
                        span,
                        NonvocalLabel::new(label),
                    ))
                })
        }
        crate::generated_traversal::NonvocalChoiceBoundView::NonvocalEnd(node) => {
            let Ok(associated) = crate::parser::typed_cst::report_admission(
                crate::parser::typed_cst::extract_inner(node),
                node.raw_node(),
                source,
                errors,
            ) else {
                return ParseOutcome::rejected();
            };
            let inner = associated.children();
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
            marker_label(associated.field_child_2().slot(), "nonvocal_end", errors)
                .filter(|_| intact)
                .map(|label| {
                    UtteranceContent::NonvocalEnd(NonvocalEnd::new(span, NonvocalLabel::new(label)))
                })
        }
        crate::generated_traversal::NonvocalChoiceBoundView::NonvocalSimple(node) => {
            let Ok(associated) = crate::parser::typed_cst::report_admission(
                crate::parser::typed_cst::extract_inner(node),
                node.raw_node(),
                source,
                errors,
            ) else {
                return ParseOutcome::rejected();
            };
            let inner = associated.children();
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
            marker_label(associated.field_child_2().slot(), "nonvocal_simple", errors)
                .filter(|_| intact)
                .map(|label| {
                    UtteranceContent::NonvocalSimple(NonvocalSimple::new(
                        span,
                        NonvocalLabel::new(label),
                    ))
                })
        }
    };
    ParseOutcome::from(built)
}
