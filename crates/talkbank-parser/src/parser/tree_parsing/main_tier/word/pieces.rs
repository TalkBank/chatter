//! The pieces of a word body: one enum over the twelve leaf tokens, the
//! exhaustive lowerings from the generated traversal's six position
//! choices, and the one conversion of a piece to its `WordContent`.

use crate::error::ErrorSink;
use crate::generated_traversal::{Absence, Never, NodeSlot};
use crate::generated_traversal::{
    AdmittedWordBodyOverlapPointChild0Choice as WordBodyOverlapPointChild0Choice,
    AdmittedWordBodyOverlapPointChild0ChoiceBoundView as WordBodyOverlapPointChild0ChoiceBoundView,
    AdmittedWordBodyOverlapPointChild1Choice as WordBodyOverlapPointChild1Choice,
    AdmittedWordBodyOverlapPointChild1ChoiceBoundView as WordBodyOverlapPointChild1ChoiceBoundView,
    AdmittedWordBodyOverlapPointChild2Choice as WordBodyOverlapPointChild2Choice,
    AdmittedWordBodyOverlapPointChild2ChoiceBoundView as WordBodyOverlapPointChild2ChoiceBoundView,
    AdmittedWordBodyOverlapPointChild3Choice as WordBodyOverlapPointChild3Choice,
    AdmittedWordBodyOverlapPointChild3ChoiceBoundView as WordBodyOverlapPointChild3ChoiceBoundView,
    AdmittedWordBodyOverlapPointChild3LengtheningChoice as WordBodyOverlapPointChild3LengtheningChoice,
    AdmittedWordBodyOverlapPointChild3LengtheningChoiceBoundView as WordBodyOverlapPointChild3LengtheningChoiceBoundView,
    AdmittedWordBodyWordSegmentChild0Choice as WordBodyWordSegmentChild0Choice,
    AdmittedWordBodyWordSegmentChild0ChoiceBoundView as WordBodyWordSegmentChild0ChoiceBoundView,
    AdmittedWordBodyWordSegmentChild1Choice as WordBodyWordSegmentChild1Choice,
    AdmittedWordBodyWordSegmentChild1ChoiceBoundView as WordBodyWordSegmentChild1ChoiceBoundView,
    AdmittedWordBodyWordSegmentChild1LengtheningChoice as WordBodyWordSegmentChild1LengtheningChoice,
    AdmittedWordBodyWordSegmentChild1LengtheningChoiceBoundView as WordBodyWordSegmentChild1LengtheningChoiceBoundView,
    AsRawNode, CaDelimiterNode, CaElementNode, LengtheningNode, OverlapPointNode, PlusNode,
    ShorteningNode, SourceBound, SourceBoundKind, SourceField, SourceSlotView, StressMarkerNode,
    SyllablePauseNode, TildeNode, UnderlineBeginNode, UnderlineEndNode, WordSegmentNode,
};
use crate::parser::tree_parsing::main_tier::content::{
    parse_overlap_point_token, report_tree_shape,
};
use crate::parser::tree_parsing::parser_helpers::{
    expect_delimiter, parse_ca_delimiter_node, parse_ca_element_node, surface_displaced,
};
use smallvec::SmallVec;
use talkbank_model::ParseOutcome;
use talkbank_model::content::word::{
    WordCliticBoundary, WordCompoundMarker, WordContent, WordLengthening, WordShortening,
    WordStressMarker, WordStressMarkerType, WordSyllablePause, WordText, WordUnderlineBegin,
    WordUnderlineEnd,
};
use talkbank_model::model::EmptyText;
use tree_sitter::Node;

/// One piece of a word body, whichever of the grammar's positions it came
/// through.
///
/// `word_body` is two sequences (segment-initial and marker-initial), each
/// a first position followed by a repeat, and the generated traversal names
/// the choice at every position separately: six enums over one set of
/// twelve leaves. This is that set, so the conversion of a leaf is written
/// once; the `From` impls below are the exhaustive lowerings, and a new
/// leaf in the grammar breaks them at compile time.
#[derive(Debug, Clone, Copy)]
pub(super) enum Piece<'tree, 'source> {
    Segment(SourceBound<'tree, 'source, WordSegmentNode<'tree>>),
    Shortening(SourceBound<'tree, 'source, ShorteningNode<'tree>>),
    Stress(SourceBound<'tree, 'source, StressMarkerNode<'tree>>),
    Lengthening(SourceBound<'tree, 'source, LengtheningNode<'tree>>),
    Overlap(SourceBound<'tree, 'source, OverlapPointNode<'tree>>),
    CaElement(SourceBound<'tree, 'source, CaElementNode<'tree>>),
    CaDelimiter(SourceBound<'tree, 'source, CaDelimiterNode<'tree>>),
    UnderlineBegin(SourceBound<'tree, 'source, UnderlineBeginNode<'tree>>),
    UnderlineEnd(SourceBound<'tree, 'source, UnderlineEndNode<'tree>>),
    SyllablePause(SourceBound<'tree, 'source, SyllablePauseNode<'tree>>),
    Tilde(SourceBound<'tree, 'source, TildeNode<'tree>>),
    Plus(SourceBound<'tree, 'source, PlusNode<'tree>>),
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, WordBodyWordSegmentChild0Choice<'tree>>>
    for Piece<'tree, 'source>
{
    fn from(choice: SourceBound<'tree, 'source, WordBodyWordSegmentChild0Choice<'tree>>) -> Self {
        use WordBodyWordSegmentChild0ChoiceBoundView as C;
        match choice.view() {
            C::WordSegment(node) => Piece::Segment(node),
            C::Shortening(node) => Piece::Shortening(node),
            C::StressMarker(node) => Piece::Stress(node),
        }
    }
}

impl<'tree, 'source>
    From<SourceBound<'tree, 'source, WordBodyWordSegmentChild1LengtheningChoice<'tree>>>
    for Piece<'tree, 'source>
{
    fn from(
        choice: SourceBound<'tree, 'source, WordBodyWordSegmentChild1LengtheningChoice<'tree>>,
    ) -> Self {
        use WordBodyWordSegmentChild1LengtheningChoiceBoundView as C;
        match choice.view() {
            C::Lengthening(node) => Piece::Lengthening(node),
            C::OverlapPoint(node) => Piece::Overlap(node),
            C::CaElement(node) => Piece::CaElement(node),
            C::CaDelimiter(node) => Piece::CaDelimiter(node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(node),
            C::UnderlineEnd(node) => Piece::UnderlineEnd(node),
            C::SyllablePause(node) => Piece::SyllablePause(node),
            C::Tilde(node) => Piece::Tilde(node),
            C::Variant8(node) => Piece::Plus(node),
        }
    }
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, WordBodyWordSegmentChild1Choice<'tree>>>
    for Piece<'tree, 'source>
{
    fn from(choice: SourceBound<'tree, 'source, WordBodyWordSegmentChild1Choice<'tree>>) -> Self {
        use WordBodyWordSegmentChild1ChoiceBoundView as C;
        match choice.view() {
            C::WordSegment(node) => Piece::Segment(node),
            C::Shortening(node) => Piece::Shortening(node),
            C::StressMarker(node) => Piece::Stress(node),
            C::Lengthening(marker) => marker.into(),
        }
    }
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, WordBodyOverlapPointChild0Choice<'tree>>>
    for Piece<'tree, 'source>
{
    fn from(choice: SourceBound<'tree, 'source, WordBodyOverlapPointChild0Choice<'tree>>) -> Self {
        use WordBodyOverlapPointChild0ChoiceBoundView as C;
        match choice.view() {
            C::OverlapPoint(node) => Piece::Overlap(node),
            C::CaElement(node) => Piece::CaElement(node),
            C::CaDelimiter(node) => Piece::CaDelimiter(node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(node),
            C::SyllablePause(node) => Piece::SyllablePause(node),
        }
    }
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, WordBodyOverlapPointChild1Choice<'tree>>>
    for Piece<'tree, 'source>
{
    fn from(choice: SourceBound<'tree, 'source, WordBodyOverlapPointChild1Choice<'tree>>) -> Self {
        use WordBodyOverlapPointChild1ChoiceBoundView as C;
        match choice.view() {
            C::OverlapPoint(node) => Piece::Overlap(node),
            C::CaElement(node) => Piece::CaElement(node),
            C::CaDelimiter(node) => Piece::CaDelimiter(node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(node),
            C::SyllablePause(node) => Piece::SyllablePause(node),
        }
    }
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, WordBodyOverlapPointChild2Choice<'tree>>>
    for Piece<'tree, 'source>
{
    fn from(choice: SourceBound<'tree, 'source, WordBodyOverlapPointChild2Choice<'tree>>) -> Self {
        use WordBodyOverlapPointChild2ChoiceBoundView as C;
        match choice.view() {
            C::WordSegment(node) => Piece::Segment(node),
            C::Shortening(node) => Piece::Shortening(node),
            C::StressMarker(node) => Piece::Stress(node),
        }
    }
}

impl<'tree, 'source>
    From<SourceBound<'tree, 'source, WordBodyOverlapPointChild3LengtheningChoice<'tree>>>
    for Piece<'tree, 'source>
{
    fn from(
        choice: SourceBound<'tree, 'source, WordBodyOverlapPointChild3LengtheningChoice<'tree>>,
    ) -> Self {
        use WordBodyOverlapPointChild3LengtheningChoiceBoundView as C;
        match choice.view() {
            C::Lengthening(node) => Piece::Lengthening(node),
            C::OverlapPoint(node) => Piece::Overlap(node),
            C::CaElement(node) => Piece::CaElement(node),
            C::CaDelimiter(node) => Piece::CaDelimiter(node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(node),
            C::UnderlineEnd(node) => Piece::UnderlineEnd(node),
            C::SyllablePause(node) => Piece::SyllablePause(node),
            C::Tilde(node) => Piece::Tilde(node),
            C::Variant8(node) => Piece::Plus(node),
        }
    }
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, WordBodyOverlapPointChild3Choice<'tree>>>
    for Piece<'tree, 'source>
{
    fn from(choice: SourceBound<'tree, 'source, WordBodyOverlapPointChild3Choice<'tree>>) -> Self {
        use WordBodyOverlapPointChild3ChoiceBoundView as C;
        match choice.view() {
            C::WordSegment(node) => Piece::Segment(node),
            C::Shortening(node) => Piece::Shortening(node),
            C::StressMarker(node) => Piece::Stress(node),
            C::Lengthening(marker) => marker.into(),
        }
    }
}

/// One typed body position: its piece, if the position holds one.
pub(super) fn push_slot<'a, 'tree: 'a, 'source: 'a, C: SourceBoundKind<'tree> + 'a, A: Absence>(
    slot: SourceField<'a, 'tree, 'source, NodeSlot<'tree, C, tree_sitter::Node<'tree>, Never, A>>,
    errors: &impl ErrorSink,
    items: &mut SmallVec<[WordContent; 2]>,
) where
    SourceBound<'tree, 'source, C>: Into<Piece<'tree, 'source>>,
{
    match slot.view() {
        SourceSlotView::Present(choice) => {
            if let Some(choice) = crate::parser::typed_cst::read_source_field(choice, errors) {
                push_piece(choice.into(), errors, items);
            }
        }
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(_) => {}
        SourceSlotView::Unexpected(never) => match never {},
    }
}

/// The span of a leaf token.
fn span_of(node: Node) -> talkbank_model::Span {
    talkbank_model::Span::from_usize(node.start_byte(), node.end_byte())
}

/// Convert one word piece to its `WordContent`, or report why it has none.
///
/// The leaf tokens are single characters or character runs by grammar, so
/// the "empty" and "unknown" arms below are unreachable from a parse; they
/// are reported rather than silently skipped, which is what the old walk
/// did with them.
fn push_piece(
    piece: Piece<'_, '_>,
    errors: &impl ErrorSink,
    items: &mut SmallVec<[WordContent; 2]>,
) {
    match piece {
        Piece::Segment(node) => {
            let raw = node.raw_node();
            let source = node.source();
            let text = node.text();
            match WordText::new(text) {
                Ok(text) => items.push(WordContent::Text(text)),
                Err(EmptyText) => {
                    report_tree_shape(raw, "Empty word_segment token".to_string(), source, errors)
                }
            }
        }
        Piece::Shortening(node) => {
            let source = node.source();
            // shortening = '(' word_segment ')'
            let Ok(bound_inner) = crate::parser::typed_cst::report_reconstruction(
                node.extract(),
                node.raw_node(),
                source,
                errors,
            ) else {
                return;
            };
            let inner = bound_inner.children();
            surface_displaced(&inner.unexpected, "shortening", source, errors);
            expect_delimiter(inner.child_0.slot(), |bad| {
                report_tree_shape(
                    bad,
                    format!("Expected '(' opening a shortening, found '{}'", bad.kind()),
                    source,
                    errors,
                );
            });
            expect_delimiter(inner.child_2.slot(), |bad| {
                report_tree_shape(
                    bad,
                    format!("Expected ')' closing a shortening, found '{}'", bad.kind()),
                    source,
                    errors,
                );
            });
            let segment = match bound_inner.field_child_1().slot().view() {
                SourceSlotView::Present(segment) => {
                    crate::parser::typed_cst::read_source_field(segment, errors)
                }
                SourceSlotView::Missing(_)
                | SourceSlotView::Error(_)
                | SourceSlotView::Absent(_) => None,
                SourceSlotView::Unexpected(never) => match never {},
            };
            if let Some(segment) = segment {
                match WordShortening::new(segment.text()) {
                    Ok(shortening) => items.push(WordContent::Shortening(shortening)),
                    Err(EmptyText) => report_tree_shape(
                        segment.raw_node(),
                        "Empty shortening content".to_string(),
                        source,
                        errors,
                    ),
                }
            }
        }
        Piece::Stress(node) => {
            let raw = node.raw_node();
            let source = node.source();
            let text = node.text();
            let marker_type = match text.chars().next() {
                Some('\u{02C8}') => WordStressMarkerType::Primary,
                Some('\u{02CC}') => WordStressMarkerType::Secondary,
                // The token is `choice('\u{02C8}', '\u{02CC}')`: one of the
                // two characters, exactly. Widening the token without a new
                // arm here would drop the piece with this report.
                other => {
                    report_tree_shape(
                        raw,
                        format!("Unknown stress marker {other:?}"),
                        source,
                        errors,
                    );
                    return;
                }
            };
            items.push(WordContent::StressMarker(WordStressMarker {
                marker_type,
                span: Some(span_of(raw)),
            }));
        }
        Piece::Lengthening(node) => {
            let raw = node.raw_node();
            let text = node.text();
            // The grammar's lengthening token consists only of colons. A
            // failed extraction already reports its error; do not turn its
            // empty fallback into an invented one-colon marker.
            if let Some(count) = std::num::NonZeroUsize::new(text.len()) {
                items.push(WordContent::Lengthening(
                    WordLengthening::with_count(count).with_span(span_of(raw)),
                ));
            }
        }
        Piece::Overlap(node) => {
            // An overlap marker inside a word (`butt⌈er⌉`): the same token as
            // a standalone one, decoded by the same function.
            if let ParseOutcome::Parsed(point) = parse_overlap_point_token(node, errors) {
                items.push(WordContent::OverlapPoint(point));
            }
        }
        Piece::CaElement(node) => {
            if let ParseOutcome::Parsed(element) =
                parse_ca_element_node(node.node(), node.source(), errors)
            {
                items.push(WordContent::CAElement(element));
            }
        }
        Piece::CaDelimiter(node) => {
            if let ParseOutcome::Parsed(delimiter) =
                parse_ca_delimiter_node(node.node(), node.source(), errors)
            {
                items.push(WordContent::CADelimiter(delimiter));
            }
        }
        Piece::UnderlineBegin(node) => items.push(WordContent::UnderlineBegin(
            WordUnderlineBegin::from_span(span_of(node.raw_node())),
        )),
        Piece::UnderlineEnd(node) => items.push(WordContent::UnderlineEnd(
            WordUnderlineEnd::from_span(span_of(node.raw_node())),
        )),
        Piece::SyllablePause(node) => items.push(WordContent::SyllablePause(
            WordSyllablePause::new().with_span(span_of(node.raw_node())),
        )),
        Piece::Tilde(node) => items.push(WordContent::CliticBoundary(
            WordCliticBoundary::new().with_span(span_of(node.raw_node())),
        )),
        Piece::Plus(node) => items.push(WordContent::CompoundMarker(WordCompoundMarker {
            span: Some(span_of(node.raw_node())),
        })),
    }
}
