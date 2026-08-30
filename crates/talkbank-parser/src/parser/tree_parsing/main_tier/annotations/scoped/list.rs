//! Parses repeated `base_annotations` lists into model annotations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::node_types::WHITESPACES;
use crate::parser::ChildCapacity;
use crate::parser::tree_parsing::parser_helpers::is_base_annotation;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::single::{ParsedAnnotation, parse_single_annotation};

/// Converts a `base_annotations` node into the ordered run of markers.
///
/// ORDERED, and that is the whole point. The run is a left-associative chain:
/// each marker scopes over everything to its left, so `dog [* p:w] [/]` (the
/// error is on the abandoned attempt) and `dog [/] [* p:w]` (the error is on
/// the retrace) are different claims about the same two words.
///
/// This used to return a PARTITION, `{ content: Vec<ContentAnnotation>,
/// retrace: Option<RetraceKind> }`, whose docstring asserted in prose that "at
/// most one retrace marker can appear in an annotation list". A partition of an
/// ordered sequence can represent neither the interleaving nor a second marker,
/// so the parser silently rewrote one ordering into the other (12,226 attested
/// places in the corpora) and let a second marker overwrite the first (105
/// places). Both were invisible to validate, to roundtrip and to `SemanticEq`.
/// See `docs/design/2026-08-07-retrace-model-and-the-lost-marker-position.md`.
///
/// Nothing is judged here. An illegal run lowers faithfully and validation
/// rejects it, so one rule covers both spellings and both parser backends.
///
/// **Grammar Rule:**
/// ```text
/// base_annotations: $ => repeat1(
///   seq($.whitespaces, $.base_annotation)
/// )
/// ```
///
/// **Expected Sequential Order:**
/// One or more pairs of: `whitespaces` then `base_annotation`
pub(crate) fn parse_scoped_annotations(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<ParsedAnnotation> {
    let child_count = node.child_count();
    // Pre-allocate: child_count / 2 pairs of (whitespace, annotation)
    let mut markers = ChildCapacity::from_upper_bound(child_count / 2).into_vec();
    let mut idx = 0;

    // Grammar: repeat1(seq(whitespaces, base_annotation))
    // Expect alternating whitespaces and base_annotation
    while idx < child_count {
        // Expect whitespaces
        if let Some(child) = node.child(idx) {
            if child.kind() == WHITESPACES {
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::ContentAnnotationParseError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!(
                        "Expected 'whitespaces' at position {} of base_annotations, found '{}'",
                        idx,
                        child.kind()
                    ),
                ));
                idx += 1;
                continue;
            }
        } else {
            break;
        }

        // Expect base_annotation (or one of its concrete subtypes)
        if idx < child_count
            && let Some(child) = node.child(idx)
        {
            if is_base_annotation(child.kind()) {
                if let ParseOutcome::Parsed(ann) = parse_single_annotation(child, source, errors) {
                    markers.push(ann);
                }
                idx += 1;
            } else {
                errors.report(ParseError::new(
                    ErrorCode::ContentAnnotationParseError,
                    Severity::Error,
                    SourceLocation::from_offsets(child.start_byte(), child.end_byte()),
                    ErrorContext::new(source, child.start_byte()..child.end_byte(), ""),
                    format!(
                        "Expected annotation at position {} of base_annotations, found '{}'",
                        idx,
                        child.kind()
                    ),
                ));
                idx += 1;
            }
        }
    }

    markers
}
