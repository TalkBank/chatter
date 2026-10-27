//! The pieces of a word body: one enum over the twelve leaf tokens, the
//! exhaustive lowerings from the generated traversal's six position
//! choices, and the one conversion of a piece to its `WordContent`.

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, CaDelimiterNode, CaElementNode, LengtheningNode, OverlapPointNode, PlusNode,
    ShorteningNode, SourceBound, StressMarkerNode, SyllablePauseNode, TildeNode,
    UnderlineBeginNode, UnderlineEndNode, WordBodyOverlapPointChild0Choice,
    WordBodyOverlapPointChild1Choice, WordBodyOverlapPointChild2Choice,
    WordBodyOverlapPointChild3Choice, WordBodyWordSegmentChild0Choice,
    WordBodyWordSegmentChild1Choice, WordSegmentNode,
};
use crate::generated_traversal::{Never, Ranged, ReadableSlot, SourceSlice};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::main_tier::content::{
    parse_overlap_point_token, report_tree_shape,
};
use crate::parser::tree_parsing::parser_helpers::{
    expect_delimiter, parse_ca_delimiter_node, parse_ca_element_node, surface_displaced,
};
use crate::parser::typed_cst::{ExtractCanonical, ReportedCstFailure, report_admission};
use smallvec::SmallVec;
use talkbank_model::ParseOutcome;
use talkbank_model::content::word::{
    WordCliticBoundary, WordCompoundMarker, WordContent, WordLengthening, WordShortening,
    WordStressMarker, WordStressMarkerType, WordSyllablePause, WordText, WordUnderlineBegin,
    WordUnderlineEnd,
};
use talkbank_model::model::EmptyText;

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

impl<'tree, 'source> From<&WordBodyWordSegmentChild0Choice<'tree, Ranged<'source>>>
    for Piece<'tree, 'source>
{
    fn from(choice: &WordBodyWordSegmentChild0Choice<'tree, Ranged<'source>>) -> Self {
        use WordBodyWordSegmentChild0Choice as C;
        match choice {
            C::WordSegment(node) => Piece::Segment(*node),
            C::Shortening(node) => Piece::Shortening(*node),
            C::StressMarker(node) => Piece::Stress(*node),
        }
    }
}

impl<'tree, 'source> From<&WordBodyWordSegmentChild1Choice<'tree, Ranged<'source>>>
    for Piece<'tree, 'source>
{
    fn from(choice: &WordBodyWordSegmentChild1Choice<'tree, Ranged<'source>>) -> Self {
        use WordBodyWordSegmentChild1Choice as C;
        match choice {
            C::WordSegment(node) => Piece::Segment(*node),
            C::Shortening(node) => Piece::Shortening(*node),
            C::StressMarker(node) => Piece::Stress(*node),
            C::Lengthening(node) => Piece::Lengthening(*node),
            C::OverlapPoint(node) => Piece::Overlap(*node),
            C::CaElement(node) => Piece::CaElement(*node),
            C::CaDelimiter(node) => Piece::CaDelimiter(*node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(*node),
            C::UnderlineEnd(node) => Piece::UnderlineEnd(*node),
            C::SyllablePause(node) => Piece::SyllablePause(*node),
            C::Tilde(node) => Piece::Tilde(*node),
            C::Variant11(node) => Piece::Plus(*node),
        }
    }
}

impl<'tree, 'source> From<&WordBodyOverlapPointChild0Choice<'tree, Ranged<'source>>>
    for Piece<'tree, 'source>
{
    fn from(choice: &WordBodyOverlapPointChild0Choice<'tree, Ranged<'source>>) -> Self {
        use WordBodyOverlapPointChild0Choice as C;
        match choice {
            C::OverlapPoint(node) => Piece::Overlap(*node),
            C::CaElement(node) => Piece::CaElement(*node),
            C::CaDelimiter(node) => Piece::CaDelimiter(*node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(*node),
            C::SyllablePause(node) => Piece::SyllablePause(*node),
        }
    }
}

impl<'tree, 'source> From<&WordBodyOverlapPointChild1Choice<'tree, Ranged<'source>>>
    for Piece<'tree, 'source>
{
    fn from(choice: &WordBodyOverlapPointChild1Choice<'tree, Ranged<'source>>) -> Self {
        use WordBodyOverlapPointChild1Choice as C;
        match choice {
            C::OverlapPoint(node) => Piece::Overlap(*node),
            C::CaElement(node) => Piece::CaElement(*node),
            C::CaDelimiter(node) => Piece::CaDelimiter(*node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(*node),
            C::SyllablePause(node) => Piece::SyllablePause(*node),
        }
    }
}

impl<'tree, 'source> From<&WordBodyOverlapPointChild2Choice<'tree, Ranged<'source>>>
    for Piece<'tree, 'source>
{
    fn from(choice: &WordBodyOverlapPointChild2Choice<'tree, Ranged<'source>>) -> Self {
        use WordBodyOverlapPointChild2Choice as C;
        match choice {
            C::WordSegment(node) => Piece::Segment(*node),
            C::Shortening(node) => Piece::Shortening(*node),
            C::StressMarker(node) => Piece::Stress(*node),
        }
    }
}

impl<'tree, 'source> From<&WordBodyOverlapPointChild3Choice<'tree, Ranged<'source>>>
    for Piece<'tree, 'source>
{
    fn from(choice: &WordBodyOverlapPointChild3Choice<'tree, Ranged<'source>>) -> Self {
        use WordBodyOverlapPointChild3Choice as C;
        match choice {
            C::WordSegment(node) => Piece::Segment(*node),
            C::Shortening(node) => Piece::Shortening(*node),
            C::StressMarker(node) => Piece::Stress(*node),
            C::Lengthening(node) => Piece::Lengthening(*node),
            C::OverlapPoint(node) => Piece::Overlap(*node),
            C::CaElement(node) => Piece::CaElement(*node),
            C::CaDelimiter(node) => Piece::CaDelimiter(*node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(*node),
            C::UnderlineEnd(node) => Piece::UnderlineEnd(*node),
            C::SyllablePause(node) => Piece::SyllablePause(*node),
            C::Tilde(node) => Piece::Tilde(*node),
            C::Variant11(node) => Piece::Plus(*node),
        }
    }
}

/// One typed body position: its piece, if the position holds one.
pub(super) fn push_slot<'tree, 'source, C, M, A>(
    slot: &ReadableSlot<C, M, SourceSlice<'tree, 'source>, Never, A>,
    errors: &impl ErrorSink,
    items: &mut SmallVec<[WordContent; 2]>,
) -> Result<(), ReportedCstFailure>
where
    for<'value> &'value C: Into<Piece<'tree, 'source>>,
{
    match slot {
        ReadableSlot::Present(choice) => push_piece(choice.into(), errors, items),
        ReadableSlot::Missing(_) | ReadableSlot::Error(_) | ReadableSlot::Absent(_) => Ok(()),
        ReadableSlot::Unexpected(never) => match *never {},
    }
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
) -> Result<(), ReportedCstFailure> {
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
            let bound_inner = report_admission(
                node.extract_canonical()
                    .and_then(|children| children.admit_ranges().map_err(Into::into)),
                node.raw_node(),
                source,
                errors,
            )?;
            let inner = bound_inner.children();
            surface_displaced(&inner.unexpected, "shortening", source, errors);
            expect_delimiter(&inner.child_0.slot, |bad| {
                report_tree_shape(
                    bad,
                    format!("Expected '(' opening a shortening, found '{}'", bad.kind()),
                    source,
                    errors,
                );
            });
            expect_delimiter(&inner.child_2.slot, |bad| {
                report_tree_shape(
                    bad,
                    format!("Expected ')' closing a shortening, found '{}'", bad.kind()),
                    source,
                    errors,
                );
            });
            let segment = match &inner.child_1.slot {
                ReadableSlot::Present(segment) => Some(segment),
                ReadableSlot::Missing(_) | ReadableSlot::Error(_) | ReadableSlot::Absent(_) => None,
                ReadableSlot::Unexpected(never) => match *never {},
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
                    return Ok(());
                }
            };
            items.push(WordContent::StressMarker(WordStressMarker {
                marker_type,
                location: span_of(raw).into(),
            }));
        }
        Piece::Lengthening(node) => {
            let raw = node.raw_node();
            let text = node.text();
            // The grammar's lengthening token consists only of colons. A
            // failed extraction already reports its error; do not turn its
            // empty fallback into an invented one-colon marker.
            if let Some(count) = std::num::NonZeroUsize::new(text.len()) {
                items.push(WordContent::Lengthening(WordLengthening::with_count(
                    span_of(raw),
                    count,
                )));
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
            if let ParseOutcome::Parsed(element) = parse_ca_element_node(node, errors) {
                items.push(WordContent::CAElement(element));
            }
        }
        Piece::CaDelimiter(node) => {
            if let ParseOutcome::Parsed(delimiter) = parse_ca_delimiter_node(node, errors) {
                items.push(WordContent::CADelimiter(delimiter));
            }
        }
        Piece::UnderlineBegin(node) => items.push(WordContent::UnderlineBegin(
            WordUnderlineBegin::new(span_of(node.raw_node())),
        )),
        Piece::UnderlineEnd(node) => items.push(WordContent::UnderlineEnd(WordUnderlineEnd::new(
            span_of(node.raw_node()),
        ))),
        Piece::SyllablePause(node) => items.push(WordContent::SyllablePause(
            WordSyllablePause::new(span_of(node.raw_node())),
        )),
        Piece::Tilde(node) => items.push(WordContent::CliticBoundary(WordCliticBoundary::new(
            span_of(node.raw_node()),
        ))),
        Piece::Plus(node) => items.push(WordContent::CompoundMarker(WordCompoundMarker {
            location: span_of(node.raw_node()).into(),
        })),
    }
    Ok(())
}
