//! Parser for `%spa` speech-act tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::SpaTier;
use tree_sitter::Node;

use crate::generated_traversal::{SpaDependentTierNode, extract_spa_dependent_tier};

use super::helpers::{parse_text_tier_content, tier_span};

/// Converts one `%spa` tier node.
///
/// **Grammar Rule:**
/// ```text
/// spa_dependent_tier: seq('%', 'spa', colon, tab, text_with_bullets, newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_spa_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// [`parse_text_tier_content`], which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_spa_tier(node: Node, source: &str, errors: &impl ErrorSink) -> SpaTier {
    let span = tier_span(node);
    let children = extract_spa_dependent_tier(SpaDependentTierNode(node));
    let content = parse_text_tier_content(
        node,
        children.child_2.slot,
        &children.unexpected,
        source,
        errors,
        "spa_dependent_tier",
        "Missing content in %spa tier",
    );
    SpaTier::new(content).with_span(span)
}
