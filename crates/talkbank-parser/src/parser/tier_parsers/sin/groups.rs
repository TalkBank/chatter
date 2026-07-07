//! Group extraction helpers for `%sin` content.
//!
//! `%sin` allows both flat gesture tokens and bracketed grouped spans.
//! These helpers normalize CST nodes into `SinItem` sequences.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gestures>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Sign_Group>

use crate::generated_traversal::{
    AsRawNode, NodeSlot, SinGroupChoice, SinGroupNode, SinGroupedContentNode, SinWordNode,
    WhitespacesNode, extract_sin_group, extract_sin_grouped_content,
};
use talkbank_model::ErrorSink;
use talkbank_model::model::{SinGroupGestures, SinItem, SinToken};
use tree_sitter::Node;

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, extract_utf8_text, surface_unexpected,
};

/// Extracts `SinItem` values from a `sin_group` node.
///
/// Driven by the generated `extract_sin_group` classifier: the group interior is
/// a fully typed [`SinGroupChoice`] enum carried by the rule's single `content`
/// position (`children.content.slot`), so the flat-token vs bracketed-group
/// discrimination that the removed code did with `node.child(0).kind()` /
/// `node.child(1)` is now a typed match with ZERO `node.kind()`:
///
/// - `Present(SinGroupChoice::SinWord(sin_word))`: the old `SIN_WORD` branch:
///   decode the `sin_word` text and emit one `SinItem::Token`, or nothing when
///   the text is empty. The NEW backend carries a typed `SinWordNode` here (OLD
///   carried a bare `Node`), so the text is read via [`AsRawNode::raw_node`];
///   the decoded bytes are unchanged.
/// - `Present(SinGroupChoice::SinBeginGroup(seq))`: the old `SIN_BEGIN_GROUP`
///   branch: read the seq's `child_1.slot` (`sin_grouped_content`). `Present`
///   grouped content drives `extract_sin_grouped_content_tokens` and emits one
///   `SinItem::SinGroup` when non-empty (byte-identical to the old
///   `node.child(1).kind() == SIN_GROUPED_CONTENT` path). Any non-`Present`
///   grouped content takes the `fallback_group_as_token` path (the seq
///   begin/end delimiter slots need no action, exactly as the old code ignored
///   the `〔` / `〕` markers).
/// - outer `Missing` / `Error` / `Unexpected`: the old `_` arm, which preserved
///   the whole group as a single token via `extract_utf8_text` on the group node.
/// - outer `Absent`: the old "no first child" branch, which returned nothing.
///
/// Every arm except the two `Present` cases is unreachable from the boundary
/// (`extract_sin_group_items` is only reached for a `Present` `sin_group` inside
/// an error-free tier); they are handled explicitly for exhaustiveness. Matching
/// the analogous `%pho`/`%mod` migration (Task 4d), the unreachable field-recovery
/// arms are collapsed onto the shared `fallback_group_as_token` for uniformity:
/// this drops the removed code's `Expected sin_grouped_content` `TreeParsingError`
/// (which only fired for a wrong-kind `child(1)`, impossible in an error-free
/// tier) rather than silently losing a reachable diagnostic.
pub(super) fn extract_sin_group_items(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<SinItem> {
    let children = extract_sin_group(SinGroupNode(node));
    surface_unexpected(&children.unexpected, source, errors);
    match children.content.slot {
        NodeSlot::Present(SinGroupChoice::SinWord(sin_word)) => {
            let text = extract_utf8_text(sin_word.raw_node(), source, errors, "sin_word", "");
            if !text.is_empty() {
                vec![SinItem::Token(SinToken::new_unchecked(text))]
            } else {
                vec![]
            }
        }
        NodeSlot::Present(SinGroupChoice::SinBeginGroup(seq)) => {
            // The bracketed group is a plain `seq`, so its interior positions are
            // the un-named `child_0` (`〔`), `child_1` (`sin_grouped_content`),
            // `child_2` (`〕`); only `child_1` carries content. Surface the seq's
            // own `unexpected` sink (R2) before descending.
            surface_unexpected(&seq.unexpected, source, errors);
            match seq.child_1.slot {
                NodeSlot::Present(grouped_content) => {
                    let gestures = extract_sin_grouped_content_tokens(
                        grouped_content.raw_node(),
                        source,
                        errors,
                    );
                    if !gestures.is_empty() {
                        vec![SinItem::SinGroup(SinGroupGestures::new(gestures))]
                    } else {
                        vec![]
                    }
                }
                NodeSlot::Missing(_)
                | NodeSlot::Error(_)
                | NodeSlot::Unexpected(_)
                | NodeSlot::Absent => {
                    // Fallback: preserve the entire group as a single token.
                    fallback_group_as_token(node, source, errors)
                }
            }
        }
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
            fallback_group_as_token(node, source, errors)
        }
        NodeSlot::Absent => vec![],
    }
}

/// Fallback: preserve the whole `sin_group` node as a single [`SinToken`].
///
/// This is the removed outer `_` arm, extracted so the outer and inner
/// unreachable recovery arms share ONE preservation path. The whole group node's
/// text is decoded and emitted as one `SinItem::Token`, or nothing when empty.
fn fallback_group_as_token(node: Node, source: &str, errors: &impl ErrorSink) -> Vec<SinItem> {
    let text = extract_utf8_text(node, source, errors, "sin_item", "");
    if !text.is_empty() {
        vec![SinItem::Token(SinToken::new_unchecked(text))]
    } else {
        vec![]
    }
}

/// Extracts `SinToken` values from grouped `%sin` content.
///
/// **Grammar Rule:**
/// ```text
/// sin_grouped_content: seq(sin_word, repeat(seq(whitespaces, sin_word)))
/// ```
///
/// Driven by the generated `extract_sin_grouped_content` visitor: the first
/// `sin_word` is `child_0` and each subsequent `(whitespaces, sin_word)` pair is
/// a `SinGroupedContentChild1Children` element in `child_1`. This replaces the
/// old `while node.child(idx)` positional walk. Unlike the OLD backend (built
/// with `--skip whitespaces`), the NEW backend models the separating
/// `whitespaces` token as its own explicit `child_0` position inside each repeat
/// element (`child_1` holds the `sin_word`); that position is purely structural
/// and handled by [`push_sin_separator`].
fn extract_sin_grouped_content_tokens(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<SinToken> {
    let contents = extract_sin_grouped_content(SinGroupedContentNode(node));
    let mut tokens: Vec<SinToken> = Vec::with_capacity(contents.child_1.slot.len() + 1);

    push_sin_token(contents.child_0.slot, source, errors, &mut tokens);
    for element in contents.child_1.slot {
        match element.slot {
            NodeSlot::Present(pair) => {
                push_sin_separator(pair.child_0.slot, source, errors, "sin_grouped_content");
                push_sin_token(pair.child_1.slot, source, errors, &mut tokens);
                surface_unexpected(&pair.unexpected, source, errors);
            }
            // The generated repeat classifies a whole item as `Present` /
            // `Error` / `Absent` only (the established `@Languages`/gra/pho
            // repeat finding); matched exhaustively regardless, per the
            // no-`_`-on-project-enums rule.
            NodeSlot::Missing(raw) | NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
                errors.report(unexpected_node_error(raw, source, "sin_grouped_content"));
            }
            NodeSlot::Absent => {}
        }
    }

    surface_unexpected(&contents.unexpected, source, errors);
    tokens
}

/// Handle the separating `whitespaces` token the NEW backend models as its own
/// explicit position inside each `sin_grouped_content` / `sin_groups` repeat
/// element.
///
/// A NEW position with no OLD counterpart: the OLD backend was generated with
/// `--skip whitespaces`, so the space between two sign tokens/groups was never a
/// modeled child. It carries no content, so `Present` is a no-op; the recovery
/// arms reuse the SAME diagnostic vocabulary the sibling content slots use
/// (`check_not_missing` / `unexpected_node_error`). Like every other slot in
/// this cluster, these arms are unreachable in production: the sin parser runs
/// only when the containing tier node has no tree-sitter error, and the CHAT
/// lexer never emits two adjacent sign tokens/groups without intervening
/// whitespace on well-formed input. `context` is the enclosing rule name, so the
/// diagnostic matches the sibling content-slot diagnostics.
pub(super) fn push_sin_separator<'tree>(
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

/// Decode one `sin_word` slot, pushing its non-empty token text onto `tokens`.
///
/// The `sin_word` slot is matched EXHAUSTIVELY over [`NodeSlot`] (no `_`
/// catch-all), reproducing the removed per-child loop byte for byte:
///
/// - `Present`: decode the token text and push it when non-empty, exactly as the
///   old `SIN_WORD` arm.
/// - `Missing`: report the `MissingRequiredElement` (E342) recovery diagnostic
///   (the returned flag is discarded because the missing child pushes nothing).
/// - `Error` / `Unexpected`: the old `_` arm reported `unexpected_node_error`;
///   reproduced here.
/// - `Absent`: no child at this position; nothing is reported or pushed.
///
/// The `Missing` / `Error` / `Unexpected` arms are unreachable from the boundary
/// (this runs only for a `Present` `sin_grouped_content` inside an error-free
/// tier); they are handled explicitly for exhaustiveness.
fn push_sin_token<'tree>(
    slot: NodeSlot<'tree, SinWordNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    tokens: &mut Vec<SinToken>,
) {
    match slot {
        NodeSlot::Present(sin_word) => {
            let text = extract_utf8_text(sin_word.raw_node(), source, errors, "sin_word", "");
            if !text.is_empty() {
                tokens.push(SinToken::new_unchecked(text));
            }
        }
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "sin_grouped_content");
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "sin_grouped_content"));
        }
        NodeSlot::Absent => {}
    }
}
