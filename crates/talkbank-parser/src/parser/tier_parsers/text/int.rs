//! Parser for `%int` (intonation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Intonation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::IntTier;
use tree_sitter::Node;

use crate::generated_traversal::{IntDependentTierNode, extract_int_dependent_tier};

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%int` tier node.
///
/// **Grammar Rule:**
/// ```text
/// int_dependent_tier: seq('%', 'int', colon, tab, text_with_bullets, newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_int_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// [`parse_text_tier_content`], which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_int_tier(node: Node, source: &str, errors: &impl ErrorSink) -> IntTier {
    let span = tier_span(node);
    let children = extract_int_dependent_tier(IntDependentTierNode(node));
    let content = parse_text_tier_content(
        node,
        children.child_2.slot,
        &children.unexpected,
        source,
        errors,
        "int_dependent_tier",
        "Missing content in %int tier",
    );
    IntTier::new(content).with_span(span)
}
