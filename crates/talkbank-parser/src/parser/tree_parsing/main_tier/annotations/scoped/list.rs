//! Parses repeated `base_annotations` lists into model annotations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    BaseAnnotationChoice, BaseAnnotationsNode, FromNodeKind, NoChild, NodeSlot, RecoveryNode,
    SeqSlot, SlotView, extract_base_annotations,
};
use crate::parser::ChildCapacity;
use crate::parser::tree_parsing::parser_helpers::{expect_delimiter, surface_displaced};
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
/// One or more pairs of: `whitespaces` then `base_annotation`. The generated
/// traversal names the first pair and the repeat separately; each pair's
/// whitespace is a delimiter (reported when it loses its shape; a MISSING
/// one is the whole-tree pass's) and its annotation the sixteen-way choice
/// the decoder matches. Until 2026-09-09 this walked the children by index
/// and `kind()` string.
///
/// Callers hold the raw `base_annotations` node; a node of another kind is
/// refused with a diagnostic.
pub(crate) fn parse_scoped_annotations(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<ParsedAnnotation> {
    let Some(typed) = BaseAnnotationsNode::from_node(node) else {
        errors.report(ParseError::new(
            ErrorCode::ContentAnnotationParseError,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
            format!("Expected a base_annotations node, found '{}'", node.kind()),
        ));
        return Vec::new();
    };
    let children = extract_base_annotations(typed);
    let repeat = children.child_1.slot();
    let mut markers = ChildCapacity::for_pairs_of(node).into_vec();
    if let Some(first) = pair(children.child_0.slot(), 0, source, errors) {
        push_annotation(
            &first.unexpected,
            first.child_0.slot(),
            first.child_1.slot(),
            0,
            source,
            errors,
            &mut markers,
        );
    }
    for (index, element) in repeat.iter().enumerate() {
        if let Some(next) = pair(element.slot(), index + 1, source, errors) {
            push_annotation(
                &next.unexpected,
                next.child_0.slot(),
                next.child_1.slot(),
                index + 1,
                source,
                errors,
                &mut markers,
            );
        }
    }
    surface_displaced(&children.unexpected, "base_annotations", source, errors);
    markers
}

/// The pair at `index` in the run: `None` for a sequence that did not match
/// (`Absent`) or lost its whole shape to an ERROR. The ERROR is reported as
/// the old walk reported it, which expected a `whitespaces` child where the
/// pair begins. A sequence is never MISSING or displaced, and the slot's
/// type says so.
fn pair<'a, 'tree, T>(
    slot: &'a SeqSlot<'tree, T>,
    index: usize,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<&'a T> {
    match slot.view() {
        SlotView::Present(element) => Some(element),
        SlotView::Error(bad) => {
            report_mismatch(
                bad,
                "'whitespaces'",
                whitespace_child_position(index),
                source,
                errors,
            );
            None
        }
        SlotView::Absent(NoChild) => None,
    }
}

/// The old walk numbered CHILDREN of `base_annotations`, not pairs: pair `k`
/// holds its whitespace at child `2k` and its annotation at child `2k + 1`,
/// and the diagnostics keep those numbers.
fn whitespace_child_position(pair_index: usize) -> usize {
    2 * pair_index
}

/// See [`whitespace_child_position`].
fn annotation_child_position(pair_index: usize) -> usize {
    2 * pair_index + 1
}

/// The one `ContentAnnotationParseError` wording, shared by the three
/// positions that can hold an ERROR node.
fn report_mismatch(
    bad: Node,
    expected: &str,
    position: usize,
    source: &str,
    errors: &impl ErrorSink,
) {
    errors.report(ParseError::new(
        ErrorCode::ContentAnnotationParseError,
        Severity::Error,
        SourceLocation::from_offsets(bad.start_byte(), bad.end_byte()),
        ErrorContext::new(source, bad.start_byte()..bad.end_byte(), ""),
        format!(
            "Expected {expected} at position {position} of base_annotations, found '{}'",
            bad.kind()
        ),
    ));
}

/// One `whitespaces` then `base_annotation` pair, the `pair_index`th in the
/// run: the whitespace is a delimiter, the annotation goes to the decoder,
/// and either child that holds an ERROR is reported as the old walk reported
/// it, numbered by child (see [`whitespace_child_position`]). `C` is the
/// generated choice at this position, lowered into the one annotation set.
fn push_annotation<'tree, W, M, U, A, C>(
    unexpected: &[Node],
    whitespace: &NodeSlot<'tree, W, M, U, A>,
    annotation: &NodeSlot<
        'tree,
        C,
        tree_sitter::Node<'tree>,
        crate::generated_traversal::Never,
        NoChild,
    >,
    pair_index: usize,
    source: &str,
    errors: &impl ErrorSink,
    markers: &mut Vec<ParsedAnnotation>,
) where
    U: RecoveryNode<'tree>,
    C: Clone + Into<BaseAnnotationChoice<'tree>>,
{
    surface_displaced(unexpected, "base_annotations", source, errors);
    expect_delimiter(whitespace, |bad| {
        report_mismatch(
            bad,
            "'whitespaces'",
            whitespace_child_position(pair_index),
            source,
            errors,
        );
    });
    match annotation.view() {
        SlotView::Present(choice) => {
            let choice: BaseAnnotationChoice<'tree> = choice.clone().into();
            if let ParseOutcome::Parsed(parsed) = parse_single_annotation(&choice, source, errors) {
                markers.push(parsed);
            }
        }
        // A MISSING annotation is the whole-tree pass's to report (E342), and
        // its kind names nothing to decode: see the decoder's doc.
        SlotView::Missing(_) | SlotView::Absent(NoChild) => {}
        SlotView::Error(bad) => report_mismatch(
            bad,
            "annotation",
            annotation_child_position(pair_index),
            source,
            errors,
        ),
    }
}
