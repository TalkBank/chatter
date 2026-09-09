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
    AsRawNode, ChildSlot, MorContentNode, MorContentsChild0Choice,
    MorContentsChild0MorContentChild2Child1Choice, MorContentsNode, MorDependentTierNode,
    WhitespacesNode, extract_mor_contents, extract_mor_dependent_tier,
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
use crate::parser::tree_parsing::parser_helpers::{SlotState, expect_present, surface_displaced};

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
pub fn parse_mor_tier_inner(
    typed: MorDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = extract_mor_dependent_tier(typed);
    surface_displaced(&children.unexpected, "mor_dependent_tier", source, errors);

    match expect_present(
        children.child_2.slot(),
        "mor_dependent_tier",
        source,
        errors,
    ) {
        SlotState::Present(contents) => parse_mor_contents_body(*contents, source, span, errors),
        // Unreachable: this parser runs only on a tier node with no
        // tree-sitter error (`dependent_tier_dispatch/parsed.rs`), and
        // `mor_contents` is a required position of a rigid `seq`, so it is
        // Present there. The recovery arm is reported by the verb; an empty
        // position would be the generator disagreeing with the grammar.
        SlotState::Recovered => ParseOutcome::Rejected,
        SlotState::Absent => {
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
/// Per-item and per-terminator failures accumulate into `had_item_failure`
/// rather than dropping the offending item silently; any accumulated failure
/// rejects the WHOLE tier (matching the removed hand-walk's `had_item_failure`
/// flag), so a partially-malformed `%mor` line never surfaces a miscounted
/// tier to cross-tier validators.
fn parse_mor_contents_body(
    typed: MorContentsNode<'_>,
    source: &str,
    span: Span,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let mor_contents_node = typed.raw_node();
    let contents = extract_mor_contents(typed);

    match expect_present(contents.child_0.slot(), "mor_contents", source, errors) {
        SlotState::Present(MorContentsChild0Choice::MorContent(items_children)) => {
            let mut had_item_failure = false;
            let mut items: Vec<Mor> = Vec::with_capacity(items_children.child_1.slot().len() + 1);

            push_mor_content_item(
                items_children.child_0.slot(),
                source,
                errors,
                &mut items,
                &mut had_item_failure,
            );
            // A MISSING item here (which the generator never produces at a
            // repeat level) is reported as the MISSING it is, where the old
            // arm folded it into the "unexpected node" wording.
            for element in items_children.child_1.slot() {
                match expect_present(element.slot(), "mor_contents", source, errors) {
                    SlotState::Present(pair) => {
                        require_structure(
                            pair.child_0.slot(),
                            source,
                            errors,
                            &mut had_item_failure,
                        );
                        push_mor_content_item(
                            pair.child_1.slot(),
                            source,
                            errors,
                            &mut items,
                            &mut had_item_failure,
                        );
                        surface_displaced(&pair.unexpected, "mor_contents", source, errors);
                    }
                    SlotState::Recovered => had_item_failure = true,
                    SlotState::Absent => {}
                }
            }
            surface_displaced(&items_children.unexpected, "mor_contents", source, errors);

            let terminator = match items_children.child_2.slot() {
                Some(slot) => match expect_present(slot, "mor_contents", source, errors) {
                    SlotState::Present(group) => {
                        require_structure(
                            group.child_0.slot(),
                            source,
                            errors,
                            &mut had_item_failure,
                        );
                        let decoded = decode_mor_terminator(
                            group.child_1.slot(),
                            source,
                            errors,
                            &mut had_item_failure,
                        );
                        surface_displaced(&group.unexpected, "mor_contents", source, errors);
                        decoded
                    }
                    SlotState::Recovered => {
                        had_item_failure = true;
                        None
                    }
                    SlotState::Absent => None,
                },
                None => None,
            };

            if let Some(slot) = contents.child_1.slot() {
                require_structure(slot, source, errors, &mut had_item_failure);
            }

            finish_mor_tier(
                items,
                terminator,
                had_item_failure,
                span,
                mor_contents_node,
                source,
                errors,
            )
        }
        SlotState::Present(MorContentsChild0Choice::BreakForCoding(term_choice)) => {
            let mut had_item_failure = false;
            let terminator = Some(terminator_from_new_choice(term_choice));

            if let Some(slot) = contents.child_1.slot() {
                require_structure(slot, source, errors, &mut had_item_failure);
            }

            finish_mor_tier(
                Vec::new(),
                terminator,
                had_item_failure,
                span,
                mor_contents_node,
                source,
                errors,
            )
        }
        SlotState::Recovered => ParseOutcome::Rejected,
        SlotState::Absent => {
            // No items and no terminator is the "missing terminator" outcome,
            // not a separate structural diagnostic.
            report_missing_terminator(mor_contents_node, source, errors);
            ParseOutcome::Rejected
        }
    }
}

/// Shared tail: enforce the "any item/terminator failure rejects the whole
/// tier" policy, then the "a terminator must have been found" policy, in that
/// order (matching the removed hand-walk's `if had_item_failure { reject }`
/// running BEFORE its terminator-missing check).
#[allow(clippy::too_many_arguments)]
fn finish_mor_tier(
    items: Vec<Mor>,
    terminator: Option<Terminator>,
    had_item_failure: bool,
    span: Span,
    mor_contents_node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    if had_item_failure {
        return ParseOutcome::Rejected;
    }
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
/// Matched EXHAUSTIVELY over [`NodeSlot`] (no `_` catch-all), reproducing the
/// removed per-child dispatch:
///
/// - `Present`: delegate to [`parse_mor_content`]; a `Rejected` item marks the
///   whole tier failed (no fabricated default) rather than being dropped
///   silently.
/// - `Missing`: the removed loop's `check_not_missing` reported
///   `MissingRequiredElement` (E342) and skipped the child without attempting
///   `parse_mor_content`; reproduced identically.
/// - `Error` / `Unexpected`: the removed loop's `_` arm reported
///   `unexpected_node_error`; reproduced identically.
/// - `Absent`: no child at this position; nothing reported, nothing pushed.
fn push_mor_content_item<'tree>(
    slot: &ChildSlot<'tree, MorContentNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut Vec<Mor>,
    had_item_failure: &mut bool,
) {
    match expect_present(slot, "mor_contents", source, errors) {
        SlotState::Present(item_node) => match parse_mor_content(*item_node, source, errors) {
            ParseOutcome::Parsed(mor) => items.push(mor),
            ParseOutcome::Rejected => *had_item_failure = true,
        },
        SlotState::Recovered => *had_item_failure = true,
        SlotState::Absent => {}
    }
}

/// A `whitespaces` position (between two items, before a trailing
/// terminator, or after the whole body): purely structural, so Present and
/// Absent need nothing, and a reported recovery marks the whole tier failed,
/// as it does for an item or a terminator.
fn require_structure<'tree>(
    slot: &ChildSlot<'tree, WhitespacesNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    had_item_failure: &mut bool,
) {
    match expect_present(slot, "mor_contents", source, errors) {
        SlotState::Present(_) | SlotState::Absent => {}
        SlotState::Recovered => *had_item_failure = true,
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
    slot: &ChildSlot<'tree, MorContentsChild0MorContentChild2Child1Choice<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    had_item_failure: &mut bool,
) -> Option<Terminator> {
    match expect_present(slot, "mor_contents", source, errors) {
        SlotState::Present(choice) => Some(terminator_from_new_choice(choice)),
        SlotState::Recovered => {
            *had_item_failure = true;
            None
        }
        SlotState::Absent => None,
    }
}
