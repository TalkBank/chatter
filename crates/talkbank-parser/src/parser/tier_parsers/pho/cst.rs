//! CST-driven parsing for `%pho` and `%mod` tiers.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

use crate::generated_traversal::{
    AsRawNode, ModDependentTierNode, NodeSlot, PhoDependentTierNode, PhoGroupNode, PhoGroupsNode,
    extract_mod_dependent_tier, extract_pho_dependent_tier, extract_pho_groups,
};
use crate::parser::node_span::span_of;
use talkbank_model::model::dependent_tier::PhoGroupWords;
use talkbank_model::model::{PhoItem, PhoTier, PhoTierType, PhoWord};
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

use super::groups::{extract_pho_group_items, push_pho_separator};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_unexpected};

/// Parse a `%pho` tier from a tree-sitter node.
///
/// **Grammar Rule:**
/// ```text
/// pho_dependent_tier: seq('%', 'pho', colon, tab, pho_groups, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. 'pho' (position 1)
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. pho_groups (position 4)
/// 6. newline (position 5)
pub fn parse_pho_tier(
    node: PhoDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> PhoTier {
    parse_pho_tier_inner(PhoBodyTier::Pho(node), source, errors)
}

/// Parse a `%mod` tier from a tree-sitter node.
///
/// **Grammar Rule:**
/// ```text
/// mod_dependent_tier: seq('%', 'mod', colon, tab, pho_groups, newline)
/// ```
///
/// **Expected Sequential Order:**
/// 1. '%' (position 0)
/// 2. 'mod' (position 1)
/// 3. colon (position 2)
/// 4. tab (position 3)
/// 5. pho_groups (position 4)
/// 6. newline (position 5)
pub fn parse_mod_tier(
    node: ModDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> PhoTier {
    parse_pho_tier_inner(PhoBodyTier::Mod(node), source, errors)
}

/// The two tiers whose body is a `pho_groups` node, each carrying its OWN node.
///
/// `parse_pho_tier_inner` used to take `(node: Node, tier_type: PhoTierType)`,
/// two parameters that had to AGREE and that nothing held together:
/// `parse_pho_tier_inner(a_mod_node, PhoTierType::Pho)` type-checked and would
/// have run the `%pho` extractor over a `%mod` carrier. The enum makes the tier
/// type and the node ONE value, so both the `extract_*` to call and the
/// [`PhoTierType`] to stamp are DERIVED from it and cannot disagree.
///
/// Concretely deleted: the `tier_type` parameter, and the comment that had to
/// explain that the extractor is chosen "for the tier type (never on
/// `node.kind()`)". There is nothing left to choose wrongly.
enum PhoBodyTier<'tree> {
    /// A `%pho` tier.
    Pho(PhoDependentTierNode<'tree>),
    /// A `%mod` tier.
    Mod(ModDependentTierNode<'tree>),
}

// Note: %xpho is a user-defined tier type and should be stored as unparsed.
// It is NOT treated as a real phonological tier type in the data model.
// The treesitter.rs parser correctly handles %xpho as an unparsed tier.

/// Shared implementation for `%pho` and `%mod` tier parsing.
///
/// Driven by the generated typed visitor. `%pho` and `%mod` share the same body
/// grammar (`pho_groups`), so both `extract_pho_dependent_tier` and
/// `extract_mod_dependent_tier` expose the body as the typed `child_2.slot`; the
/// choice between the two extractors is made on the typed [`PhoTierType`], NEVER
/// on `node.kind()`. The body slot is matched EXHAUSTIVELY over [`NodeSlot`] (no
/// `_` catch-all, no `.ok()`), reproducing the removed hand-walk byte for byte:
///
/// - `Present` / `Missing`: the removed code LOCATED the body by scanning for a
///   child of kind `pho_groups`; a tree-sitter MISSING node retains that kind, so
///   both a real body and a MISSING body were found (the old `Some(pho_groups)`
///   branch) and drive group iteration. A MISSING/empty `pho_groups` yields zero
///   items with no diagnostic, identical to the old loop iterating an empty node.
///   Both are reached through `NodeSlot::node_or_placeholder`, which answers for
///   exactly the two states where the position identifies itself. (This
///   paragraph used to explain why the two had to be written as separate arms.
///   That was true of the backend at the time and is no longer: the generator
///   now owns the question.)
/// - `Absent` / `Error` / `Unexpected`: no child of kind `pho_groups` was found
///   (the old `None` branch): an ERROR node or an unexpected-kind node does not
///   match `pho_groups`, and an absent child is not there at all. Return the EMPTY
///   tier SILENTLY (no diagnostic). This silent-partial is PRESERVED behavior; it
///   is unreachable from the boundary (`parse_pho_tier_inner` is only invoked when
///   the tier node has no tree-sitter error) but is reproduced for exhaustiveness.
fn parse_pho_tier_inner(tier: PhoBodyTier<'_>, source: &str, errors: &impl ErrorSink) -> PhoTier {
    // ONE match. Both carriers expose the body at `child_2` and their own
    // top-level `unexpected` sink, and `child_2.slot`'s type is the same
    // `NodeSlot<PhoGroupsNode>` for `%pho` and `%mod`, so the only thing that
    // varies is which `extract_*` reads it and which model tag it carries. Those
    // were three separate matches on the same two-variant enum, which is three
    // chances for the arms to disagree about what `Pho` means.
    let (tier_type, node, body_slot, unexpected): (
        PhoTierType,
        tree_sitter::Node<'_>,
        NodeSlot<PhoGroupsNode>,
        Vec<tree_sitter::Node>,
    ) = match tier {
        PhoBodyTier::Pho(n) => {
            let raw = n.raw_node();
            let children = extract_pho_dependent_tier(n);
            (
                PhoTierType::Pho,
                raw,
                children.child_2.slot().clone(),
                children.unexpected,
            )
        }
        PhoBodyTier::Mod(n) => {
            let raw = n.raw_node();
            let children = extract_mod_dependent_tier(n);
            (
                PhoTierType::Mod,
                raw,
                children.child_2.slot().clone(),
                children.unexpected,
            )
        }
    };
    let span = span_of(node);
    surface_unexpected(&unexpected, source, errors);

    // The two-state reading: every way of NOT having a `pho_groups` node yields
    // the empty tier, so naming the four separately was four arms saying one
    // thing. `present_or_placeholder` discards WHICH state occurred, which is
    // sound only because nothing here branches on it.
    match body_slot.typed_or_placeholder().present_or_placeholder() {
        Some(groups) => {
            let items = parse_pho_groups(groups, source, errors);
            PhoTier::new(tier_type, items).with_span(span)
        }
        None => PhoTier::new(tier_type, Vec::new()).with_span(span),
    }
}

/// Decode every `pho_group` under a `pho_groups` node into phonology items,
/// driven by the generated `extract_pho_groups` visitor.
///
/// `pho_groups = seq(pho_group, repeat(seq(whitespaces, pho_group)))`, so the
/// visitor exposes the first group as `child_0` and each subsequent
/// `(whitespaces, pho_group)` pair as a `PhoGroupsChild1Children` element in
/// `child_1`. This replaces the old `while pho_groups.child(idx)` positional
/// walk. Unlike the OLD backend (built with `--skip whitespaces`), the NEW
/// backend models the separating `whitespaces` token as its own explicit
/// `child_0` position inside each repeat element (`child_1` holds the
/// `pho_group`); that position is purely structural and handled by
/// [`push_pho_separator`].
fn parse_pho_groups(
    typed: PhoGroupsNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<PhoItem> {
    let groups = extract_pho_groups(typed);
    let mut items: Vec<PhoItem> = Vec::with_capacity(groups.child_1.slot().len() + 1);

    push_pho_group(groups.child_0.slot(), source, errors, &mut items);
    for element in groups.child_1.slot() {
        match element.slot() {
            NodeSlot::Present(pair) => {
                push_pho_separator(pair.child_0.slot(), source, errors, "pho_groups");
                push_pho_group(pair.child_1.slot(), source, errors, &mut items);
                surface_unexpected(&pair.unexpected, source, errors);
            }
            // The generated repeat classifies a whole item as `Present` /
            // `Error` / `Absent` only (the established `@Languages`/gra repeat
            // finding); matched exhaustively regardless, per the no-`_`-on-
            // project-enums rule.
            NodeSlot::Missing(raw) | NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
                errors.report(unexpected_node_error(*raw, source, "pho_groups"));
            }
            NodeSlot::Absent => {}
        }
    }

    surface_unexpected(&groups.unexpected, source, errors);
    items
}

/// Decode one `pho_group` slot, extending `items` with its phonology items.
///
/// The `pho_group` slot is matched EXHAUSTIVELY over [`NodeSlot`] (no `_`
/// catch-all), reproducing the removed per-child loop byte for byte:
///
/// - `Present`: classify the group interior via `extract_pho_group_items` (flat
///   words vs bracketed group) and extend, exactly as the old `PHO_GROUP` arm.
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
/// (`parse_pho_tier_inner` is only entered when the tier node has no tree-sitter
/// error); they are handled explicitly for exhaustiveness.
fn push_pho_group<'tree>(
    slot: &NodeSlot<'tree, PhoGroupNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut Vec<PhoItem>,
) {
    match slot {
        NodeSlot::Present(group_node) => {
            items.extend(extract_pho_group_items(*group_node, source, errors));
        }
        NodeSlot::Missing(raw) => {
            check_not_missing(*raw, source, errors, "pho_groups");
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(*raw, source, "pho_groups"));
        }
        NodeSlot::Absent => {}
    }
}

/// Build a fallback `PhoWord` item from raw group text when detailed parsing fails.
pub(crate) fn fallback_group_as_text(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<PhoItem> {
    match node.utf8_text(source.as_bytes()) {
        Ok(text) if !text.is_empty() => {
            vec![PhoItem::Word(PhoWord::new(text))]
        }
        Ok(_) => vec![],
        Err(err) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), "pho_group"),
                format!("Invalid UTF-8 in %pho group fallback text: {}", err),
            ));
            vec![]
        }
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
