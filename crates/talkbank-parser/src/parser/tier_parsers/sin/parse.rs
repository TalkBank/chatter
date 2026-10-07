//! Parser for `%sin` tier bodies.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::generated_traversal::{
    AsRawNode, NoChild, NonMissingKindSlot, SinDependentTierNode, SinGroupNode, SinGroupsNode,
    SourceBound, SourceField, SourceSlotView,
};
use crate::parser::node_span::span_of;
use talkbank_model::ErrorSink;
use talkbank_model::model::{SinItem, SinTier};

use super::groups::{extract_sin_group_items, push_sin_separator};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;

/// Converts one `%sin` tier node into `SinTier`.
///
/// **Grammar Rule:**
/// ```text
/// sin_dependent_tier: seq('%', 'sin', colon, tab, sin_groups, newline)
/// ```
///
/// Source-bound extraction retains ownership through body, groups and tokens.
/// Compiled-grammar admission excludes missing composite bodies; Error/Absent
/// retain the existing empty-tier policy. Unexpected is uninhabited in the slot.
/// Source/reconstruction failures propagate as `CstFailure` rather than empty
/// content. Source ownership does not prove remaining recovery impossible.
pub fn parse_sin_tier<'tree>(
    typed: SourceBound<'tree, '_, SinDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<SinTier, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let span = span_of(node);

    let children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    surface_displaced(
        &children.children().unexpected,
        "sin_dependent_tier",
        source,
        errors,
    );

    Ok(match children.field_child_2().slot().view() {
        SourceSlotView::Present(groups) => {
            let items = parse_sin_groups(groups.read()?, errors)?;
            SinTier::new(items).with_span(span)
        }
        SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => {
            SinTier::new(Vec::new()).with_span(span)
        }
    })
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
fn parse_sin_groups<'tree>(
    typed: SourceBound<'tree, '_, SinGroupsNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<SinItem>, crate::CstFailure> {
    let source = typed.source();
    let groups = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let mut items: Vec<SinItem> =
        Vec::with_capacity(groups.field_child_1().slot().iter().len() + 1);

    push_sin_group(groups.field_child_0().slot(), errors, &mut items)?;
    for element in groups.field_child_1().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(pair) => {
                push_sin_separator(pair.field_child_0().slot(), errors, "sin_groups");
                push_sin_group(pair.field_child_1().slot(), errors, &mut items)?;
                for node in pair.field_unexpected().iter() {
                    surface_displaced(&[node.raw_node()], "sin_groups", node.source(), errors);
                }
            }
            // An inline sequence is never MISSING or displaced; `SeqSlot` says so.
            SourceSlotView::Error(raw) => {
                errors.report(unexpected_node_error(raw.raw_node(), source, "sin_groups"));
            }
            SourceSlotView::Absent(never) => match never {},
            SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
        }
    }

    surface_displaced(&groups.children().unexpected, "sin_groups", source, errors);
    Ok(items)
}

/// Decode one `sin_group` slot, extending `items` with its gesture/sign items.
///
/// Grammar admission rules out missing composite groups, not Error or Absent.
/// Present groups retain flat/grouped decoding; Error is diagnosed and Absent
/// emits nothing. Source failures propagate independently of CHAT recovery.
fn push_sin_group<'tree>(
    slot: SourceField<'_, 'tree, '_, NonMissingKindSlot<'tree, SinGroupNode<'tree>>>,
    errors: &impl ErrorSink,
    items: &mut Vec<SinItem>,
) -> Result<(), crate::CstFailure> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(group_node) => {
            items.extend(extract_sin_group_items(group_node.read()?, errors)?);
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(raw.raw_node(), source, "sin_groups"));
        }
        SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Unexpected(never) => match never {},
    }
    Ok(())
}
