//! Shared helper functions for header parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::{ErrorSink, Span};
use crate::generated_traversal::{Absence, Never};

use crate::generated_traversal::{
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, FromNodeKind, HeaderSepNode,
    L1OfHeaderNode, NodeSlot, SlotView, SourceBound, SourceBoundKind, SourceField, SourceSlotView,
    extract_birth_of_header, extract_birthplace_of_header, extract_header_sep,
    extract_l1_of_header,
};
use crate::generated_traversal::{
    FreeTextChild0Choice, FreeTextChild0ChoiceBoundView, FreeTextChild1Choice,
    FreeTextChild1ChoiceBoundView, FreeTextNode,
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
pub(crate) fn parse_optional_gem_label<'tree>(
    node: Option<SourceBound<'tree, '_, FreeTextNode<'tree>>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<model::GemLabel>> {
    let Some(node) = node else {
        return ParseOutcome::parsed(None);
    };
    let input = node.source();
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        node.extract(),
        node.raw_node(),
        input,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let mut label = String::new();
    let mut saw_text = false;
    let mut push = |piece: Piece<'_>| match piece {
        Piece::Text(text) => {
            if !text.is_empty() {
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
    let ParseOutcome::Parsed(first) = piece(children.field_child_0().slot(), errors) else {
        return ParseOutcome::Rejected;
    };
    if let Some(first) = first {
        push(first.into());
    }
    for element in children.field_child_1().slot().iter() {
        let ParseOutcome::Parsed(next) = piece(element.slot(), errors) else {
            return ParseOutcome::Rejected;
        };
        if let Some(next) = next {
            push(next.into());
        }
    }
    for displaced in children.field_unexpected().iter() {
        surface_displaced(&[displaced.raw_node()], "free_text", input, errors);
    }

    if label.is_empty() {
        ParseOutcome::parsed(None)
    } else {
        ParseOutcome::parsed(Some(model::GemLabel::new(label)))
    }
}

/// The piece a `free_text` position holds, if it holds one: an ERROR
/// standing in the position is reported in the gem-label
/// context; a MISSING piece and an absent position yield nothing.
fn piece<'a, 'tree: 'a, 'source: 'a, T: SourceBoundKind<'tree> + 'a, A: Absence>(
    slot: SourceField<'a, 'tree, 'source, NodeSlot<'tree, T, tree_sitter::Node<'tree>, Never, A>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<SourceBound<'tree, 'source, T>>> {
    match slot.view() {
        SourceSlotView::Present(value) => {
            match crate::parser::typed_cst::read_source_field(value, errors) {
                Some(bound) => ParseOutcome::Parsed(Some(bound)),
                None => ParseOutcome::Rejected,
            }
        }
        SourceSlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                bad.source(),
                "gem label",
            ));
            ParseOutcome::Parsed(None)
        }
        SourceSlotView::Missing(_) | SourceSlotView::Absent(_) => ParseOutcome::Parsed(None),
        SourceSlotView::Unexpected(never) => match never {},
    }
}

/// One piece of free text, whichever position it came through.
enum Piece<'source> {
    /// A run of text to the end of its line.
    Text(&'source str),
    /// A line break and its tab, which the text continues after.
    Continuation,
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, FreeTextChild0Choice<'tree>>>
    for Piece<'source>
{
    fn from(choice: SourceBound<'tree, 'source, FreeTextChild0Choice<'tree>>) -> Self {
        match choice.view() {
            FreeTextChild0ChoiceBoundView::RestOfLine(node) => Piece::Text(node.text()),
            FreeTextChild0ChoiceBoundView::Continuation(_) => Piece::Continuation,
        }
    }
}

impl<'tree, 'source> From<SourceBound<'tree, 'source, FreeTextChild1Choice<'tree>>>
    for Piece<'source>
{
    fn from(choice: SourceBound<'tree, 'source, FreeTextChild1Choice<'tree>>) -> Self {
        match choice.view() {
            FreeTextChild1ChoiceBoundView::RestOfLine(node) => Piece::Text(node.text()),
            FreeTextChild1ChoiceBoundView::Continuation(_) => Piece::Continuation,
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
pub(crate) fn header_separator(
    header_node: Node,
) -> Result<TierSeparator, crate::generated_traversal::ReconstructionFault> {
    let qualified = if let Some(header) = BirthOfHeaderNode::from_node(header_node) {
        Some(extract_birth_of_header(header)?.child_3.slot().clone())
    } else if let Some(header) = BirthplaceOfHeaderNode::from_node(header_node) {
        Some(extract_birthplace_of_header(header)?.child_3.slot().clone())
    } else {
        L1OfHeaderNode::from_node(header_node)
            .map(|header| {
                extract_l1_of_header(header).map(|children| children.child_3.slot().clone())
            })
            .transpose()?
    };
    let sep = match qualified {
        Some(NodeSlot::Present(sep)) => sep,
        Some(_) => return Ok(TierSeparator::CLEAN),
        None => {
            let Some(sep) = header_node
                .named_child(1)
                .and_then(HeaderSepNode::from_node)
            else {
                return Ok(TierSeparator::CLEAN);
            };
            sep
        }
    };
    let sep_children = extract_header_sep(sep)?;
    let trailing = sep_children.child_2.slot();
    Ok(match trailing.as_ref().map(NodeSlot::view) {
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
        Some(SlotView::Present(_) | SlotView::Missing(_) | SlotView::Error(_)) | None => {
            TierSeparator::CLEAN
        }
    })
}

#[cfg(test)]
mod gem_admission_tests {
    use super::*;

    #[test]
    fn gem_label_requires_its_parse_owner() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/edge-cases/postcodes-and-gems.cha"
        ));
        let parser = crate::TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("reference");
        let other = parser
            .parse_source_incremental(source, None)
            .expect("independent reference parse");
        let mut pending = vec![parsed.root_node()];
        let mut witnessed = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(text) = FreeTextNode::from_node(node) else {
                continue;
            };
            let errors = talkbank_model::ErrorCollector::new();
            let bound = parsed.bind_typed(text).expect("owning parse");
            assert!(parse_optional_gem_label(Some(bound), &errors).is_some());
            assert!(errors.into_vec().is_empty());
            assert!(matches!(
                other.bind_typed(text),
                Err(crate::generated_traversal::SourceBindingError::ForeignTree)
            ));
            witnessed += 1;
        }
        assert!(witnessed > 0);
    }
}
