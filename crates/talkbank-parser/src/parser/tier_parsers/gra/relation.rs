//! Field-level parser for `%gra` relation tuples.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use std::num::NonZeroUsize;

use crate::generated_traversal::{
    AsRawNode, GraRelationChildren, GraRelationNode, Never, PresenceProof, RangeAdmittedChildren,
    Ranged, ReadableSlot, SourceBound, SourceBoundKind,
};
use crate::parser::node_span::span_of;
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::ParseOutcome;
use talkbank_model::model::GrammaticalRelation;
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
/// Generated slots retain recovery states. Field admission rejects recovery or
/// unreadable ranges before numeric conversion; an admitted index is nonzero.
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

    let ParseOutcome::Parsed(index_text) = RelationField::Index.read(&children, errors) else {
        return Ok(ParseOutcome::rejected());
    };

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

    let ParseOutcome::Parsed(head_text) = RelationField::Head.read(&children, errors) else {
        return Ok(ParseOutcome::rejected());
    };

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

    let ParseOutcome::Parsed(relation_text) = RelationField::Label.read(&children, errors) else {
        return Ok(ParseOutcome::rejected());
    };

    if relation_text.is_empty() {
        errors.report(ParseError::new(
            ErrorCode::MalformedGrammarRelation,
            Severity::Error,
            DiagnosticLocation::new(span_of(node)),
            ErrorContext::new(source, relation_span, relation_text),
            "Missing grammatical relation label".to_string(),
        ));
        return Ok(ParseOutcome::rejected());
    }

    Ok(ParseOutcome::parsed(GrammaticalRelation::new(
        index.get(),
        head,
        relation_text,
    )))
}

/// A role projects only its matching field from the complete admitted relation.
/// No unrelated field slot can be paired with this parent's diagnostic context.
enum RelationField {
    Index,
    Head,
    Label,
}

impl RelationField {
    fn read<'tree, 'source, P: PresenceProof>(
        self,
        relation: &RangeAdmittedChildren<
            'tree,
            'source,
            GraRelationChildren<'tree, Ranged<'source>, P>,
        >,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<&'source str> {
        let (field, name) = match self {
            Self::Index => (read_present_text(&relation.children().index.slot), "index"),
            Self::Head => (read_present_text(&relation.children().head.slot), "head"),
            Self::Label => (
                read_present_text(&relation.children().relation.slot),
                "relation name",
            ),
        };
        let Some(field) = field else {
            let parent = relation.parent();
            let node = parent.raw_node();
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                DiagnosticLocation::new(span_of(node)),
                ErrorContext::new(parent.source(), span_of(node), ""),
                format!("Missing {name} in grammatical relation"),
            ));
            return ParseOutcome::rejected();
        };
        ParseOutcome::parsed(field)
    }
}

fn read_present_text<'tree, 'source, T: SourceBoundKind<'tree>, E, A>(
    slot: &ReadableSlot<
        SourceBound<'tree, 'source, T>,
        SourceBound<'tree, 'source, T>,
        E,
        Never,
        A,
    >,
) -> Option<&'source str> {
    match slot {
        ReadableSlot::Present(field) => Some(field.text()),
        ReadableSlot::Missing(_) | ReadableSlot::Error(_) | ReadableSlot::Absent(_) => None,
        ReadableSlot::Unexpected(never) => match *never {},
    }
}

/// Test-only counted-work observation; field roles remain private to their owner.
#[cfg(test)]
pub(super) fn assert_repeated_field_reads<'tree, 'source>(
    relation: &RangeAdmittedChildren<'tree, 'source, GraRelationChildren<'tree, Ranged<'source>>>,
) {
    let checks = relation.checked_range_count();
    let errors = talkbank_model::ErrorCollector::new();
    for _ in 0..100 {
        for role in [
            RelationField::Index,
            RelationField::Head,
            RelationField::Label,
        ] {
            let _ = role.read(relation, &errors);
        }
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
