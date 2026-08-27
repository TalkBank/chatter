//! Parser for `%com` (comment) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::ComTier;

use crate::generated_traversal::{AsRawNode, ComDependentTierNode, extract_com_dependent_tier};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%com` tier node.
///
/// **Grammar Rule:**
/// ```text
/// com_dependent_tier: seq('%', 'com', colon, tab, optional(text_with_bullets_and_pics), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_com_dependent_tier` yields the
/// prefix / tier-sep / body / newline as typed `Positioned` slots, and the body
/// (`child_2.slot`, a `text_with_bullets_and_pics` node) is matched exhaustively by
/// the shared [`parse_optional_text_tier_content`], which also surfaces the carrier's
/// `unexpected` sink (R2).
pub fn parse_com_tier(
    typed: ComDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ComTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = extract_com_dependent_tier(typed);
    let content = parse_optional_text_tier_content(
        typed,
        children.child_2.slot(),
        &children.unexpected,
        source,
        errors,
    );
    ComTier::new(content).with_span(span)
}
