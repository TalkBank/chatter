//! Parser for `%add` (addressee) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::AddTier;
use tree_sitter::Node;

use crate::generated_traversal::{AddDependentTierNode, extract_add_dependent_tier};

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%add` tier node.
///
/// **Grammar Rule:**
/// ```text
/// add_dependent_tier: seq('%', 'add', colon, tab, text_with_bullets, newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_add_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// [`parse_text_tier_content`], which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_add_tier(node: Node, source: &str, errors: &impl ErrorSink) -> AddTier {
    let span = tier_span(node);
    let children = extract_add_dependent_tier(AddDependentTierNode(node));
    let content = parse_text_tier_content(
        node,
        children.child_2.slot,
        &children.unexpected,
        source,
        errors,
        "add_dependent_tier",
        "Missing content in %add tier",
    );
    AddTier::new(content).with_span(span)
}
