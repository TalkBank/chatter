//! `%gra` tier-level parsing logic.
//!
//! Converts one `%gra` line into a `GraTier` by decoding each whitespace-
//! separated `index|head|relation` triple.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::generated_traversal::{
    AsRawNode, GraContentsNode, GraDependentTierNode, GraRelationNode, NodeSlot, WhitespacesNode,
    extract_gra_contents, extract_gra_dependent_tier,
};
use talkbank_model::ParseOutcome;
use talkbank_model::model::{GraTier, GraTierType, GrammaticalRelation};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

use super::relation::parse_gra_relation;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_unexpected};

/// Converts one `%gra` tier node into `GraTier`.
///
/// **Grammar Rule:**
/// ```text
/// gra_dependent_tier: seq(gra_tier_prefix, tier_sep, gra_contents, newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_gra_dependent_tier` yields the
/// prefix / tier-sep / body / newline as typed `Positioned` slots. The body
/// (`child_2.slot`, a `gra_contents` node) is matched EXHAUSTIVELY over
/// [`NodeSlot`] (no `_` catch-all, no `.ok()`), reproducing the removed
/// hand-walk byte for byte:
///
/// - `Present` / `Missing`: the removed code LOCATED the body by scanning for a
///   child of kind `gra_contents`, and a tree-sitter MISSING node reports that
///   expected kind, so both a real body and a MISSING body were found (the old
///   `Some(gra_contents)` branch) and drive relation iteration. A MISSING/empty
///   `gra_contents` yields zero relations with no diagnostic, identical to the
///   old loop iterating an empty node. The two arms can no longer share one
///   `|`-pattern binding: the NEW backend's `NodeSlot::Missing` carries the raw
///   `tree_sitter::Node` directly, not the typed `GraContentsNode` wrapper OLD
///   carried, so `Present` calls [`AsRawNode::raw_node`] while `Missing` passes
///   its raw node straight through; the observable parse is unchanged.
/// - `Absent` / `Error` / `Unexpected`: no child of kind `gra_contents` was
///   found (the old `None` branch): an ERROR node or an unexpected-kind node does
///   not match `gra_contents`, and an absent child is not there at all. Emit the
///   `MalformedGrammarRelation` diagnostic and return the EMPTY tier. This
///   silent-partial is PRESERVED behavior; it is unreachable from the boundary
///   (`parse_gra_tier` is only invoked when the tier node has no tree-sitter
///   error) but is reproduced here for exhaustiveness.
pub fn parse_gra_tier(node: Node, source: &str, errors: &impl ErrorSink) -> GraTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let children = extract_gra_dependent_tier(GraDependentTierNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    match children.child_2.slot {
        NodeSlot::Present(contents) => {
            let relations = parse_gra_relations(contents.raw_node(), source, errors);
            GraTier::new(GraTierType::Gra, relations).with_span(span)
        }
        NodeSlot::Missing(raw_contents) => {
            let relations = parse_gra_relations(raw_contents, source, errors);
            GraTier::new(GraTierType::Gra, relations).with_span(span)
        }
        NodeSlot::Absent | NodeSlot::Error(_) | NodeSlot::Unexpected(_) => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                "Missing gra_contents node in %gra tier".to_string(),
            ));
            GraTier::new(GraTierType::Gra, Vec::new()).with_span(span)
        }
    }
}

/// Decode every `gra_relation` under a `gra_contents` node into the relation
/// model, driven by the generated `extract_gra_contents` visitor.
///
/// `gra_contents = seq(gra_relation, repeat(seq(whitespaces, gra_relation)))`,
/// so the visitor exposes the first relation as `child_0` and each subsequent
/// `(whitespaces, gra_relation)` pair as a `GraContentsChild1Children` element
/// in `child_1`. This replaces the old `while gra_contents.child(idx)`
/// positional walk. Unlike the OLD backend (built with `--skip whitespaces`),
/// the NEW backend models the separating `whitespaces` token as its own
/// explicit `child_0` position inside each repeat element (`child_1` holds the
/// `gra_relation` itself); that position is purely structural (no content to
/// decode) and, per [`push_gra_separator`], unreachable in practice for the
/// same reason the relation slots below are.
fn parse_gra_relations(
    gra_contents: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> Vec<GrammaticalRelation> {
    let contents = extract_gra_contents(GraContentsNode(gra_contents));
    let mut relations: Vec<GrammaticalRelation> =
        Vec::with_capacity(contents.child_1.slot.len() + 1);

    push_gra_relation(contents.child_0.slot, source, errors, &mut relations);
    for element in contents.child_1.slot {
        match element.slot {
            NodeSlot::Present(pair) => {
                push_gra_separator(pair.child_0.slot, source, errors);
                push_gra_relation(pair.child_1.slot, source, errors, &mut relations);
                surface_unexpected(&pair.unexpected, source, errors);
            }
            // The generated repeat classifies a whole item as `Present` /
            // `Error` / `Absent` only (see the established finding for the
            // analogous `@Languages` code-list repeat); matched exhaustively
            // regardless, per the project no-`_`-on-project-enums rule.
            NodeSlot::Missing(raw) | NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
                errors.report(unexpected_node_error(raw, source, "gra_contents"));
            }
            NodeSlot::Absent => {}
        }
    }

    surface_unexpected(&contents.unexpected, source, errors);
    relations
}

/// Decode the separating `whitespaces` token inside one `gra_contents` repeat
/// element.
///
/// A NEW position with no OLD counterpart: the OLD backend was generated with
/// `--skip whitespaces`, so the space between two `index|head|relation`
/// triples was never a modeled child at all. It carries no content, so
/// `Present` is a no-op; the recovery arms reuse the SAME diagnostic mechanism
/// [`push_gra_relation`] already uses for this file's other `gra_contents`
/// positions (`check_not_missing` / `unexpected_node_error`), for consistency.
/// Like every other slot in this function, these arms are unreachable in
/// production: `parse_gra_tier` (and therefore `parse_gra_relations`) is only
/// entered when the containing tier node has no tree-sitter error, and the
/// CHAT lexer never emits two adjacent `index|head|relation` triples without
/// intervening whitespace on well-formed input.
fn push_gra_separator<'tree>(
    slot: NodeSlot<'tree, WhitespacesNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
) {
    match slot {
        NodeSlot::Present(_) | NodeSlot::Absent => {}
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "gra_contents");
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "gra_contents"));
        }
    }
}

/// Decode one relation slot, pushing it onto `relations` when it parses.
///
/// The `gra_relation` slot is matched EXHAUSTIVELY over [`NodeSlot`] (no `_`
/// catch-all), reproducing the removed per-child loop byte for byte:
///
/// - `Present`: parse the relation; push it only when `parse_gra_relation`
///   returns [`ParseOutcome::Parsed`] (a rejected relation is dropped without a
///   fabricated default, exactly as before).
/// - `Missing`: the old loop's `check_not_missing` reported the
///   `MissingRequiredElement` (E342) recovery diagnostic and skipped the child;
///   reproduced here (the returned flag is discarded because the missing child is
///   dropped either way).
/// - `Error` / `Unexpected`: the old loop's `_` arm reported
///   `unexpected_node_error` (ERROR nodes route through the error analyzer);
///   reproduced here.
/// - `Absent`: no child at this position; the old loop simply did not iterate
///   here, so nothing is reported and nothing is pushed.
///
/// The `Missing` / `Error` / `Unexpected` arms are unreachable from the boundary
/// (`parse_gra_tier` is only entered when the tier node has no tree-sitter
/// error); they are handled explicitly for exhaustiveness.
fn push_gra_relation<'tree>(
    slot: NodeSlot<'tree, GraRelationNode<'tree>>,
    source: &str,
    errors: &impl ErrorSink,
    relations: &mut Vec<GrammaticalRelation>,
) {
    match slot {
        NodeSlot::Present(relation_node) => {
            if let ParseOutcome::Parsed(relation) =
                parse_gra_relation(relation_node.raw_node(), source, errors)
            {
                relations.push(relation);
            }
        }
        NodeSlot::Missing(raw) => {
            check_not_missing(raw, source, errors, "gra_contents");
        }
        NodeSlot::Error(raw) | NodeSlot::Unexpected(raw) => {
            errors.report(unexpected_node_error(raw, source, "gra_contents"));
        }
        NodeSlot::Absent => {}
    }
}
