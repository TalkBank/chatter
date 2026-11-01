//! Field-level parser for `%gra` relation tuples.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use std::num::NonZeroUsize;

use crate::generated_traversal::{
    AsRawNode, GraRelationChildren, GraRelationNode, KindAdmitted, NarrowedMissing, Never,
    PresenceError, RangeAdmittedChildren, Ranged, ReadableSlot, RequiredPresence, SourceBound,
    SourceBoundKind,
};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::ParseOutcome;
use talkbank_model::model::{GrammaticalRelation, GrammaticalRelationType};
use talkbank_model::{
    DiagnosticLocation, ErrorCode, ErrorContext, ErrorSink, ParseError, Severity,
};

/// Converts one `gra_relation` node (`index|head|label`) into `GrammaticalRelation`.
///
/// **Grammar Rule:**
/// ```text
/// gra_relation: seq(gra_index, '|', gra_head, '|', gra_relation_name)
/// ```
///
/// Read under the rule's admission (the admitted kind proof and required
/// presence), so no position can be ERROR or absent, and the narrowed index
/// and head cannot be MISSING: they are read infallibly. Only the relation
/// name, whose kind is not narrowed, can be MISSING, and that is the one
/// structural diagnostic here. Range admission rejects unreadable ranges
/// before numeric conversion; an admitted index is nonzero.
pub(super) fn parse_gra_relation<'tree>(
    typed: SourceBound<'tree, '_, GraRelationNode<'tree>>,
    errors: &impl ErrorSink,
) -> Result<ParseOutcome<GrammaticalRelation>, crate::CstFailure> {
    let source = typed.source();
    let node = typed.raw_node();
    let relation_span = span_of(node);
    let children = crate::parser::typed_cst::extract_inner(typed)?;
    surface_displaced(
        &children.children().unexpected,
        "gra_relation",
        source,
        errors,
    );
    let children = children.admit_ranges()?;

    let index_text = index_text(&children);

    let index = match index_text.parse::<usize>() {
        Ok(idx) => {
            if let Some(index) = NonZeroUsize::new(idx) {
                index
            } else {
                errors.report(
                    ParseError::new(
                        ErrorCode::InvalidGrammarIndex,
                        Severity::Error,
                        DiagnosticLocation::new(span_of(node)),
                        ErrorContext::new(source, relation_span, index_text),
                        "Index cannot be 0 (indices are 1-indexed)".to_string(),
                    )
                    .with_suggestion("Index must start at 1 for the first word"),
                );
                return Ok(ParseOutcome::rejected());
            }
        }
        Err(_) => {
            errors.report(
                ParseError::new(
                    ErrorCode::MalformedGrammarRelation,
                    Severity::Error,
                    DiagnosticLocation::new(span_of(node)),
                    ErrorContext::new(source, relation_span, index_text),
                    format!("Invalid index '{}': must be a positive integer", index_text),
                )
                .with_suggestion("Index must be 1, 2, 3, ... (1-indexed)"),
            );
            return Ok(ParseOutcome::rejected());
        }
    };

    let head_text = head_text(&children);

    let head = match head_text.parse::<usize>() {
        Ok(h) => h,
        Err(_) => {
            errors.report(
                ParseError::new(
                    ErrorCode::UnexpectedGrammarNode,
                    Severity::Error,
                    DiagnosticLocation::new(span_of(node)),
                    ErrorContext::new(source, relation_span, head_text),
                    format!(
                        "Invalid head '{}': must be a non-negative integer",
                        head_text
                    ),
                )
                .with_suggestion("Head must be 0 (ROOT) or a valid word index"),
            );
            return Ok(ParseOutcome::rejected());
        }
    };

    let Some(relation_text) = relation_name_text(&children) else {
        errors.report(ParseError::new(
            ErrorCode::MalformedGrammarRelation,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(source, relation_span, ""),
            "Missing relation name in grammatical relation".to_string(),
        ));
        return Ok(ParseOutcome::rejected());
    };

    let relation = match GrammaticalRelationType::new(relation_text) {
        Ok(relation) => relation,
        Err(error) => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                DiagnosticLocation::new(span_of(node)),
                ErrorContext::new(source, relation_span, relation_text),
                error.to_string(),
            ));
            return Ok(ParseOutcome::rejected());
        }
    };

    Ok(ParseOutcome::parsed(GrammaticalRelation::new(
        index.get(),
        head,
        relation,
    )))
}

/// A relation's children as its rule admits them, with readable ranges.
type AdmittedRelation<'tree, 'source> = RangeAdmittedChildren<
    'tree,
    'source,
    GraRelationChildren<'tree, KindAdmitted, Ranged<'source>, RequiredPresence>,
>;

/// The index's text: always present under the relation's admission.
fn index_text<'source>(relation: &AdmittedRelation<'_, 'source>) -> &'source str {
    admitted_text(&relation.children().index.slot)
}

/// The head's text: always present under the relation's admission.
fn head_text<'source>(relation: &AdmittedRelation<'_, 'source>) -> &'source str {
    admitted_text(&relation.children().head.slot)
}

/// The relation name's text, or `None` when it is MISSING: its kind is not
/// narrowed, so that is the one state admission leaves it besides present.
fn relation_name_text<'source>(relation: &AdmittedRelation<'_, 'source>) -> Option<&'source str> {
    match &relation.children().relation.slot {
        ReadableSlot::Present(field) => Some(field.text()),
        ReadableSlot::Missing(_) => None,
        ReadableSlot::Error(PresenceError(never))
        | ReadableSlot::Unexpected(never)
        | ReadableSlot::Absent(never) => match *never {},
    }
}

/// A required field whose kind the admitted proof narrows: every state but
/// present is uninhabited.
type AdmittedRequiredSlot<'tree, 'source, T, M, E> = ReadableSlot<
    SourceBound<'tree, 'source, T>,
    NarrowedMissing<KindAdmitted, M>,
    PresenceError<RequiredPresence, E>,
    Never,
    Never,
>;

/// The text of an [`AdmittedRequiredSlot`]: the present field's, since no
/// other state can be built.
fn admitted_text<'tree, 'source, T: SourceBoundKind<'tree>, M, E>(
    slot: &AdmittedRequiredSlot<'tree, 'source, T, M, E>,
) -> &'source str {
    match slot {
        ReadableSlot::Present(field) => field.text(),
        ReadableSlot::Missing(NarrowedMissing(never)) => match *never {},
        ReadableSlot::Error(PresenceError(never))
        | ReadableSlot::Unexpected(never)
        | ReadableSlot::Absent(never) => match *never {},
    }
}

/// Test-only counted-work observation: reading every field again performs no
/// further range checks.
#[cfg(test)]
pub(super) fn assert_repeated_field_reads(relation: &AdmittedRelation<'_, '_>) {
    let checks = relation.checked_range_count();
    for _ in 0..100 {
        let _ = (
            index_text(relation),
            head_text(relation),
            relation_name_text(relation),
        );
        assert_eq!(relation.checked_range_count(), checks);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::SourceBindingError;
    use talkbank_model::ErrorCollector;

    /// Relation readers require the owning parsed source, even for equal bytes.
    #[test]
    fn relation_fields_require_their_source_owner() {
        let source = include_str!("../../../../../../corpus/reference/tiers/mor-gra.cha");
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let other = parser
            .parse_source_incremental(source, None)
            .expect("independent owner");
        let mut relations = 0;
        for node in parsed.root().expect("root").descendants() {
            let Some(relation) = node.expect("readable node").typed::<GraRelationNode>() else {
                continue;
            };
            relations += 1;
            let errors = ErrorCollector::new();
            assert!(
                parse_gra_relation(relation, &errors)
                    .expect("source-bound extraction")
                    .is_some()
            );
            assert!(errors.to_vec().is_empty());
            assert!(matches!(
                other.bind(relation.raw_node()),
                Err(SourceBindingError::ForeignTree)
            ));
        }
        assert_eq!(relations, 11);
    }
}
