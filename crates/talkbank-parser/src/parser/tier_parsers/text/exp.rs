//! Parser for `%exp` (explanation) tiers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorSink;
use talkbank_model::model::ExpTier;

use crate::generated_traversal::{AsRawNode, ExpDependentTierNode, extract_exp_dependent_tier};

use super::helpers::{parse_optional_text_tier_content, span_of};

/// Converts one `%exp` tier node.
///
/// **Grammar Rule:**
/// ```text
/// exp_dependent_tier: seq('%', 'exp', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Driven by the generated typed visitor: `extract_exp_dependent_tier` yields the
/// body as `child_2.slot`, matched exhaustively by the shared
/// [`parse_optional_text_tier_content`], which also surfaces the carrier's `unexpected`
/// sink (R2).
pub fn parse_exp_tier(
    typed: ExpDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ExpTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = extract_exp_dependent_tier(typed);
    let content = parse_optional_text_tier_content(
        typed,
        children.child_2.slot(),
        &children.unexpected,
        source,
        errors,
    );
    ExpTier::new(content).with_span(span)
}
