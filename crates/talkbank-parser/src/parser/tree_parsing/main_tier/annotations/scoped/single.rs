//! Parses one scoped annotation token (or wrapper) at a time.
//!
//! This module is the main dispatch point from coarsened tree-sitter nodes
//! into typed `ContentAnnotation` values.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Scope>

use crate::error::{DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity};
use crate::generated_traversal::{
    AsRawNode, BaseAnnotationChoiceBoundView, BaseAnnotationsChild0Child1Choice,
    BaseAnnotationsChild1Child1Choice, CodeSwitchAnnotationNode, Ranged, ReadableSlot, SourceBound,
    SourceBoundKind,
};
use crate::model::ContentAnnotation;
use crate::parser::node_span::span_of;
use crate::parser::typed_cst::{ReportedCstFailure, report_admission};
use crate::tokens;
use talkbank_model::LanguageCode;
use talkbank_model::ParseOutcome;
use talkbank_model::Span;
use talkbank_model::model::CodeSwitchSpan;
use talkbank_model::model::RetraceKind;

/// Result of parsing a single annotation token: either a content annotation
/// or a retrace marker.
pub(crate) enum ParsedAnnotation {
    /// A non-retrace content annotation (`[*]`, `[= text]`, `[!]`, etc.),
    /// with the annotation node's own bytes: the wrapper that receives it
    /// ends where this annotation ends, not where the whole chain ends.
    Content {
        annotation: ContentAnnotation,
        span: Span,
    },
    /// A retrace marker (`[/]`, `[//]`, `[///]`, `[/-]`), with the marker
    /// token's own bytes, which the retrace it builds records as
    /// `marker_span` so a diagnostic about the marker can point at it. The
    /// chain's span cannot: it runs through the last annotation of the chain.
    Retrace { kind: RetraceKind, span: Span },
}

/// The byte span of a CST node.
fn node_span<'tree, T: SourceBoundKind<'tree>>(node: SourceBound<'tree, '_, T>) -> Span {
    let node = node.raw_node();
    span_of(node)
}

/// The one annotation set, whichever list position it came through.
///
/// `base_annotations` is a first pair and a repeat of pairs, and the
/// generated traversal names the choice at each position separately: two
/// enums over one set of sixteen annotation kinds, which is also the
/// `base_annotation` supertype's own choice. These are the exhaustive
/// lowerings into that one, so the decoder is written once and a kind the
/// grammar gains breaks them at compile time.
impl<'tree, 'source> From<&BaseAnnotationsChild0Child1Choice<'tree, Ranged<'source>>>
    for BaseAnnotationChoiceBoundView<'tree, 'source>
{
    fn from(choice: &BaseAnnotationsChild0Child1Choice<'tree, Ranged<'source>>) -> Self {
        match choice {
            BaseAnnotationsChild0Child1Choice::AltAnnotation(node) => {
                BaseAnnotationChoiceBoundView::AltAnnotation(*node)
            }
            BaseAnnotationsChild0Child1Choice::CodeSwitchAnnotation(node) => {
                BaseAnnotationChoiceBoundView::CodeSwitchAnnotation(*node)
            }
            BaseAnnotationsChild0Child1Choice::ErrorMarkerAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ErrorMarkerAnnotation(*node)
            }
            BaseAnnotationsChild0Child1Choice::ExcludeMarker(node) => {
                BaseAnnotationChoiceBoundView::ExcludeMarker(*node)
            }
            BaseAnnotationsChild0Child1Choice::ExplanationAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ExplanationAnnotation(*node)
            }
            BaseAnnotationsChild0Child1Choice::IndexedOverlapFollows(node) => {
                BaseAnnotationChoiceBoundView::IndexedOverlapFollows(*node)
            }
            BaseAnnotationsChild0Child1Choice::IndexedOverlapPrecedes(node) => {
                BaseAnnotationChoiceBoundView::IndexedOverlapPrecedes(*node)
            }
            BaseAnnotationsChild0Child1Choice::ParaAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ParaAnnotation(*node)
            }
            BaseAnnotationsChild0Child1Choice::PercentAnnotation(node) => {
                BaseAnnotationChoiceBoundView::PercentAnnotation(*node)
            }
            BaseAnnotationsChild0Child1Choice::RetraceComplete(node) => {
                BaseAnnotationChoiceBoundView::RetraceComplete(*node)
            }
            BaseAnnotationsChild0Child1Choice::RetraceMultiple(node) => {
                BaseAnnotationChoiceBoundView::RetraceMultiple(*node)
            }
            BaseAnnotationsChild0Child1Choice::RetracePartial(node) => {
                BaseAnnotationChoiceBoundView::RetracePartial(*node)
            }
            BaseAnnotationsChild0Child1Choice::RetraceReformulation(node) => {
                BaseAnnotationChoiceBoundView::RetraceReformulation(*node)
            }
            BaseAnnotationsChild0Child1Choice::ScopedContrastiveStressing(node) => {
                BaseAnnotationChoiceBoundView::ScopedContrastiveStressing(*node)
            }
            BaseAnnotationsChild0Child1Choice::ScopedStressing(node) => {
                BaseAnnotationChoiceBoundView::ScopedStressing(*node)
            }
            BaseAnnotationsChild0Child1Choice::ScopedUncertain(node) => {
                BaseAnnotationChoiceBoundView::ScopedUncertain(*node)
            }
        }
    }
}

impl<'tree, 'source> From<&BaseAnnotationsChild1Child1Choice<'tree, Ranged<'source>>>
    for BaseAnnotationChoiceBoundView<'tree, 'source>
{
    fn from(choice: &BaseAnnotationsChild1Child1Choice<'tree, Ranged<'source>>) -> Self {
        match choice {
            BaseAnnotationsChild1Child1Choice::AltAnnotation(node) => {
                BaseAnnotationChoiceBoundView::AltAnnotation(*node)
            }
            BaseAnnotationsChild1Child1Choice::CodeSwitchAnnotation(node) => {
                BaseAnnotationChoiceBoundView::CodeSwitchAnnotation(*node)
            }
            BaseAnnotationsChild1Child1Choice::ErrorMarkerAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ErrorMarkerAnnotation(*node)
            }
            BaseAnnotationsChild1Child1Choice::ExcludeMarker(node) => {
                BaseAnnotationChoiceBoundView::ExcludeMarker(*node)
            }
            BaseAnnotationsChild1Child1Choice::ExplanationAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ExplanationAnnotation(*node)
            }
            BaseAnnotationsChild1Child1Choice::IndexedOverlapFollows(node) => {
                BaseAnnotationChoiceBoundView::IndexedOverlapFollows(*node)
            }
            BaseAnnotationsChild1Child1Choice::IndexedOverlapPrecedes(node) => {
                BaseAnnotationChoiceBoundView::IndexedOverlapPrecedes(*node)
            }
            BaseAnnotationsChild1Child1Choice::ParaAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ParaAnnotation(*node)
            }
            BaseAnnotationsChild1Child1Choice::PercentAnnotation(node) => {
                BaseAnnotationChoiceBoundView::PercentAnnotation(*node)
            }
            BaseAnnotationsChild1Child1Choice::RetraceComplete(node) => {
                BaseAnnotationChoiceBoundView::RetraceComplete(*node)
            }
            BaseAnnotationsChild1Child1Choice::RetraceMultiple(node) => {
                BaseAnnotationChoiceBoundView::RetraceMultiple(*node)
            }
            BaseAnnotationsChild1Child1Choice::RetracePartial(node) => {
                BaseAnnotationChoiceBoundView::RetracePartial(*node)
            }
            BaseAnnotationsChild1Child1Choice::RetraceReformulation(node) => {
                BaseAnnotationChoiceBoundView::RetraceReformulation(*node)
            }
            BaseAnnotationsChild1Child1Choice::ScopedContrastiveStressing(node) => {
                BaseAnnotationChoiceBoundView::ScopedContrastiveStressing(*node)
            }
            BaseAnnotationsChild1Child1Choice::ScopedStressing(node) => {
                BaseAnnotationChoiceBoundView::ScopedStressing(*node)
            }
            BaseAnnotationsChild1Child1Choice::ScopedUncertain(node) => {
                BaseAnnotationChoiceBoundView::ScopedUncertain(*node)
            }
        }
    }
}

/// Decode an already source-bound generated alternative. Kind/source admission
/// is not lexical validity: payload tokens retain their policy refusals, and
/// recovery slots are never passed here as present marker values.
pub(super) fn parse_single_annotation(
    choice: BaseAnnotationChoiceBoundView<'_, '_>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<ParsedAnnotation>, ReportedCstFailure> {
    Ok(match choice {
        BaseAnnotationChoiceBoundView::ExplanationAnnotation(node) => {
            parse_content_token(node, tokens::parse_explanation_token, errors)
        }
        BaseAnnotationChoiceBoundView::ParaAnnotation(node) => {
            parse_content_token(node, tokens::parse_para_token, errors)
        }
        BaseAnnotationChoiceBoundView::AltAnnotation(node) => {
            parse_content_token(node, tokens::parse_alt_token, errors)
        }
        BaseAnnotationChoiceBoundView::PercentAnnotation(node) => {
            parse_content_token(node, tokens::parse_percent_token, errors)
        }
        BaseAnnotationChoiceBoundView::ErrorMarkerAnnotation(node) => {
            parse_content_token(node, tokens::parse_error_marker_token, errors)
        }
        BaseAnnotationChoiceBoundView::IndexedOverlapPrecedes(node) => {
            parse_content_token(node, tokens::parse_overlap_precedes_token, errors)
        }
        BaseAnnotationChoiceBoundView::IndexedOverlapFollows(node) => {
            parse_content_token(node, tokens::parse_overlap_follows_token, errors)
        }
        BaseAnnotationChoiceBoundView::ScopedStressing(node) => {
            ParseOutcome::parsed(ParsedAnnotation::Content {
                annotation: ContentAnnotation::Stressing,
                span: node_span(node),
            })
        }
        BaseAnnotationChoiceBoundView::ScopedContrastiveStressing(node) => {
            ParseOutcome::parsed(ParsedAnnotation::Content {
                annotation: ContentAnnotation::ContrastiveStressing,
                span: node_span(node),
            })
        }
        BaseAnnotationChoiceBoundView::ScopedUncertain(node) => {
            ParseOutcome::parsed(ParsedAnnotation::Content {
                annotation: ContentAnnotation::Uncertain,
                span: node_span(node),
            })
        }
        BaseAnnotationChoiceBoundView::RetraceComplete(node) => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace {
                kind: RetraceKind::Full,
                span: node_span(node),
            })
        }
        BaseAnnotationChoiceBoundView::RetracePartial(node) => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace {
                kind: RetraceKind::Partial,
                span: node_span(node),
            })
        }
        BaseAnnotationChoiceBoundView::RetraceMultiple(node) => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace {
                kind: RetraceKind::Multiple,
                span: node_span(node),
            })
        }
        BaseAnnotationChoiceBoundView::RetraceReformulation(node) => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace {
                kind: RetraceKind::Reformulation,
                span: node_span(node),
            })
        }
        BaseAnnotationChoiceBoundView::ExcludeMarker(node) => {
            ParseOutcome::parsed(ParsedAnnotation::Content {
                annotation: ContentAnnotation::Exclude,
                span: node_span(node),
            })
        }
        BaseAnnotationChoiceBoundView::CodeSwitchAnnotation(node) => {
            return parse_code_switch(node, errors);
        }
    })
}

/// Token policy consumes only the readable spelling owned by this node.
fn parse_content_token<'tree, T: SourceBoundKind<'tree>>(
    node: SourceBound<'tree, '_, T>,
    parse: fn(&str) -> Option<ContentAnnotation>,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParsedAnnotation> {
    delegate_content_or_error(parse(node.text()), node, errors)
}

/// Only actual optional absence names the shortcut. A recovered explicit
/// language position cannot silently become `[@s]`.
fn parse_code_switch<'tree>(
    node: SourceBound<'tree, '_, CodeSwitchAnnotationNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<ParsedAnnotation>, ReportedCstFailure> {
    let associated = report_admission(
        node.extract().admit_ranges().map_err(Into::into),
        node.raw_node(),
        node.source(),
        errors,
    )?;
    let span = match &associated.children().child_1.slot {
        None => Some(CodeSwitchSpan::Shortcut),
        Some(slot) => match slot {
            ReadableSlot::Present(group) => match &group.code.slot {
                ReadableSlot::Present(code) => match LanguageCode::new(code.text()) {
                    Ok(code) => Some(CodeSwitchSpan::Explicit(code)),
                    Err(_) => None,
                },
                ReadableSlot::Missing(_) | ReadableSlot::Error(_) | ReadableSlot::Absent(_) => None,
                ReadableSlot::Unexpected(never) => match *never {},
            },
            ReadableSlot::Error(_) => None,
            ReadableSlot::Missing(never)
            | ReadableSlot::Unexpected(never)
            | ReadableSlot::Absent(never) => match *never {},
        },
    };
    Ok(delegate_content_or_error(
        span.map(ContentAnnotation::CodeSwitch),
        node,
        errors,
    ))
}

/// Policy failure retains this token's own source and readable spelling.
fn delegate_content_or_error<'tree, T: SourceBoundKind<'tree>>(
    result: Option<ContentAnnotation>,
    token: SourceBound<'tree, '_, T>,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParsedAnnotation> {
    match result {
        Some(annotation) => ParseOutcome::parsed(ParsedAnnotation::Content {
            annotation,
            span: span_of(token.raw_node()),
        }),
        None => {
            let node = token.raw_node();
            errors.report(ParseError::new(
                ErrorCode::ContentAnnotationParseError,
                Severity::Error,
                DiagnosticLocation::new(span_of(node)),
                ErrorContext::new(token.source(), span_of(node), ""),
                format!("Failed to parse {} token: '{}'", node.kind(), token.text()),
            ));
            ParseOutcome::rejected()
        }
    }
}
