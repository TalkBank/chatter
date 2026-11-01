//! Shared helper functions for header parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>

use crate::generated_traversal::{AsRawNode, NodeSlot, SlotView};
use crate::model::TierSeparator;
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::find_header_separator;
use tree_sitter::Node;

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
pub(crate) fn header_separator(header_node: Node) -> Result<TierSeparator, crate::CstFailure> {
    let Some(sep) = find_header_separator(header_node) else {
        return Ok(TierSeparator::CLEAN);
    };
    let sep_children = crate::parser::typed_cst::extract_header_separator(sep)?;
    let trailing = sep_children.child_2.slot();
    Ok(match trailing.as_ref().map(NodeSlot::view) {
        Some(SlotView::Present(sep))
            if matches!(sep_children.child_1.slot(), NodeSlot::Present(tab)
                if tab.raw_node().end_byte() == sep.raw_node().start_byte()) =>
        {
            let raw = sep.raw_node();
            TierSeparator::with_trailing_space(span_of(raw))
        }
        Some(SlotView::Present(_) | SlotView::Missing(_) | SlotView::Error(_)) | None => {
            TierSeparator::CLEAN
        }
    })
}
