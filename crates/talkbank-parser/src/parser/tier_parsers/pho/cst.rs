//! Source-bound CST decoding for `%pho` and `%mod` tiers.

use crate::generated_traversal::{
    AsRawNode, ModDependentTierNode, NoChild, NonMissingKindSlot, PhoDependentTierNode,
    PhoGroupNode, PhoGroupsNode, SourceBound, SourceField, SourceSlotView,
};
use crate::parser::node_span::span_of;
use talkbank_model::ErrorSink;
use talkbank_model::model::dependent_tier::PhoGroupWords;
use talkbank_model::model::{PhoItem, PhoTier, PhoTierType, PhoWord};

use super::groups::{extract_pho_group_items, push_pho_separator};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;

/// Decode a `%pho` tier admitted by its owning parsed source.
pub fn parse_pho_tier<'tree>(
    node: SourceBound<'tree, '_, PhoDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<PhoTier, crate::CstFailure> {
    parse_pho_tier_inner(PhoBodyTier::Pho(node), errors)
}

/// Decode a `%mod` tier admitted by its owning parsed source.
pub fn parse_mod_tier<'tree>(
    node: SourceBound<'tree, '_, ModDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<PhoTier, crate::CstFailure> {
    parse_pho_tier_inner(PhoBodyTier::Mod(node), errors)
}

/// Each tier owns its node and source. The model tag and extractor are derived
/// together; neither a mismatched tier kind nor unrelated text can be supplied.
enum PhoBodyTier<'tree, 'source> {
    Pho(SourceBound<'tree, 'source, PhoDependentTierNode<'tree>>),
    Mod(SourceBound<'tree, 'source, ModDependentTierNode<'tree>>),
}

/// Compiled-grammar admission rules out missing composite bodies; Error/Absent
/// retain the empty-tier policy.
fn parse_pho_tier_inner(
    tier: PhoBodyTier<'_, '_>,
    errors: &impl ErrorSink,
) -> Result<PhoTier, crate::CstFailure> {
    let (tier_type, node, body) = match tier {
        PhoBodyTier::Pho(n) => {
            let children = n.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
            surface_displaced(
                &children.children().unexpected,
                "pho_dependent_tier",
                n.source(),
                errors,
            );
            (
                PhoTierType::Pho,
                n.raw_node(),
                read_body(children.field_child_2().slot())?,
            )
        }
        PhoBodyTier::Mod(n) => {
            let children = n.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
            surface_displaced(
                &children.children().unexpected,
                "mod_dependent_tier",
                n.source(),
                errors,
            );
            (
                PhoTierType::Mod,
                n.raw_node(),
                read_body(children.field_child_2().slot())?,
            )
        }
    };
    let items = match body {
        Some(groups) => parse_pho_groups(groups, errors)?,
        None => Vec::new(),
    };
    Ok(PhoTier::new(tier_type, items).with_span(span_of(node)))
}

/// Read an identified, non-missing composite body. A source
/// admission failure is an internal failure, never an absent CHAT body.
fn read_body<'tree, 'source>(
    slot: SourceField<'_, 'tree, 'source, NonMissingKindSlot<'tree, PhoGroupsNode<'tree>>>,
) -> Result<Option<SourceBound<'tree, 'source, PhoGroupsNode<'tree>>>, crate::CstFailure> {
    match slot.view() {
        SourceSlotView::Present(body) => Ok(Some(body.read()?)),
        SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => Ok(None),
    }
}

/// Traverse the generated first group and repeated separator/group pairs.
fn parse_pho_groups<'tree>(
    typed: SourceBound<'tree, '_, PhoGroupsNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<PhoItem>, crate::CstFailure> {
    let source = typed.source();
    let groups = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let mut items = Vec::with_capacity(groups.field_child_1().slot().iter().len() + 1);
    push_pho_group(groups.field_child_0().slot(), errors, &mut items)?;
    for element in groups.field_child_1().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(pair) => {
                push_pho_separator(pair.field_child_0().slot(), errors, "pho_groups");
                push_pho_group(pair.field_child_1().slot(), errors, &mut items)?;
                for node in pair.field_unexpected().iter() {
                    surface_displaced(&[node.raw_node()], "pho_groups", node.source(), errors);
                }
            }
            SourceSlotView::Error(raw) => {
                errors.report(unexpected_node_error(raw.raw_node(), source, "pho_groups"));
            }
            SourceSlotView::Absent(never) => match never {},
            SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
        }
    }
    surface_displaced(&groups.children().unexpected, "pho_groups", source, errors);
    Ok(items)
}

/// Decode present groups, diagnose Error, and omit absent positions.
/// Compiled-grammar admission excludes missing composite groups.
fn push_pho_group<'tree>(
    slot: SourceField<'_, 'tree, '_, NonMissingKindSlot<'tree, PhoGroupNode<'tree>>>,
    errors: &impl ErrorSink,
    items: &mut Vec<PhoItem>,
) -> Result<(), crate::CstFailure> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(group) => {
            items.extend(extract_pho_group_items(group.read()?, errors)?);
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(raw.raw_node(), source, "pho_groups"));
        }
        SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Unexpected(never) => match never {},
    }
    Ok(())
}

/// Preserve the entire admitted group's text when structural recovery needs it.
pub(crate) fn fallback_group_as_text<'tree>(
    node: SourceBound<'tree, '_, PhoGroupNode<'tree>>,
) -> Vec<PhoItem> {
    let text = node.text();
    if text.is_empty() {
        Vec::new()
    } else {
        vec![PhoItem::Word(PhoWord::new(text))]
    }
}

/// Builds group from words for downstream use.
pub(crate) fn build_group_from_words(words: Vec<&str>) -> Vec<PhoItem> {
    if !words.is_empty() {
        let pho_words: Vec<PhoWord> = words.into_iter().map(PhoWord::new).collect();
        vec![PhoItem::Group(PhoGroupWords::new(pho_words))]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::SourceBindingError;

    /// Direct ownership boundary evidence, not a claim that the valid fixture
    /// selects fallback through the production recovery extractor.
    #[test]
    fn typed_phonology_fallback_preserves_text_from_its_owning_parse() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/tiers/pho-groupings.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let other = parser
            .parse_source_incremental(source, None)
            .expect("independent parse");
        let mut witnessed = 0;
        for node in parsed.root().expect("root").descendants() {
            let node = node.expect("source-bound descendant");
            let Some(group) = node.typed::<PhoGroupNode>() else {
                continue;
            };
            assert_eq!(
                fallback_group_as_text(group),
                vec![PhoItem::Word(PhoWord::new(group.text()))]
            );
            assert!(matches!(
                other.bind(group.raw_node()),
                Err(SourceBindingError::ForeignTree)
            ));
            witnessed += 1;
        }
        assert!(witnessed > 0);
    }
}
