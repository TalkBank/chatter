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
//! instead of a `node.kind()` string dispatch. Slot recovery and displaced
//! nodes retain their region-specific reporter; structured bullet rejection
//! remains distinct from malformed syntax and source-admission failure.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Terminators>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Postcodes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::error::ErrorSink;
use crate::generated_traversal::{
    AsRawNode, ReadableSlot, SlotView, SourceBound, UtteranceEndNode,
};
use crate::model::{Postcode, Terminator};
use crate::parser::tree_parsing::media_bullet::{
    BulletRejection, parse_bullet_node, report_bullet_rejection,
};
use crate::parser::tree_parsing::postcode::parse_postcode_node;
use crate::parser::typed_cst::ExtractCanonical;
use talkbank_model::ParseOutcome;

use super::super::super::content::{
    MainTierRegion, classify_main_tier_recovery, surface_main_tier_sink,
};
use super::super::terminator::terminator_from_new_choice;

/// The decoded ending; only ordinary syntax recovery may yield an empty tail.
#[derive(Default)]
pub(super) struct UtteranceEndTail {
    pub terminator: Option<Terminator>,
    pub postcodes: Vec<Postcode>,
    pub bullet: Option<talkbank_model::BulletState>,
}

/// Admit the selected ending fields once, retaining kind and source ownership.
/// Nested payload admission and ordinary recovery remain separate.
pub(super) fn parse_utterance_end<'tree>(
    typed: SourceBound<'tree, '_, UtteranceEndNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<UtteranceEndTail, crate::CstFailure> {
    let source = typed.source();
    let associated = typed.extract_canonical()?.admit_ranges()?;
    let end = associated.children();

    let terminator = match end.child_0.slot.as_ref() {
        Some(ReadableSlot::Present(choice)) => Some(terminator_from_new_choice(choice)),
        Some(ReadableSlot::Error(error_node)) => {
            errors.report(classify_main_tier_recovery(
                error_node.raw_node(),
                source,
                MainTierRegion::Body,
            ));
            None
        }
        Some(ReadableSlot::Missing(_) | ReadableSlot::Unexpected(_)) | None => None,
        Some(ReadableSlot::Absent(never)) => match *never {},
    };

    let mut postcodes = Vec::new();
    match end.child_1.slot.as_ref().map(ReadableSlot::view) {
        Some(SlotView::Present(final_codes)) => {
            let associated_codes = final_codes.extract_canonical()?.admit_ranges()?;
            let codes = associated_codes.children();
            // First/repeated groups have distinct generated types. One
            // expansion preserves each group's reporting-before-decoding order.
            macro_rules! append_code {
                ($slot:expr) => {
                    if let SlotView::Present(group) = $slot.view() {
                        surface_main_tier_sink(group, source, errors);
                        match group.child_1.slot.view() {
                            SlotView::Present(node) => {
                                if let ParseOutcome::Parsed(postcode) =
                                    parse_postcode_node(*node, errors)
                                {
                                    postcodes.push(postcode);
                                }
                            }
                            SlotView::Error(_) | SlotView::Absent(_) => {}
                        }
                    }
                };
            }
            append_code!(&codes.child_0.slot);
            for element in &codes.child_1.slot {
                append_code!(&element.slot);
            }
            surface_main_tier_sink(codes, source, errors);
        }
        Some(SlotView::Error(_)) | None => {}
    }

    let bullet = match end.child_2.slot.as_ref().map(ReadableSlot::view) {
        Some(SlotView::Present(group)) => {
            surface_main_tier_sink(group, source, errors);
            match group.child_1.slot.view() {
                SlotView::Present(bullet_node) => match parse_bullet_node(*bullet_node, errors) {
                    Ok(bullet) => Some(bullet.into()),
                    Err(BulletRejection::Producer(fault)) => return Err(fault),
                    Err(why) => {
                        report_bullet_rejection(bullet_node.raw_node(), source, &why, errors);
                        Some(
                            talkbank_model::UnparsedBullet::new(
                                crate::parser::node_span::span_of(bullet_node.raw_node()),
                                bullet_node.text(),
                            )
                            .into(),
                        )
                    }
                },
                SlotView::Missing(_) | SlotView::Error(_) | SlotView::Absent(_) => None,
            }
        }
        Some(SlotView::Error(_)) | None => None,
    };

    // Trailing whitespace and newline are structural, with no model payload.
    // Range admission retains their recovery; matching every state to a no-op
    // would add no policy. The whole-tree recovery backstop remains in place.
    surface_main_tier_sink(end, source, errors);
    Ok(UtteranceEndTail {
        terminator,
        postcodes,
        bullet,
    })
}
