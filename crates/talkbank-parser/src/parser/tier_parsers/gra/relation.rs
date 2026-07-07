//! Field-level parser for `%gra` relation tuples.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Grammatical_Relations>
//! - <https://talkbank.org/0info/manuals/CHAT.html#GrammaticalRelations_Tier>

use crate::generated_traversal::{AsRawNode, GraRelationNode, NodeSlot, extract_gra_relation};
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::ParseOutcome;
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation};
use tree_sitter::Node;

/// Converts one `gra_relation` node (`index|head|label`) into `GrammaticalRelation`.
///
/// **Grammar Rule:**
/// ```text
/// gra_relation: seq(gra_index, '|', gra_head, '|', gra_relation_name)
/// ```
///
/// Driven by the generated typed visitor: `extract_gra_relation` yields the
/// `index` / `head` / `relation` fields (and the two `|` pipes) as named typed
/// `Positioned` slots (`children.index.slot`, etc.), replacing the previous
/// positional `node.child(0/2/4)` hand-walk. Each field slot is matched
/// EXHAUSTIVELY over [`NodeSlot`] (no `_` catch-all, no `.ok()`), reproducing
/// the removed positional walk byte for byte:
///
/// - `Present`: decode the field's raw-node bytes exactly as the old
///   `Some(child)` arm did (via `utf8_text`, so a UTF-8 error is still reported
///   as `MalformedGrammarRelation` at the field's span), then apply the same
///   value checks (index must be a positive 1-indexed integer, head must be a
///   non-negative integer, relation name must be non-empty).
/// - `Missing` / `Error` / `Unexpected` / `Absent`: the field is not a usable
///   node, which corresponds to the old positional `None` branch (no child at
///   that position); report the same `MalformedGrammarRelation` "Missing
///   `<field>`" diagnostic at the relation span and reject. These arms are
///   unreachable in production: `parse_gra_relation` is reached only when the
///   containing tier node has no tree-sitter error, so every field is `Present`;
///   they are handled explicitly for exhaustiveness, never fabricating a value.
pub(super) fn parse_gra_relation(
    node: Node,
    source: &str,
    errors: &impl ErrorSink,
) -> ParseOutcome<GrammaticalRelation> {
    let relation_span = node.start_byte()..node.end_byte();
    let children = extract_gra_relation(GraRelationNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    let index_text = match children.index.slot {
        NodeSlot::Present(index_node) => {
            let field = index_node.raw_node();
            match field.utf8_text(source.as_bytes()) {
                Ok(text) => text,
                Err(err) => {
                    errors.report(ParseError::new(
                        ErrorCode::MalformedGrammarRelation,
                        Severity::Error,
                        SourceLocation::from_offsets(field.start_byte(), field.end_byte()),
                        ErrorContext::new(source, field.start_byte()..field.end_byte(), ""),
                        format!("UTF-8 decoding error in grammatical relation index: {err}"),
                    ));
                    return ParseOutcome::rejected();
                }
            }
        }
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, relation_span.clone(), ""),
                "Missing index in grammatical relation".to_string(),
            ));
            return ParseOutcome::rejected();
        }
    };

    let index = match index_text.parse::<usize>() {
        Ok(idx) => {
            if idx == 0 {
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
            } else {
                idx
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

    let head_text = match children.head.slot {
        NodeSlot::Present(head_node) => {
            let field = head_node.raw_node();
            match field.utf8_text(source.as_bytes()) {
                Ok(text) => text,
                Err(err) => {
                    errors.report(ParseError::new(
                        ErrorCode::MalformedGrammarRelation,
                        Severity::Error,
                        SourceLocation::from_offsets(field.start_byte(), field.end_byte()),
                        ErrorContext::new(source, field.start_byte()..field.end_byte(), ""),
                        format!("UTF-8 decoding error in grammatical relation head: {err}"),
                    ));
                    return ParseOutcome::rejected();
                }
            }
        }
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, relation_span.clone(), ""),
                "Missing head in grammatical relation".to_string(),
            ));
            return ParseOutcome::rejected();
        }
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

    let relation_text = match children.relation.slot {
        NodeSlot::Present(relation_node) => {
            let field = relation_node.raw_node();
            match field.utf8_text(source.as_bytes()) {
                Ok(text) => text,
                Err(err) => {
                    errors.report(ParseError::new(
                        ErrorCode::MalformedGrammarRelation,
                        Severity::Error,
                        SourceLocation::from_offsets(field.start_byte(), field.end_byte()),
                        ErrorContext::new(source, field.start_byte()..field.end_byte(), ""),
                        format!("UTF-8 decoding error in grammatical relation label: {err}"),
                    ));
                    return ParseOutcome::rejected();
                }
            }
        }
        NodeSlot::Missing(_) | NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent => {
            errors.report(ParseError::new(
                ErrorCode::MalformedGrammarRelation,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(source, relation_span.clone(), ""),
                "Missing relation name in grammatical relation".to_string(),
            ));
            return ParseOutcome::rejected();
        }
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

    ParseOutcome::parsed(GrammaticalRelation::new(index, head, relation_text))
}
