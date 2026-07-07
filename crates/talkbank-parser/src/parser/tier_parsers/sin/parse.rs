//! Parser for `%sin` tier bodies.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::generated_traversal::{
    AsRawNode, NodeSlot, SinDependentTierNode, SinGroupNode, SinGroupsNode,
    extract_sin_dependent_tier, extract_sin_groups,
};
use talkbank_model::model::{SinItem, SinTier};
use talkbank_model::{ErrorSink, Span};
use tree_sitter::Node;

use super::groups::{extract_sin_group_items, push_sin_separator};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_unexpected};

/// Converts one `%sin` tier node into `SinTier`.
///
/// **Grammar Rule:**
/// ```text
/// sin_dependent_tier: seq('%', 'sin', colon, tab, sin_groups, newline)
/// ```
///
/// Driven by the generated typed visitor. `extract_sin_dependent_tier` exposes the
/// body (`sin_groups`) as the typed `child_2.slot`; the removed code LOCATED it by
/// scanning for a child of kind `sin_groups` (NEVER on `node.kind()` any more). The
/// body slot is matched EXHAUSTIVELY over [`NodeSlot`] (no `_` catch-all, no
/// `.ok()`), reproducing the removed hand-walk byte for byte:
///
/// - `Present` / `Missing`: the removed code located the body by kind; a
///   tree-sitter MISSING node retains that kind, so both a real body and a MISSING
///   body were found (the old `Some(sin_groups)` branch) and drive group
///   iteration. A MISSING/empty `sin_groups` yields zero items with no diagnostic,
///   identical to the old loop iterating an empty node. The two arms can no longer
///   share one `|`-pattern binding: the NEW backend's `NodeSlot::Missing` carries
///   the raw `tree_sitter::Node` directly, not the typed `SinGroupsNode` wrapper
///   OLD carried, so `Present` calls [`AsRawNode::raw_node`] while `Missing` passes
///   its raw node straight through; the observable parse is unchanged.
/// - `Absent` / `Error` / `Unexpected`: no child of kind `sin_groups` was found
///   (the old `None` branch): an ERROR node or an unexpected-kind node does not
///   match `sin_groups`, and an absent child is not there at all. Return the EMPTY
///   tier SILENTLY (no diagnostic). This silent-partial is PRESERVED behavior; it
///   is unreachable from the boundary (`parse_sin_tier` is only invoked when the
///   tier node has no tree-sitter error) but is reproduced for exhaustiveness.
pub fn parse_sin_tier(node: Node, source: &str, errors: &impl ErrorSink) -> SinTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);

    let children = extract_sin_dependent_tier(SinDependentTierNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    match children.child_2.slot {
        NodeSlot::Present(groups) => {
            let items = parse_sin_groups(groups.raw_node(), source, errors);
            SinTier::new(items).with_span(span)
        }
        NodeSlot::Missing(raw) => {
            let items = parse_sin_groups(raw, source, errors);
            SinTier::new(items).with_span(span)
        }
        NodeSlot::Absent | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
            SinTier::new(Vec::new()).with_span(span)
        }
    }
}

/// Decode every `sin_group` under a `sin_groups` node into gesture/sign items,
/// driven by the generated `extract_sin_groups` visitor.
///
/// `sin_groups = seq(sin_group, repeat(seq(whitespaces, sin_group)))`, so the
/// visitor exposes the first group as `child_0` and each subsequent
/// `(whitespaces, sin_group)` pair as a `SinGroupsChild1Children` element in
/// `child_1`. This replaces the old `while sin_groups.child(idx)` positional
/// walk. Unlike the OLD backend (built with `--skip whitespaces`), the NEW
/// backend models the separating `whitespaces` token as its own explicit
/// `child_0` position inside each repeat element (`child_1` holds the
/// `sin_group`); that position is purely structural and handled by
/// [`push_sin_separator`].
fn parse_sin_groups(sin_groups: Node, source: &str, errors: &impl ErrorSink) -> Vec<SinItem> {
    let groups = extract_sin_groups(SinGroupsNode(sin_groups));
    let mut items: Vec<SinItem> = Vec::with_capacity(groups.child_1.slot.len() + 1);

    push_sin_group(groups.child_0.slot, source, errors, &mut items);
    for element in groups.child_1.slot {
        match element.slot {
            NodeSlot::Present(pair) => {
                push_sin_separator(pair.child_0.slot, source, errors, "sin_groups");
                push_sin_group(pair.child_1.slot, source, errors, &mut items);
                surface_unexpected(&pair.unexpected, source, errors);
            }
            // The generated repeat classifies a whole item as `Present` /
            // `Error` / `Absent` only (the established `@Languages`/gra/pho
            // repeat finding); matched exhaustively regardless, per the
            // no-`_`-on-project-enums rule.
            NodeSlot::Missing(raw) | NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
                errors.report(unexpected_node_error(raw, source, "sin_groups"));
            }
            NodeSlot::Absent => {}
        }
    }

    surface_unexpected(&groups.unexpected, source, errors);
    items
}

/// Decode one `sin_group` slot, extending `items` with its gesture/sign items.
///
/// The `sin_group` slot is matched EXHAUSTIVELY over [`NodeSlot`] (no `_`
/// catch-all), reproducing the removed per-child loop byte for byte:
///
/// - `Present`: classify the group interior via `extract_sin_group_items` (flat
///   token vs bracketed group) and extend, exactly as the old `SIN_GROUP` arm.
/// - `Missing`: the old loop's `check_not_missing` reported the
///   `MissingRequiredElement` (E342) recovery diagnostic and skipped the child;
///   reproduced here (the returned flag is discarded because the missing child is
///   dropped either way).
/// - `Error` / `Unexpected`: the old loop's `_` arm reported
///   `unexpected_node_error` (ERROR nodes route through the error analyzer);
///   reproduced here.
/// - `Absent`: no child at this position; the old loop simply did not iterate
///   here, so nothing is reported and nothing is pushed.
///
/// The `Missing` / `Error` / `Unexpected` arms are unreachable from the boundary
/// (`parse_sin_tier` is only entered when the tier node has no tree-sitter error);
/// they are handled explicitly for exhaustiveness.
fn push_sin_group<'tree>(
    slot: NodeSlot<'tree, SinGroupNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut Vec<SinItem>,
) {
    match slot {
        NodeSlot::Present(group_node) => {
            items.extend(extract_sin_group_items(
                group_node.raw_node(),
                source,
                errors,
            ));
        }
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "sin_groups");
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "sin_groups"));
        }
        NodeSlot::Absent => {}
    }
}
