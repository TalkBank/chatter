//! Parser for `%cod` coding tiers.
//!
//! `%cod` carries analyst-defined coding content and reuses the same
//! bullet-capable free-text structure as `%act`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Coding_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::generated_traversal::{
    AsRawNode, CodDependentTierNode, NodeSlot, extract_cod_dependent_tier,
};
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::model::{BulletContent, CodTier};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

/// Converts one `%cod` tier node into a `CodTier`.
///
/// **Grammar Rule:**
/// ```text
/// cod_dependent_tier: seq('%', 'cod', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_cod_dependent_tier` yields the
/// prefix / tier-sep / body / newline as typed `Positioned` slots. The body
/// (`child_2.slot`, a `text_with_bullets` node) is matched EXHAUSTIVELY over
/// [`NodeSlot`] (no `_` catch-all, no `.ok()`), reproducing the removed hand-walk
/// byte for byte:
///
/// - `Some(Present)` / `Some(Missing)`: the removed loop matched the body by kind,
///   and a
///   tree-sitter MISSING node carries that expected kind, so both parse the raw
///   body node via [`parse_bullet_content`]. The two arms can no longer share one
///   `|`-pattern binding: the NEW backend's `NodeSlot::Missing` carries the raw
///   `tree_sitter::Node` directly, not the typed wrapper OLD carried, so `Present`
///   reads it via [`AsRawNode::raw_node`] and `Missing` passes its raw node
///   straight through; the observable parse is unchanged.
/// - `Some(Error / Unexpected / Absent)`: unlike the shared text-tier helper, the
///   removed cod loop had NO unexpected-node report; a non-text body simply left
///   `content` unset and fell through to the "Missing content" rejection. That is
///   preserved exactly (no `unexpected_node_error`), at the same code and span.
/// - `None`: the body slot is an `Option` because the grammar marks the body
///   `optional(...)` (E756 widening, 2026-08-16). An absent body is the empty
///   tier, not a parse failure; it lowers to [`BulletContent::empty`] with no
///   diagnostic and the validator reports E756.
pub fn parse_cod_tier(node: Node, source: &str, errors: &impl ErrorSink) -> CodTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let children = extract_cod_dependent_tier(CodDependentTierNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    let content = match children.child_2.slot() {
        // The grammar body is `optional(...)`, so an ABSENT body is not a parse
        // failure: `%cod:` with nothing after the separator is a real (if
        // invalid) construct, and it lowers to content that says it is empty so
        // `DependentTier::empty_content_span` can hand it to E756. Reporting
        // here instead would drop the tier and lose the line on roundtrip.
        None => BulletContent::empty(),
        Some(NodeSlot::Present(text)) => parse_bullet_content(text.raw_node(), source, errors),
        Some(NodeSlot::Missing(raw)) => parse_bullet_content(*raw, source, errors),
        Some(NodeSlot::Error(_) | NodeSlot::Unexpected(_) | NodeSlot::Absent) => {
            errors.report(ParseError::new(
                ErrorCode::TreeParsingError,
                Severity::Error,
                SourceLocation::from_offsets(node.start_byte(), node.end_byte()),
                ErrorContext::new(
                    source,
                    node.start_byte()..node.end_byte(),
                    "cod_dependent_tier",
                ),
                "Missing content in %cod tier".to_string(),
            ));
            BulletContent::empty()
        }
    };

    CodTier::new(content).with_span(span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    /// Tests cod tier construction.
    #[test]
    fn test_cod_tier_construction() {
        let tier = CodTier::from_text("general coding");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\tgeneral coding");
    }

    /// Tests cod tier single index.
    #[test]
    fn test_cod_tier_single_index() {
        let tier = CodTier::from_text("<1> atul");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1> atul");
    }

    /// Tests cod tier compound index.
    #[test]
    fn test_cod_tier_compound_index() {
        let tier = CodTier::from_text("<1+2> eje");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1+2> eje");
    }

    /// Tests cod tier multiple indices.
    #[test]
    fn test_cod_tier_multiple_indices() {
        let tier = CodTier::from_text("<1 , 3> atul");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<1 , 3> atul");
    }

    /// Tests cod tier complex.
    #[test]
    fn test_cod_tier_complex() {
        let tier = CodTier::from_text("<2 , 7> ledet <8> Itamar");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t<2 , 7> ledet <8> Itamar");
    }

    /// Tests cod tier empty.
    #[test]
    fn test_cod_tier_empty() {
        let tier = CodTier::from_text("");
        assert!(tier.is_empty());
        assert_eq!(tier.to_chat_string(), "%cod:\t");
    }
}
