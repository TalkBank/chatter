//! Parser for `%act` action tiers.
//!
//! `%act` content is modeled as bullet-capable free text and is typically
//! aligned with events around the main tier.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Action_Code>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>

use crate::generated_traversal::{ActDependentTierNode, AsRawNode, extract_act_dependent_tier};
use crate::parser::node_span::span_of;
use crate::parser::tier_parsers::text::helpers::parse_optional_text_tier_content;
use talkbank_model::ErrorSink;
use talkbank_model::model::ActTier;

/// Converts one `%act` tier node into a `ActTier`.
///
/// **Grammar Rule:**
/// ```text
/// act_dependent_tier: seq('%', 'act', colon, tab, optional(text_with_bullets), newline)
/// ```
///
/// Delegates the whole five-state body policy to
/// [`parse_optional_text_tier_content`], the owner shared with the seven
/// text-like tiers. This file used to carry a private copy of that policy,
/// differing in ONE state: a malformed body gets no `unexpected_node_error`
/// here, because the removed hand-walk loop had none and simply fell through to
/// the "Missing content" rejection. That difference is now
/// [`MalformedBody::ReportMissingOnly`], a value rather than a fork, so the four
/// states nobody disputes cannot drift between the copies.
///
/// The `Option` on the body slot is the grammar's `optional(...)` (E756
/// widening, 2026-08-16): an absent body is the empty tier, not a parse
/// failure, so it lowers to empty content with no diagnostic and the validator
/// reports E756.
pub fn parse_act_tier(
    typed: ActDependentTierNode<'_>,
    source: &str,
    errors: &impl ErrorSink,
) -> ActTier {
    let node = typed.raw_node();
    let span = span_of(node);
    let children = extract_act_dependent_tier(typed);
    let content = parse_optional_text_tier_content(
        typed,
        children.child_2.slot(),
        &children.unexpected,
        source,
        errors,
    );
    ActTier::new(content).with_span(span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::WriteChat;

    /// Tests act tier construction.
    #[test]
    fn test_act_tier_construction() {
        let tier = ActTier::from_text("picks up toy");
        assert!(!tier.content.is_empty());
        assert_eq!(tier.to_chat_string(), "%act:\tpicks up toy");
    }

    /// Tests act tier with timing.
    #[test]
    fn test_act_tier_with_timing() {
        let tier = ActTier::from_text("<1w-2w> holds object out to Amy");
        assert!(!tier.content.is_empty());
        assert_eq!(
            tier.to_chat_string(),
            "%act:\t<1w-2w> holds object out to Amy"
        );
    }

    /// Tests act tier empty.
    #[test]
    fn test_act_tier_empty() {
        let tier = ActTier::from_text("");
        assert!(tier.is_empty());
        assert_eq!(tier.to_chat_string(), "%act:\t");
    }

    /// Tests act tier complex.
    #[test]
    fn test_act_tier_complex() {
        let tier = ActTier::from_text("<aft> manipulates chicken in hands");
        assert!(!tier.content.is_empty());
        assert_eq!(
            tier.to_chat_string(),
            "%act:\t<aft> manipulates chicken in hands"
        );
    }
}
