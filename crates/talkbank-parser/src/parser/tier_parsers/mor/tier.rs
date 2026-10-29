//! Tier-level `%mor` parsing.
//!
//! This file parses one morphology tier line, then delegates each item to
//! `parse_mor_content` and records any terminator token carried in the tier.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>
//!
//! **Recovery arms.** [`parse_mor_tier_inner`] takes a
//! [`HealthyTier`]: a tier tree-sitter flagged `has_error() == false`, which
//! means no descendant is an ERROR or MISSING node. The dispatcher admits
//! every `%mor` tier through [`TierAdmission`](crate::parser::tier_parsers::admission::TierAdmission)
//! before parsing (the whole-tier recovery walk reports a recovery carrier
//! and substitutes an empty placeholder), and every fragment route wraps its
//! input in a document and takes the same dispatcher. So a slot here in the
//! `Missing` or `Error` state contradicts the admission; each such arm
//! reports [`CstFailure::RecoveryInsideHealthyTier`] and rejects the tier,
//! and none builds a CHAT diagnostic. Confirmed, not established, by
//! measurement: a sweep of 1,662 single-edit mutations of a `%mor` line
//! (2026-10-19, coverage drive batch 2) produced none of the MISSING or
//! unexpected-node messages these arms used to build, and the slot-state
//! census over the repository corpora (2026-09-09) saw `mor_word.child_2`,
//! `mor_feature.child_1` and `mor_contents.child_1` in a recovery state only
//! in trees that carry `has_error()`. An `Absent` required position is a
//! different walk fault and reports [`CstFailure::AbsentRequiredPosition`].
//! The CHAT diagnostics this file still builds are E305 for a tier whose
//! grammar-optional terminator is absent (reachable and pinned) and E331 for
//! a displaced node the walk's sink hands `surface_source_displaced`.

use crate::generated_traversal::{
    Absence, AsRawNode, KindSlot, MorContentNode, MorContentsChild0Choice,
    MorContentsChild0MorContentChild2Child1Choice, MorContentsNode, MorDependentTierNode, Never,
    NoChild, Ranged, ReadableSlot, SlotView, SourceBound, SourceSlice, WhitespacesNode,
};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::main_tier::structure::terminator::{
    RangedTerminatorChoice, terminator_from_new_choice,
};
use crate::parser::typed_cst::ExtractCanonical;
use talkbank_model::ParseOutcome;
use talkbank_model::model::content::Terminator;
use talkbank_model::model::{Mor, MorTier, MorTierType};
use talkbank_model::{
    DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, Span,
};
use tree_sitter::Node;

use super::item::parse_mor_content;
use crate::node_types::MOR_CONTENTS;
use crate::parser::tier_parsers::admission::{HealthyTier, recovery_inside_healthy_tier};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use crate::parser::typed_cst::{CstFailure, GeneratedPosition, report_cst_failure};

/// Converts `%mor` tier content into a `MorTier`.
///
/// **Grammar Rule:**
/// ```text
/// mor_dependent_tier: $ => seq(
///     $.mor_tier_prefix,   // Position 0
///     $.tier_sep,          // Position 1
///     $.mor_contents,      // Position 2
///     $.newline            // Position 3
/// )
///
/// mor_contents: $ => seq(
///     choice(
///         seq(mor_content, repeat(seq(whitespaces, mor_content)), optional(seq(whitespaces, terminator))),
///         terminator
///     ),
///     optional(whitespaces)
/// )
/// ```
///
/// Driven by the generated typed visitor: `extract_mor_dependent_tier` yields the
/// prefix / tier-sep / body / newline as typed `Positioned` slots, and
/// `extract_mor_contents` exposes the body's own choice (items-with-optional-
/// terminator, or a bare terminator) plus the trailing optional whitespace. This
/// replaces the removed positional `expect_child_at(node, 2, ...)` get plus the
/// flat `while mor_contents.child(idx)` scan that dispatched each child by
/// `node.kind()`.
///
/// All selected ranges are admitted together before lowering. Only the body's
/// slot is classified here; prefix, separator and newline retain their existing
/// recovery policy rather than gaining lexical checks from range admission.
pub fn parse_mor_tier_inner<'tree>(
    tier: HealthyTier<'tree, '_, MorDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let typed = tier.node();
    let source = typed.source();
    let node = typed.raw_node();
    let span = span_of(node);
    let extraction = crate::parser::typed_cst::extract_envelope(typed);
    let Ok(children) =
        crate::parser::typed_cst::report_admission(extraction, typed.raw_node(), source, errors)
    else {
        return ParseOutcome::Rejected;
    };
    surface_displaced(
        &children.children().unexpected,
        "mor_dependent_tier",
        source,
        errors,
    );

    let ranges = children.admit_ranges().map_err(Into::into);
    let Ok(children) = crate::parser::typed_cst::report_admission(ranges, node, source, errors)
    else {
        return ParseOutcome::Rejected;
    };
    let SlotView::Present(contents) = children.children().child_2.slot.view();
    parse_mor_contents_body(*contents, span, errors)
}

/// Decodes the `mor_contents` body: either items (with an optional trailing
/// terminator) or a bare terminator, followed by optional trailing whitespace.
///
/// Per-item and per-terminator failures irreversibly reject the collection.
/// Traversal continues to report diagnostics, but later successfully decoded
/// items cannot restore admission or expose a partial tier to validators.
fn parse_mor_contents_body<'tree>(
    typed: SourceBound<'tree, '_, MorContentsNode<'tree>>,
    span: Span,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let source = typed.source();
    let mor_contents_node = typed.raw_node();
    let extraction = typed
        .extract_canonical()
        .and_then(|children| children.admit_ranges().map_err(Into::into));
    let Ok(contents) =
        crate::parser::typed_cst::report_admission(extraction, typed.raw_node(), source, errors)
    else {
        return ParseOutcome::Rejected;
    };
    let contents = contents.children();
    let choice = match &contents.child_0.slot {
        ReadableSlot::Present(choice) => choice,
        ReadableSlot::Missing(missing) => {
            recovery_inside_healthy_tier(missing.raw_node(), source, MOR_CONTENTS, errors);
            return ParseOutcome::Rejected;
        }
        ReadableSlot::Error(bad) => {
            recovery_inside_healthy_tier(bad.raw_node(), bad.source(), MOR_CONTENTS, errors);
            return ParseOutcome::Rejected;
        }
        ReadableSlot::Unexpected(never) => match *never {},
        // The body choice is a required position; the walk cannot leave it
        // empty (the grammar has no fail-closed partials), so this is the
        // producer fault, not a terminator the tier lacks.
        ReadableSlot::Absent(NoChild) => {
            report_cst_failure(
                mor_contents_node,
                source,
                CstFailure::AbsentRequiredPosition {
                    rule: MOR_CONTENTS,
                    position: GeneratedPosition::new("MorContentsChildren.child_0"),
                },
                errors,
            );
            return ParseOutcome::Rejected;
        }
    };
    match choice {
        MorContentsChild0Choice::MorContent(items_children) => {
            let mut items = MorItems::with_capacity(items_children.child_1.slot.len() + 1);

            push_mor_content_item(&items_children.child_0.slot, errors, &mut items);
            // Sequence payloads retain their source through repetition. The
            // existing SeqSlot type makes Missing/Unexpected uninhabited;
            // Error and Absent remain explicit, as do each item's own states.
            for element in &items_children.child_1.slot {
                match &element.slot {
                    ReadableSlot::Present(pair) => {
                        require_structure(&pair.child_0.slot, errors, &mut items);
                        push_mor_content_item(&pair.child_1.slot, errors, &mut items);
                        surface_source_displaced(&pair.unexpected, errors);
                    }
                    ReadableSlot::Error(bad) => {
                        recovery_inside_healthy_tier(
                            bad.raw_node(),
                            bad.source(),
                            MOR_CONTENTS,
                            errors,
                        );
                        items.reject();
                    }
                    ReadableSlot::Missing(never) | ReadableSlot::Unexpected(never) => {
                        match *never {}
                    }
                    ReadableSlot::Absent(never) => match *never {},
                }
            }
            surface_source_displaced(&items_children.unexpected, errors);

            let terminator = match &items_children.child_2.slot {
                Some(slot) => match slot {
                    ReadableSlot::Present(group) => {
                        require_structure(&group.child_0.slot, errors, &mut items);
                        let decoded =
                            decode_mor_terminator(&group.child_1.slot, errors, &mut items);
                        surface_source_displaced(&group.unexpected, errors);
                        decoded
                    }
                    ReadableSlot::Error(bad) => {
                        recovery_inside_healthy_tier(
                            bad.raw_node(),
                            bad.source(),
                            MOR_CONTENTS,
                            errors,
                        );
                        items.reject();
                        None
                    }
                    ReadableSlot::Missing(never) | ReadableSlot::Unexpected(never) => {
                        match *never {}
                    }
                    ReadableSlot::Absent(never) => match *never {},
                },
                None => None,
            };

            if let Some(slot) = &contents.child_1.slot {
                require_structure(slot, errors, &mut items);
            }

            finish_mor_tier(items, terminator, span, mor_contents_node, source, errors)
        }
        MorContentsChild0Choice::BreakForCoding(term_choice) => {
            let mut items = MorItems::with_capacity(0);
            let terminator = Some(terminator_from_new_choice(term_choice));

            if let Some(slot) = &contents.child_1.slot {
                require_structure(slot, errors, &mut items);
            }

            finish_mor_tier(items, terminator, span, mor_contents_node, source, errors)
        }
    }
}

/// Collected items and admission health cannot be supplied independently.
/// Reporting continues after rejection, but later items cannot revive a tier.
enum MorItems {
    Collecting(Vec<Mor>),
    Rejected,
}

impl MorItems {
    fn with_capacity(capacity: usize) -> Self {
        Self::Collecting(Vec::with_capacity(capacity))
    }

    fn reject(&mut self) {
        *self = Self::Rejected;
    }

    fn push(&mut self, item: Mor) {
        if let Self::Collecting(items) = self {
            items.push(item);
        }
    }
}

/// Shared tail: enforce the "any item/terminator failure rejects the whole
/// tier" policy, then the "a terminator must have been found" policy, in that
/// order. Rejected collection precedes the terminator-missing check.
fn finish_mor_tier(
    items: MorItems,
    terminator: Option<Terminator>,
    span: Span,
    mor_contents_node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let MorItems::Collecting(items) = items else {
        return ParseOutcome::Rejected;
    };
    let Some(typed_terminator) = terminator else {
        report_missing_terminator(mor_contents_node, source, errors);
        return ParseOutcome::Rejected;
    };
    // `MorTierType` has one variant, so it carried no information as a
    // parameter: it was threaded through three signatures from one caller that
    // always passed `Mor`, and examined nowhere but here. Named at the single
    // site that builds the tier. If a second `%mor`-family tier ever lands, the
    // answer is a sum type carrying its own node, on the `PhoBodyTier` model,
    // not a re-added argument that a caller can get wrong.
    ParseOutcome::Parsed(MorTier::new(
        span,
        MorTierType::Mor,
        items,
        typed_terminator,
    ))
}

/// Reports the same `MissingTerminator` diagnostic the removed hand-walk
/// reported when its loop ended without ever setting `terminator`.
fn report_missing_terminator(mor_contents_node: Node, source: &str, errors: &impl ErrorSink) {
    errors.report(ParseError::new(
        ErrorCode::MissingTerminator,
        Severity::Error,
        DiagnosticLocation::new(span_of(mor_contents_node)),
        ErrorContext::new(source, span_of(mor_contents_node), "mor_dependent_tier"),
        "%mor tier is missing a terminator".to_string(),
    ));
}

/// Decode one `mor_content` item slot, pushing it onto `items` when it parses.
///
/// Matched exhaustively over the canonical admitted item slot:
///
/// - `Present`: delegate to [`parse_mor_content`]; a `Rejected` item marks the
///   whole tier failed (no fabricated default) rather than being dropped
///   silently.
/// - `Missing`: excluded by compiled admission of this composite kind.
/// - `Unexpected`: uninhabited for this kind-selected slot.
/// - `Error`: contradicts the tier's admission; report the fault and reject
///   the whole tier.
/// - `Absent`: no child at this position; nothing reported, nothing pushed.
fn push_mor_content_item<'tree, 'source, A: crate::generated_traversal::Absence>(
    slot: &crate::generated_traversal::NonMissingKindSlot<
        'tree,
        SourceBound<'tree, 'source, MorContentNode<'tree>>,
        Ranged<'source>,
        A,
    >,
    errors: &impl ErrorSink,
    items: &mut MorItems,
) {
    let item_node = match slot {
        ReadableSlot::Present(item) => *item,
        ReadableSlot::Error(bad) => {
            recovery_inside_healthy_tier(bad.raw_node(), bad.source(), MOR_CONTENTS, errors);
            items.reject();
            return;
        }
        ReadableSlot::Missing(missing) => match missing.0 {},
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Absent(_) => return,
    };
    match parse_mor_content(item_node, errors) {
        Ok(ParseOutcome::Parsed(mor)) => items.push(mor),
        Ok(ParseOutcome::Rejected) => items.reject(),
        Err(failure) => {
            crate::parser::typed_cst::report_cst_failure(
                item_node.raw_node(),
                item_node.source(),
                failure,
                errors,
            );
            items.reject();
        }
    }
}

/// A `whitespaces` position (between two items, before a trailing
/// terminator, or after the whole body): purely structural, so Present and
/// Absent need nothing, and a recovery state contradicts the tier's
/// admission and marks the whole tier failed, as it does for an item or a
/// terminator.
fn require_structure<'tree, 'source, A: Absence>(
    slot: &ReadableSlot<
        SourceBound<'tree, 'source, WhitespacesNode<'tree>>,
        SourceBound<'tree, 'source, WhitespacesNode<'tree>>,
        SourceSlice<'tree, 'source>,
        Never,
        A,
    >,
    errors: &impl ErrorSink,
    items: &mut MorItems,
) {
    match slot {
        ReadableSlot::Present(_) | ReadableSlot::Absent(_) => {}
        ReadableSlot::Missing(missing) => {
            recovery_inside_healthy_tier(
                missing.raw_node(),
                missing.source(),
                MOR_CONTENTS,
                errors,
            );
            items.reject();
        }
        ReadableSlot::Error(bad) => {
            recovery_inside_healthy_tier(bad.raw_node(), bad.source(), MOR_CONTENTS, errors);
            items.reject();
        }
        ReadableSlot::Unexpected(never) => match *never {},
    }
}

/// Decode the terminator inside the optional trailing `(whitespaces,
/// terminator)` group of the items alternative. `Present` maps through the
/// SHARED exhaustive [`terminator_from_new_choice`] (the same 13-arm mapping
/// `convert/ending.rs` and `tier_parsers/wor.rs` use); a recovery state
/// contradicts the tier's admission, as at every other position in
/// `mor_contents`.
fn decode_mor_terminator<'tree, 'source>(
    slot: &KindSlot<
        'tree,
        MorContentsChild0MorContentChild2Child1Choice<'tree, Ranged<'source>>,
        Ranged<'source>,
    >,
    errors: &impl ErrorSink,
    items: &mut MorItems,
) -> Option<Terminator> {
    match slot {
        ReadableSlot::Present(choice) => Some(terminator_from_new_choice(choice)),
        ReadableSlot::Missing(missing) => {
            let missing = missing.source_slice();
            recovery_inside_healthy_tier(
                missing.raw_node(),
                missing.source(),
                MOR_CONTENTS,
                errors,
            );
            items.reject();
            None
        }
        ReadableSlot::Error(bad) => {
            recovery_inside_healthy_tier(bad.raw_node(), bad.source(), MOR_CONTENTS, errors);
            items.reject();
            None
        }
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Absent(NoChild) => None,
    }
}

fn surface_source_displaced<'tree>(unexpected: &[SourceSlice<'tree, '_>], errors: &impl ErrorSink) {
    for node in unexpected {
        surface_displaced(&[node.raw_node()], "mor_contents", node.source(), errors);
    }
}
