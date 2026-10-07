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
    AdmittedSinGroupChoiceSourceView, AsRawNode, KindSlot, NoChild, NonMissingKindSlot,
    SinGroupNode, SinGroupedContentNode, SinWordNode, SourceBound, SourceField, SourceSlotView,
    WhitespacesNode,
};
use talkbank_model::ErrorSink;
use talkbank_model::model::{SinGroupGestures, SinItem, SinToken};

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};

/// The token's grammatical origin owns its read context. Recovery preserves
/// the entire typed group; ordinary tokens retain their generated word kind.
enum SinTokenSource<'tree, 'source> {
    Word(SourceBound<'tree, 'source, SinWordNode<'tree>>),
    RecoveryGroup(SourceBound<'tree, 'source, SinGroupNode<'tree>>),
}

impl SinTokenSource<'_, '_> {
    fn decode(self) -> Option<SinToken> {
        let text = match self {
            Self::Word(word) => word.text(),
            Self::RecoveryGroup(group) => group.text(),
        };
        // Preserve the existing empty-token omission policy; source admission
        // occurred before constructing this source-bound token capability.
        SinToken::new(text).ok()
    }
}

/// Extracts `SinItem` values from a `sin_group` node.
///
/// The generated [`AdmittedSinGroupChoiceSourceView`] retains ownership while selecting
/// a flat token or bracketed group. Present grouped content is decoded normally;
/// inner recovery retains the whole-group fallback. Outer Missing/Error also
/// preserve that fallback, Absent emits no items, and Unexpected is uninhabited.
/// Token reads use admitted text; internal source faults cannot select fallback.
///
/// Recovery states remain explicit even when the admitted tier has no parser
/// error: that boundary is not a type-level proof about every reconstructed
/// slot. Compiled-grammar admission excludes missing composite content, while
/// outer choice recovery and inner Error/Absent retain whole-group preservation
/// policy through `fallback_group_as_token`, without fabricating a token.
pub(super) fn extract_sin_group_items<'tree>(
    typed: SourceBound<'tree, '_, SinGroupNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<SinItem>, crate::CstFailure> {
    let source = typed.source();
    let children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    surface_displaced(&children.children().unexpected, "sin_group", source, errors);
    let choice = match children.field_content().slot().view() {
        SourceSlotView::Present(choice) => choice,
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) => {
            return Ok(fallback_group_as_token(typed));
        }
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => return Ok(Vec::new()),
    };
    Ok(match choice.view() {
        AdmittedSinGroupChoiceSourceView::SinWord(sin_word) => {
            SinTokenSource::Word(sin_word.read()?)
                .decode()
                .into_iter()
                .map(SinItem::Token)
                .collect()
        }
        AdmittedSinGroupChoiceSourceView::SinBeginGroup(seq) => {
            // The bracketed group is a plain `seq`, so its interior positions are
            // the un-named `child_0` (`〔`), `child_1` (`sin_grouped_content`),
            // `child_2` (`〕`); only `child_1` carries content. Surface the seq's
            // own `unexpected` sink (R2) before descending.
            for node in seq.field_unexpected().iter() {
                surface_displaced(&[node.raw_node()], "sin_group", node.source(), errors);
            }
            match seq.field_child_1().slot().view() {
                SourceSlotView::Present(grouped_content) => {
                    let gestures =
                        extract_sin_grouped_content_tokens(grouped_content.read()?, errors)?;
                    if !gestures.is_empty() {
                        vec![SinItem::SinGroup(SinGroupGestures::new(gestures))]
                    } else {
                        vec![]
                    }
                }
                SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => {
                    // Fallback: preserve the entire group as a single token.
                    fallback_group_as_token(typed)
                }
            }
        }
    })
}

/// Fallback: preserve the whole `sin_group` node as a single [`SinToken`].
///
/// This is the removed outer `_` arm, extracted so the outer and inner
/// recovery arms share ONE preservation path. The whole group node's
/// text is decoded and emitted as one `SinItem::Token`, or nothing when empty.
fn fallback_group_as_token<'tree>(
    node: SourceBound<'tree, '_, SinGroupNode<'tree>>,
) -> Vec<SinItem> {
    SinTokenSource::RecoveryGroup(node)
        .decode()
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
fn extract_sin_grouped_content_tokens<'tree>(
    typed: SourceBound<'tree, '_, SinGroupedContentNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<Vec<SinToken>, crate::CstFailure> {
    let source = typed.source();
    let contents = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let mut tokens: Vec<SinToken> =
        Vec::with_capacity(contents.field_child_1().slot().iter().len() + 1);

    push_sin_token(contents.field_child_0().slot(), errors, &mut tokens)?;
    for element in contents.field_child_1().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(pair) => {
                push_sin_separator(pair.field_child_0().slot(), errors, "sin_grouped_content");
                push_sin_token(pair.field_child_1().slot(), errors, &mut tokens)?;
                for node in pair.field_unexpected().iter() {
                    surface_displaced(
                        &[node.raw_node()],
                        "sin_grouped_content",
                        node.source(),
                        errors,
                    );
                }
            }
            // An inline sequence is never MISSING or displaced; `SeqSlot` says so.
            SourceSlotView::Error(raw) => {
                errors.report(unexpected_node_error(
                    raw.raw_node(),
                    source,
                    "sin_grouped_content",
                ));
            }
            SourceSlotView::Absent(never) => match never {},
            SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
        }
    }

    surface_displaced(
        &contents.children().unexpected,
        "sin_grouped_content",
        source,
        errors,
    );
    Ok(tokens)
}

/// Handle the separating `whitespaces` token the NEW backend models as its own
/// explicit position inside each `sin_grouped_content` / `sin_groups` repeat
/// element.
///
/// A NEW position with no OLD counterpart: the OLD backend was generated with
/// `--skip whitespaces`, so the space between two sign tokens/groups was never a
/// modeled child. It carries no content, so `Present` is a no-op; the recovery
/// arms reuse the SAME diagnostic vocabulary the sibling content slots use
/// (`check_not_missing` / `unexpected_node_error`). This lexical slot retains
/// Missing even after compiled-grammar admission; the composite content slots
/// do not. An error-free containing tier alone proves neither property.
/// `context` is the enclosing rule name, so the
/// diagnostic matches the sibling content-slot diagnostics.
pub(super) fn push_sin_separator<'tree>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, WhitespacesNode<'tree>>>,
    errors: &impl ErrorSink,
    context: &str,
) {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(_) | SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Missing(raw) => {
            check_not_missing(raw.raw_node(), source, errors, context);
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(raw.raw_node(), source, context));
        }
        SourceSlotView::Unexpected(never) => match never {},
    }
}

/// Decode one `sin_word` slot, pushing its non-empty token text onto `tokens`.
///
/// Compiled-grammar admission proves this named composite carrier cannot be
/// missing. Present text retains empty-token admission; Error is diagnosed and
/// Absent emits nothing. This does not prove its lexical children are complete.
fn push_sin_token<'tree>(
    slot: SourceField<'_, 'tree, '_, NonMissingKindSlot<'tree, SinWordNode<'tree>>>,
    errors: &impl ErrorSink,
    tokens: &mut Vec<SinToken>,
) -> Result<(), crate::CstFailure> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(sin_word) => {
            if let Some(token) = SinTokenSource::Word(sin_word.read()?).decode() {
                tokens.push(token);
            }
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                source,
                "sin_grouped_content",
            ));
        }
        SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Unexpected(never) => match never {},
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::SourceBindingError;

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
        let other = parser
            .parse_source_incremental(source, None)
            .expect("independent owner");
        let mut groups = 0;
        for node in parsed.root().expect("root").descendants() {
            let Some(group) = node.expect("readable node").typed::<SinGroupNode>() else {
                continue;
            };
            groups += 1;
            let items = fallback_group_as_token(group);
            let [SinItem::Token(token)] = items.as_slice() else {
                panic!("whole group must remain one token");
            };
            assert_eq!(token.as_ref(), group.text());
            assert!(matches!(
                other.bind(group.raw_node()),
                Err(SourceBindingError::ForeignTree)
            ));
        }
        assert!(groups > 0, "fixture must supply sign groups");
    }
}
