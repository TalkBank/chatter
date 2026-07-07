//! Parser for `%exp` (explanation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::ExpTier;
use tree_sitter::Node;

use crate::generated_traversal::{ExpDependentTierNode, extract_exp_dependent_tier};

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%exp` tier node.
///
/// **Grammar Rule:**
/// ```text
/// exp_dependent_tier: seq('%', 'exp', colon, tab, text_with_bullets, newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_exp_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// [`parse_text_tier_content`], which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_exp_tier(node: Node, source: &str, errors: &impl ErrorSink) -> ExpTier {
    let span = tier_span(node);
    let children = extract_exp_dependent_tier(ExpDependentTierNode(node));
    let content = parse_text_tier_content(
        node,
        children.child_2.slot,
        &children.unexpected,
        source,
        errors,
        "exp_dependent_tier",
        "Missing content in %exp tier",
    );
    ExpTier::new(content).with_span(span)
}
