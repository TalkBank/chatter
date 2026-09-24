//! Field-level parser for `%gra` relation tuples.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use std::num::NonZeroUsize;

use crate::generated_traversal::{
    AsRawNode, GraHeadNode, GraIndexNode, GraRelationNameNode, GraRelationNode, KindSlot,
    extract_gra_relation,
};
use crate::parser::tree_parsing::parser_helpers::{present, surface_displaced};
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
pub(super) fn parse_gra_relation(
    typed: GraRelationNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<GrammaticalRelation> {
    let node = typed.raw_node();
    let relation_span = node.start_byte()..node.end_byte();
    let children = extract_gra_relation(typed);
    surface_displaced(&children.unexpected, "gra_relation", source, errors);

    let ParseOutcome::Parsed(index_text) =
        RelationField::Index(children.index.slot()).read(typed, source, errors)
    else {
        return ParseOutcome::rejected();
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
                return ParseOutcome::rejected();
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
            return ParseOutcome::rejected();
        }
    };

    let ParseOutcome::Parsed(head_text) =
        RelationField::Head(children.head.slot()).read(typed, source, errors)
    else {
        return ParseOutcome::rejected();
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
            return ParseOutcome::rejected();
        }
    };

    let ParseOutcome::Parsed(relation_text) =
        RelationField::Label(children.relation.slot()).read(typed, source, errors)
    else {
        return ParseOutcome::rejected();
    };

    if relation_text.is_empty() {
        errors.report(ParseError::new(
            ErrorCode::MalformedGrammarRelation,
            Severity::Error,
            SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
            ErrorContext::new(source, relation_span, relation_text),
            "Missing grammatical relation label".to_string(),
        ));
        return ParseOutcome::rejected();
    }

    ParseOutcome::parsed(GrammaticalRelation::new(index.get(), head, relation_text))
}

/// Each role carries only its own generated slot type: a head cannot be read
/// or diagnosed as an index. Recovery never supplies fabricated field text.
enum RelationField<'slot, 'tree> {
    Index(&'slot KindSlot<'tree, GraIndexNode<'tree>>),
    Head(&'slot KindSlot<'tree, GraHeadNode<'tree>>),
    Label(&'slot KindSlot<'tree, GraRelationNameNode<'tree>>),
}

impl RelationField<'_, '_> {
    fn read<'source>(
        self,
        relation: GraRelationNode<'_>,
        source: &'source str,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<&'source str> {
        let (field, name) = match self {
            Self::Index(slot) => (present(slot).map(AsRawNode::raw_node), "index"),
            Self::Head(slot) => (present(slot).map(AsRawNode::raw_node), "head"),
            Self::Label(slot) => (present(slot).map(AsRawNode::raw_node), "relation name"),
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
            return ParseOutcome::rejected();
        };
        match source.get(field.byte_range()) {
            Some(text) => ParseOutcome::parsed(text),
            None => {
                errors.report(ParseError::new(
                    ErrorCode::MalformedGrammarRelation,
                    Severity::Error,
                    SourceLocation::from_offsets(field.start_byte(), field.end_byte()),
                    ErrorContext::new(source, field.byte_range(), ""),
                    format!("Grammatical relation {name} range is not a UTF-8 slice of the source"),
                ));
                ParseOutcome::rejected()
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::TreeSitterParser;
    use crate::generated_traversal::FromNodeKind;
    use talkbank_model::ErrorCollector;

    /// Exercise the raw-node compatibility boundary with real CHAT CST nodes,
    /// not manufactured slots. A bad source must reject rather than panic.
    #[test]
    fn relation_fields_reject_out_of_source_ranges() {
        let source = include_str!("../../../../../../corpus/reference/tiers/mor-gra.cha");
        let parser = TreeSitterParser::new().expect("grammar");
        let parsed = parser
            .parse_source_incremental(source, None)
            .expect("parse");
        let mut pending = vec![parsed.root_node()];
        let mut relations = 0;
        while let Some(node) = pending.pop() {
            let mut cursor = node.walk();
            pending.extend(node.children(&mut cursor));
            let Some(relation) = GraRelationNode::from_node(node) else {
                continue;
            };
            relations += 1;
            let errors = ErrorCollector::new();
            assert!(parse_gra_relation(relation, source, &errors).is_some());
            assert!(errors.to_vec().is_empty());
            let children = extract_gra_relation(relation);
            for field in [
                RelationField::Index(children.index.slot()),
                RelationField::Head(children.head.slot()),
                RelationField::Label(children.relation.slot()),
            ] {
                let errors = ErrorCollector::new();
                assert!(field.read(relation, "", &errors).is_none());
                let diagnostics = errors.into_vec();
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, ErrorCode::MalformedGrammarRelation);
            }
        }
        assert_eq!(relations, 11);
    }
}
