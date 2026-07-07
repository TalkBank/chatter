//! Parser for `%com` (comment) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::ComTier;
use tree_sitter::Node;

use crate::generated_traversal::{ComDependentTierNode, extract_com_dependent_tier};

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%com` tier node.
///
/// **Grammar Rule:**
/// ```text
/// com_dependent_tier: seq('%', 'com', colon, tab, text_with_bullets_and_pics, newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_com_dependent_tier` yields the
/// prefix / tier-sep / body / newline as typed `Positioned` slots, and the body
/// (`child_2.slot`, a `text_with_bullets_and_pics` node) is matched exhaustively by
/// the shared [`parse_text_tier_content`], which also surfaces the carrier's
/// `unexpected` sink (R2).
pub fn parse_com_tier(node: Node, source: &str, errors: &impl ErrorSink) -> ComTier {
    let span = tier_span(node);
    let children = extract_com_dependent_tier(ComDependentTierNode(node));
    let content = parse_text_tier_content(
        node,
        children.child_2.slot,
        &children.unexpected,
        source,
        errors,
        "com_dependent_tier",
        "Missing content in %com tier",
    );
    ComTier::new(content).with_span(span)
}
