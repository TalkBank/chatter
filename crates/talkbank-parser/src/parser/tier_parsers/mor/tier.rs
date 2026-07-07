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
    MorContentNode, MorContentsChild0Choice, MorContentsChild0MorContentChild2Child1Choice,
    MorContentsNode, MorDependentTierNode, NodeSlot, WhitespacesNode, extract_mor_contents,
    extract_mor_dependent_tier,
};
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
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_unexpected};

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
    node: Node,
    source: &str,
    tier_type: MorTierType,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let children = extract_mor_dependent_tier(MorDependentTierNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    match children.child_2.slot {
        NodeSlot::Present(contents) => {
            parse_mor_contents_body(contents.0, source, tier_type, span, errors)
        }
        NodeSlot::Missing(raw) => {
            // Reproduces the removed `expect_child_at`'s MISSING arm exactly
            // (its message text, not the shared `check_not_missing` wording,
            // since the removed helper was more specific: it named both the
            // context AND the numeric position). Unreachable in production:
            // `parse_mor_tier` is only invoked when the containing tier node
            // has no tree-sitter error (`dependent_tier_dispatch/parsed.rs`),
            // and a MISSING `mor_contents` at this required, non-optional
            // position would make the tier node `has_error()`.
            errors.report(
                ParseError::new(
                    ErrorCode::MissingRequiredElement,
                    Severity::Error,
                    SourceLocation::from_offsets(raw.start_byte(), raw.end_byte()),
                    ErrorContext::new(source, raw.start_byte()..raw.end_byte(), raw.kind()),
                    format!(
                        "Tree-sitter error recovery: MISSING '{}' node inserted at mor_dependent_tier position 2",
                        raw.kind()
                    ),
                )
                .with_suggestion(
                    "This CHAT construct appears to be invalid or malformed. Check the CHAT format specification for correct syntax.",
                )
                .with_help_url("https://talkbank.org/0info/manuals/CHAT.html"),
            );
            ParseOutcome::Rejected
        }
        NodeSlot::Absent | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
            // Reproduces the removed `expect_child_at`'s "no child at
            // position" arm. Unreachable in production for the same reason
            // as the `Missing` arm above: `mor_dependent_tier` is a rigid
            // 4-position `seq` with no optional/choice ahead of `mor_contents`,
            // so a real, error-free tier always has a `mor_contents`-kind (or
            // MISSING-placeholder) child at this position.
            errors.report(
                ParseError::new(
                    ErrorCode::TreeParsingError,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, node.start_byte()..node.end_byte(), node.kind()),
                    "CST structure mismatch in mor_dependent_tier: no child at position 2. Grammar may have changed!".to_string(),
                )
                .with_suggestion(
                    "Check tree-sitter grammar for 'mor_dependent_tier' - expected at least 3 children",
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
    mor_contents_node: Node,
    source: &str,
    tier_type: MorTierType,
    span: Span,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let contents = extract_mor_contents(MorContentsNode(mor_contents_node));

    match contents.child_0.slot {
        NodeSlot::Present(MorContentsChild0Choice::MorContent(items_children)) => {
            let mut had_item_failure = false;
            let mut items: Vec<Mor> = Vec::with_capacity(items_children.child_1.slot.len() + 1);

            push_mor_content_item(
                items_children.child_0.slot,
                source,
                errors,
                &mut items,
                &mut had_item_failure,
            );
            for element in items_children.child_1.slot {
                match element.slot {
                    NodeSlot::Present(pair) => {
                        push_mor_separator(
                            pair.child_0.slot,
                            source,
                            errors,
                            &mut had_item_failure,
                        );
                        push_mor_content_item(
                            pair.child_1.slot,
                            source,
                            errors,
                            &mut items,
                            &mut had_item_failure,
                        );
                        surface_unexpected(&pair.unexpected, source, errors);
                    }
                    // The generated repeat classifies a whole item as
                    // Present / Error / Absent only (the established finding
                    // for analogous repeats elsewhere); matched exhaustively
                    // regardless, per the project no-`_`-on-project-enums
                    // rule.
                    NodeSlot::Missing(raw) | NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
                        errors.report(unexpected_node_error(raw, source, "mor_contents"));
                        had_item_failure = true;
                    }
                    NodeSlot::Absent => {}
                }
            }
            surface_unexpected(&items_children.unexpected, source, errors);

            let terminator = match items_children.child_2.slot {
                Some(NodeSlot::Present(group)) => {
                    push_mor_separator(group.child_0.slot, source, errors, &mut had_item_failure);
                    let decoded = decode_mor_terminator(
                        group.child_1.slot,
                        source,
                        errors,
                        &mut had_item_failure,
                    );
                    surface_unexpected(&group.unexpected, source, errors);
                    decoded
                }
                Some(NodeSlot::Missing(raw)) => {
                    check_not_missing(raw, source, errors, "mor_contents");
                    had_item_failure = true;
                    None
                }
                Some(NodeSlot::Error(raw)) | Some(NodeSlot::Unexpected(raw)) => {
                    errors.report(unexpected_node_error(raw, source, "mor_contents"));
                    had_item_failure = true;
                    None
                }
                Some(NodeSlot::Absent) | None => None,
            };

            push_mor_trailing_whitespace(
                contents.child_1.slot,
                source,
                errors,
                &mut had_item_failure,
            );

            finish_mor_tier(
                tier_type,
                items,
                terminator,
                had_item_failure,
                span,
                mor_contents_node,
                source,
                errors,
            )
        }
        NodeSlot::Present(MorContentsChild0Choice::BreakForCoding(term_choice)) => {
            let mut had_item_failure = false;
            let terminator = Some(terminator_from_new_choice(&term_choice));

            push_mor_trailing_whitespace(
                contents.child_1.slot,
                source,
                errors,
                &mut had_item_failure,
            );

            finish_mor_tier(
                tier_type,
                Vec::new(),
                terminator,
                had_item_failure,
                span,
                mor_contents_node,
                source,
                errors,
            )
        }
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "mor_contents");
            ParseOutcome::Rejected
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_contents"));
            ParseOutcome::Rejected
        }
        NodeSlot::Absent => {
            // Matches the removed flat walk's empty-loop fallthrough: no real
            // children at all means no items were found and no terminator was
            // found, which is exactly the "missing terminator" outcome below,
            // not a separate structural diagnostic the old code never emitted.
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
    tier_type: MorTierType,
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
    ParseOutcome::Parsed(MorTier::new(tier_type, items, typed_terminator).with_span(span))
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
    slot: NodeSlot<'tree, MorContentNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    items: &mut Vec<Mor>,
    had_item_failure: &mut bool,
) {
    match slot {
        NodeSlot::Present(item_node) => match parse_mor_content(item_node.0, source, errors) {
            ParseOutcome::Parsed(mor) => items.push(mor),
            ParseOutcome::Rejected => *had_item_failure = true,
        },
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "mor_contents");
            *had_item_failure = true;
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_contents"));
            *had_item_failure = true;
        }
        NodeSlot::Absent => {}
    }
}

/// Decode the separating `whitespaces` token between two `mor_content` items,
/// or before a trailing terminator.
///
/// A no-op on `Present`/`Absent` (purely structural, matching the removed
/// loop's `kind::WHITESPACES => {}` arm); `Missing`/`Error`/`Unexpected` report
/// the same diagnostic the removed loop's uniform `check_not_missing`-first
/// gate reported for ANY child (item, separator, or terminator alike), and
/// mark the whole tier failed.
fn push_mor_separator<'tree>(
    slot: NodeSlot<'tree, WhitespacesNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    had_item_failure: &mut bool,
) {
    match slot {
        NodeSlot::Present(_) | NodeSlot::Absent => {}
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "mor_contents");
            *had_item_failure = true;
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_contents"));
            *had_item_failure = true;
        }
    }
}

/// Decode the optional trailing `whitespaces` token after the whole
/// items-or-terminator choice (`mor_contents`'s own `optional(whitespaces)`,
/// a NEW position with no OLD counterpart since `--skip whitespaces` never
/// modeled it). Purely structural; the recovery arms reuse the same
/// diagnostic mechanism as [`push_mor_separator`] for consistency, mirroring
/// the removed loop's uniform "check every child" gate.
fn push_mor_trailing_whitespace<'tree>(
    slot: Option<NodeSlot<'tree, WhitespacesNode<'tree>>>,
    source: &str,
    errors: &impl ErrorSink,
    had_item_failure: &mut bool,
) {
    match slot {
        None | Some(NodeSlot::Present(_)) | Some(NodeSlot::Absent) => {}
        Some(NodeSlot::Missing(raw)) => {
            check_not_missing(raw, source, errors, "mor_contents");
            *had_item_failure = true;
        }
        Some(NodeSlot::Error(raw)) | Some(NodeSlot::Unexpected(raw)) => {
            errors.report(unexpected_node_error(raw, source, "mor_contents"));
            *had_item_failure = true;
        }
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
    slot: NodeSlot<'tree, MorContentsChild0MorContentChild2Child1Choice<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    had_item_failure: &mut bool,
) -> Option<Terminator> {
    match slot {
        NodeSlot::Present(choice) => Some(terminator_from_new_choice(&choice)),
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "mor_contents");
            *had_item_failure = true;
            None
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "mor_contents"));
            *had_item_failure = true;
            None
        }
        NodeSlot::Absent => None,
    }
}
