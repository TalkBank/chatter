//! Per-kind parsing for GEM-style headers (`@Bg`, `@Eg`, `@G`).
//!
//! Each function here is the LEVEL-1 entry for one `HeaderChoice` GEM variant.
//! The label-extraction logic (`parse_gem_label`) and the
//! `Header::BeginGem` / `EndGem` / `LazyGem` construction are byte-identical
//! to the pre-migration string-dispatch arms.
//!
//! LEVEL 2: the optional `free_text` content child is read through the NEW
//! backend's free, typed `extract_<kind>(node)` (replacing the local
//! `find_child_by_kind(header_actual, FREE_TEXT)` scan, and, as of Task B2, the
//! OLD `TypedTraversal.extract_<kind>` trait-receiver call). The NEW backend does
//! not skip whitespace, so `bg_header`/`eg_header` no longer expose the
//! `header_sep` + `free_text` pair as two flat OPTIONAL positions the way the OLD
//! module's `child_2: Option<NodeSlot<FreeTextNode>>` did; they are grouped into
//! ONE optional GROUP position (`child_1: Option<NodeSlot<BgHeaderChild1Children>>`
//! / `EgHeaderChild1Children`, each `{ child_0: header_sep, child_1: free_text }`)
//! because the whole `seq(header_sep, free_text)` pair is what is optional at the
//! grammar level, not `free_text` alone. `g_header` is unaffected (its `free_text`
//! is grammar-REQUIRED, so it stays a flat `child_2: NodeSlot<FreeTextNode>`, same
//! index as the OLD module). In every case the slot is mapped to the
//! `Option<Node>` that `parse_gem_label` / `parse_optional_gem_label` expects,
//! exhaustively, with no `_ =>` arm that silently drops variants: `Present` reads
//! the node (for `Bg`/`Eg`, reached by descending one level into the group's own
//! `child_1`, i.e. the `free_text` member of the pair); all other states map to
//! `None` (the VALID path is byte-identical, the rest are malformed-only).
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, BgHeaderNode, EgHeaderNode, GHeaderNode, extract_bg_header, extract_eg_header,
    extract_g_header,
};
use crate::model::{self, Header};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::ParseOutcome;
use tree_sitter::Node;

use super::super::helpers::parse_optional_gem_label;

/// Parse optional GEM label payload from a pre-extracted `free_text` child node.
///
/// `free_text_child` is the typed optional slot result from
/// `extract_<kind>(header_actual).child_2`, already mapped to `Option<Node>`. If
/// `parse_optional_gem_label` returns `None` and the raw `header_actual` text
/// contains a colon, a fallback extraction reads the text after the colon.
fn parse_gem_label(
    free_text_child: Option<Node>,
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Option<model::GemLabel>> {
    let parsed = match parse_optional_gem_label(free_text_child, input, errors) {
        ParseOutcome::Parsed(label) => label,
        ParseOutcome::Rejected => return ParseOutcome::rejected(),
    };

    if parsed.is_some() {
        return ParseOutcome::parsed(parsed);
    }

    let fallback = header_actual
        .utf8_text(input.as_bytes())
        .ok()
        .and_then(|raw| {
            let colon = raw.find(':')?;
            let label = raw[colon + 1..].trim_matches(|c| matches!(c, '\r' | '\n' | '\t' | ' '));
            if label.is_empty() {
                None
            } else {
                Some(model::GemLabel::new(label.to_string()))
            }
        });
    ParseOutcome::parsed(fallback)
}

/// `@Bg` -> `Header::BeginGem` (always, even when malformed).
///
/// A `@Bg` input NEVER produces `Header::LazyGem`; only `@G` does. An empty or
/// malformed label (e.g. `@Bg:` with no label text) yields `BeginGem { label: None }`.
/// Any parse-time diagnostic for the malformed input (E316 for an unparsable `:`,
/// E342 for a missing required element) is emitted by the tree-sitter backstop and
/// is orthogonal to the model kind.
pub(super) fn bg(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // LEVEL 2: read the optional `seq(header_sep, free_text)` GROUP through the
    // typed positional slot (extract_bg_header child_1:
    // Option<NodeSlot<BgHeaderChild1Children>>), then descend one level into the
    // group's own `child_1` (the free_text member) via `present_or_recover().ok()`
    // at each level. Every non-Present state at EITHER level (and the outer None,
    // meaning the whole group is grammar-absent) collapses to None, mapping to the
    // Option<Node> parse_gem_label expects; the VALID path is byte-identical to
    // the old `child_2.map(|w| w.raw_node())`.
    let children = extract_bg_header(BgHeaderNode(header_actual));
    let group = children
        .child_1
        .slot
        .and_then(|s| s.present_or_recover().ok());
    let free_text_child = group
        .as_ref()
        .and_then(|group| group.child_1.slot.clone().present_or_recover().ok())
        .map(|w| w.raw_node());
    let outcome = match parse_gem_label(free_text_child, header_actual, input, errors) {
        ParseOutcome::Parsed(label) => ParseOutcome::parsed(Header::BeginGem { label }),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    };
    if let Some(group) = &group {
        surface_unexpected(&group.unexpected, input, errors);
    }
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@Eg` -> `Header::EndGem`.
pub(super) fn eg(
    header_actual: Node,
    input: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<Header> {
    // Same optional-GROUP shape as `bg` (see its comment); `eg_header`'s pair is
    // `EgHeaderChild1Children { child_0: header_sep, child_1: free_text }`.
    let children = extract_eg_header(EgHeaderNode(header_actual));
    let group = children
        .child_1
        .slot
        .and_then(|s| s.present_or_recover().ok());
    let free_text_child = group
        .as_ref()
        .and_then(|group| group.child_1.slot.clone().present_or_recover().ok())
        .map(|w| w.raw_node());
    let outcome = match parse_gem_label(free_text_child, header_actual, input, errors) {
        ParseOutcome::Parsed(label) => ParseOutcome::parsed(Header::EndGem { label }),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    };
    if let Some(group) = &group {
        surface_unexpected(&group.unexpected, input, errors);
    }
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}

/// `@G` -> `Header::LazyGem`.
pub(super) fn g(header_actual: Node, input: &str, errors: &impl ErrorSink) -> ParseOutcome<Header> {
    // LEVEL 2: read the required free_text child through the typed positional slot
    // (extract_g_header child_2: NodeSlot<FreeTextNode>, the SAME index as the OLD
    // module: g_header's free_text is grammar-required, so there is no optional
    // group to descend into here). `present_or_recover().ok()` maps Present to the
    // node and every non-Present recovery variant to None, the Option<Node>
    // parse_gem_label expects; the VALID path is byte-identical to the old read.
    let children = extract_g_header(GHeaderNode(header_actual));
    let free_text_child = children
        .child_2
        .slot
        .present_or_recover()
        .ok()
        .map(|w| w.raw_node());
    let outcome = match parse_gem_label(free_text_child, header_actual, input, errors) {
        ParseOutcome::Parsed(label) => ParseOutcome::parsed(Header::LazyGem { label }),
        ParseOutcome::Rejected => ParseOutcome::rejected(),
    };
    surface_unexpected(&children.unexpected, input, errors);
    outcome
}
