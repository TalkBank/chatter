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

mod lowering;
use lowering::{WorItemRejected, WorLowering};

use crate::generated_traversal::{
    AsRawNode, BulletNode, ChoiceSlot, KindAdmitted, KindSlot, LangcodeNode, NoChild, NodeSlot,
    SlotView, SourceBound, SourceBoundKind, SourceField, SourceSlotView, WhitespacesNode,
    WorDependentTierNode, WorTierBodyChild1Child0Choice, WorTierBodyChild1Child0ChoiceBoundView,
    WorTierBodyChild1Child0Missing, WorTierBodyNode,
};
use crate::parser::node_span::span_of;
use crate::parser::typed_cst::ExtractCanonical;
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
/// Per-rule admission proves the ordinary `wor_tier_body` at child_2 present.
/// Its nested optional language, word sequence and terminator retain their
/// recovery states. Envelope presence does not prove syntax validity, and
/// source-range refusal propagates as a producer failure.
pub fn parse_wor_tier<'tree>(
    typed: SourceBound<'tree, '_, WorDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<WorTier>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.node().raw_node();
    let span = span_of(node);

    let associated = crate::parser::typed_cst::extract_envelope(typed)?;
    let children = associated.children();
    surface_displaced(&children.unexpected, "wor_dependent_tier", source, errors);

    let SourceSlotView::Present(body) = associated.field_child_2().slot().view();
    parse_wor_tier_body(span, body.read()?, errors)
}

/// Decode the `wor_tier_body` node into a `WorTier` (langcode, items, terminator),
/// driven by the generated `extract_wor_tier_body` visitor.
///
/// Each typed field is handled explicitly. Item rejection prevents constructing
/// a partial tier; successful construction receives the owning tier location.
fn parse_wor_tier_body<'tree>(
    location: impl Into<talkbank_model::Location>,
    typed: SourceBound<'tree, '_, WorTierBodyNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<WorTier>, crate::CstFailure> {
    let source = typed.source();
    let associated = typed.extract_canonical()?;
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
    let mut items = WorLowering::new(children.child_1.slot().len());
    for element in associated.field_child_1().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(pair) => {
                items = match push_wor_item(pair.field_child_0().slot(), errors, items) {
                    Ok(next) => next,
                    Err(WorItemRejected) => return Ok(ParseOutcome::Rejected),
                };
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

    Ok(ParseOutcome::Parsed(
        WorTier::new(location, items.finish())
            .with_terminator(terminator)
            .with_language_code(language_code),
    ))
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

/// Decode one source item into the pending association phase.
///
/// A bullet consumes the immediately preceding untimed word. Completed items
/// cannot receive another bullet. A missing, unreadable or rejected item refuses
/// the whole tier instead of constructing a partial timing model. The file owner
/// then retains original source; fragments return explicit rejection.
fn push_wor_item<'tree>(
    slot: SourceField<
        '_,
        'tree,
        '_,
        ChoiceSlot<
            'tree,
            WorTierBodyChild1Child0Choice<'tree>,
            WorTierBodyChild1Child0Missing<'tree, KindAdmitted>,
        >,
    >,
    errors: &impl ErrorSink,
    mut items: WorLowering,
) -> Result<WorLowering, WorItemRejected> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(item) => {
            let Some(item) = read_source_field(item, errors) else {
                return Err(WorItemRejected);
            };
            match item.view() {
                WorTierBodyChild1Child0ChoiceBoundView::WorWordItem(word_item) => {
                    let Ok(associated) = crate::parser::typed_cst::report_admission(
                        word_item.extract_canonical(),
                        word_item.raw_node(),
                        source,
                        errors,
                    ) else {
                        return Err(WorItemRejected);
                    };
                    let word_children = associated.children();
                    surface_displaced(&word_children.unexpected, "wor_word_item", source, errors);
                    match associated.field_content().slot().view() {
                        SourceSlotView::Present(word_node) => {
                            if let Some(word_node) = read_source_field(word_node, errors)
                                && let Ok(lowered) = convert_word_node(word_node, errors)
                                && let ParseOutcome::Parsed(word) = lowered.into_word()
                            {
                                items = items.push(WorItem::Word(Box::new(word)));
                            } else {
                                return Err(WorItemRejected);
                            }
                        }
                        SourceSlotView::Error(bad) => {
                            errors.report(unexpected_node_error(
                                bad.raw_node(),
                                source,
                                "wor_word_item",
                            ));
                            return Err(WorItemRejected);
                        }
                        SourceSlotView::Absent(NoChild) => return Err(WorItemRejected),
                    }
                }
                WorTierBodyChild1Child0ChoiceBoundView::Bullet(bullet_node) => {
                    let bullet = parse_inline_bullet(bullet_node, errors).ok_or(WorItemRejected)?;
                    items = items.attach(bullet).inspect_err(|_| {
                        errors.report(talkbank_model::ParseError::at_span(
                            talkbank_model::ErrorCode::UnparsableContent,
                            talkbank_model::Severity::Error,
                            span_of(bullet_node.raw_node()),
                            "WOR timing bullet requires an immediately preceding untimed word",
                        ));
                    })?;
                }
                // Retain each marker's checked source slice through model construction.
                WorTierBodyChild1Child0ChoiceBoundView::Comma(marker) => {
                    items = push_marker_separator(marker, items);
                }
                WorTierBodyChild1Child0ChoiceBoundView::TagMarker(marker) => {
                    items = push_marker_separator(marker, items);
                }
                WorTierBodyChild1Child0ChoiceBoundView::VocativeMarker(marker) => {
                    items = push_marker_separator(marker, items);
                }
            }
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                source,
                "wor_tier_body",
            ));
            return Err(WorItemRejected);
        }
        SourceSlotView::Missing(_) | SourceSlotView::Absent(NoChild) => {
            return Err(WorItemRejected);
        }
        SourceSlotView::Unexpected(never) => match never {},
    }
    Ok(items)
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
    items: WorLowering,
) -> WorLowering {
    items.push(WorItem::Separator {
        text: marker.text().to_string(),
        location: span_of(marker.raw_node()).into(),
    })
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
    // Alignment cannot diagnose numeric overflow after its value was dropped.
    // Producer faults remain internal failures, separate from source rejection.
    // The bullet carries its own source span, so E362 about it is located
    // inside this `%wor` tier.
    match crate::parser::tree_parsing::media_bullet::parse_bullet_node(node, errors) {
        Ok(bullet) => Some(bullet),
        Err(BulletRejection::Producer(fault)) => {
            crate::parser::typed_cst::report_cst_failure(
                node.raw_node(),
                node.source(),
                fault,
                errors,
            );
            None
        }
        Err(why @ BulletRejection::TimeNotRepresentable { .. }) => {
            crate::parser::tree_parsing::media_bullet::report_bullet_rejection(
                node.raw_node(),
                node.source(),
                &why,
                errors,
            );
            None
        }
        Err(BulletRejection::ContainsRecoveryNode | BulletRejection::TimeFieldAbsent { .. }) => {
            None
        }
    }
}
