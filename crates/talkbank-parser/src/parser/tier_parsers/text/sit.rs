//! Parser for `%sit` (situation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::SitTier;
use tree_sitter::Node;

use crate::generated_traversal::{SitDependentTierNode, extract_sit_dependent_tier};

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%sit` tier node.
///
/// **Grammar Rule:**
/// ```text
/// sit_dependent_tier: seq('%', 'sit', colon, tab, text_with_bullets, newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_sit_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// [`parse_text_tier_content`], which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_sit_tier(node: Node, source: &str, errors: &impl ErrorSink) -> SitTier {
    let span = tier_span(node);
    let children = extract_sit_dependent_tier(SitDependentTierNode(node));
    let content = parse_text_tier_content(
        node,
        children.child_2.slot,
        &children.unexpected,
        source,
        errors,
        "sit_dependent_tier",
        "Missing content in %sit tier",
    );
    SitTier::new(content).with_span(span)
}
