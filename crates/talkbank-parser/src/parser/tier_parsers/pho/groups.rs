//! Helpers for parsing grouped `%pho` content.
//!
//! These routines decode either flat `pho_words` or bracketed grouped phonology
//! (`‹ ... ›`) into `PhoItem` values while preserving fallback text when needed.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Phonology>

use crate::generated_traversal::{
    AsRawNode, NodeSlot, PhoGroupChoice, PhoGroupNode, PhoGroupedContentNode, PhoWordsNode,
    WhitespacesNode, extract_pho_group, extract_pho_grouped_content,
};
use talkbank_model::ErrorSink;
use talkbank_model::model::{PhoItem, PhoWord};
use tree_sitter::Node;

use super::cst::{build_group_from_words, fallback_group_as_text};
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, extract_utf8_text, surface_unexpected,
};

/// Extracts `PhoItem`s from one `pho_group` CST node.
///
/// **Grammar Rule:**
/// ```text
/// pho_group: choice(pho_words, seq('‹', pho_grouped_content, '›'))
/// pho_words: seq(pho_word, repeat(seq('+', pho_word)))
/// ```
///
/// Driven by the generated `extract_pho_group` classifier: the group interior is
/// a fully typed [`PhoGroupChoice`] enum carried by the rule's single `content`
/// position (`children.content.slot`), so the flat-words vs bracketed-group
/// discrimination that the removed code did with `node.child(0).kind()` /
/// `node.child(1)` is now a typed match with ZERO `node.kind()`:
///
/// - `Present(PhoGroupChoice::PhoWords(pho_words))`: the old `PHO_WORDS` branch:
///   decode the whole `pho_words` node text (which folds any `pho_word + '+' +
///   pho_word` compound into one token) and emit one `PhoItem::Word`, or nothing
///   when the text is empty. The NEW backend carries a typed `PhoWordsNode` here
///   (OLD carried a bare `Node`), so the text is read via
///   [`AsRawNode::raw_node`]; the decoded bytes are unchanged.
/// - `Present(PhoGroupChoice::PhoBeginGroup(seq))`: the old `PHO_BEGIN_GROUP`
///   branch: read the seq's `child_1.slot` (`pho_grouped_content`). `Present`
///   grouped content drives `extract_pho_grouped_content_words` +
///   `build_group_from_words` (byte-identical to the old `node.child(1).kind() ==
///   PHO_GROUPED_CONTENT` path). Any non-`Present` grouped content takes the old
///   `fallback_group_as_text` path (the seq begin/end delimiter slots need no
///   action, exactly as the old code ignored the `‹` / `›` markers).
/// - outer `Missing` / `Error` / `Unexpected`: the old `_` arm, which preserved
///   the whole group as text via `fallback_group_as_text`.
/// - outer `Absent`: the old "no first child" branch, which returned nothing.
///
/// Every arm except the two `Present` cases is unreachable from the boundary
/// (`extract_pho_group_items` is only reached for a `Present` `pho_group` inside
/// an error-free tier); they are handled explicitly for exhaustiveness.
///
/// Returns a vector of PhoItems (usually just one).
pub(super) fn extract_pho_group_items(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<PhoItem> {
    let children = extract_pho_group(PhoGroupNode(node));
    surface_unexpected(&children.unexpected, source, errors);
    match children.content.slot {
        NodeSlot::Present(PhoGroupChoice::PhoWords(pho_words)) => {
            // Extract text from pho_words (handles pho_word + '+' + pho_word structure).
            let text = extract_utf8_text(pho_words.raw_node(), source, errors, "pho_words", "");
            if !text.is_empty() {
                vec![PhoItem::Word(PhoWord::new(text))]
            } else {
                vec![]
            }
        }
        NodeSlot::Present(PhoGroupChoice::PhoBeginGroup(seq)) => {
            // The bracketed group is a plain `seq`, so its interior positions are
            // the un-named `child_0` (`‹`), `child_1` (`pho_grouped_content`),
            // `child_2` (`›`); only `child_1` carries content. Surface the seq's
            // own `unexpected` sink (R2) before descending.
            surface_unexpected(&seq.unexpected, source, errors);
            match seq.child_1.slot {
                NodeSlot::Present(grouped_content) => {
                    let words = extract_pho_grouped_content_words(
                        grouped_content.raw_node(),
                        source,
                        errors,
                    );
                    build_group_from_words(words)
                }
                NodeSlot::Missing(_)
                | NodeSlot::Error(_)
                | NodeSlot::Unexpected(_)
                | NodeSlot::Absent => {
                    // Fallback: preserve entire group as text.
                    fallback_group_as_text(node, source, errors)
                }
            }
        }
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
            fallback_group_as_text(node, source, errors)
        }
        NodeSlot::Absent => vec![],
    }
}

/// Extracts grouped phonology words from `pho_grouped_content`.
///
/// **Grammar Rule:**
/// ```text
/// pho_grouped_content: seq(pho_words, repeat(seq(whitespaces, pho_words)))
/// ```
///
/// Driven by the generated `extract_pho_grouped_content` visitor: the first
/// `pho_words` is `child_0` and each subsequent `(whitespaces, pho_words)` pair
/// is a `PhoGroupedContentChild1Children` element in `child_1`. This replaces
/// the old `while node.child(idx)` positional walk. Unlike the OLD backend
/// (built with `--skip whitespaces`), the NEW backend models the separating
/// `whitespaces` token as its own explicit `child_0` position inside each
/// repeat element (`child_1` holds the `pho_words`); that position is purely
/// structural and handled by [`push_pho_separator`].
pub(super) fn extract_pho_grouped_content_words<'a>(
    node: Node<'a>,
    source: &'a str,
    errors: &impl ErrorSink,
) -> Vec<&'a str> {
    let contents = extract_pho_grouped_content(PhoGroupedContentNode(node));
    let mut words: Vec<&'a str> = Vec::with_capacity(contents.child_1.slot.len() + 1);

    push_pho_word(contents.child_0.slot, source, errors, &mut words);
    for element in contents.child_1.slot {
        match element.slot {
            NodeSlot::Present(pair) => {
                push_pho_separator(pair.child_0.slot, source, errors, "pho_grouped_content");
                push_pho_word(pair.child_1.slot, source, errors, &mut words);
                surface_unexpected(&pair.unexpected, source, errors);
            }
            // The generated repeat classifies a whole item as `Present` /
            // `Error` / `Absent` only (the established `@Languages`/gra repeat
            // finding); matched exhaustively regardless, per the no-`_`-on-
            // project-enums rule.
            NodeSlot::Missing(raw) | NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
                errors.report(unexpected_node_error(raw, source, "pho_grouped_content"));
            }
            NodeSlot::Absent => {}
        }
    }

    surface_unexpected(&contents.unexpected, source, errors);
    words
}

/// Handle the separating `whitespaces` token the NEW backend models as its own
/// explicit position inside each `pho_grouped_content` / `pho_groups` repeat
/// element.
///
/// A NEW position with no OLD counterpart: the OLD backend was generated with
/// `--skip whitespaces`, so the space between two pho words/groups was never a
/// modeled child. It carries no content, so `Present` is a no-op; the recovery
/// arms reuse the SAME diagnostic vocabulary the sibling content slots use
/// (`check_not_missing` / `unexpected_node_error`). Like every other slot in
/// this cluster, these arms are unreachable in production: the pho/mod parsers
/// run only when the containing tier node has no tree-sitter error, and the
/// CHAT lexer never emits two adjacent pho words/groups without intervening
/// whitespace on well-formed input. `context` is the enclosing rule name, so
/// the diagnostic matches the sibling content-slot diagnostics.
pub(super) fn push_pho_separator<'tree>(
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

/// Decode one `pho_words` slot, pushing its non-empty text onto `words`.
///
/// The `pho_words` slot is matched EXHAUSTIVELY over [`NodeSlot`] (no `_`
/// catch-all), reproducing the removed per-child loop byte for byte:
///
/// - `Present`: decode the token text and push it when non-empty, exactly as the
///   old `PHO_WORDS` arm.
/// - `Missing`: report the `MissingRequiredElement` (E342) recovery diagnostic
///   (the returned flag is discarded because the missing child pushes nothing).
/// - `Error` / `Unexpected`: the old `_` arm reported `unexpected_node_error`;
///   reproduced here.
/// - `Absent`: no child at this position; nothing is reported or pushed.
///
/// The `Missing` / `Error` / `Unexpected` arms are unreachable from the boundary
/// (this runs only for a `Present` `pho_grouped_content` inside an error-free
/// tier); they are handled explicitly for exhaustiveness.
fn push_pho_word<'a>(
    slot: NodeSlot<'a, PhoWordsNode<'a>>,
    source: &'a str,
    errors: &impl ErrorSink,
    words: &mut Vec<&'a str>,
) {
    match slot {
        NodeSlot::Present(pho_words) => {
            let text = extract_utf8_text(pho_words.raw_node(), source, errors, "pho_words", "");
            if !text.is_empty() {
                words.push(text);
            }
        }
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "pho_grouped_content");
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "pho_grouped_content"));
        }
        NodeSlot::Absent => {}
    }
}
