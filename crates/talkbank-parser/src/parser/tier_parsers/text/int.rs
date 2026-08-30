//! Parser for `%int` (intonation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Intonation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::IntTier;

use crate::generated_traversal::{AsRawNode, IntDependentTierNode, extract_int_dependent_tier};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%int` tier node.
///
/// **Grammar Rule:**
/// ```text
/// int_dependent_tier: seq('%', 'int', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_int_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// `parse_optional_text_tier_content`, which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_int_tier(
    typed: IntDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> IntTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = extract_int_dependent_tier(typed);
    let content = parse_optional_text_tier_content(
        typed,
        children.child_2.slot(),
        &children.unexpected,
        source,
        errors,
    );
    IntTier::new(content).with_span(span)
}
