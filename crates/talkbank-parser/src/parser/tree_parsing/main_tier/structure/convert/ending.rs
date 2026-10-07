//! Utterance-end tail extraction for `main_tier` conversion.
//!
//! Driven by the generated typed visitor: one `extract_utterance_end` call yields
//! the optional terminator (as a typed per-position `UtteranceEndChild0Choice`),
//! the optional `final_codes` (postcodes), the optional trailing media `bullet`
//! (nested one level under an explicit-whitespace group), an explicit trailing
//! `whitespace` slot, and the required `newline`, all as typed slots. This
//! replaces the previous flat positional `node.kind()` loop (the removed
//! `parse_utterance_end` in `utterance_end.rs`), so the 13 terminator subtypes
//! are now a compiler-exhaustive typed match (the shared `terminator_from_new_choice`)
//! instead of a `node.kind()` string dispatch. Each slot is matched EXHAUSTIVELY
//! over [`NodeSlot`], so a recovery node is handled explicitly rather than
//! silently dropped, and the ONE reachable recovery diagnostic (E360
//! `InvalidMediaBullet` on a malformed trailing bullet) is reproduced
//! byte-identically.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Postcodes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::error::ErrorSink;
use crate::generated_traversal::{Absence, Never};
use crate::generated_traversal::{
    AsRawNode, NodeSlot, NonMissingKindSlot as KindSlot, PostcodeNode, SlotView, SourceBound,
    SourceField, SourceSlotView, UtteranceEndNode,
};
use crate::model::{Bullet, Postcode, Terminator};
use crate::parser::tree_parsing::media_bullet::parse_bullet_node;
use crate::parser::tree_parsing::postcode::parse_postcode_node;
use talkbank_model::ParseOutcome;

use super::super::super::content::{
    MainTierBodyCarrier, MainTierRegion, classify_main_tier_recovery, surface_main_tier_sink,
};
use super::super::terminator::terminator_from_new_choice;

/// The utterance-end tail parsed from an `utterance_end` node: the optional
/// terminator, the ordered postcodes, and the optional trailing media bullet. The
/// `parse_tier_body` seam folds these fields into `TierBodyData`. A named struct
/// (not a tuple) keeps this domain seam self-documenting, matching the sibling
/// `TierBodyData`; `Default` is the empty tail used on the recovery arms.
#[derive(Default)]
pub(super) struct UtteranceEndTail {
    pub terminator: Option<Terminator>,
    pub postcodes: Vec<Postcode>,
    pub bullet: Option<Bullet>,
}

/// Decode a typed `utterance_end` node into its terminator, postcodes, and trailing
/// media bullet, reporting any diagnostics into `errors`.
///
/// Driven by `extract_utterance_end`, which yields five ordered slots
/// (`terminator` optional, `final_codes` optional, `bullet` optional -
/// nested under an explicit-whitespace group, an explicit trailing
/// `whitespace` optional, `newline` required). Every slot is matched
/// EXHAUSTIVELY; the valid path emits no diagnostics. Replaces the removed
/// flat-loop `parse_utterance_end`.
pub(super) fn parse_utterance_end<'tree>(
    typed: SourceBound<'tree, '_, UtteranceEndNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<UtteranceEndTail, crate::CstFailure> {
    let source = typed.source();
    let grammar = crate::parser::typed_cst::canonical_grammar()?;
    let associated = typed.extract_admitted(grammar)?;
    let end = associated.children();

    // child_0 (`terminator` supertype, optional). A `Present` choice maps through
    // the exhaustive typed match; an `Error` slot routes to the shared word-error
    // analyzer (as the body.rs seam does for its ERROR arms) and yields no
    // terminator; `Missing` / `Unexpected` / an absent slot yield no terminator and
    // no invented diagnostic (a truly missing terminator is a validation concern,
    // reported at validation time, not here). The NEW-backend `Missing` recovery
    // variant carries a raw `Node` (not a typed choice), matching `Error` /
    // `Unexpected` below, unlike the OLD flattened `Option<NodeSlot<..>>` where
    // `Missing` still carried the typed choice.
    let terminator = match end.child_0.slot() {
        Some(NodeSlot::Present(choice)) => Some(terminator_from_new_choice(choice)),
        Some(NodeSlot::Error(error_node)) => {
            errors.report(classify_main_tier_recovery(
                *error_node,
                source,
                MainTierRegion::Body,
            ));
            None
        }
        Some(NodeSlot::Missing(_) | NodeSlot::Unexpected(_)) | None => None,
        Some(NodeSlot::Absent(never)) => match *never {},
    };

    // child_1 (`final_codes`, optional). Only a `Present` `final_codes` contributes
    // postcodes: re-extract its elements and parse each `Present` `postcode`. The
    // NEW backend models `final_codes = repeat1(seq(whitespaces, postcode))` as a
    // required first group (`child_0`) plus a repeated tail (`child_1`, a Vec of
    // the SAME two-position group shape), because whitespace between codes is now
    // an explicit grammar position rather than skipped; the postcode itself moved
    // from the OLD flat `element.child_0` to the group's `child_1`. Every other
    // element/group slot state is skipped (the safe default, matching the removed
    // loop which acted only on `postcode`-kind children). Error or optional
    // absence yields no postcodes; compiled admission excludes Missing here.
    let mut postcodes: Vec<Postcode> = Vec::new();
    match associated
        .field_child_1()
        .slot()
        .optional()
        .map(|slot| slot.view())
    {
        Some(SourceSlotView::Present(final_codes)) => {
            let associated_codes = final_codes.read()?.extract_admitted(grammar)?;
            let codes = associated_codes.children();
            surface_final_codes_group(codes.child_0.slot(), source, errors);
            // Both generated sequence shapes project the same source-bound
            // postcode slot. Diagnostics above retain their existing owner;
            // semantic decoding never detaches a node from its source.
            let first = match associated_codes.field_child_0().slot().view() {
                SourceSlotView::Present(group) => Some(group.field_child_1().slot()),
                SourceSlotView::Missing(_)
                | SourceSlotView::Error(_)
                | SourceSlotView::Absent(_) => None,
                SourceSlotView::Unexpected(never) => match never {},
            };
            // The raw and associated iterations project the same generated
            // repetition. Keep reporting lazy so each group's diagnostics
            // precede its own decoding, not all decoding in the whole tail.
            let remaining = codes
                .child_1
                .slot()
                .iter()
                .zip(associated_codes.field_child_1().slot().iter())
                .filter_map(|(raw, element)| {
                    surface_final_codes_group(raw.slot(), source, errors);
                    match element.slot().view() {
                        SourceSlotView::Present(group) => Some(group.field_child_1().slot()),
                        SourceSlotView::Missing(_)
                        | SourceSlotView::Error(_)
                        | SourceSlotView::Absent(_) => None,
                        SourceSlotView::Unexpected(never) => match never {},
                    }
                });
            for slot in first.into_iter().chain(remaining) {
                if let Some(postcode) = decode_postcode_slot(slot, errors)? {
                    postcodes.push(postcode);
                }
            }
            surface_main_tier_sink(codes, source, errors);
        }
        Some(SourceSlotView::Error(_)) | None => {}
    }

    // child_2 (`bullet`, optional). The NEW backend groups the trailing
    // `[whitespace?, bullet]` pair into one nested optional carrier
    // (`UtteranceEndChild2Children`) because the interstitial whitespace is now
    // an explicit position; descend to its `child_1` (the bullet) exactly as the
    // B2 nested-group precedent does. A `Present` bullet with valid timestamps
    // becomes a `Bullet` carrying the node span; on `None` timestamps (a
    // grammar-rejected or malformed bullet, e.g. the deprecated `·N_N-·` skip
    // marker) the E360 diagnostic is emitted byte-identically to the removed
    // flat loop, so the file still fails validation. Every other slot state
    // (at either nesting level) yields no bullet, no diagnostic.
    // Keep the established region-specific recovery reporter. Semantic reads
    // below use the associated projection, never this raw diagnostic carrier.
    if let Some(SlotView::Present(group)) = end.child_2.slot().as_ref().map(NodeSlot::view) {
        surface_main_tier_sink(group, source, errors);
    }
    let bullet = match associated
        .field_child_2()
        .slot()
        .optional()
        .map(|slot| slot.view())
    {
        Some(SourceSlotView::Present(group)) => {
            match group.field_child_1().slot().view() {
                SourceSlotView::Present(bullet_node) => {
                    let bullet_node = bullet_node.read()?;
                    let raw = bullet_node.raw_node();
                    match parse_bullet_node(bullet_node, errors) {
                        Ok(bullet) => Some(bullet),
                        Err(
                            crate::parser::tree_parsing::media_bullet::BulletRejection::Producer(
                                fault,
                            ),
                        ) => {
                            return Err(fault);
                        }
                        // The rejection says WHICH route, so the message is
                        // not a guess between four of them. The reporter lives
                        // beside that type; a local copy here wrote a sentence
                        // that was false for its only reachable input.
                        Err(why) => {
                            crate::parser::tree_parsing::media_bullet::report_bullet_rejection(
                                raw, source, &why, errors,
                            );
                            None
                        }
                    }
                }
                SourceSlotView::Missing(_) | SourceSlotView::Error(_) => None,
                SourceSlotView::Absent(_) => None,
            }
        }
        Some(SourceSlotView::Error(_)) | None => None,
    };

    // child_3 (trailing `whitespace`, optional). NEWLY MATERIALIZED position (the
    // OLD backend's `--skip whitespaces` absorbed this silently): structural
    // only, carries no terminator, postcode, or bullet, so every slot state is a
    // no-op. Matched explicitly so no state is silently dropped.
    match end.child_3.slot().as_ref().map(NodeSlot::view) {
        Some(SlotView::Present(_) | SlotView::Missing(_) | SlotView::Error(_)) | None => {}
    }

    // child_4 (`newline`, required; was `child_3` under OLD). Structural only: it
    // carries no terminator, postcode, or bullet, so every slot state is a no-op.
    // Matched explicitly so the required newline slot is never silently dropped.
    match end.child_4.slot().view() {
        SlotView::Present(_) | SlotView::Missing(_) | SlotView::Error(_) | SlotView::Absent(_) => {}
    }

    // Surface the carrier's own `unexpected` sink (R2), classified by region.
    //
    // This is NOT the empty set the comment here used to claim. The same
    // sentence stood over `tier_body`'s sink until a generator fix started
    // absorbing ERROR nodes at the cursor position, which put real content in
    // it and degraded six error codes to E316. "Empty on every fixture probed
    // so far" was a statement about our fixtures, not about the grammar, and it
    // was read as the latter for months.
    surface_main_tier_sink(end, source, errors);

    Ok(UtteranceEndTail {
        terminator,
        postcodes,
        bullet,
    })
}

/// Preserve the original region-specific displaced-node reporting independently
/// of source-bound semantic decoding.
fn surface_final_codes_group<'tree, G: MainTierBodyCarrier<'tree>, A: Absence>(
    group_slot: &NodeSlot<'tree, G, Never, Never, A>,
    source: &str,
    errors: &impl ErrorSink,
) {
    match group_slot.view() {
        SlotView::Present(group) => {
            surface_main_tier_sink(group, source, errors);
        }
        SlotView::Missing(_) | SlotView::Error(_) | SlotView::Absent(_) => {}
    }
}

/// Admit a selected leaf's range before passing its source-bound token to the
/// decoder. Ordinary recovery remains absence; source failure propagates.
fn decode_postcode_slot<'tree>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, PostcodeNode<'tree>>>,
    errors: &impl ErrorSink,
) -> Result<Option<Postcode>, crate::CstFailure> {
    Ok(match slot.view() {
        SourceSlotView::Present(node) => match parse_postcode_node(node.read()?, errors) {
            ParseOutcome::Parsed(postcode) => Some(postcode),
            ParseOutcome::Rejected => None,
        },
        SourceSlotView::Error(_) | SourceSlotView::Absent(_) => None,
    })
}
