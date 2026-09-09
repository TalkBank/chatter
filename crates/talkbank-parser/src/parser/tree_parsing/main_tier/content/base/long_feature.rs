//! Parsing for long-feature markers in base content, over the generated
//! typed traversal.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, FromNodeKind, LongFeatureChoice, LongFeatureNode, extract_long_feature,
    extract_long_feature_begin, extract_long_feature_end,
};
use crate::model::{LongFeatureBegin, LongFeatureEnd, LongFeatureLabel, UtteranceContent};
use crate::parser::tree_parsing::parser_helpers::{SlotState, expect_present, surface_displaced};
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::report_tree_shape;
use super::{delimiter, marker_label, span_of};

/// Parse one `long_feature` node: a begin marker (`&{l=label`) or an end
/// marker (`&}l=label`), over the generated traversal.
///
/// Grammar: `long_feature: choice(long_feature_begin, long_feature_end)`,
/// each `seq(ampersand, <marker>, long_feature_label)`. This parser
/// validates only the local structure and captures the label and span;
/// cross-token begin/end pairing is validation's. Until 2026-09-09 this
/// asserted child counts and kinds by position and matched `kind()`
/// strings.
pub(crate) fn parse_long_feature(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<UtteranceContent> {
    let Some(typed) = LongFeatureNode::from_node(node) else {
        report_tree_shape(
            node,
            format!("Expected a long_feature node, found '{}'", node.kind()),
            source,
            errors,
        );
        return ParseOutcome::rejected();
    };
    let children = extract_long_feature(typed);
    surface_displaced(&children.unexpected, "long_feature", source, errors);
    let SlotState::Present(choice) =
        expect_present(children.content.slot(), "long_feature", source, errors)
    else {
        return ParseOutcome::rejected();
    };
    let built = match choice {
        LongFeatureChoice::LongFeatureBegin(node) => {
            let inner = extract_long_feature_begin(*node);
            surface_displaced(&inner.unexpected, "long_feature_begin", source, errors);
            // Every delimiter is checked (and reported) before the verdict;
            // a marker whose shape is not intact is not built.
            let intact = inner.unexpected.is_empty()
                & delimiter(
                    inner.child_0.slot(),
                    "'&'",
                    "long_feature_begin",
                    source,
                    errors,
                )
                & delimiter(
                    inner.child_1.slot(),
                    "'{l='",
                    "long_feature_begin",
                    source,
                    errors,
                );
            let span = span_of(node.raw_node());
            marker_label(
                inner.child_2.slot(),
                "long_feature_begin",
                "long_feature_begin_label",
                source,
                errors,
            )
            .filter(|_| intact)
            .map(|label| {
                UtteranceContent::LongFeatureBegin(
                    LongFeatureBegin::new(LongFeatureLabel::new(label)).with_span(span),
                )
            })
        }
        LongFeatureChoice::LongFeatureEnd(node) => {
            let inner = extract_long_feature_end(*node);
            surface_displaced(&inner.unexpected, "long_feature_end", source, errors);
            // Every delimiter is checked (and reported) before the verdict;
            // a marker whose shape is not intact is not built.
            let intact = inner.unexpected.is_empty()
                & delimiter(
                    inner.child_0.slot(),
                    "'&'",
                    "long_feature_end",
                    source,
                    errors,
                )
                & delimiter(
                    inner.child_1.slot(),
                    "'}l='",
                    "long_feature_end",
                    source,
                    errors,
                );
            let span = span_of(node.raw_node());
            marker_label(
                inner.child_2.slot(),
                "long_feature_end",
                "long_feature_end_label",
                source,
                errors,
            )
            .filter(|_| intact)
            .map(|label| {
                UtteranceContent::LongFeatureEnd(
                    LongFeatureEnd::new(LongFeatureLabel::new(label)).with_span(span),
                )
            })
        }
    };
    ParseOutcome::from(built)
}
