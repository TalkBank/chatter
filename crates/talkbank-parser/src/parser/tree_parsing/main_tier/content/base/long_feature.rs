//! Parsing for long-feature markers in base content, over the generated
//! typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::ErrorSink;
use crate::generated_traversal::{AsRawNode, LongFeatureNode};
use crate::model::UtteranceContent;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::ExtractCanonical;
use talkbank_model::ParseOutcome;

use super::labeled_marker::MarkerSource;

/// Parse one `long_feature` node: a begin marker (`&{l=label`) or an end
/// marker (`&}l=label`), over the generated traversal.
///
/// Grammar: `long_feature: choice(long_feature_begin, long_feature_end)`,
/// each `seq(ampersand, <marker>, long_feature_label)`. This parser
/// validates only the local structure and captures the label and span;
/// cross-token begin/end pairing is validation's. Until 2026-09-09 this
/// asserted child counts and kinds by position and matched `kind()`
/// strings.
pub(crate) fn parse_long_feature<'tree>(
    typed: crate::generated_traversal::SourceBound<'tree, '_, LongFeatureNode<'tree>>,
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
    surface_displaced(&children.unexpected, "long_feature", source, errors);
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
                "long_feature",
            ));
            return ParseOutcome::rejected();
        }
        crate::generated_traversal::SourceSlotView::Absent(_) => return ParseOutcome::rejected(),
    };
    let marker = match choice.view() {
        crate::generated_traversal::LongFeatureChoiceBoundView::LongFeatureBegin(node) => {
            MarkerSource::LongFeatureBegin(node)
        }
        crate::generated_traversal::LongFeatureChoiceBoundView::LongFeatureEnd(node) => {
            MarkerSource::LongFeatureEnd(node)
        }
    };
    marker.parse(errors)
}
