//! Shared helper functions for header parsing.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::{ErrorSink, Span};
use crate::generated_traversal::{AsRawNode, HeaderSepNode, NodeSlot, extract_header_sep};
use crate::model;
use crate::model::TierSeparator;
use crate::node_types::{CONTINUATION, HEADER_SEP, REST_OF_LINE};
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
/// Every header grammar rule has the uniform shape
/// `seq(<kind>_prefix, header_sep, <body>, newline)`, so `header_sep` sits at
/// raw named-child index 1 for every header kind, EXCEPT `@Bg`/`@Eg`, whose
/// grammar wraps it as `optional(seq(header_sep, free_text))`: when that
/// whole group is absent, named-child index 1 is `newline`, not
/// `header_sep`, and the kind check below correctly reports a clean
/// separator for that case.
///
/// This is called from the single point (`document_lowering`'s line
/// dispatch, and the pre-`@Begin` header dispatch) that builds a
/// `Line::Header` for a concrete header CST node BEFORE any per-kind typed
/// dispatch runs, so no per-kind typed carrier (e.g. `CommentHeaderChildren`)
/// is available there; reaching a typed `header_sep` therefore requires
/// first locating it by its raw kind. This is PROVENANCE extraction over a
/// grammar position that is uniform across every header rule, never
/// structural dispatch that classifies or drops content (the repo's
/// `node.kind()` ban targets hand-walked parsing, not this kind of
/// positional provenance read). Once located, the actual trailing-space read
/// reuses the same typed accessor `dependent_tier_separator`
/// (`dependent_tier_dispatch/helpers.rs`) uses for tier separators:
/// `extract_header_sep`'s `child_2` slot, never a second `node.kind()` scan.
pub(crate) fn header_separator(header_node: Node) -> TierSeparator {
    let Some(sep_node) = header_node.named_child(1) else {
        return TierSeparator::CLEAN;
    };
    if sep_node.kind() != HEADER_SEP {
        return TierSeparator::CLEAN;
    }
    let trailing = extract_header_sep(HeaderSepNode(sep_node)).child_2.slot;
    match trailing {
        Some(NodeSlot::Present(sep)) => {
            let raw = sep.raw_node();
            TierSeparator::with_trailing_space(Span::new(
                raw.start_byte() as u32,
                raw.end_byte() as u32,
            ))
        }
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => TierSeparator::CLEAN,
    }
}
