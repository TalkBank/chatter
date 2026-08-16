//! Parser for `%act` action tiers.
//!
//! `%act` content is modeled as bullet-capable free text and is typically
//! aligned with events around the main tier.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Action_Code>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::generated_traversal::{
    ActDependentTierNode, AsRawNode, NodeSlot, extract_act_dependent_tier,
};
use crate::parser::tree_parsing::bullet_content::parse_bullet_content;
use crate::parser::tree_parsing::parser_helpers::surface_unexpected;
use talkbank_model::model::{ActTier, BulletContent};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span,
};
use tree_sitter::Node;

/// Converts one `%act` tier node into an `ActTier`.
///
/// **Grammar Rule:**
/// ```text
/// act_dependent_tier: seq('%', 'act', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_act_dependent_tier` yields the
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
///   removed act loop had NO unexpected-node report; a non-text body simply left
///   `content` unset and fell through to the "Missing content" rejection. That is
///   preserved exactly (no `unexpected_node_error`), at the same code and span.
/// - `None`: the body slot is an `Option` because the grammar marks the body
///   `optional(...)` (E756 widening, 2026-08-16). An absent body is the empty
///   tier, not a parse failure; it lowers to [`BulletContent::empty`] with no
///   diagnostic and the validator reports E756.
pub fn parse_act_tier(node: Node, source: &str, errors: &impl ErrorSink) -> ActTier {
    let span = Span::new(node.start_byte() as u32, node.end_byte() as u32);
    let children = extract_act_dependent_tier(ActDependentTierNode(node));
    surface_unexpected(&children.unexpected, source, errors);

    let content = match children.child_2.slot() {
        // The grammar body is `optional(...)`, so an ABSENT body is not a parse
        // failure: `%act:` with nothing after the separator is a real (if
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
                    "act_dependent_tier",
                ),
                "Missing content in %act tier".to_string(),
            ));
            BulletContent::empty()
        }
    };

    ActTier::new(content).with_span(span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    /// Tests act tier construction.
    #[test]
    fn test_act_tier_construction() {
        let tier = ActTier::from_text("picks up toy");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%act:\tpicks up toy");
    }

    /// Tests act tier with timing.
    #[test]
    fn test_act_tier_with_timing() {
        let tier = ActTier::from_text("<1w-2w> holds object out to Amy");
        assert!(!tier.content.is_empty());
        assert_eq!(
            tier.to_chat_string(),
            "%act:\t<1w-2w> holds object out to Amy"
        );
    }

    /// Tests act tier empty.
    #[test]
    fn test_act_tier_empty() {
        let tier = ActTier::from_text("");
        assert!(tier.is_empty());
        assert_eq!(tier.to_chat_string(), "%act:\t");
    }

    /// Tests act tier complex.
    #[test]
    fn test_act_tier_complex() {
        let tier = ActTier::from_text("<aft> manipulates chicken in hands");
        assert!(!tier.content.is_empty());
        assert_eq!(
            tier.to_chat_string(),
            "%act:\t<aft> manipulates chicken in hands"
        );
    }
}
