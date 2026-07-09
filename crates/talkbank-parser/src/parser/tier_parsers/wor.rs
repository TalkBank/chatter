//! Word timing tier (%wor) parser
//!
//! Parses %wor tiers which contain word-level timing annotations.
//!
//! The grammar gives %wor its own `wor_tier_body` rule containing a flat
//! whitespace-separated sequence of `wor_word_item` (standalone words),
//! `bullet` (timing), and tag-marker separators. Words and bullets are
//! siblings; the parser pairs each word with its following bullet.
//!
//! Driven by the generated typed visitor. `extract_wor_dependent_tier` exposes the
//! body (`wor_tier_body`) as the typed `child_2.slot`; `extract_wor_tier_body` then
//! exposes the body's four ordered fields as typed `Positioned` slots: the optional
//! `language_code` (a NESTED `(langcode, whitespaces)` group, since the grammar
//! makes the whole pair optional), the item repeat (`child_1`, each element a
//! `(choice, whitespaces)` pair whose `child_0` carries the typed
//! [`WorTierBodyChild1Child0Choice`]), the optional `terminator` supertype
//! (`child_2`, decoded via the SHARED [`terminator_from_new_choice`]), and the
//! required `newline` (`child_3`). Every slot is matched EXHAUSTIVELY over
//! [`NodeSlot`], so a recovery node is handled explicitly rather than silently
//! dropped, and there is NO `node.kind()` string dispatch and NO positional
//! `node.child(idx)` tier-structure hand-walk. The single `child(0)` in the
//! `WorWordItem` arm is not dispatch: it is the fixed 1:1 unwrap of the
//! `wor_word_item` alias wrapper (`wor_word_item: $ => $.standalone_word`, which has
//! no generated `extract_*`) that hands the inner `standalone_word` to the existing
//! `convert_word_node` subsystem, exactly as the removed code did.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::generated_traversal::{
    AsRawNode, LangcodeNode, NodeSlot, WhitespacesNode, WorDependentTierNode,
    WorTierBodyChild1Child0Choice, WorTierBodyNode, extract_wor_dependent_tier,
    extract_wor_tier_body,
};
use talkbank_model::ErrorSink;
use talkbank_model::model::Bullet;
use talkbank_model::model::dependent_tier::{WorItem, WorTier};
use tree_sitter::Node;

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::main_tier::structure::terminator::terminator_from_new_choice;
use crate::parser::tree_parsing::main_tier::word::convert_word_node;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_unexpected};
use talkbank_model::ParseOutcome;

/// Converts `%wor` into a `WorTier`.
///
/// The CST body (`wor_tier_body`) is a flat sequence:
///   `langcode? (wor_word_item | bullet | comma | tag_marker | vocative_marker)*
///    terminator? newline`
/// Words and bullets are siblings; each word is paired with the immediately
/// following `bullet` (if any) by attaching it to the last-pushed word item.
///
/// Driven by the generated typed visitor. `extract_wor_dependent_tier` exposes the
/// body as the typed `child_2` slot; the removed code LOCATED it by scanning for a
/// child of kind `wor_tier_body`. The body slot is matched EXHAUSTIVELY over
/// [`NodeSlot`] (no `_` catch-all, no `.ok()`), reproducing the removed hand-walk
/// byte for byte:
///
/// - `Present` / `Missing`: the removed code located the body by kind; a
///   tree-sitter MISSING node retains that kind, so both a real body and a MISSING
///   body were found (the old `Some(body)` branch) and drive item iteration. An
///   empty (but present) `wor_tier_body` yields an empty tier, identical to the old
///   loop iterating a body with only a newline child. The two arms can no longer
///   share one `|`-pattern binding: the NEW backend's `NodeSlot::Missing` carries
///   the raw `tree_sitter::Node` directly, not the typed `WorTierBodyNode` wrapper
///   OLD carried, so `Present` calls [`AsRawNode::raw_node`] while `Missing` passes
///   its raw node straight through; the observable parse is unchanged.
/// - `Absent` / `Error` / `Unexpected`: no child of kind `wor_tier_body` was found
///   (the old `None` branch): return the EMPTY tier SILENTLY (no diagnostic). This
///   silent-partial is PRESERVED; it is unreachable from the boundary
///   (`parse_wor_tier` is only invoked when the tier node has no tree-sitter error)
///   but is reproduced for exhaustiveness.
pub fn parse_wor_tier(node: Node, source: &str, errors: &impl ErrorSink) -> WorTier {
    let span = talkbank_model::Span::new(node.start_byte() as u32, node.end_byte() as u32);

    let children = extract_wor_dependent_tier(WorDependentTierNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    match children.child_2.slot {
        NodeSlot::Present(body) => {
            parse_wor_tier_body(body.raw_node(), source, errors).with_span(span)
        }
        NodeSlot::Missing(raw) => parse_wor_tier_body(raw, source, errors).with_span(span),
        NodeSlot::Absent | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
            WorTier::new(Vec::new()).with_span(span)
        }
    }
}

/// Decode the `wor_tier_body` node into a `WorTier` (langcode, items, terminator),
/// driven by the generated `extract_wor_tier_body` visitor.
///
/// Each of the four typed fields is handled explicitly; the returned tier has no
/// span yet (the caller attaches the dep-tier span).
fn parse_wor_tier_body(body: Node, source: &str, errors: &impl ErrorSink) -> WorTier {
    let children = extract_wor_tier_body(WorTierBodyNode(body));
    surface_unexpected(&children.unexpected, source, errors);

    // `language_code` (optional): reproduce the old LANGCODE arm. Unlike the OLD
    // backend's flat `Option<NodeSlot<LangcodeNode>>`, the NEW backend groups the
    // whole grammar-optional `(langcode, whitespaces)` pair into one NESTED
    // carrier (`WorTierBodyLanguageCodeChildren`), because that pair together is
    // what is optional, not `langcode` alone (the B2 nested-group precedent).
    // Descend one level to the `langcode` slot (`child_0`); only a `Present`
    // langcode contributes a code, every other state (a malformed group, an absent
    // group) yields none, exactly as the old loop only acted on a real
    // `LANGCODE`-kind child. Surface the nested group's own `unexpected` sink (R2).
    let language_code = match children.language_code.slot {
        Some(NodeSlot::Present(group)) => {
            surface_unexpected(&group.unexpected, source, errors);
            match group.child_0.slot {
                NodeSlot::Present(langcode) => extract_langcode(langcode, source),
                NodeSlot::Missing(_)
                | NodeSlot::Error(_)
                | NodeSlot::Unexpected(_)
                | NodeSlot::Absent => None,
            }
        }
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => None,
    };

    // Item repeat (`child_1`): each element is a `(choice, whitespaces)` pair, so
    // the item is `pair.child_0` (the typed choice) and `pair.child_1` is the
    // trailing separator whitespace, handled by `push_wor_separator` (mirrors the
    // gra/pho/sin separator helpers; the NEW backend models it explicitly since it
    // does not use `--skip whitespaces`). Iterate the typed elements, pairing each
    // bullet with its preceding word.
    let mut items: Vec<WorItem> = Vec::with_capacity(children.child_1.slot.len());
    for element in children.child_1.slot {
        match element.slot {
            NodeSlot::Present(pair) => {
                push_wor_item(pair.child_0.slot, source, errors, &mut items);
                push_wor_separator(pair.child_1.slot, source, errors, "wor_tier_body");
                surface_unexpected(&pair.unexpected, source, errors);
            }
            // The generated repeat classifies a whole item as `Present` / `Error`
            // / `Absent` only (the established `@Languages`/gra/pho/sin repeat
            // finding); matched exhaustively regardless, per the no-`_`-on-project-
            // enums rule. Unreachable from the boundary.
            NodeSlot::Missing(raw) | NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
                errors.report(unexpected_node_error(raw, source, "wor_tier_body"));
            }
            NodeSlot::Absent => {}
        }
    }

    // `child_2` (`terminator` supertype, optional, previously UNCONSUMED): a
    // `Present` choice maps through the SHARED exhaustive `terminator_from_new_choice`
    // (the NEW-backend twin; wor's terminator is `WorTierBodyChild2Choice`). `None`
    // (absent from the source), `Missing`, `Error`, or `Unexpected` yield no
    // terminator, matching the old behavior when no terminator child was seen.
    let terminator = match children.child_2.slot {
        Some(NodeSlot::Present(choice)) => Some(terminator_from_new_choice(&choice)),
        Some(
            NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent,
        )
        | None => None,
    };

    // `child_3` (`newline`, required): structural only, no model representation.
    // Every slot state is a no-op, matched explicitly so the required newline slot
    // is never silently dropped (as the old `NEWLINE => {}` arm did).
    match children.child_3.slot {
        NodeSlot::Present(_)
        | NodeSlot::Missing(_)
        | NodeSlot::Error(_)
        | NodeSlot::Unexpected(_)
        | NodeSlot::Absent => {}
    }

    WorTier::new(items)
        .with_terminator(terminator)
        .with_language_code(language_code)
}

/// Handle the separating `whitespaces` token trailing each `wor_tier_body`
/// item-repeat pair.
///
/// A NEW position with no OLD counterpart: the OLD backend was generated with
/// `--skip whitespaces`, so the space after each word/bullet/marker item was
/// never a modeled child at all. It carries no content, so `Present` is a
/// no-op; the recovery arms reuse the SAME diagnostic vocabulary the sibling
/// item-slot handling uses (`check_not_missing` / `unexpected_node_error`),
/// mirroring the gra/pho/sin separator helpers (`push_gra_separator` /
/// `push_pho_separator` / `push_sin_separator`). Like every other slot in this
/// cluster, these arms are unreachable in production: `parse_wor_tier` (and
/// therefore `parse_wor_tier_body`) is only entered when the containing tier
/// node has no tree-sitter error, and the CHAT lexer never emits two adjacent
/// wor items without intervening whitespace on well-formed input. `context` is
/// the enclosing rule name, so the diagnostic matches the sibling item-slot
/// diagnostics.
fn push_wor_separator<'tree>(
    slot: NodeSlot<'tree, WhitespacesNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) {
    match slot {
        NodeSlot::Present(_) | NodeSlot::Absent => {}
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, context);
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, context));
        }
    }
}

/// Decode one `wor_tier_body` element, extending `items` with the resulting word or
/// separator, or pairing a bullet onto the preceding word.
///
/// The item-choice slot ([`WorTierBodyChild1Child0Choice`]) is matched EXHAUSTIVELY
/// (no `_` catch-all), reproducing the removed per-child loop byte for byte:
///
/// - `Present`: the classified item alternative (NEW backend carries a typed leaf
///   wrapper per variant, so each is unwrapped via [`AsRawNode::raw_node`]; OLD
///   carried a bare `Node`):
///   - `WorWordItem`: `wor_word_item` is a `standalone_word`; extract the word and
///     push it as `WorItem::Word` (the old `WOR_WORD_ITEM` arm).
///   - `Bullet`: pair the parsed bullet with the PRECEDING word via
///     `items.last_mut()` (the old `BULLET` arm; the word/bullet pairing).
///   - `Comma` / `TagMarker` / `VocativeMarker`: push a `WorItem::Separator`
///     carrying the marker text and span (the old `COMMA | TAG_MARKER |
///     VOCATIVE_MARKER` arm).
/// - `Error` / `Unexpected`: report `unexpected_node_error` (the old `_` arm; ERROR
///   nodes route through the shared error analyzer). No model value is invented.
/// - `Missing` / `Absent`: nothing reported, nothing pushed. OLD folded `Missing`
///   into the typed item match (a MISSING carried the typed variant); the NEW
///   `Missing` carries only a raw node with no typed classification, so there is no
///   alternative to run and nothing to push. Reporting nothing (rather than a
///   fabricated separator or diagnostic) honors the "no fabricated model values
///   during recovery" rule.
///
/// The `Missing` / `Error` / `Unexpected` / `Absent` arms are unreachable from the
/// boundary (`parse_wor_tier` is only entered when the tier node has no tree-sitter
/// error); they are handled explicitly for exhaustiveness, reproducing the old
/// behavior without inventing new diagnostics.
fn push_wor_item(
    slot: NodeSlot<'_, WorTierBodyChild1Child0Choice<'_>>,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut Vec<WorItem>,
) {
    match slot {
        NodeSlot::Present(item) => match item {
            WorTierBodyChild1Child0Choice::WorWordItem(word_item) => {
                // wor_word_item is a standalone_word; unwrap it to the word node.
                if let Some(word_node) = word_item.raw_node().child(0)
                    && let ParseOutcome::Parsed(word) = convert_word_node(word_node, source, errors)
                {
                    items.push(WorItem::Word(Box::new(word)));
                }
            }
            WorTierBodyChild1Child0Choice::Bullet(bullet_node) => {
                // Pair this bullet with the preceding word (if any).
                if let Some(bullet) = parse_inline_bullet(bullet_node.raw_node(), source, errors)
                    && let Some(WorItem::Word(word)) = items.last_mut()
                {
                    word.inline_bullet = Some(bullet);
                }
            }
            // Tag-marker separators: comma, tag „, vocative ‡. OLD folded these
            // three into one `|`-arm because each carried a bare `Node`; the NEW
            // backend gives each variant its OWN typed leaf wrapper
            // (`CommaNode`/`TagMarkerNode`/`VocativeMarkerNode`), which cannot
            // share one binding, so each unwraps via [`AsRawNode::raw_node`] and
            // delegates to the shared [`push_marker_separator`] (identical body).
            WorTierBodyChild1Child0Choice::Comma(marker) => {
                push_marker_separator(marker.raw_node(), source, items);
            }
            WorTierBodyChild1Child0Choice::TagMarker(marker) => {
                push_marker_separator(marker.raw_node(), source, items);
            }
            WorTierBodyChild1Child0Choice::VocativeMarker(marker) => {
                push_marker_separator(marker.raw_node(), source, items);
            }
        },
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "wor_tier_body"));
        }
        NodeSlot::Missing(_) | NodeSlot::Absent => {}
    }
}

/// Push a tag-marker separator (`comma` / `tag_marker` / `vocative_marker`) onto
/// `items` as a [`WorItem::Separator`], carrying the marker's raw text and span.
///
/// Shared by the three marker arms of [`push_wor_item`] (the NEW backend types
/// each marker variant separately, so they cannot share a `match` binding but
/// have byte-identical handling). Reproduces the removed
/// `COMMA | TAG_MARKER | VOCATIVE_MARKER` arm exactly; a UTF-8 error on the
/// marker text drops the separator without a fabricated value, as before.
fn push_marker_separator(marker: Node, source: &str, items: &mut Vec<WorItem>) {
    let item_span = talkbank_model::Span::new(marker.start_byte() as u32, marker.end_byte() as u32);
    if let Ok(text) = marker.utf8_text(source.as_bytes()) {
        items.push(WorItem::Separator {
            text: text.to_string(),
            span: item_span,
        });
    }
}

/// Extract language code from a `langcode` node.
///
/// Delegates to the shared token parser in the direct parser crate. Reads the raw
/// text of the whole `langcode` node (`[- code]`) exactly as the old LANGCODE arm
/// did.
fn extract_langcode(
    node: LangcodeNode,
    source: &str,
) -> Option<talkbank_model::model::LanguageCode> {
    let raw = node.raw_node().utf8_text(source.as_bytes()).ok()?;
    crate::tokens::parse_langcode_token(raw)
}

/// Parse a `bullet` node into a `Bullet`.
///
/// After grammar coarsening, `bullet` is a single token.
fn parse_inline_bullet(node: Node, source: &str, errors: &impl ErrorSink) -> Option<Bullet> {
    let (start_ms, end_ms) =
        crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps(
            node, source, errors,
        )?;
    Some(Bullet::new(start_ms, end_ms))
}
