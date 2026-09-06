//! Shared helper functions for header parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::{ErrorSink, Span};
use crate::generated_traversal::{
    AsRawNode, BirthOfHeaderNode, BirthplaceOfHeaderNode, FromNodeKind, HeaderSepNode,
    L1OfHeaderNode, NodeSlot, extract_birth_of_header, extract_birthplace_of_header,
    extract_header_sep, extract_l1_of_header,
};
use crate::model;
use crate::model::TierSeparator;
use crate::node_types::{CONTINUATION, REST_OF_LINE};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

/// Parse optional label text used by `@Bg`, `@Eg`, and `@G` headers.
pub(crate) fn parse_optional_gem_label(
    node: Option<Node>,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<model::GemLabel>> {
    let Some(node) = node else {
        return ParseOutcome::parsed(None);
    };
    let mut cursor = node.walk();
    let mut label = String::new();
    let mut saw_text = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            REST_OF_LINE => {
                if let Ok(text) = child.utf8_text(input.as_bytes())
                    && !text.is_empty()
                {
                    label.push_str(text);
                    saw_text = true;
                }
            }
            CONTINUATION => {
                if saw_text {
                    label.push(' ');
                }
            }
            _ => errors.report(unexpected_node_error(child, input, "gem label")),
        }
    }

    if label.is_empty() {
        ParseOutcome::parsed(None)
    } else {
        ParseOutcome::parsed(Some(model::GemLabel::new(label)))
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
    match trailing {
        Some(NodeSlot::Present(sep))
            if matches!(sep_children.child_1.slot(), NodeSlot::Present(tab)
                if tab.raw_node().end_byte() == sep.raw_node().start_byte()) =>
        {
            let raw = sep.raw_node();
            TierSeparator::with_trailing_space(Span::new(
                raw.start_byte() as u32,
                raw.end_byte() as u32,
            ))
        }
        Some(NodeSlot::Present(_))
        | Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => TierSeparator::CLEAN,
    }
}
