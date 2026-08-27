//! Parser for `%spa` speech-act tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::SpaTier;

use crate::generated_traversal::{AsRawNode, SpaDependentTierNode, extract_spa_dependent_tier};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%spa` tier node.
///
/// **Grammar Rule:**
/// ```text
/// spa_dependent_tier: seq('%', 'spa', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_spa_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// [`parse_optional_text_tier_content`], which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_spa_tier(
    typed: SpaDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> SpaTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = extract_spa_dependent_tier(typed);
    let content = parse_optional_text_tier_content(
        typed,
        children.child_2.slot(),
        &children.unexpected,
        source,
        errors,
    );
    SpaTier::new(content).with_span(span)
}
