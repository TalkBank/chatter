//! Parses repeated `base_annotations` lists into model annotations.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>

use crate::error::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use crate::generated_traversal::{
    Absence, AsRawNode, BaseAnnotationChoiceBoundView, BaseAnnotationsNode, KindSlot, Never,
    NodeSlot, SourceBound, SourceBoundKind, SourceField, SourceSlotView, WhitespacesNode,
};
use crate::parser::ChildCapacity;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::read_source_field;
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
/// Callers retain the generated `base_annotations` wrapper through dispatch.
pub(crate) fn parse_scoped_annotations<'tree>(
    typed: SourceBound<'tree, '_, BaseAnnotationsNode<'tree>>,
    errors: &impl ErrorSink,
) -> Vec<ParsedAnnotation> {
    let node = typed.raw_node();
    let associated = typed.extract();
    let children = associated.children();
    let mut markers = ChildCapacity::for_pairs_of(node).into_vec();
    if let Some(first) = pair(associated.field_child_0().slot(), 0, errors) {
        push_annotation(
            first.field_unexpected(),
            first.field_child_0().slot(),
            first.field_child_1().slot(),
            0,
            |choice| choice.view().into(),
            errors,
            &mut markers,
        );
    }
    for (index, element) in associated.field_child_1().slot().iter().enumerate() {
        if let Some(next) = pair(element.slot(), index + 1, errors) {
            push_annotation(
                next.field_unexpected(),
                next.field_child_0().slot(),
                next.field_child_1().slot(),
                index + 1,
                |choice| choice.view().into(),
                errors,
                &mut markers,
            );
        }
    }
    surface_displaced(
        &children.unexpected,
        "base_annotations",
        typed.source(),
        errors,
    );
    markers
}

/// The pair at `index` in the run: `None` for a sequence that did not match
/// (`Absent`) or lost its whole shape to an ERROR. The ERROR is reported as
/// the old walk reported it, which expected a `whitespaces` child where the
/// pair begins. A sequence is never MISSING or displaced, and the slot's
/// type says so.
fn pair<'value, 'tree: 'value, 'source, T, A: Absence>(
    slot: SourceField<'value, 'tree, 'source, NodeSlot<'tree, T, Never, Never, A>>,
    index: usize,
    errors: &impl ErrorSink,
) -> Option<SourceField<'value, 'tree, 'source, T>> {
    match slot.view() {
        SourceSlotView::Present(element) => Some(element),
        SourceSlotView::Error(bad) => {
            report_mismatch(
                bad,
                "'whitespaces'",
                whitespace_child_position(index),
                errors,
            );
            None
        }
        SourceSlotView::Absent(_) => None,
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
    bad: SourceField<'_, '_, '_, Node<'_>>,
    expected: &str,
    position: usize,
    errors: &impl ErrorSink,
) {
    let source = bad.source();
    let bad = bad.raw_node();
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
fn push_annotation<'value, 'tree: 'value, 'source: 'value, C: SourceBoundKind<'tree>>(
    unexpected: SourceField<'value, 'tree, 'source, Vec<Node<'tree>>>,
    whitespace: SourceField<'value, 'tree, 'source, KindSlot<'tree, WhitespacesNode<'tree>>>,
    annotation: SourceField<'value, 'tree, 'source, KindSlot<'tree, C>>,
    pair_index: usize,
    lower: impl FnOnce(SourceBound<'tree, 'source, C>) -> BaseAnnotationChoiceBoundView<'tree, 'source>,
    errors: &impl ErrorSink,
    markers: &mut Vec<ParsedAnnotation>,
) {
    for bad in unexpected.iter() {
        surface_displaced(&[bad.raw_node()], "base_annotations", bad.source(), errors);
    }
    if let SourceSlotView::Error(bad) = whitespace.view() {
        report_mismatch(
            bad,
            "'whitespaces'",
            whitespace_child_position(pair_index),
            errors,
        );
    }
    match annotation.view() {
        SourceSlotView::Present(choice) => {
            let Some(choice) = read_source_field(choice, errors) else {
                return;
            };
            if let ParseOutcome::Parsed(parsed) = parse_single_annotation(lower(choice), errors) {
                markers.push(parsed);
            }
        }
        // A MISSING annotation is the whole-tree pass's to report (E342), and
        // its kind names nothing to decode: see the decoder's doc.
        SourceSlotView::Missing(_) | SourceSlotView::Absent(_) => {}
        SourceSlotView::Error(bad) => report_mismatch(
            bad,
            "annotation",
            annotation_child_position(pair_index),
            errors,
        ),
    }
}
