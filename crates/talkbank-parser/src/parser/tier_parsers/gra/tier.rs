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
    AsRawNode, GraContentsNode, GraDependentTierNode, GraRelationNode, KindSlot, NoChild,
    NonMissingKindSlot, SourceBound, SourceField, SourceSlotView, WhitespacesNode,
};
use crate::parser::node_span::span_of;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{GraCompleteness, GraTier, GrammaticalRelation};
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

use super::relation::parse_gra_relation;
use crate::parser::tree_parsing::helpers::unexpected_node_error;
use crate::parser::tree_parsing::parser_helpers::{check_not_missing, surface_displaced};

/// Converts one `%gra` tier node into `GraTier`.
///
/// **Grammar Rule:**
/// ```text
/// gra_dependent_tier: seq(gra_tier_prefix, tier_sep, gra_contents, newline)
/// ```
///
/// Source-bound extraction retains the owner through body, repeats and relation
/// fields. Compiled-grammar admission excludes missing composite bodies;
/// Error and Absent report E708 and return a truncated tier. Unexpected is
/// uninhabited in the generated kind slot. Lexical recovery remains explicit;
/// neither source ownership nor the caller's error gate proves it impossible.
/// Source/reconstruction failures propagate as `CstFailure`, not CHAT invalidity.
pub fn parse_gra_tier<'tree>(
    typed: SourceBound<'tree, '_, GraDependentTierNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<GraTier, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let span = span_of(node);
    let children = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    surface_displaced(
        &children.children().unexpected,
        "gra_dependent_tier",
        source,
        errors,
    );

    Ok(match children.field_child_2().slot().view() {
        SourceSlotView::Present(contents) => {
            let (relations, completeness) = parse_gra_relations(contents.read()?, errors)?;
            GraTier::lowered_from(relations, completeness).with_span(span)
        }
        SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.start_byte()..node.end_byte(), ""),
                "Missing gra_contents node in %gra tier".to_string(),
            ));
            // TRUNCATED, not empty: the `%gra:` line is there and its
            // contents node is not, so whatever the author wrote was lost. An
            // empty tier reported as whole would be judged as a complete graph
            // with no relations.
            GraTier::lowered_from(Vec::new(), GraCompleteness::Truncated).with_span(span)
        }
    })
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
/// decode) and retains the recovery handling in [`push_gra_separator`].
///
/// Returns the relations WITH whether every declared one is among them. A
/// rejected relation is dropped, so the tier can come back shorter than the
/// line, and `talkbank-model`'s E721 and E722 describe the tier as a whole. The
/// count of `gra_relation` slots that were `Present` is the denominator: it is
/// what the LINE declared, and it cannot disagree with the numerator because
/// both are counted here.
fn parse_gra_relations<'tree>(
    typed: SourceBound<'tree, '_, GraContentsNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<(Vec<GrammaticalRelation>, GraCompleteness), crate::CstFailure> {
    let source = typed.source();
    let contents = typed.extract_admitted(crate::parser::typed_cst::canonical_grammar()?)?;
    let mut relations: Vec<GrammaticalRelation> =
        Vec::with_capacity(contents.field_child_1().slot().iter().len() + 1);
    let mut declared = 0usize;

    declared += push_gra_relation(contents.field_child_0().slot(), errors, &mut relations)?;
    for element in contents.field_child_1().slot().iter() {
        match element.slot().view() {
            SourceSlotView::Present(pair) => {
                push_gra_separator(pair.field_child_0().slot(), errors);
                declared += push_gra_relation(pair.field_child_1().slot(), errors, &mut relations)?;
                for node in pair.field_unexpected().iter() {
                    surface_displaced(&[node.raw_node()], "gra_contents", node.source(), errors);
                }
            }
            // An inline sequence is never MISSING or displaced; `SeqSlot` says so.
            SourceSlotView::Error(raw) => {
                errors.report(unexpected_node_error(
                    raw.raw_node(),
                    source,
                    "gra_contents",
                ));
            }
            SourceSlotView::Missing(never) | SourceSlotView::Unexpected(never) => match never {},
            SourceSlotView::Absent(never) => match never {},
        }
    }

    surface_displaced(
        &contents.children().unexpected,
        "gra_contents",
        source,
        errors,
    );
    let completeness = match relations.len() == declared {
        true => GraCompleteness::Whole,
        false => GraCompleteness::Truncated,
    };
    Ok((relations, completeness))
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
/// Source ownership does not narrow these recovery states. Keep their handling
/// until the producer's slot types establish a stronger invariant.
fn push_gra_separator<'tree>(
    slot: SourceField<'_, 'tree, '_, KindSlot<'tree, WhitespacesNode<'tree>>>,
    errors: &impl ErrorSink,
) {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(_) | SourceSlotView::Absent(NoChild) => {}
        SourceSlotView::Missing(raw) => {
            check_not_missing(raw.raw_node(), source, errors, "gra_contents");
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                source,
                "gra_contents",
            ));
        }
        SourceSlotView::Unexpected(never) => match never {},
    }
}

/// Decode one relation slot, pushing it onto `relations` when it parses.
///
/// Compiled-grammar admission excludes missing composite relations. Present
/// relations are retained only after successful field admission; no default is
/// fabricated for a rejected relation. Error is diagnosed and Absent emits
/// nothing. Lexical fields retain their independent recovery checks.
///
/// Returns how many relations the LINE declared at this slot: one when the slot
/// held a `gra_relation` node, whether or not it survived lowering, and zero
/// otherwise. The caller compares that total against `relations.len()`, so
/// "was anything dropped" is derived from the same walk that does the dropping
/// rather than recounted afterwards.
fn push_gra_relation<'tree>(
    slot: SourceField<'_, 'tree, '_, NonMissingKindSlot<'tree, GraRelationNode<'tree>>>,
    errors: &impl ErrorSink,
    relations: &mut Vec<GrammaticalRelation>,
) -> Result<usize, crate::CstFailure> {
    let source = slot.source();
    match slot.view() {
        SourceSlotView::Present(relation_node) => {
            if let ParseOutcome::Parsed(relation) =
                parse_gra_relation(relation_node.read()?, errors)?
            {
                relations.push(relation);
            }
            return Ok(1);
        }
        SourceSlotView::Error(raw) => {
            errors.report(unexpected_node_error(
                raw.raw_node(),
                source,
                "gra_contents",
            ));
        }
        SourceSlotView::Unexpected(never) => match never {},
        SourceSlotView::Absent(NoChild) => {}
    }
    Ok(0)
}
