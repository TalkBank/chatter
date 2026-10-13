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
    Absence, AdmittedMorContentsChild0Choice, AsRawNode, KindSlot, MorContentNode,
    MorContentsChild0MorContentChild2Child1Choice, MorContentsNode, MorDependentTierNode, Never,
    NoChild, NonMissingKindSlot, Ranged, ReadableSlot, SourceBound, SourceSlice, WhitespacesNode,
};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::main_tier::structure::terminator::{
    RangedTerminatorChoice, terminator_from_new_choice,
};
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
/// All selected ranges are admitted together before lowering. Only the body's
/// slot is classified here; prefix, separator and newline retain their existing
/// recovery policy rather than gaining lexical checks from range admission.
pub fn parse_mor_tier_inner<'tree>(
    typed: SourceBound<'tree, '_, MorDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> ParseOutcome<MorTier> {
    let source = typed.source();
    let node = typed.raw_node();
    let span = span_of(node);
    let extraction = crate::parser::typed_cst::canonical_grammar()
        .and_then(|grammar| typed.extract_admitted(grammar).map_err(Into::into));
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
    match &children.children().child_2.slot {
        ReadableSlot::Present(contents) => parse_mor_contents_body(*contents, span, errors),
        ReadableSlot::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                bad.source(),
                "mor_dependent_tier",
            ));
            ParseOutcome::Rejected
        }
        ReadableSlot::Missing(never) => match never.0 {},
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Absent(NoChild) => {
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
        .and_then(|grammar| typed.extract_admitted(grammar).map_err(Into::into))
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
            check_not_missing(missing.raw_node(), missing.source(), errors, "mor_contents");
            return ParseOutcome::Rejected;
        }
        ReadableSlot::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                bad.source(),
                "mor_contents",
            ));
            return ParseOutcome::Rejected;
        }
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Absent(NoChild) => {
            report_missing_terminator(mor_contents_node, source, errors);
            return ParseOutcome::Rejected;
        }
    };
    match choice {
        AdmittedMorContentsChild0Choice::MorContent(items_children) => {
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
                        errors.report(unexpected_node_error(
                            bad.raw_node(),
                            bad.source(),
                            "mor_contents",
                        ));
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
                        errors.report(unexpected_node_error(
                            bad.raw_node(),
                            bad.source(),
                            "mor_contents",
                        ));
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
        AdmittedMorContentsChild0Choice::BreakForCoding(term_choice) => {
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
fn push_mor_content_item<'tree, 'source>(
    slot: &NonMissingKindSlot<
        'tree,
        SourceBound<'tree, 'source, MorContentNode<'tree>>,
        Ranged<'source>,
    >,
    errors: &impl ErrorSink,
    items: &mut MorItems,
) {
    let item_node = match slot {
        ReadableSlot::Present(item) => *item,
        ReadableSlot::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                bad.source(),
                "mor_contents",
            ));
            items.reject();
            return;
        }
        ReadableSlot::Missing(never) => match never.0 {},
        ReadableSlot::Unexpected(never) => match *never {},
        ReadableSlot::Absent(NoChild) => return,
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
fn require_structure<'tree, 'source, A: Absence>(
    slot: &ReadableSlot<
        SourceBound<'tree, 'source, WhitespacesNode<'tree>>,
        SourceBound<'tree, 'source, WhitespacesNode<'tree>>,
        Never,
        A,
    >,
    errors: &impl ErrorSink,
    items: &mut MorItems,
) {
    match slot {
        ReadableSlot::Present(_) | ReadableSlot::Absent(_) => {}
        ReadableSlot::Missing(missing) => {
            check_not_missing(missing.raw_node(), missing.source(), errors, "mor_contents");
            items.reject();
        }
        ReadableSlot::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                bad.source(),
                "mor_contents",
            ));
            items.reject();
        }
        ReadableSlot::Unexpected(never) => match *never {},
    }
}

/// Decode the terminator inside the optional trailing `(whitespaces,
/// terminator)` group of the items alternative. `Present` maps through the
/// SHARED exhaustive [`terminator_from_new_choice`] (the same 13-arm mapping
/// `convert/ending.rs` and `tier_parsers/wor.rs` use); the recovery arms
/// retain the structural whitespace policy since the removed loop applied the
/// SAME uniform `check_not_missing`-first gate to the terminator child as to
/// every other child in `mor_contents`.
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
            check_not_missing(missing.raw_node(), missing.source(), errors, "mor_contents");
            items.reject();
            None
        }
        ReadableSlot::Error(bad) => {
            errors.report(unexpected_node_error(
                bad.raw_node(),
                bad.source(),
                "mor_contents",
            ));
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
