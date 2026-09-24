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
    AsRawNode, KindSlot, NoChild, NodeSlot, SinGroupChoice, SinGroupNode, SinGroupedContentNode,
    SinWordNode, SlotView, WhitespacesNode, extract_sin_group, extract_sin_grouped_content,
};
use talkbank_model::ErrorSink;
use talkbank_model::model::{SinGroupGestures, SinItem, SinToken};

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, extract_utf8_text, surface_displaced,
};

/// The token's grammatical origin owns its read context. Recovery preserves
/// the entire typed group; ordinary tokens retain their generated word kind.
enum SinTokenSource<'tree> {
    Word(SinWordNode<'tree>),
    RecoveryGroup(SinGroupNode<'tree>),
}

impl SinTokenSource<'_> {
    fn decode(self, source: &str, errors: &impl ErrorSink) -> Option<SinToken> {
        let (node, context) = match self {
            Self::Word(word) => (word.raw_node(), "sin_word"),
            Self::RecoveryGroup(group) => (group.raw_node(), "sin_item"),
        };
        let talkbank_model::ParseOutcome::Parsed(text) =
            extract_utf8_text(node, source, errors, context)
        else {
            return None;
        };
        // Preserve the existing empty-token omission policy; unreadable source
        // has already produced a diagnostic at the checked read boundary.
        SinToken::new(text).ok()
    }
}

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
/// Recovery states remain explicit even when the admitted tier has no parser
/// error: that boundary is not a type-level proof about every reconstructed
/// slot. Outer and inner recovery retain the existing whole-group preservation
/// policy through `fallback_group_as_token`, without fabricating a token.
pub(super) fn extract_sin_group_items(
    typed: SinGroupNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<SinItem> {
    let children = extract_sin_group(typed);
    surface_displaced(&children.unexpected, "sin_group", source, errors);
    match children.content.slot() {
        NodeSlot::Present(SinGroupChoice::SinWord(sin_word)) => SinTokenSource::Word(*sin_word)
            .decode(source, errors)
            .into_iter()
            .map(SinItem::Token)
            .collect(),
        NodeSlot::Present(SinGroupChoice::SinBeginGroup(seq)) => {
            // The bracketed group is a plain `seq`, so its interior positions are
            // the un-named `child_0` (`〔`), `child_1` (`sin_grouped_content`),
            // `child_2` (`〕`); only `child_1` carries content. Surface the seq's
            // own `unexpected` sink (R2) before descending.
            surface_displaced(&seq.unexpected, "sin_group", source, errors);
            match seq.child_1.slot().view() {
                SlotView::Present(grouped_content) => {
                    let gestures =
                        extract_sin_grouped_content_tokens(*grouped_content, source, errors);
                    if !gestures.is_empty() {
                        vec![SinItem::SinGroup(SinGroupGestures::new(gestures))]
                    } else {
                        vec![]
                    }
                }
                SlotView::Missing(_) | SlotView::Error(_) | SlotView::Absent(NoChild) => {
                    // Fallback: preserve the entire group as a single token.
                    fallback_group_as_token(typed, source, errors)
                }
            }
        }
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
            fallback_group_as_token(typed, source, errors)
        }
        NodeSlot::Absent(NoChild) => vec![],
    }
}

/// Fallback: preserve the whole `sin_group` node as a single [`SinToken`].
///
/// This is the removed outer `_` arm, extracted so the outer and inner
/// recovery arms share ONE preservation path. The whole group node's
/// text is decoded and emitted as one `SinItem::Token`, or nothing when empty.
fn fallback_group_as_token(
    node: SinGroupNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<SinItem> {
    SinTokenSource::RecoveryGroup(node)
        .decode(source, errors)
        .into_iter()
        .map(SinItem::Token)
        .collect()
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
    typed: SinGroupedContentNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<SinToken> {
    let contents = extract_sin_grouped_content(typed);
    let mut tokens: Vec<SinToken> = Vec::with_capacity(contents.child_1.slot().len() + 1);

    push_sin_token(contents.child_0.slot(), source, errors, &mut tokens);
    for element in contents.child_1.slot() {
        match element.slot().view() {
            SlotView::Present(pair) => {
                push_sin_separator(pair.child_0.slot(), source, errors, "sin_grouped_content");
                push_sin_token(pair.child_1.slot(), source, errors, &mut tokens);
                surface_displaced(&pair.unexpected, "sin_grouped_content", source, errors);
            }
            // An inline sequence is never MISSING or displaced; `SeqSlot` says so.
            SlotView::Error(raw) => {
                errors.report(unexpected_node_error(raw, source, "sin_grouped_content"));
            }
            SlotView::Absent(NoChild) => {}
        }
    }

    surface_displaced(&contents.unexpected, "sin_grouped_content", source, errors);
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
/// this cluster, recovery remains represented by the generated slots even when
/// the containing tier was admitted without a tree-sitter error.
/// `context` is the enclosing rule name, so the
/// diagnostic matches the sibling content-slot diagnostics.
pub(super) fn push_sin_separator<'tree>(
    slot: &KindSlot<'tree, WhitespacesNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    context: &str,
) {
    match slot.view() {
        SlotView::Present(_) | SlotView::Absent(NoChild) => {}
        SlotView::Missing(raw) => {
            check_not_missing(raw, source, errors, context);
        }
        SlotView::Error(raw) => {
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
/// - `Error`: the old `_` arm reported `unexpected_node_error`; reproduced here.
/// - `Absent`: no child at this position; nothing is reported or pushed.
///
/// The `Missing` / `Error` arms remain explicit; a typed grouped-content node
/// alone is not proof that its reconstructed slots contain no recovery.
fn push_sin_token<'tree>(
    slot: &KindSlot<'tree, SinWordNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    tokens: &mut Vec<SinToken>,
) {
    match slot.view() {
        SlotView::Present(sin_word) => {
            if let Some(token) = SinTokenSource::Word(*sin_word).decode(source, errors) {
                tokens.push(token);
            }
        }
        SlotView::Missing(raw) => {
            check_not_missing(raw, source, errors, "sin_grouped_content");
        }
        SlotView::Error(raw) => {
            errors.report(unexpected_node_error(raw, source, "sin_grouped_content"));
        }
        SlotView::Absent(NoChild) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::FromNodeKind;
    use talkbank_model::{ErrorCode, ErrorCollector, Span};

    /// Direct fallback boundary evidence, not a production recovery witness.
    #[test]
    fn typed_group_fallback_preserves_text_and_refuses_incompatible_source() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/reference/annotation/groups-sign.cha"
        ));
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut groups = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(group) = SinGroupNode::from_node(node) else {
                continue;
            };
            groups += 1;
            let errors = ErrorCollector::new();
            let items = fallback_group_as_token(group, source, &errors);
            let [SinItem::Token(token)] = items.as_slice() else {
                panic!("whole group must remain one token");
            };
            assert_eq!(token.as_ref(), &source[node.byte_range()]);
            assert!(errors.to_vec().is_empty());
            assert!(fallback_group_as_token(group, "", &errors).is_empty());
            let diagnostics = errors.into_vec();
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, ErrorCode::TreeParsingError);
            assert_eq!(
                diagnostics[0].location.span,
                Span::from_usize(node.start_byte(), node.end_byte())
            );
        }
        assert!(groups > 0, "fixture must supply sign groups");
    }
}
