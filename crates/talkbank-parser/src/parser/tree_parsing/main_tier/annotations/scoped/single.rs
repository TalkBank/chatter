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

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    AsRawNode, BaseAnnotationChoiceBoundView, BaseAnnotationsChild0Child1ChoiceBoundView,
    BaseAnnotationsChild1Child1ChoiceBoundView, CodeSwitchAnnotationNode, SourceBound,
    SourceBoundKind, SourceSlotView,
};
use crate::model::ContentAnnotation;
use crate::parser::typed_cst::read_source_field;
use crate::tokens;
use talkbank_model::LanguageCode;
use talkbank_model::ParseOutcome;
use talkbank_model::Span;
use talkbank_model::model::CodeSwitchSpan;
use talkbank_model::model::RetraceKind;

/// Result of parsing a single annotation token: either a content annotation
/// or a retrace marker.
pub(crate) enum ParsedAnnotation {
    /// A non-retrace content annotation (`[*]`, `[= text]`, `[!]`, etc.)
    Content(ContentAnnotation),
    /// A retrace marker (`[/]`, `[//]`, `[///]`, `[/-]`), with the marker
    /// token's own bytes, which the retrace it builds records as
    /// `marker_span` so a diagnostic about the marker can point at it. The
    /// chain's span cannot: it runs through the last annotation of the chain.
    Retrace { kind: RetraceKind, span: Span },
}

/// The byte span of a CST node.
fn node_span<'tree, T: SourceBoundKind<'tree>>(node: SourceBound<'tree, '_, T>) -> Span {
    let node = node.raw_node();
    Span::from_usize(node.start_byte(), node.end_byte())
}

/// The one annotation set, whichever list position it came through.
///
/// `base_annotations` is a first pair and a repeat of pairs, and the
/// generated traversal names the choice at each position separately: two
/// enums over one set of sixteen annotation kinds, which is also the
/// `base_annotation` supertype's own choice. These are the exhaustive
/// lowerings into that one, so the decoder is written once and a kind the
/// grammar gains breaks them at compile time.
impl<'tree, 'source> From<BaseAnnotationsChild0Child1ChoiceBoundView<'tree, 'source>>
    for BaseAnnotationChoiceBoundView<'tree, 'source>
{
    fn from(choice: BaseAnnotationsChild0Child1ChoiceBoundView<'tree, 'source>) -> Self {
        match choice {
            BaseAnnotationsChild0Child1ChoiceBoundView::AltAnnotation(node) => {
                BaseAnnotationChoiceBoundView::AltAnnotation(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::CodeSwitchAnnotation(node) => {
                BaseAnnotationChoiceBoundView::CodeSwitchAnnotation(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::ErrorMarkerAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ErrorMarkerAnnotation(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::ExcludeMarker(node) => {
                BaseAnnotationChoiceBoundView::ExcludeMarker(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::ExplanationAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ExplanationAnnotation(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::IndexedOverlapFollows(node) => {
                BaseAnnotationChoiceBoundView::IndexedOverlapFollows(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::IndexedOverlapPrecedes(node) => {
                BaseAnnotationChoiceBoundView::IndexedOverlapPrecedes(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::ParaAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ParaAnnotation(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::PercentAnnotation(node) => {
                BaseAnnotationChoiceBoundView::PercentAnnotation(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::RetraceComplete(node) => {
                BaseAnnotationChoiceBoundView::RetraceComplete(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::RetraceMultiple(node) => {
                BaseAnnotationChoiceBoundView::RetraceMultiple(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::RetracePartial(node) => {
                BaseAnnotationChoiceBoundView::RetracePartial(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::RetraceReformulation(node) => {
                BaseAnnotationChoiceBoundView::RetraceReformulation(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::ScopedContrastiveStressing(node) => {
                BaseAnnotationChoiceBoundView::ScopedContrastiveStressing(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::ScopedStressing(node) => {
                BaseAnnotationChoiceBoundView::ScopedStressing(node)
            }
            BaseAnnotationsChild0Child1ChoiceBoundView::ScopedUncertain(node) => {
                BaseAnnotationChoiceBoundView::ScopedUncertain(node)
            }
        }
    }
}

impl<'tree, 'source> From<BaseAnnotationsChild1Child1ChoiceBoundView<'tree, 'source>>
    for BaseAnnotationChoiceBoundView<'tree, 'source>
{
    fn from(choice: BaseAnnotationsChild1Child1ChoiceBoundView<'tree, 'source>) -> Self {
        match choice {
            BaseAnnotationsChild1Child1ChoiceBoundView::AltAnnotation(node) => {
                BaseAnnotationChoiceBoundView::AltAnnotation(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::CodeSwitchAnnotation(node) => {
                BaseAnnotationChoiceBoundView::CodeSwitchAnnotation(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::ErrorMarkerAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ErrorMarkerAnnotation(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::ExcludeMarker(node) => {
                BaseAnnotationChoiceBoundView::ExcludeMarker(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::ExplanationAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ExplanationAnnotation(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::IndexedOverlapFollows(node) => {
                BaseAnnotationChoiceBoundView::IndexedOverlapFollows(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::IndexedOverlapPrecedes(node) => {
                BaseAnnotationChoiceBoundView::IndexedOverlapPrecedes(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::ParaAnnotation(node) => {
                BaseAnnotationChoiceBoundView::ParaAnnotation(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::PercentAnnotation(node) => {
                BaseAnnotationChoiceBoundView::PercentAnnotation(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::RetraceComplete(node) => {
                BaseAnnotationChoiceBoundView::RetraceComplete(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::RetraceMultiple(node) => {
                BaseAnnotationChoiceBoundView::RetraceMultiple(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::RetracePartial(node) => {
                BaseAnnotationChoiceBoundView::RetracePartial(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::RetraceReformulation(node) => {
                BaseAnnotationChoiceBoundView::RetraceReformulation(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::ScopedContrastiveStressing(node) => {
                BaseAnnotationChoiceBoundView::ScopedContrastiveStressing(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::ScopedStressing(node) => {
                BaseAnnotationChoiceBoundView::ScopedStressing(node)
            }
            BaseAnnotationsChild1Child1ChoiceBoundView::ScopedUncertain(node) => {
                BaseAnnotationChoiceBoundView::ScopedUncertain(node)
            }
        }
    }
}

/// Decode an already source-bound generated alternative. Kind/source admission
/// is not lexical validity: payload tokens retain their policy refusals, and
/// recovery slots are never passed here as present marker values.
pub(crate) fn parse_single_annotation(
    choice: BaseAnnotationChoiceBoundView<'_, '_>,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParsedAnnotation> {
    match choice {
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
        BaseAnnotationChoiceBoundView::ScopedStressing(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Stressing))
        }
        BaseAnnotationChoiceBoundView::ScopedContrastiveStressing(_) => ParseOutcome::parsed(
            ParsedAnnotation::Content(ContentAnnotation::ContrastiveStressing),
        ),
        BaseAnnotationChoiceBoundView::ScopedUncertain(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Uncertain))
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
        BaseAnnotationChoiceBoundView::ExcludeMarker(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Exclude))
        }
        BaseAnnotationChoiceBoundView::CodeSwitchAnnotation(node) => {
            parse_code_switch(node, errors)
        }
    }
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
) -> ParseOutcome<ParsedAnnotation> {
    let associated = node.extract();
    let span = match associated.field_child_1().slot().optional() {
        None => Some(CodeSwitchSpan::Shortcut),
        Some(slot) => match slot.view() {
            SourceSlotView::Present(group) => match group.field_code().slot().view() {
                SourceSlotView::Present(code) => {
                    let Some(code) = read_source_field(code, errors) else {
                        return ParseOutcome::Rejected;
                    };
                    match LanguageCode::new(code.text()) {
                        Ok(code) => Some(CodeSwitchSpan::Explicit(code)),
                        Err(_) => None,
                    }
                }
                SourceSlotView::Missing(_)
                | SourceSlotView::Error(_)
                | SourceSlotView::Absent(_) => None,
            },
            SourceSlotView::Error(_) => None,
            SourceSlotView::Absent(never) => match never {},
        },
    };
    delegate_content_or_error(span.map(ContentAnnotation::CodeSwitch), node, errors)
}

/// Policy failure retains this token's own source and readable spelling.
fn delegate_content_or_error<'tree, T: SourceBoundKind<'tree>>(
    result: Option<ContentAnnotation>,
    token: SourceBound<'tree, '_, T>,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParsedAnnotation> {
    match result {
        Some(annotation) => ParseOutcome::parsed(ParsedAnnotation::Content(annotation)),
        None => {
            let node = token.raw_node();
            errors.report(ParseError::new(
                ErrorCode::ContentAnnotationParseError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(token.source(), node.start_byte()..node.end_byte(), ""),
                format!("Failed to parse {} token: '{}'", node.kind(), token.text()),
            ));
            ParseOutcome::rejected()
        }
    }
}
