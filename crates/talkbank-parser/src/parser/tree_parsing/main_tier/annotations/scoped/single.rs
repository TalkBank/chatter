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
    AsRawNode, BaseAnnotationChoice, BaseAnnotationsChild0Child1Choice,
    BaseAnnotationsChild1Child1Choice,
};
use crate::model::ContentAnnotation;
use crate::parser::tree_parsing::parser_helpers::extract_utf8_text;
use crate::tokens;
use talkbank_model::LanguageCode;
use talkbank_model::ParseOutcome;
use talkbank_model::Span;
use talkbank_model::model::CodeSwitchSpan;
use talkbank_model::model::RetraceKind;
use tree_sitter::Node;

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
fn node_span(node: Node) -> Span {
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
impl<'tree> From<BaseAnnotationsChild0Child1Choice<'tree>> for BaseAnnotationChoice<'tree> {
    fn from(choice: BaseAnnotationsChild0Child1Choice<'tree>) -> Self {
        match choice {
            BaseAnnotationsChild0Child1Choice::AltAnnotation(node) => {
                BaseAnnotationChoice::AltAnnotation(node)
            }
            BaseAnnotationsChild0Child1Choice::CodeSwitchAnnotation(node) => {
                BaseAnnotationChoice::CodeSwitchAnnotation(node)
            }
            BaseAnnotationsChild0Child1Choice::ErrorMarkerAnnotation(node) => {
                BaseAnnotationChoice::ErrorMarkerAnnotation(node)
            }
            BaseAnnotationsChild0Child1Choice::ExcludeMarker(node) => {
                BaseAnnotationChoice::ExcludeMarker(node)
            }
            BaseAnnotationsChild0Child1Choice::ExplanationAnnotation(node) => {
                BaseAnnotationChoice::ExplanationAnnotation(node)
            }
            BaseAnnotationsChild0Child1Choice::IndexedOverlapFollows(node) => {
                BaseAnnotationChoice::IndexedOverlapFollows(node)
            }
            BaseAnnotationsChild0Child1Choice::IndexedOverlapPrecedes(node) => {
                BaseAnnotationChoice::IndexedOverlapPrecedes(node)
            }
            BaseAnnotationsChild0Child1Choice::ParaAnnotation(node) => {
                BaseAnnotationChoice::ParaAnnotation(node)
            }
            BaseAnnotationsChild0Child1Choice::PercentAnnotation(node) => {
                BaseAnnotationChoice::PercentAnnotation(node)
            }
            BaseAnnotationsChild0Child1Choice::RetraceComplete(node) => {
                BaseAnnotationChoice::RetraceComplete(node)
            }
            BaseAnnotationsChild0Child1Choice::RetraceMultiple(node) => {
                BaseAnnotationChoice::RetraceMultiple(node)
            }
            BaseAnnotationsChild0Child1Choice::RetracePartial(node) => {
                BaseAnnotationChoice::RetracePartial(node)
            }
            BaseAnnotationsChild0Child1Choice::RetraceReformulation(node) => {
                BaseAnnotationChoice::RetraceReformulation(node)
            }
            BaseAnnotationsChild0Child1Choice::ScopedContrastiveStressing(node) => {
                BaseAnnotationChoice::ScopedContrastiveStressing(node)
            }
            BaseAnnotationsChild0Child1Choice::ScopedStressing(node) => {
                BaseAnnotationChoice::ScopedStressing(node)
            }
            BaseAnnotationsChild0Child1Choice::ScopedUncertain(node) => {
                BaseAnnotationChoice::ScopedUncertain(node)
            }
        }
    }
}

impl<'tree> From<BaseAnnotationsChild1Child1Choice<'tree>> for BaseAnnotationChoice<'tree> {
    fn from(choice: BaseAnnotationsChild1Child1Choice<'tree>) -> Self {
        match choice {
            BaseAnnotationsChild1Child1Choice::AltAnnotation(node) => {
                BaseAnnotationChoice::AltAnnotation(node)
            }
            BaseAnnotationsChild1Child1Choice::CodeSwitchAnnotation(node) => {
                BaseAnnotationChoice::CodeSwitchAnnotation(node)
            }
            BaseAnnotationsChild1Child1Choice::ErrorMarkerAnnotation(node) => {
                BaseAnnotationChoice::ErrorMarkerAnnotation(node)
            }
            BaseAnnotationsChild1Child1Choice::ExcludeMarker(node) => {
                BaseAnnotationChoice::ExcludeMarker(node)
            }
            BaseAnnotationsChild1Child1Choice::ExplanationAnnotation(node) => {
                BaseAnnotationChoice::ExplanationAnnotation(node)
            }
            BaseAnnotationsChild1Child1Choice::IndexedOverlapFollows(node) => {
                BaseAnnotationChoice::IndexedOverlapFollows(node)
            }
            BaseAnnotationsChild1Child1Choice::IndexedOverlapPrecedes(node) => {
                BaseAnnotationChoice::IndexedOverlapPrecedes(node)
            }
            BaseAnnotationsChild1Child1Choice::ParaAnnotation(node) => {
                BaseAnnotationChoice::ParaAnnotation(node)
            }
            BaseAnnotationsChild1Child1Choice::PercentAnnotation(node) => {
                BaseAnnotationChoice::PercentAnnotation(node)
            }
            BaseAnnotationsChild1Child1Choice::RetraceComplete(node) => {
                BaseAnnotationChoice::RetraceComplete(node)
            }
            BaseAnnotationsChild1Child1Choice::RetraceMultiple(node) => {
                BaseAnnotationChoice::RetraceMultiple(node)
            }
            BaseAnnotationsChild1Child1Choice::RetracePartial(node) => {
                BaseAnnotationChoice::RetracePartial(node)
            }
            BaseAnnotationsChild1Child1Choice::RetraceReformulation(node) => {
                BaseAnnotationChoice::RetraceReformulation(node)
            }
            BaseAnnotationsChild1Child1Choice::ScopedContrastiveStressing(node) => {
                BaseAnnotationChoice::ScopedContrastiveStressing(node)
            }
            BaseAnnotationsChild1Child1Choice::ScopedStressing(node) => {
                BaseAnnotationChoice::ScopedStressing(node)
            }
            BaseAnnotationsChild1Child1Choice::ScopedUncertain(node) => {
                BaseAnnotationChoice::ScopedUncertain(node)
            }
        }
    }
}

/// Converts one annotation like `[*]`, `[= text]`, or `[<2]`.
///
/// After Phase 5 coarsening, most annotations are atomic tokens. Parsing is
/// delegated to [`crate::tokens`] which provides the canonical parse
/// functions for each coarsened token type. Until 2026-09-09 this took a
/// raw node, unwrapped a `base_annotation` wrapper by `child(0)`, matched
/// `kind()` strings with a catch-all reporting "unknown base annotation
/// type", and had to refuse a MISSING node by hand (a placeholder decoded
/// by its kind minted a retrace marker nobody wrote); the generated choice
/// carries none of those states, so none of that is written here.
pub(crate) fn parse_single_annotation(
    choice: &BaseAnnotationChoice<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParsedAnnotation> {
    let annotation_node = choice.raw_node();
    let raw = &source[annotation_node.start_byte()..annotation_node.end_byte()];
    match choice {
        // Text-bearing annotations, delegate to tokens API
        BaseAnnotationChoice::ExplanationAnnotation(_) => delegate_content_or_error(
            tokens::parse_explanation_token(raw),
            annotation_node,
            source,
            errors,
        ),
        BaseAnnotationChoice::ParaAnnotation(_) => delegate_content_or_error(
            tokens::parse_para_token(raw),
            annotation_node,
            source,
            errors,
        ),
        BaseAnnotationChoice::AltAnnotation(_) => delegate_content_or_error(
            tokens::parse_alt_token(raw),
            annotation_node,
            source,
            errors,
        ),
        BaseAnnotationChoice::PercentAnnotation(_) => delegate_content_or_error(
            tokens::parse_percent_token(raw),
            annotation_node,
            source,
            errors,
        ),
        BaseAnnotationChoice::ErrorMarkerAnnotation(_) => delegate_content_or_error(
            tokens::parse_error_marker_token(raw),
            annotation_node,
            source,
            errors,
        ),
        // Overlap annotations, delegate to tokens API
        BaseAnnotationChoice::IndexedOverlapPrecedes(_) => delegate_content_or_error(
            tokens::parse_overlap_precedes_token(raw),
            annotation_node,
            source,
            errors,
        ),
        BaseAnnotationChoice::IndexedOverlapFollows(_) => delegate_content_or_error(
            tokens::parse_overlap_follows_token(raw),
            annotation_node,
            source,
            errors,
        ),
        // Scoped symbols, already atomic tokens, no payload
        BaseAnnotationChoice::ScopedStressing(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Stressing))
        }
        BaseAnnotationChoice::ScopedContrastiveStressing(_) => ParseOutcome::parsed(
            ParsedAnnotation::Content(ContentAnnotation::ContrastiveStressing),
        ),
        BaseAnnotationChoice::ScopedUncertain(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Uncertain))
        }
        // Retrace markers, parsed as RetraceKind, not ContentAnnotation. The
        // marker is one token, so the annotation node's bytes are its own;
        // the chain fold hands them to the retrace as `marker_span`.
        BaseAnnotationChoice::RetraceComplete(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace {
                kind: RetraceKind::Full,
                span: node_span(annotation_node),
            })
        }
        BaseAnnotationChoice::RetracePartial(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace {
                kind: RetraceKind::Partial,
                span: node_span(annotation_node),
            })
        }
        BaseAnnotationChoice::RetraceMultiple(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace {
                kind: RetraceKind::Multiple,
                span: node_span(annotation_node),
            })
        }
        BaseAnnotationChoice::RetraceReformulation(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Retrace {
                kind: RetraceKind::Reformulation,
                span: node_span(annotation_node),
            })
        }
        // Exclude marker, already atomic token
        BaseAnnotationChoice::ExcludeMarker(_) => {
            ParseOutcome::parsed(ParsedAnnotation::Content(ContentAnnotation::Exclude))
        }
        // Code-switch span: `[@s]` or `[@s:lang]`. The optional `code` field is
        // what distinguishes the two forms, and its ABSENCE is the bare form
        // rather than a missing value, so it maps to a variant and never to a
        // defaulted code.
        BaseAnnotationChoice::CodeSwitchAnnotation(_) => {
            let span = match annotation_node.child_by_field_name("code") {
                None => Some(CodeSwitchSpan::Shortcut),
                Some(code_node) => LanguageCode::new(extract_utf8_text(
                    code_node,
                    source,
                    errors,
                    "code_switch_annotation language code",
                    "",
                ))
                .ok()
                .map(CodeSwitchSpan::Explicit),
            };
            delegate_content_or_error(
                span.map(ContentAnnotation::CodeSwitch),
                annotation_node,
                source,
                errors,
            )
        }
    }
}

/// Convert a content annotation token parse result into a ParseOutcome, reporting an error on failure.
fn delegate_content_or_error(
    result: Option<ContentAnnotation>,
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<ParsedAnnotation> {
    match result {
        Some(annotation) => ParseOutcome::parsed(ParsedAnnotation::Content(annotation)),
        None => {
            errors.report(ParseError::new(
                ErrorCode::ContentAnnotationParseError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                format!(
                    "Failed to parse {} token: '{}'",
                    node.kind(),
                    &source[node.start_byte()..node.end_byte()]
                ),
            ));
            ParseOutcome::rejected()
        }
    }
}
