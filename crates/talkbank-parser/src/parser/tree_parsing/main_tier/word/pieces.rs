//! The pieces of a word body: one enum over the twelve leaf tokens, the
//! exhaustive lowerings from the generated traversal's six position
//! choices, and the one conversion of a piece to its `WordContent`.

use super::taken;
use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, CaDelimiterNode, CaElementNode, LengtheningNode, NodeSlot, OverlapPointNode,
    PlusNode, RecoveryNode, ShorteningNode, StressMarkerNode, SyllablePauseNode, TildeNode,
    UnderlineBeginNode, UnderlineEndNode, WordBodyOverlapPointChild0Choice,
    WordBodyOverlapPointChild1Choice, WordBodyOverlapPointChild2Choice,
    WordBodyOverlapPointChild3Choice, WordBodyOverlapPointChild3LengtheningChoice,
    WordBodyWordSegmentChild0Choice, WordBodyWordSegmentChild1Choice,
    WordBodyWordSegmentChild1LengtheningChoice, WordSegmentNode, extract_shortening,
};
use crate::parser::tree_parsing::main_tier::content::{
    parse_overlap_point_token, report_tree_shape,
};
use crate::parser::tree_parsing::parser_helpers::{
    expect_delimiter, extract_utf8_text, parse_ca_delimiter_node, parse_ca_element_node,
    surface_displaced,
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
pub(super) enum Piece<'tree> {
    Segment(WordSegmentNode<'tree>),
    Shortening(ShorteningNode<'tree>),
    Stress(StressMarkerNode<'tree>),
    Lengthening(LengtheningNode<'tree>),
    Overlap(OverlapPointNode<'tree>),
    CaElement(CaElementNode<'tree>),
    CaDelimiter(CaDelimiterNode<'tree>),
    UnderlineBegin(UnderlineBeginNode<'tree>),
    UnderlineEnd(UnderlineEndNode<'tree>),
    SyllablePause(SyllablePauseNode<'tree>),
    Tilde(TildeNode<'tree>),
    Plus(PlusNode<'tree>),
}

impl<'tree> From<WordBodyWordSegmentChild0Choice<'tree>> for Piece<'tree> {
    fn from(choice: WordBodyWordSegmentChild0Choice<'tree>) -> Self {
        match choice {
            WordBodyWordSegmentChild0Choice::WordSegment(node) => Piece::Segment(node),
            WordBodyWordSegmentChild0Choice::Shortening(node) => Piece::Shortening(node),
            WordBodyWordSegmentChild0Choice::StressMarker(node) => Piece::Stress(node),
        }
    }
}

impl<'tree> From<WordBodyWordSegmentChild1LengtheningChoice<'tree>> for Piece<'tree> {
    fn from(choice: WordBodyWordSegmentChild1LengtheningChoice<'tree>) -> Self {
        use WordBodyWordSegmentChild1LengtheningChoice as C;
        match choice {
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

impl<'tree> From<WordBodyWordSegmentChild1Choice<'tree>> for Piece<'tree> {
    fn from(choice: WordBodyWordSegmentChild1Choice<'tree>) -> Self {
        match choice {
            WordBodyWordSegmentChild1Choice::WordSegment(node) => Piece::Segment(node),
            WordBodyWordSegmentChild1Choice::Shortening(node) => Piece::Shortening(node),
            WordBodyWordSegmentChild1Choice::StressMarker(node) => Piece::Stress(node),
            WordBodyWordSegmentChild1Choice::Lengthening(marker) => marker.into(),
        }
    }
}

impl<'tree> From<WordBodyOverlapPointChild0Choice<'tree>> for Piece<'tree> {
    fn from(choice: WordBodyOverlapPointChild0Choice<'tree>) -> Self {
        use WordBodyOverlapPointChild0Choice as C;
        match choice {
            C::OverlapPoint(node) => Piece::Overlap(node),
            C::CaElement(node) => Piece::CaElement(node),
            C::CaDelimiter(node) => Piece::CaDelimiter(node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(node),
            C::SyllablePause(node) => Piece::SyllablePause(node),
        }
    }
}

impl<'tree> From<WordBodyOverlapPointChild1Choice<'tree>> for Piece<'tree> {
    fn from(choice: WordBodyOverlapPointChild1Choice<'tree>) -> Self {
        use WordBodyOverlapPointChild1Choice as C;
        match choice {
            C::OverlapPoint(node) => Piece::Overlap(node),
            C::CaElement(node) => Piece::CaElement(node),
            C::CaDelimiter(node) => Piece::CaDelimiter(node),
            C::UnderlineBegin(node) => Piece::UnderlineBegin(node),
            C::SyllablePause(node) => Piece::SyllablePause(node),
        }
    }
}

impl<'tree> From<WordBodyOverlapPointChild2Choice<'tree>> for Piece<'tree> {
    fn from(choice: WordBodyOverlapPointChild2Choice<'tree>) -> Self {
        match choice {
            WordBodyOverlapPointChild2Choice::WordSegment(node) => Piece::Segment(node),
            WordBodyOverlapPointChild2Choice::Shortening(node) => Piece::Shortening(node),
            WordBodyOverlapPointChild2Choice::StressMarker(node) => Piece::Stress(node),
        }
    }
}

impl<'tree> From<WordBodyOverlapPointChild3LengtheningChoice<'tree>> for Piece<'tree> {
    fn from(choice: WordBodyOverlapPointChild3LengtheningChoice<'tree>) -> Self {
        use WordBodyOverlapPointChild3LengtheningChoice as C;
        match choice {
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

impl<'tree> From<WordBodyOverlapPointChild3Choice<'tree>> for Piece<'tree> {
    fn from(choice: WordBodyOverlapPointChild3Choice<'tree>) -> Self {
        match choice {
            WordBodyOverlapPointChild3Choice::WordSegment(node) => Piece::Segment(node),
            WordBodyOverlapPointChild3Choice::Shortening(node) => Piece::Shortening(node),
            WordBodyOverlapPointChild3Choice::StressMarker(node) => Piece::Stress(node),
            WordBodyOverlapPointChild3Choice::Lengthening(marker) => marker.into(),
        }
    }
}

/// One typed body position: its piece, if the position holds one.
pub(super) fn push_slot<'tree, C, M, U, A>(
    slot: &NodeSlot<'tree, C, M, U, A>,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut SmallVec<[WordContent; 2]>,
) where
    C: Clone + Into<Piece<'tree>>,
    U: RecoveryNode<'tree>,
{
    if let Some(choice) = taken(slot, "word piece", "word_body", source, errors) {
        push_piece(choice.clone().into(), source, errors, items);
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
    piece: Piece<'_>,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut SmallVec<[WordContent; 2]>,
) {
    match piece {
        Piece::Segment(node) => {
            let raw = node.raw_node();
            let text = extract_utf8_text(raw, source, errors, "word_segment", "");
            match WordText::new(text) {
                Ok(text) => items.push(WordContent::Text(text)),
                Err(EmptyText) => {
                    report_tree_shape(raw, "Empty word_segment token".to_string(), source, errors)
                }
            }
        }
        Piece::Shortening(node) => {
            // shortening = '(' word_segment ')'
            let inner = extract_shortening(node);
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
            if let Some(segment) = taken(
                inner.child_1.slot(),
                "shortening content",
                "shortening",
                source,
                errors,
            ) {
                let raw = segment.raw_node();
                let text = extract_utf8_text(raw, source, errors, "shortening_content", "");
                match WordShortening::new(text) {
                    Ok(shortening) => items.push(WordContent::Shortening(shortening)),
                    Err(EmptyText) => report_tree_shape(
                        raw,
                        "Empty shortening content".to_string(),
                        source,
                        errors,
                    ),
                }
            }
        }
        Piece::Stress(node) => {
            let raw = node.raw_node();
            let text = extract_utf8_text(raw, source, errors, "stress_marker", "");
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
            let text = extract_utf8_text(raw, source, errors, "lengthening", "");
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
            if let ParseOutcome::Parsed(point) =
                parse_overlap_point_token(node.raw_node(), source, errors)
            {
                items.push(WordContent::OverlapPoint(point));
            }
        }
        Piece::CaElement(node) => {
            if let ParseOutcome::Parsed(element) =
                parse_ca_element_node(node.raw_node(), source, errors)
            {
                items.push(WordContent::CAElement(element));
            }
        }
        Piece::CaDelimiter(node) => {
            if let ParseOutcome::Parsed(delimiter) =
                parse_ca_delimiter_node(node.raw_node(), source, errors)
            {
                items.push(WordContent::CADelimiter(delimiter));
            }
        }
        Piece::UnderlineBegin(node) => {
            items.push(WordContent::UnderlineBegin(WordUnderlineBegin {
                span: span_of(node.raw_node()),
            }))
        }
        Piece::UnderlineEnd(node) => items.push(WordContent::UnderlineEnd(WordUnderlineEnd {
            span: span_of(node.raw_node()),
        })),
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
