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
//! `node.child(idx)` tier-structure hand-walk. `extract_wor_word_item` retains
//! the nested standalone-word type through conversion and exposes recovery
//! slots and displaced children explicitly.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::generated_traversal::{
    AdmittedWorTierBodyChild1Child0Choice as WorTierBodyChild1Child0Choice,
    AdmittedWorTierBodyChild1Child0ChoiceBoundView as WorTierBodyChild1Child0ChoiceBoundView,
    AsRawNode, BulletNode, ChoiceSlot, KindSlot, LangcodeNode, NoChild, NodeSlot, SlotView,
    SourceBound, SourceBoundKind, SourceField, SourceSlotView, WhitespacesNode,
    WorDependentTierNode, WorTierBodyNode,
};
use crate::parser::node_span::span_of;
use talkbank_model::ErrorSink;
use talkbank_model::model::Bullet;
use talkbank_model::model::dependent_tier::{WorItem, WorTier};

use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::main_tier::structure::terminator::terminator_from_new_choice;
use crate::parser::tree_parsing::main_tier::word::convert_word_node;
use crate::parser::tree_parsing::parser_helpers::{
    check_not_missing, extract_utf8_text, surface_displaced,
};
use crate::parser::typed_cst::read_source_field;
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
/// - `Present`: a possibly empty body drives item iteration. Compiled canonical
///   grammar admission proves that this composite body cannot itself be Missing;
///   lexical placeholders inside it remain independently represented.
/// - `Absent` / `Error`: no child of kind `wor_tier_body` was found
///   (the old `None` branch): return the EMPTY tier SILENTLY (no diagnostic). This
///   silent-partial is preserved. The document attachment caller gates malformed
///   tiers, but this public adapter's bound-node type proves source ownership,
///   not syntax validity. Range refusal reports a binding diagnostic.
pub fn parse_wor_tier<'tree>(
    typed: SourceBound<'tree, '_, WorDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<WorTier, crate::generated_traversal::ReconstructionFault> {
    let source = typed.source();
    let node = typed.node().raw_node();
    let span = span_of(node);

    let associated = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let children = associated.children();
    surface_displaced(&children.unexpected, "wor_dependent_tier", source, errors);

    Ok(match associated.field_child_2().slot().view() {
        SourceSlotView::Present(body) => match read_source_field(body, errors) {
            Some(body) => parse_wor_tier_body(body, errors)?.with_span(span),
            None => WorTier::new(Vec::new()).with_span(span),
        },
        SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => {
            WorTier::new(Vec::new()).with_span(span)
        }
        SourceSlotView::Missing(never) => match never {},
    })
}

/// Decode the `wor_tier_body` node into a `WorTier` (langcode, items, terminator),
/// driven by the generated `extract_wor_tier_body` visitor.
///
/// Each of the four typed fields is handled explicitly; the returned tier has no
/// span yet (the caller attaches the dep-tier span).
fn parse_wor_tier_body<'tree>(
    typed: SourceBound<'tree, '_, WorTierBodyNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<WorTier, crate::generated_traversal::ReconstructionFault> {
    let source = typed.source();
    let associated = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let children = associated.children();
    surface_displaced(&children.unexpected, "wor_tier_body", source, errors);

    // `language_code` (optional): reproduce the old LANGCODE arm. Unlike the OLD
    // backend's flat `Option<KindSlot<LangcodeNode>>`, the NEW backend groups the
    // whole grammar-optional `(langcode, whitespaces)` pair into one NESTED
    // carrier (`WorTierBodyLanguageCodeChildren`), because that pair together is
    // what is optional, not `langcode` alone (the B2 nested-group precedent).
    // Descend one level to the `langcode` slot (`child_0`); only a `Present`
    // langcode contributes a code, every other state (a malformed group, an absent
    // group) yields none, exactly as the old loop only acted on a real
    // `LANGCODE`-kind child. Surface the nested group's own `unexpected` sink (R2).
    let language_code = match children.language_code.slot().as_ref().map(NodeSlot::view) {
        Some(SlotView::Present(group)) => {
            surface_displaced(&group.unexpected, "wor_tier_body", source, errors);
            match group.child_0.slot().view() {
                SlotView::Present(langcode) => extract_langcode(*langcode, source, errors),
                SlotView::Error(_) | SlotView::Absent(NoChild) => None,
                SlotView::Missing(never) => match never {},
            }
        }
        Some(SlotView::Missing(_) | SlotView::Error(_)) | None => None,
    };

    // Item repeat (`child_1`): each element is a `(choice, whitespaces)` pair, so
    // the item is `pair.child_0` (the typed choice) and `pair.child_1` is the
    // trailing separator whitespace, handled by `push_wor_separator` (mirrors the
    // gra/pho/sin separator helpers; the NEW backend models it explicitly since it
    // does not use `--skip whitespaces`). Iterate the typed elements, pairing each
    // bullet with its preceding word.
    let mut items: Vec<WorItem> = Vec::with_capacity(children.child_1.slot().len());
    for element in associated.field_child_1().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(pair) => {
                push_wor_item(pair.field_child_0().slot(), errors, &mut items);
                push_wor_separator(pair.field_child_1().slot(), errors, "wor_tier_body");
                for bad in pair.field_unexpected().iter() {
                    surface_displaced(&[bad.raw_node()], "wor_tier_body", source, errors);
                }
            }
            // An inline sequence is never MISSING or displaced; `SeqSlot` says so.
            SourceSlotView::Error(raw) => {
                errors.report(unexpected_node_error(
                    raw.raw_node(),
                    source,
                    "wor_tier_body",
                ));
            }
            SourceSlotView::Absent(never) => match never {},
        }
    }

    // `child_2` (`terminator` supertype, optional, previously UNCONSUMED): a
    // `Present` choice maps through the SHARED exhaustive `terminator_from_new_choice`
    // (the NEW-backend twin; wor's terminator is `WorTierBodyChild2Choice`). `None`
    // (absent from the source), `Missing` or `Error` yield no
    // terminator, matching the old behavior when no terminator child was seen.
    let terminator = match children.child_2.slot().as_ref().map(NodeSlot::view) {
        Some(SlotView::Present(choice)) => Some(terminator_from_new_choice(choice)),
        Some(SlotView::Missing(_) | SlotView::Error(_)) | None => None,
    };

    // `child_3` (`newline`, required): structural only, no model representation.
    // Every slot state is a no-op, matched explicitly so the required newline slot
    // is never silently dropped (as the old `NEWLINE => {}` arm did).
    match children.child_3.slot().view() {
        SlotView::Present(_)
        | SlotView::Missing(_)
        | SlotView::Error(_)
        | SlotView::Absent(NoChild) => {}
    }

    Ok(WorTier::new(items)
        .with_terminator(terminator)
        .with_language_code(language_code))
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
/// `push_pho_separator` / `push_sin_separator`). Whitespace is lexical, so
/// canonical grammar admission does not remove its Missing state. A caller's
/// clean-tier policy is not encoded in this source-bound API; preserve recovery.
/// `context` is the enclosing rule name used by the sibling diagnostics.
fn push_wor_separator<'tree>(
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
/// This mixed lexical/composite choice retains Missing and other recovery
/// states. Only the nested standalone-word slot has a compiled nonmissing proof;
/// neither that proof nor source ownership certifies the whole tier as clean.
fn push_wor_item<'tree>(
    slot: SourceField<'_, 'tree, '_, ChoiceSlot<'tree, WorTierBodyChild1Child0Choice<'tree>>>,
    errors: &impl ErrorSink,
    items: &mut Vec<WorItem>,
) {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(item) => {
            let Some(item) = read_source_field(item, errors) else {
                return;
            };
            match item.view() {
                WorTierBodyChild1Child0ChoiceBoundView::WorWordItem(word_item) => {
                    let Ok(associated) = crate::parser::typed_cst::report_reconstruction(
                        crate::parser::typed_cst::canonical_grammar()
                            .and_then(|grammar| word_item.extract_admitted(grammar)),
                        word_item.raw_node(),
                        source,
                        errors,
                    ) else {
                        return;
                    };
                    let word_children = associated.children();
                    surface_displaced(&word_children.unexpected, "wor_word_item", source, errors);
                    match associated.field_content().slot().view() {
                        SourceSlotView::Present(word_node) => {
                            if let Some(word_node) = read_source_field(word_node, errors)
                                && let ParseOutcome::Parsed(word) =
                                    convert_word_node(word_node, errors)
                            {
                                items.push(WorItem::Word(Box::new(word)));
                            }
                        }
                        SourceSlotView::Missing(never) => match never {},
                        SourceSlotView::Error(bad) => {
                            errors.report(unexpected_node_error(
                                bad.raw_node(),
                                source,
                                "wor_word_item",
                            ));
                        }
                        SourceSlotView::Absent(NoChild) => {}
                    }
                }
                WorTierBodyChild1Child0ChoiceBoundView::Bullet(bullet_node) => {
                    // Pair this bullet with the preceding word (if any).
                    if let Some(bullet) = parse_inline_bullet(bullet_node, errors)
                        && let Some(WorItem::Word(word)) = items.last_mut()
                    {
                        word.inline_bullet = Some(bullet);
                    }
                }
                // Retain each marker's checked source slice through model construction.
                WorTierBodyChild1Child0ChoiceBoundView::Comma(marker) => {
                    push_marker_separator(marker, items);
                }
                WorTierBodyChild1Child0ChoiceBoundView::TagMarker(marker) => {
                    push_marker_separator(marker, items);
                }
                WorTierBodyChild1Child0ChoiceBoundView::VocativeMarker(marker) => {
                    push_marker_separator(marker, items);
                }
            }
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                source,
                "wor_tier_body",
            ));
        }
        SourceSlotView::Missing(_) | SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Unexpected(never) => match never {},
    }
}

/// Push a tag-marker separator (`comma` / `tag_marker` / `vocative_marker`) onto
/// `items` as a [`WorItem::Separator`], carrying the marker's raw text and span.
///
/// Shared by the three marker arms of [`push_wor_item`] (the NEW backend types
/// each marker variant separately, so they cannot share a `match` binding but
/// have byte-identical handling). Admission already proved the text readable;
/// the marker cannot be silently dropped through a second decoding attempt.
fn push_marker_separator<'tree, T: SourceBoundKind<'tree>>(
    marker: SourceBound<'tree, '_, T>,
    items: &mut Vec<WorItem>,
) {
    items.push(WorItem::Separator {
        text: marker.text().to_string(),
        span: span_of(marker.raw_node()),
    });
}

/// Extract language code from a `langcode` node.
///
/// Delegates to the shared token parser in the direct parser crate. Reads the raw
/// text of the whole `langcode` node (`[- code]`) exactly as the old LANGCODE arm
/// did.
fn extract_langcode(
    node: LangcodeNode,
    source: &str,
    errors: &impl ErrorSink,
) -> Option<talkbank_model::model::LanguageCode> {
    let raw =
        extract_utf8_text(node.raw_node(), source, errors, "wor language code").into_option()?;
    crate::tokens::parse_langcode_token(raw)
}

/// Parse a `bullet` node into a `Bullet`.
///
/// The generated wrapper retains the structured timestamp fields.
fn parse_inline_bullet<'tree>(
    node: SourceBound<'tree, '_, BulletNode<'tree>>,
    errors: &impl ErrorSink,
) -> Option<Bullet> {
    use crate::parser::tree_parsing::media_bullet::BulletRejection;
    // Alignment owns ordinary timestamp rejection, but a producer fault must
    // remain an internal failure rather than become an absent timing value.
    let (start_ms, end_ms) =
        match crate::parser::tree_parsing::media_bullet::parse_bullet_node_timestamps(node, errors)
        {
            Ok(times) => times,
            Err(BulletRejection::Producer(fault)) => {
                crate::parser::typed_cst::report_cst_failure(
                    node.raw_node(),
                    node.source(),
                    fault,
                    errors,
                );
                return None;
            }
            Err(
                BulletRejection::ContainsRecoveryNode
                | BulletRejection::TimeFieldAbsent { .. }
                | BulletRejection::TimeNotRepresentable { .. },
            ) => return None,
        };
    Some(Bullet::new(start_ms, end_ms))
}
