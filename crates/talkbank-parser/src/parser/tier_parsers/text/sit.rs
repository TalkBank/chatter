//! Parser for `%sit` (situation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::SitTier;

use crate::generated_traversal::{AsRawNode, SitDependentTierNode, extract_sit_dependent_tier};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%sit` tier node.
///
/// **Grammar Rule:**
/// ```text
/// sit_dependent_tier: seq('%', 'sit', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_sit_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// [`parse_optional_text_tier_content`], which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_sit_tier(
    typed: SitDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> SitTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = extract_sit_dependent_tier(typed);
    let content = parse_optional_text_tier_content(
        typed,
        children.child_2.slot(),
        &children.unexpected,
        source,
        errors,
    );
    SitTier::new(content).with_span(span)
}
