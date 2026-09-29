//! Field-level parser for `%gra` relation tuples.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use std::num::NonZeroUsize;

use crate::generated_traversal::{
    AsRawNode, GraHeadNode, GraIndexNode, GraRelationNameNode, GraRelationNode, KindSlot, NoChild,
    SourceBound, SourceBoundKind, SourceField, SourceSlotView,
};
use crate::parser::tree_parsing::parser_helpers::surface_displaced;
use talkbank_model::ParseOutcome;
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};

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
    let relation_span = node.start_byte()..node.end_byte();
    let children = typed.extract()?;
    surface_displaced(
        &children.children().unexpected,
        "gra_relation",
        source,
        errors,
    );

    let ParseOutcome::Parsed(index_text) =
        RelationField::Index(children.field_index().slot()).read(typed, errors)?
    else {
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
                        SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                        ErrorContext::new(source, relation_span.clone(), index_text),
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
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, relation_span.clone(), index_text),
                    format!("Invalid index '{}': must be a positive integer", index_text),
                )
                .with_suggestion("Index must be 1, 2, 3, ... (1-indexed)"),
            );
            return Ok(ParseOutcome::rejected());
        }
    };

    let ParseOutcome::Parsed(head_text) =
        RelationField::Head(children.field_head().slot()).read(typed, errors)?
    else {
        return Ok(ParseOutcome::rejected());
    };

    let head = match head_text.parse::<usize>() {
        Ok(h) => h,
        Err(_) => {
            errors.report(
                ParseError::new(
                    ErrorCode::UnexpectedGrammarNode,
                    Severity::Error,
                    SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                    ErrorContext::new(source, relation_span.clone(), head_text),
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

    let ParseOutcome::Parsed(relation_text) =
        RelationField::Label(children.field_relation().slot()).read(typed, errors)?
    else {
        return Ok(ParseOutcome::rejected());
    };

    if relation_text.is_empty() {
        errors.report(ParseError::new(
            ErrorCode::MalformedGrammarRelation,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
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

/// Each role carries only its own generated slot type: a head cannot be read
/// or diagnosed as an index. Recovery never supplies fabricated field text.
enum RelationField<'slot, 'tree, 'source> {
    Index(SourceField<'slot, 'tree, 'source, KindSlot<'tree, GraIndexNode<'tree>>>),
    Head(SourceField<'slot, 'tree, 'source, KindSlot<'tree, GraHeadNode<'tree>>>),
    Label(SourceField<'slot, 'tree, 'source, KindSlot<'tree, GraRelationNameNode<'tree>>>),
}

impl<'tree, 'source> RelationField<'_, 'tree, 'source> {
    fn read(
        self,
        relation: SourceBound<'tree, 'source, GraRelationNode<'tree>>,
        errors: &impl ErrorSink,
    ) -> Result<ParseOutcome<&'source str>, crate::CstFailure> {
        let source = relation.source();
        let (field, name) = match self {
            Self::Index(slot) => (read_present_text(slot)?, "index"),
            Self::Head(slot) => (read_present_text(slot)?, "head"),
            Self::Label(slot) => (read_present_text(slot)?, "relation name"),
        };
        let Some(field) = field else {
            let node = relation.raw_node();
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, node.byte_range(), ""),
                format!("Missing {name} in grammatical relation"),
            ));
            return Ok(ParseOutcome::rejected());
        };
        Ok(ParseOutcome::parsed(field))
    }
}

fn read_present_text<'tree, 'source, T: SourceBoundKind<'tree>>(
    slot: SourceField<'_, 'tree, 'source, KindSlot<'tree, T>>,
) -> Result<Option<&'source str>, crate::CstFailure> {
    Ok(match slot.view() {
        SourceSlotView::Present(field) => Some(field.read()?.text()),
        SourceSlotView::Missing(_) | SourceSlotView::Error(_) | SourceSlotView::Absent(NoChild) => {
            None
        }
        SourceSlotView::Unexpected(never) => match never {},
    })
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
