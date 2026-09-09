//! Shared helper functions for header parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::{ErrorSink, Span};
use crate::generated_traversal::ChoiceSlot;
use crate::generated_traversal::{
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, FromNodeKind, HeaderSepNode,
    L1OfHeaderNode, NoChild, NodeSlot, SlotView, extract_birth_of_header,
    extract_birthplace_of_header, extract_header_sep, extract_l1_of_header,
};
use crate::generated_traversal::{
    FreeTextChild0Choice, FreeTextChild1Choice, FreeTextNode, RestOfLineNode, extract_free_text,
};
use crate::model;
use crate::model::TierSeparator;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse optional label text used by `@Bg`, `@Eg`, and `@G` headers, from
/// the header's `free_text` node when it has one.
///
/// Grammar: `free_text: repeat1(choice(rest_of_line, continuation))`. The
/// pieces of text are joined, a continuation (a line break and its tab)
/// contributing one space between them once text has been seen; no text
/// at all is no label. Until 2026-09-09 this walked the node's children by
/// `kind()` string, reporting anything else as unexpected; the generated
/// choice names the two kinds, and a piece position holding an ERROR or a
/// displaced node is reported as that catch-all reported it (a MISSING
/// piece is the whole-tree pass's, E342, and contributes no text, as its
/// empty placeholder contributed none before).
pub(crate) fn parse_optional_gem_label(
    node: Option<FreeTextNode<'_>>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<model::GemLabel>> {
    let Some(node) = node else {
        return ParseOutcome::parsed(None);
    };
    let children = extract_free_text(node);
    let mut label = String::new();
    let mut saw_text = false;
    let mut push = |piece: Piece<'_>| match piece {
        Piece::Text(text) => {
            if let Ok(text) = text.raw_node().utf8_text(input.as_bytes())
                && !text.is_empty()
            {
                label.push_str(text);
                saw_text = true;
            }
        }
        Piece::Continuation => {
            if saw_text {
                label.push(' ');
            }
        }
    };
    if let Some(first) = piece(children.child_0.slot(), input, errors) {
        push(first.clone().into());
    }
    for element in children.child_1.slot() {
        if let Some(next) = piece(element.slot(), input, errors) {
            push(next.clone().into());
        }
    }
    surface_displaced(&children.unexpected, "free_text", input, errors);

    if label.is_empty() {
        ParseOutcome::parsed(None)
    } else {
        ParseOutcome::parsed(Some(model::GemLabel::new(label)))
    }
}

/// The piece a `free_text` position holds, if it holds one: an ERROR or a
/// displaced node standing in the position is reported in the gem-label
/// context; a MISSING piece and an absent position yield nothing.
fn piece<'a, 'tree, T>(
    slot: &'a ChoiceSlot<'tree, T>,
    input: &str,
    errors: &impl ErrorSink,
) -> Option<&'a T> {
    match slot.view() {
        SlotView::Present(value) => Some(value),
        SlotView::Error(bad) | SlotView::Unexpected(bad) => {
            errors.report(unexpected_node_error(bad, input, "gem label"));
            None
        }
        SlotView::Missing(_) | SlotView::Absent(NoChild) => None,
    }
}

/// One piece of free text, whichever position it came through.
enum Piece<'tree> {
    /// A run of text to the end of its line.
    Text(RestOfLineNode<'tree>),
    /// A line break and its tab, which the text continues after.
    Continuation,
}

impl<'tree> From<FreeTextChild0Choice<'tree>> for Piece<'tree> {
    fn from(choice: FreeTextChild0Choice<'tree>) -> Self {
        match choice {
            FreeTextChild0Choice::RestOfLine(node) => Piece::Text(node),
            FreeTextChild0Choice::Continuation(_) => Piece::Continuation,
        }
    }
}

impl<'tree> From<FreeTextChild1Choice<'tree>> for Piece<'tree> {
    fn from(choice: FreeTextChild1Choice<'tree>) -> Self {
        match choice {
            FreeTextChild1Choice::RestOfLine(node) => Piece::Text(node),
            FreeTextChild1Choice::Continuation(_) => Piece::Continuation,
        }
    }
}

/// Decode a header line's `header_sep` node (E758 provenance) into a
/// [`TierSeparator`].
///
/// Most headers place `header_sep` after their label. Speaker-qualified
/// headers also carry an optional gap and speaker; their generated carriers
/// identify the separator without assuming a raw child index. A missing or
/// recovered separator carries no proof of trailing whitespace.
///
/// The trailing space must remain adjacent to the same separator's actual
/// tab. Recovery can otherwise put a content space in this optional slot.
pub(crate) fn header_separator(header_node: Node) -> TierSeparator {
    let qualified = if let Some(header) = BirthOfHeaderNode::from_node(header_node) {
        Some(extract_birth_of_header(header).child_3.slot().clone())
    } else if let Some(header) = BirthplaceOfHeaderNode::from_node(header_node) {
        Some(extract_birthplace_of_header(header).child_3.slot().clone())
    } else {
        L1OfHeaderNode::from_node(header_node)
            .map(|header| extract_l1_of_header(header).child_3.slot().clone())
    };
    let sep = match qualified {
        Some(NodeSlot::Present(sep)) => sep,
        Some(_) => return TierSeparator::CLEAN,
        None => {
            let Some(sep) = header_node
                .named_child(1)
                .and_then(HeaderSepNode::from_node)
            else {
                return TierSeparator::CLEAN;
            };
            sep
        }
    };
    let sep_children = extract_header_sep(sep);
    let trailing = sep_children.child_2.slot();
    match trailing.as_ref().map(NodeSlot::view) {
        Some(SlotView::Present(sep))
            if matches!(sep_children.child_1.slot(), NodeSlot::Present(tab)
                if tab.raw_node().end_byte() == sep.raw_node().start_byte()) =>
        {
            let raw = sep.raw_node();
            TierSeparator::with_trailing_space(Span::new(
                raw.start_byte() as u32,
                raw.end_byte() as u32,
            ))
        }
        Some(
            SlotView::Present(_)
            | SlotView::Missing(_)
            | SlotView::Error(_)
            | SlotView::Absent(NoChild),
        )
        | None => TierSeparator::CLEAN,
    }
}
