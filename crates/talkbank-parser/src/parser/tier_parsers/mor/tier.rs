//! Tier-level `%mor` parsing.
//!
//! This file parses one morphology tier line, then delegates each item to
//! `parse_mor_content` and records any terminator token carried in the tier.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#MOR_Format>

use crate::generated_traversal::{
    Absence, AdmittedMorContentsChild0ChoiceSourceView, AsRawNode, KindMissing, KindSlot,
    MorContentNode, MorContentsChild0MorContentChild2Child1Choice, MorContentsNode,
    MorDependentTierNode, Never, NoChild, NodeSlot, NonMissingKindSlot, SourceBound,
    SourceBoundKind, SourceField, SourceSlotView, WhitespacesNode,
};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::main_tier::structure::terminator::terminator_from_new_choice;
use talkbank_model::ParseOutcome;
use talkbank_model::model::content::Terminator;
use talkbank_model::model::{Mor, MorTier, MorTierType};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

use super::item::parse_mor_content;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};

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
/// Only `mor_dependent_tier`'s `child_2` (the body) is ever inspected, exactly
/// as the removed hand-walk never looked at the prefix / tier-sep / newline
/// positions either; `child_0`/`child_1`/`child_3` stay unexamined.
pub fn parse_mor_tier_inner<'tree>(
    typed: SourceBound<'tree, '_, MorDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let source = typed.source();
    let node = typed.raw_node();
    let span = span_of(node);
    let extraction = crate::parser::typed_cst::canonical_grammar()
        .and_then(|grammar| typed.extract_admitted(grammar));
    let Ok(children) = crate::parser::typed_cst::report_reconstruction(
        extraction,
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    surface_displaced(
        &children.children().unexpected,
        "mor_dependent_tier",
        source,
        errors,
    );

    match children.field_child_2().slot().view() {
        SourceSlotView::Present(contents) => {
            match crate::parser::typed_cst::read_source_field(contents, errors) {
                Some(contents) => parse_mor_contents_body(contents, span, errors),
                None => ParseOutcome::Rejected,
            }
        }
        SourceSlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                source,
                "mor_dependent_tier",
            ));
            ParseOutcome::Rejected
        }
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => {
            // The loud signal for a grammar that no longer matches the
            // generator, kept in its own words: the generic reporter would
            // say "Unexpected 'mor_dependent_tier' in mor_dependent_tier".
            errors.report(
                ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
                    "CST structure mismatch in mor_dependent_tier: no mor_contents child. Grammar may have changed!",
                )
                .with_suggestion(
                    "Check tree-sitter grammar for 'mor_dependent_tier' - expected a mor_contents child",
                ),
            );
            ParseOutcome::Rejected
        }
    }
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
    let extraction = crate::parser::typed_cst::canonical_grammar()
        .and_then(|grammar| typed.extract_admitted(grammar));
    let Ok(contents) = crate::parser::typed_cst::report_reconstruction(
        extraction,
        typed.raw_node(),
        source,
        errors,
    ) else {
        return ParseOutcome::Rejected;
    };
    let choice = match contents.field_child_0().slot().view() {
        SourceSlotView::Present(choice) => choice,
        SourceSlotView::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, "mor_contents");
            return ParseOutcome::Rejected;
        }
        SourceSlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                source,
                "mor_contents",
            ));
            return ParseOutcome::Rejected;
        }
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => {
            report_missing_terminator(mor_contents_node, source, errors);
            return ParseOutcome::Rejected;
        }
    };
    match choice.view() {
        AdmittedMorContentsChild0ChoiceSourceView::MorContent(items_children) => {
            let mut items =
                MorItems::with_capacity(items_children.field_child_1().slot().iter().len() + 1);

            push_mor_content_item(items_children.field_child_0().slot(), errors, &mut items);
            // Sequence payloads retain their source through repetition. The
            // existing SeqSlot type makes Missing/Unexpected uninhabited;
            // Error and Absent remain explicit, as do each item's own states.
            for element in items_children.field_child_1().slot().iter() {
                match element.slot().view() {
                    SourceSlotView::Present(pair) => {
                        require_structure(pair.field_child_0().slot(), errors, &mut items);
                        push_mor_content_item(pair.field_child_1().slot(), errors, &mut items);
                        surface_source_displaced(pair.field_unexpected(), errors);
                    }
                    SourceSlotView::Error(bad) => {
                        errors.report(unexpected_node_error(
                            bad.raw_node(),
                            source,
                            "mor_contents",
                        ));
                        items.reject();
                    }
                    SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => {
                        match never {}
                    }
                    SourceSlotView::Absent(never) => match never {},
                }
            }
            surface_source_displaced(items_children.field_unexpected(), errors);

            let terminator = match items_children.field_child_2().slot().optional() {
                Some(slot) => match slot.view() {
                    SourceSlotView::Present(group) => {
                        require_structure(group.field_child_0().slot(), errors, &mut items);
                        let decoded =
                            decode_mor_terminator(group.field_child_1().slot(), errors, &mut items);
                        surface_source_displaced(group.field_unexpected(), errors);
                        decoded
                    }
                    SourceSlotView::Error(bad) => {
                        errors.report(unexpected_node_error(
                            bad.raw_node(),
                            source,
                            "mor_contents",
                        ));
                        items.reject();
                        None
                    }
                    SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => {
                        match never {}
                    }
                    SourceSlotView::Absent(never) => match never {},
                },
                None => None,
            };

            if let Some(slot) = contents.field_child_1().slot().optional() {
                require_structure(slot, errors, &mut items);
            }

            finish_mor_tier(items, terminator, span, mor_contents_node, source, errors)
        }
        AdmittedMorContentsChild0ChoiceSourceView::BreakForCoding(term_choice) => {
            let mut items = MorItems::with_capacity(0);
            let Some(term_choice) =
                crate::parser::typed_cst::read_source_field(term_choice, errors)
            else {
                return ParseOutcome::Rejected;
            };
            let terminator = Some(terminator_from_new_choice(&term_choice.node()));

            if let Some(slot) = contents.field_child_1().slot().optional() {
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
    ParseOutcome::Parsed(MorTier::new(MorTierType::Mor, items, typed_terminator).with_span(span))
}

/// Reports the same `MissingTerminator` diagnostic the removed hand-walk
/// reported when its loop ended without ever setting `terminator`.
fn report_missing_terminator(mor_contents_node: Node, source: &str, errors: &impl ErrorSink) {
    errors.report(ParseError::new(
        ErrorCode::MissingTerminator,
        Severity::Error,
        SourceLocation::from_offsets(mor_contents_node.start_byte(), mor_contents_node.end_byte()),
        ErrorContext::new(
            source,
            mor_contents_node.start_byte()..mor_contents_node.end_byte(),
            "mor_dependent_tier",
        ),
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
/// - `Missing` / `Unexpected`: uninhabited for this admitted composite slot.
/// - `Error`: report recovery and reject the whole tier.
/// - `Absent`: no child at this position; nothing reported, nothing pushed.
fn push_mor_content_item<'tree>(
    slot: SourceField<'_, 'tree, '_, NonMissingKindSlot<'tree, MorContentNode<'tree>>>,
    errors: &impl ErrorSink,
    items: &mut MorItems,
) {
    let item_node = match slot.view() {
        SourceSlotView::Present(item) => {
            match crate::parser::typed_cst::read_source_field(item, errors) {
                Some(item) => item,
                None => {
                    items.reject();
                    return;
                }
            }
        }
        SourceSlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                slot.source(),
                "mor_contents",
            ));
            items.reject();
            return;
        }
        SourceSlotView::Absent(NoChild) => return,
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
/// Absent need nothing, and a reported recovery marks the whole tier failed,
/// as it does for an item or a terminator.
fn require_structure<'tree, A: Absence>(
    slot: SourceField<
        '_,
        'tree,
        '_,
        NodeSlot<'tree, WhitespacesNode<'tree>, KindMissing<WhitespacesNode<'tree>>, Never, A>,
    >,
    errors: &impl ErrorSink,
    items: &mut MorItems,
) {
    match admit_present_kind(slot, errors) {
        MorSlotAdmission::Present(_) | MorSlotAdmission::Absent(_) => {}
        MorSlotAdmission::Rejected => items.reject(),
    }
}

/// Decode the terminator inside the optional trailing `(whitespaces,
/// terminator)` group of the items alternative. `Present` maps through the
/// SHARED exhaustive [`terminator_from_new_choice`] (the same 13-arm mapping
/// `convert/ending.rs` and `tier_parsers/wor.rs` use); the recovery arms
/// mirror [`push_mor_separator`]'s policy since the removed loop applied the
/// SAME uniform `check_not_missing`-first gate to the terminator child as to
/// every other child in `mor_contents`.
fn decode_mor_terminator<'tree>(
    slot: SourceField<
        '_,
        'tree,
        '_,
        KindSlot<'tree, MorContentsChild0MorContentChild2Child1Choice<'tree>>,
    >,
    errors: &impl ErrorSink,
    items: &mut MorItems,
) -> Option<Terminator> {
    match admit_present_kind(slot, errors) {
        MorSlotAdmission::Present(choice) => Some(terminator_from_new_choice(&choice.node())),
        MorSlotAdmission::Rejected => {
            items.reject();
            None
        }
        MorSlotAdmission::Absent(_) => None,
    }
}

/// Present content is readable; absence is not a reported recovery failure.
enum MorSlotAdmission<T, A = NoChild> {
    Present(T),
    Absent(A),
    Rejected,
}

/// Preserve this tier's check-not-missing policy before admitting source text.
fn admit_present_kind<'tree, 'source, T: SourceBoundKind<'tree>, A: Absence>(
    slot: SourceField<'_, 'tree, 'source, NodeSlot<'tree, T, KindMissing<T>, Never, A>>,
    errors: &impl ErrorSink,
) -> MorSlotAdmission<SourceBound<'tree, 'source, T>, A> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(field) => {
            match crate::parser::typed_cst::read_source_field(field, errors) {
                Some(node) => MorSlotAdmission::Present(node),
                None => MorSlotAdmission::Rejected,
            }
        }
        SourceSlotView::Missing(missing) => {
            check_not_missing(missing.raw_node(), source, errors, "mor_contents");
            MorSlotAdmission::Rejected
        }
        SourceSlotView::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                source,
                "mor_contents",
            ));
            MorSlotAdmission::Rejected
        }
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(absent) => MorSlotAdmission::Absent(absent),
    }
}

fn surface_source_displaced<'tree>(
    unexpected: SourceField<'_, 'tree, '_, Vec<Node<'tree>>>,
    errors: &impl ErrorSink,
) {
    for node in unexpected.iter() {
        surface_displaced(&[node.raw_node()], "mor_contents", node.source(), errors);
    }
}
