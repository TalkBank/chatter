//! Checked construction of located long-feature and nonvocal markers.
//!
//! Only the source transition can produce `CheckedMarker`. Callers hand over
//! a generated marker node, never a label beside an independent shape flag.

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, LongFeatureBeginNode, LongFeatureEndNode, LongFeatureLabelNode, NodeSlot,
    NonvocalBeginNode, NonvocalEndNode, NonvocalSimpleNode, RecoveryNode, SourceBound,
    SourceSlotView,
};
use crate::model::{
    LongFeatureBegin, LongFeatureEnd, LongFeatureLabel, NonvocalBegin, NonvocalEnd, NonvocalLabel,
    NonvocalSimple, UtteranceContent,
};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::main_tier::content::report_tree_shape;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, expect_delimiter, surface_displaced,
};
use crate::parser::typed_cst::RequiredLexicalField;
use talkbank_model::{ParseOutcome, Span};

/// The closed set of generated shapes that carry these labels.
pub(super) enum MarkerSource<'tree, 'source> {
    LongFeatureBegin(SourceBound<'tree, 'source, LongFeatureBeginNode<'tree>>),
    LongFeatureEnd(SourceBound<'tree, 'source, LongFeatureEndNode<'tree>>),
    NonvocalBegin(SourceBound<'tree, 'source, NonvocalBeginNode<'tree>>),
    NonvocalEnd(SourceBound<'tree, 'source, NonvocalEndNode<'tree>>),
    NonvocalSimple(SourceBound<'tree, 'source, NonvocalSimpleNode<'tree>>),
}

/// An enclosing or local producer has already reported the rejection.
struct ReportedMarkerDefect;

/// Each payload is available only after all of its local checks complete.
enum CheckedMarker {
    LongFeatureBegin { span: Span, label: LongFeatureLabel },
    LongFeatureEnd { span: Span, label: LongFeatureLabel },
    NonvocalBegin { span: Span, label: NonvocalLabel },
    NonvocalEnd { span: Span, label: NonvocalLabel },
    NonvocalSimple { span: Span, label: NonvocalLabel },
}

// The generated carriers have distinct position types. Keep each field at its
// generated type while sharing the consuming admission and reporting order.
macro_rules! admit_marker {
    ($node:ident, $errors:ident, $within:literal,
     [$($field:ident => $what:literal),+], $variant:ident, $label:ident) => {{
        let source = $node.source();
        let associated = crate::parser::typed_cst::report_admission(
            crate::parser::typed_cst::extract_inner($node),
            $node.raw_node(),
            source,
            $errors,
        )
        .map_err(|_| ReportedMarkerDefect)?;
        let children = associated.children();
        surface_displaced(&children.unexpected, $within, source, $errors);
        let displaced = if children.unexpected.is_empty() {
            Ok(())
        } else {
            Err(ReportedMarkerDefect)
        };

        // Evaluate every delimiter before combining results. Reporting must
        // continue even when an earlier delimiter or displaced node failed.
        let delimiters = [$(delimiter(
            children.$field.slot(), $what, $within, source, $errors,
        )),+];
        let label = marker_label(associated.field_child_2().slot(), $within, $errors);
        displaced
            .and(delimiters.into_iter().collect::<Result<(), _>>())
            .and(label)
            .and_then(|label| {
                let label = $label::new(label).map_err(|error| {
                    report_tree_shape($node.raw_node(), error.to_string(), source, $errors);
                    ReportedMarkerDefect
                })?;
                Ok(CheckedMarker::$variant { span: span_of($node.raw_node()), label })
            })
    }};
}

impl MarkerSource<'_, '_> {
    pub(super) fn parse(self, errors: &impl ErrorSink) -> ParseOutcome<UtteranceContent> {
        match self.admit(errors) {
            Ok(marker) => ParseOutcome::Parsed(marker.into_content()),
            Err(ReportedMarkerDefect) => ParseOutcome::Rejected,
        }
    }

    fn admit(self, errors: &impl ErrorSink) -> Result<CheckedMarker, ReportedMarkerDefect> {
        match self {
            Self::LongFeatureBegin(node) => admit_marker!(
                node, errors, "long_feature_begin",
                [child_0 => "'&'", child_1 => "'{l='"], LongFeatureBegin, LongFeatureLabel
            ),
            Self::LongFeatureEnd(node) => admit_marker!(
                node, errors, "long_feature_end",
                [child_0 => "'&'", child_1 => "'}l='"], LongFeatureEnd, LongFeatureLabel
            ),
            Self::NonvocalBegin(node) => admit_marker!(
                node, errors, "nonvocal_begin",
                [child_0 => "'&'", child_1 => "'{n='"], NonvocalBegin, NonvocalLabel
            ),
            Self::NonvocalEnd(node) => admit_marker!(
                node, errors, "nonvocal_end",
                [child_0 => "'&'", child_1 => "'}n='"], NonvocalEnd, NonvocalLabel
            ),
            Self::NonvocalSimple(node) => admit_marker!(
                node, errors, "nonvocal_simple",
                [child_0 => "'&'", child_1 => "'{n='", child_3 => "'}'"],
                NonvocalSimple, NonvocalLabel
            ),
        }
    }
}

impl CheckedMarker {
    fn into_content(self) -> UtteranceContent {
        match self {
            Self::LongFeatureBegin { span, label } => {
                UtteranceContent::LongFeatureBegin(LongFeatureBegin::new(span, label))
            }
            Self::LongFeatureEnd { span, label } => {
                UtteranceContent::LongFeatureEnd(LongFeatureEnd::new(span, label))
            }
            Self::NonvocalBegin { span, label } => {
                UtteranceContent::NonvocalBegin(NonvocalBegin::new(span, label))
            }
            Self::NonvocalEnd { span, label } => {
                UtteranceContent::NonvocalEnd(NonvocalEnd::new(span, label))
            }
            Self::NonvocalSimple { span, label } => {
                UtteranceContent::NonvocalSimple(NonvocalSimple::new(span, label))
            }
        }
    }
}

/// Preserve the existing delimiter policy: lexical Missing is reported by the
/// whole-tree owner, while structural defects reject the local marker.
fn delimiter<'tree, T, M, E: RecoveryNode<'tree>, U: RecoveryNode<'tree>, A>(
    slot: &NodeSlot<T, M, E, U, A>,
    what: &str,
    within: &str,
    source: &str,
    errors: &impl ErrorSink,
) -> Result<(), ReportedMarkerDefect> {
    let mut result = Ok(());
    expect_delimiter(slot, |bad| {
        result = Err(ReportedMarkerDefect);
        report_tree_shape(
            bad,
            format!("Expected {what} in {within}, found '{}'", bad.kind()),
            source,
            errors,
        );
    });
    result
}

fn marker_label<'value, 'tree: 'value, 'source>(
    slot: RequiredLexicalField<'value, 'tree, 'source, LongFeatureLabelNode<'tree>>,
    within: &str,
    errors: &impl ErrorSink,
) -> Result<String, ReportedMarkerDefect> {
    match slot.view() {
        SourceSlotView::Present(node) => crate::parser::typed_cst::read_source_field(node, errors)
            .map(|node| node.text().to_owned())
            .ok_or(ReportedMarkerDefect),
        SourceSlotView::Missing(node) => {
            check_not_missing(node.raw_node(), slot.source(), errors, within);
            Err(ReportedMarkerDefect)
        }
    }
}
